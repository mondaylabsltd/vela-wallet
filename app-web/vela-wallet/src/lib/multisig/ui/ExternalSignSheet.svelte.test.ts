/* eslint-disable @typescript-eslint/no-explicit-any -- a stand-in session */
/**
 * The "Sign with another wallet" sheet in a browser.
 *
 * r4C-6: the pairing is shown — which account and wallet — with Use another
 * wallet (a fresh room, its own code) and Unpair (the pairing ended), and the
 * code is there for a paired room too. r4C-4: Vela not reaching the relay is
 * said as that, with Retry — not "Waiting for the wallet page…". r4C-3: a
 * wallet silent for a while is said to be waiting, with Cancel and Ask
 * again. r4C-11: a refusal in words. r4C-2: an approval that reaches the
 * mailbox from the link alone ends the request here too.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const OWNER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';

const fake = vi.hoisted(() => ({
	made: [] as any[],
	sign: null as null | ((w: any) => Promise<{ owner: string; signature: string }>),
	mailbox: [] as unknown[],
	/** What the next session starts as (the stand-in is not reactive: set before it is read). */
	next: {} as Record<string, unknown>
}));

vi.mock('../external/session.svelte', async () => {
	class FakeWallet {
		phase = 'waiting';
		since = Date.now();
		relay: any = { kind: 'up' };
		accounts: string[] = [];
		wallet: string | null = null;
		away = false;
		link: string;
		resumed: boolean;
		ended = false;
		left = false;
		retried = 0;
		constructor(resumed: boolean) {
			this.resumed = resumed;
			this.link = resumed ? 'https://w/en/connect#for=x' : 'https://w/en/connect#wss%3A&for=x';
			if (resumed) {
				this.accounts = [OWNER];
				this.wallet = 'Rabby';
			}
			Object.assign(this, fake.next);
			fake.made.push(this);
		}
		get pairedAccount() {
			return this.accounts[0] ?? null;
		}
		static for() {
			return new FakeWallet(true);
		}
		static fresh() {
			return new FakeWallet(false);
		}
		to(p: string) {
			this.phase = p;
		}
		retryRelay() {
			this.retried += 1;
		}
		signTypedData() {
			return fake.sign ? fake.sign(this) : new Promise(() => {});
		}
		leave() {
			this.left = true;
		}
		end() {
			this.ended = true;
		}
	}
	class WalletDeclinedError extends Error {}
	class WalletOtherChainError extends Error {}
	return { ExternalWallet: FakeWallet, WalletDeclinedError, WalletOtherChainError };
});
vi.mock('../external/signing.svelte', () => ({
	typedDataOf: () => ({}),
	checked: (_hash: string, owners: string[], a: { owner: string; signature: string }) =>
		owners.includes(a.owner) && a.signature === '0xgood' ? a : null
}));
vi.mock('../core/service', () => ({
	fetchProposals: async () => ({ members: {}, proposals: fake.mailbox })
}));
vi.mock('../core/kernel', async (original) => ({
	...(await original<typeof import('../core/kernel')>()),
	reviewTx: () => ({ action: { kind: 'transfer' } }),
	isEcdsaApproval: (a: object) => 'signature' in a,
	sameAddress: (a: string, b: string) => a?.toLowerCase() === b?.toLowerCase()
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

function open() {
	const resolved: unknown[] = [];
	const request = {
		chainId: 5042002,
		safe: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
		tx: {} as never,
		safeTxHash: SAFE_TX_HASH,
		owners: [OWNER],
		resolve: (a: unknown) => resolved.push(a),
		reject: () => {}
	};
	const screen = render(Sheet, {
		props: {
			m: words(),
			signing: new Proxy({}, { get: (_, k) => String(k) }) as never,
			locale: 'en',
			request,
			wide: true,
			closeLabel: 'Close'
		}
	});
	return { screen, resolved };
}

const testid = (id: string) => document.querySelector(`[data-testid="${id}"]`);
const button = (label: string) =>
	[...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as
		HTMLButtonElement | undefined;

afterEach(() => {
	fake.next = {};
	fake.made = [];
	fake.sign = null;
	fake.mailbox = [];
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
		expect(fake.made).toHaveLength(2);
		expect(fake.made[0].left).toBe(true);
		expect(fake.made[1].resumed).toBe(false);
		await vi.waitFor(() => expect(testid('external-paired')).toBeNull());
		expect(testid('external-link')?.textContent).toContain('wss');
		await screen.unmount();
	});

	it('r4C-6: Unpair ends the pairing and shows a fresh code', async () => {
		const { screen } = open();
		await vi.waitFor(() => expect(testid('external-unpair')).not.toBeNull());
		(testid('external-unpair') as HTMLButtonElement).click();
		await tick();
		expect(fake.made[0].ended).toBe(true);
		expect(fake.made[1].resumed).toBe(false);
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
		expect(fake.made[0].retried).toBe(1);
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
		fake.sign = async () => {
			throw new Error('Ledger: denied');
		};
		const { screen } = open();
		await vi.waitFor(() =>
			expect(testid('external-failure')?.textContent).toBe('Rabby could not sign: Ledger: denied')
		);
		expect(button('external.retry')).toBeDefined();
		await screen.unmount();
	});

	it('r4C-2: an approval that reaches the mailbox from the link alone ends the request', async () => {
		fake.mailbox = [
			{
				safeTxHash: SAFE_TX_HASH,
				approvals: [{ owner: OWNER, signature: '0xgood', createdMs: 1 }]
			}
		];
		vi.useFakeTimers({ toFake: ['setInterval'] });
		const { screen, resolved } = open();
		await vi.advanceTimersByTimeAsync(9_000);
		vi.useRealTimers();
		await vi.waitFor(() => expect(resolved).toMatchObject([{ owner: OWNER, signature: '0xgood' }]));
		expect(fake.made[0].left).toBe(true);
		await screen.unmount();
	});
});
