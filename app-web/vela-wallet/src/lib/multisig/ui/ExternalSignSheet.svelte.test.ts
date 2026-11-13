/* eslint-disable @typescript-eslint/no-explicit-any -- a stand-in session */
/**
 * The "Sign with another wallet" sheet in a browser, drawing the core's
 * session machine's view (a stand-in host, `fake-session.svelte.ts`: the
 * machine itself is pinned by the Rust suite).
 *
 * r4C-6: the pairing is shown — which account and wallet — with Use another
 * wallet (a fresh room, its own code) and Unpair (the pairing ended), and the
 * code is there for a paired room too. r4C-4: Vela not reaching the relay is
 * said as that, with Retry — not "Waiting for the wallet page…". r4C-3: a
 * wallet silent for a while is said to be waiting, with Cancel and Ask
 * again. r4C-11: a refusal in words. r4C-2: an approval that reaches the
 * mailbox from the link alone ends the request here too. r5reg-11: a wallet
 * answer that arrives once the request is gone throws nothing.
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { MultisigReviewOut, SafeTx } from '../core/kernel';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';
import type { OutsideSessionOutcome } from '../generated/OutsideSessionOutcome';
import type { OutsideSessionView } from '../generated/OutsideSessionView';

const OWNER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';

const fake = vi.hoisted(() => ({
	session: null as any,
	/** What the session starts as (a pairing kept before, unless a test says). */
	next: {} as Record<string, unknown>,
	/** An approval the mailbox holds from the link alone. */
	mailbox: null as null | { owner: string; signature: string }
}));

vi.mock('../shell/outside-session.svelte', async () => {
	const { FakeSession, sessionView } = await import('./fake-session.svelte');
	const fresh = (): Partial<OutsideSessionView> => ({
		resumed: false,
		invite: 'wss://relay#invite',
		accounts: [],
		wallet: null,
		pairedAccount: null,
		phase: 'waiting',
		failure: null
	});
	return {
		MAILBOX_EVERY_MS: 8_000,
		connectPage: () => 'https://w/en/connect',
		OutsideSessionHost: class extends FakeSession {
			constructor() {
				super(
					sessionView({
						resumed: true,
						invite: null,
						accounts: [OWNER],
						wallet: 'Rabby',
						pairedAccount: OWNER,
						...fake.next
					}),
					(session, event) => {
						switch (event.type) {
							case 'another_wallet':
								session.set(fresh());
								return;
							case 'stop_waiting':
								session.set({ failure: { kind: 'stoppedWaiting' } });
								return;
							case 'mailbox_tick':
								if (fake.mailbox) {
									session.set({ outcome: { kind: 'approved', approval: fake.mailbox } });
								}
								return;
							case 'close':
								session.set({ outcome: { kind: 'cancelled' } });
						}
					}
				);
				fake.session = this;
			}
		}
	};
});
vi.mock('../core/kernel', async (original) => ({
	...(await original<typeof import('../core/kernel')>()),
	reviewTx: () => ({
		action: {
			kind: 'transfer',
			token: null,
			to: '0x000000000000000000000000000000000000dEaD',
			amount: '1'
		}
	})
}));
vi.mock('$lib/wallet/qr', () => ({ encodeQr: () => ({ modules: 21, path: 'M0 0h1v1H0z' }) }));

const { default: Sheet } = await import('./ExternalSignSheet.svelte');

function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.external.pairedWith = 'Paired with {{account}} · {{wallet}}';
	out.external.velaRelayFailed = "Vela can't reach the signing relay ({{reason}}).";
	out.external.relayRefused = 'refused';
	out.external.inWallet = 'Confirm in {{wallet}} · {{elapsed}}';
	out.external.walletFailed = '{{wallet}} could not sign: {{reason}}';
	return out;
}

const SAFE_TX_HASH = `0x${'ab'.repeat(32)}`;

function open(over: { review?: MultisigReviewOut; tx?: SafeTx; m?: MultisigMessages } = {}) {
	const settled: OutsideSessionOutcome[] = [];
	const request = {
		chainId: 5042002,
		safe: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
		tx: (over.tx ?? {}) as never,
		safeTxHash: SAFE_TX_HASH,
		owners: [OWNER],
		settle: (o: OutsideSessionOutcome) => settled.push(o),
		reject: () => {}
	};
	const screen = render(Sheet, {
		props: {
			m: over.m ?? words(),
			review: over.review,
			signing: new Proxy({}, { get: (_, k) => String(k) }) as never,
			locale: 'en',
			request,
			wide: true,
			closeLabel: 'Close'
		}
	});
	return { screen, settled };
}

const testid = (id: string) => document.querySelector(`[data-testid="${id}"]`);
const button = (label: string) =>
	[...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as
		HTMLButtonElement | undefined;
const events = () => (fake.session?.events ?? []) as { type: string }[];

beforeAll(async () => {
	await loadCore();
});

afterEach(() => {
	fake.next = {};
	fake.session = null;
	fake.mailbox = null;
});

describe('the outside-wallet sheet', () => {
	it('r4C-6: says the pairing, keeps the code, and offers another wallet and Unpair', async () => {
		const { screen } = open();
		await vi.waitFor(() =>
			expect(testid('external-paired')?.textContent).toContain('Paired with 0x7099')
		);
		expect(testid('external-paired')?.textContent).toContain('Rabby');
		expect(testid('external-qr')).not.toBeNull();
		button('external.useAnother')!.click();
		await tick();
		expect(events()).toContainEqual({ type: 'another_wallet', unpair: false });
		await vi.waitFor(() => expect(testid('external-paired')).toBeNull());
		expect(testid('external-link')?.textContent).toContain('wss');
		await screen.unmount();
	});

	it('r4C-6: Unpair ends the pairing and shows a fresh code', async () => {
		const { screen } = open();
		await vi.waitFor(() => expect(testid('external-unpair')).not.toBeNull());
		(testid('external-unpair') as HTMLButtonElement).click();
		await tick();
		expect(events()).toContainEqual({ type: 'another_wallet', unpair: true });
		await vi.waitFor(() => expect(testid('external-paired')).toBeNull());
		await screen.unmount();
	});

	it("r4C-4: Vela off the relay is said as Vela's, with Retry", async () => {
		fake.next = { relay: { kind: 'failed', why: 'refused', detail: 'HTTP 4403' } };
		const { screen } = open();
		await vi.waitFor(
			() =>
				expect(testid('external-status')?.textContent).toContain(
					"Vela can't reach the signing relay (refused)."
				),
			{ timeout: 3000 }
		);
		button('external.retry')!.click();
		expect(fake.session.retried).toBe(1);
		await screen.unmount();
	});

	it('r16 leftover 4: a transport error is never the reason — the relay host, else "offline"', async () => {
		fake.next = {
			relay: { kind: 'failed', why: 'unreachable', detail: 'TypeError: Failed to fetch' }
		};
		const { screen } = open();
		await vi.waitFor(
			() => expect(testid('external-status')?.textContent).toContain('external.velaOffline'),
			{ timeout: 3000 }
		);
		expect(document.body.textContent).not.toContain('Failed to fetch');
		await screen.unmount();
	});

	it('r4C-3: a wallet silent for a while: timed, said, with Cancel and Ask again', async () => {
		fake.next = { phase: 'confirming', since: Date.now() - 25_000 };
		const { screen } = open();
		await vi.waitFor(() =>
			expect(testid('external-status')?.textContent).toMatch(/Confirm in Rabby · 0:2\d/)
		);
		await vi.waitFor(() => expect(testid('external-still-waiting')).not.toBeNull());
		expect(button('external.stopWaiting')).toBeDefined();
		expect(button('external.askAgain')).toBeDefined();
		button('external.stopWaiting')!.click();
		await vi.waitFor(() =>
			expect(testid('external-failure')?.textContent).toBe('external.stoppedWaiting')
		);
		await screen.unmount();
	});

	it('r4C-11: a wallet failure in its own words, with Try again', async () => {
		fake.next = {
			phase: 'confirming',
			failure: { kind: 'wallet', problem: { kind: 'unknown', reason: 'Ledger: denied' } }
		};
		const { screen } = open();
		await vi.waitFor(() =>
			expect(testid('external-failure')?.textContent).toBe('Rabby could not sign: Ledger: denied')
		);
		expect(button('external.retry')).toBeDefined();
		button('external.retry')!.click();
		expect(events()).toContainEqual({ type: 'ask_again' });
		await screen.unmount();
	});

	it('r5reg-11: "Use another wallet", the request gone, then the wallet fails: nothing uncaught', async () => {
		const uncaught: unknown[] = [];
		const onRejection = (event: PromiseRejectionEvent) => {
			uncaught.push(event.reason);
			event.preventDefault();
		};
		window.addEventListener('unhandledrejection', onRejection);
		try {
			const { screen } = open();
			await vi.waitFor(() => expect(button('external.useAnother')).toBeDefined());
			button('external.useAnother')!.click();
			await tick();
			// The host's request is answered and gone (closed, or approved from the
			// link) while the fresh room's wallet call is still out…
			await screen.rerender({ request: null as never });
			// …and fails late: the relay never answered.
			fake.session.set({
				phase: 'confirming',
				failure: { kind: 'wallet', problem: { kind: 'disconnected' } }
			});
			await new Promise((r) => setTimeout(r, 50));
			expect(uncaught.map(String)).toEqual([]);
			await screen.unmount();
		} finally {
			window.removeEventListener('unhandledrejection', onRejection);
		}
	});

	it('r4C-2: an approval that reaches the mailbox from the link alone ends the request', async () => {
		fake.mailbox = { owner: OWNER, signature: '0xgood' };
		vi.useFakeTimers({ toFake: ['setInterval'] });
		const { screen, settled } = open();
		await vi.advanceTimersByTimeAsync(9_000);
		vi.useRealTimers();
		await vi.waitFor(() =>
			expect(settled).toMatchObject([
				{ kind: 'approved', approval: { owner: OWNER, signature: '0xgood' } }
			])
		);
		expect(events()).toContainEqual({ type: 'mailbox_tick' });
		await screen.unmount();
	});

	it('closing the sheet withdraws the ask: the caller is told it was cancelled, once', async () => {
		const { screen, settled } = open();
		await vi.waitFor(() => expect(fake.session).not.toBeNull());
		(document.querySelector('button[aria-label="Close"]') as HTMLButtonElement).click();
		await vi.waitFor(() => expect(settled).toEqual([{ kind: 'cancelled' }]));
		expect(events()).toContainEqual({ type: 'close' });
		await tick();
		expect(settled).toHaveLength(1);
		await screen.unmount();
	});
});

describe('the outside-wallet sheet, with the one review', () => {
	it('draws the core review as the connect page does: the member change and its rule', async () => {
		const { multisigReview } = await import('../core/kernel');
		const safe = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
		const newcomer = '0x15d34aaf54267db7d7c367839aaf71a00a2c6a65';
		const tx: SafeTx = {
			to: safe,
			value: '0x0',
			data: `0x0d582f13${newcomer.slice(2).padStart(64, '0')}${(2).toString(16).padStart(64, '0')}`,
			operation: 0,
			safeTxGas: '0x0',
			baseGas: '0x0',
			gasPrice: '0x0',
			gasToken: '0x0000000000000000000000000000000000000000',
			refundReceiver: '0x0000000000000000000000000000000000000000',
			nonce: '0x3'
		};
		const review = multisigReview({
			chainId: 5042002,
			safe,
			tx,
			owners: [
				OWNER,
				'0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc',
				'0x90f79bf6eb2c4f870365e785982e1f101e93b906'
			],
			threshold: 2,
			deployed: true
		});
		const m = words();
		m.change.rule = 'Approvals: {{before}} → {{after}}';
		m.create.thresholdValue = '{{m}} of {{n}}';
		const { screen } = open({ review, tx, m });
		await vi.waitFor(() =>
			expect(testid('member-change-rule')?.textContent).toBe('Approvals: 2 of 3 → 2 of 4')
		);
		expect(testid('review-hashes')?.textContent).toContain(review.hashes.safeTxHash);
		await screen.unmount();
	});
});
