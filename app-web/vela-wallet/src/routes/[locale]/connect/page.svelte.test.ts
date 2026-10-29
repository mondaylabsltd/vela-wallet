/* eslint-disable @typescript-eslint/no-explicit-any -- a stand-in bridge and wallet */
/**
 * The connect page in a browser.
 *
 * ms-14: it says which account is connecting, and an account that is not one
 * of the members the link named is refused aloud — Connect is off, and the
 * page says what to do — never a calm "Connected. Keep this page open".
 *
 * r4: a spinner and "Connecting to Vela…" from the first frame (r4D-16); a
 * request reviewed before the wallet is asked — the multisig, network, what
 * it does, the number and the hashes (r4C-7); the wait in the wallet timed
 * (r4C-3); "Signed ✓" and each request's outcome (r4C-12); the relay's
 * refusal said with Retry (r4C-1); and the link's own transaction approved
 * or executed from here (r4C-2, r4C-9).
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { MULTISIG_SHAPE, type MultisigMessages } from '$lib/multisig/messages';
import { safeTxHashes, type SafeTx } from '$lib/multisig/core/kernel';

const MEMBER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
const STRANGER = '0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc';
const INVITE = `wss://relay.getvela.app/r/abc#${'A'.repeat(86)}`;
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const TX: SafeTx = {
	to: '0x000000000000000000000000000000000000dEaD',
	value: '0x2386f26fc10000',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x3'
};

const fake = vi.hoisted(() => ({
	accounts: [] as string[],
	joined: [] as unknown[][],
	bridge: null as any,
	approved: false,
	/** The live session's connection state, and since when. */
	state: 'connected',
	stateSince: 0,
	retried: 0,
	discover: null as null | Promise<void>,
	outside: null as any
}));

vi.mock('$lib/ui/history', () => ({ replaceUrl: () => {} }));
vi.mock('$lib/multisig/external/injected', () => ({ currentChain: async () => 'eip155:100' }));
vi.mock('$lib/multisig/external/eip6963', () => ({
	discoverProviders: async () => {
		if (fake.discover) await fake.discover;
		return [
			{
				info: { uuid: 'u', name: 'MetaMask', icon: '', rdns: '' },
				provider: { request: async () => fake.accounts }
			}
		];
	}
}));
vi.mock('$lib/multisig/external/bridge', () => ({
	Bridge: class extends EventTarget {
		signer: { accounts: string[]; chain: string };
		activity: unknown[] = [];
		prompt: unknown = undefined;
		request: unknown = undefined;
		relay: unknown = { kind: 'up' };
		constructor(_provider: unknown, accounts: string[], chain: string) {
			super();
			this.signer = { accounts, chain };
			fake.bridge = this;
		}
		get sessions() {
			return fake.joined.length
				? [
						{
							id: 's',
							state: fake.state,
							approved: fake.approved,
							declined: false,
							created: 1,
							stateSince: fake.stateSince || Date.now()
						}
					]
				: [];
		}
		resumeAll() {}
		retryRelay() {
			fake.retried += 1;
		}
		join(...args: unknown[]) {
			fake.joined.push(args);
			this.prompt = { session: 's', params: { name: 'Vela' }, answer: () => {} };
			return 's';
		}
		end() {}
	}
}));
vi.mock('$lib/multisig/external/outside.svelte', async () => {
	const { OutsideTransaction: Real } = await vi.importActual<
		typeof import('$lib/multisig/external/outside.svelte')
	>('$lib/multisig/external/outside.svelte');
	return {
		OutsideTransaction: class extends Real {
			constructor(link: never) {
				super(link);
				fake.outside = this;
			}
			async refresh() {}
		}
	};
});

const { default: Page } = await import('./+page.svelte');

function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.external.connectingAs = 'as {{account}}';
	out.external.expectedMember = 'for {{accounts}}';
	out.external.notExpectedMember = '{{account}} is not {{accounts}}';
	out.external.signInWallet = 'Sign with {{wallet}}';
	out.external.inWallet = 'Confirm in {{wallet}} · {{elapsed}}';
	out.external.relayFailed = "Can't connect to the signing relay ({{reason}}).";
	out.external.relayRefused = "it refused this device's pass";
	out.external.executeWith = 'Execute with {{wallet}}';
	out.external.executeFee = 'fee {{fee}} by {{account}}';
	out.status.progress = '{{approved}} of {{threshold}}';
	out.action.send = 'Send {{amount}} {{symbol}}';
	out.external.tokenNative = '{{symbol}} native on {{network}}';
	return out;
}

const signing = new Proxy({}, { get: (_, key) => String(key) });

async function open(accounts: string[], hash: string, { wait = true } = {}) {
	fake.accounts = accounts;
	fake.joined = [];
	fake.approved = false;
	history.replaceState(null, '', `${location.pathname}${hash}`);
	const screen = render(Page, {
		props: {
			data: { multisigMessages: words(), signingMessages: signing, locale: 'en' } as any
		}
	});
	if (wait) {
		await vi.waitFor(() => expect(fake.bridge?.signer).toBeDefined(), { timeout: 15_000 });
		if (hash.includes('wss')) {
			await vi.waitFor(() => expect(fake.joined).toHaveLength(1), { timeout: 15_000 });
		}
	}
	await tick();
	return screen;
}

const button = (label: string) =>
	[...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as
		HTMLButtonElement | undefined;
const testid = (id: string) => document.querySelector(`[data-testid="${id}"]`);

beforeAll(async () => {
	await loadCore();
});

afterEach(() => {
	fake.state = 'connected';
	fake.stateSince = 0;
	history.replaceState(null, '', location.pathname);
	fake.bridge = null;
	fake.outside = null;
	fake.discover = null;
});

describe('the connect page names who connects (ms-14)', () => {
	it('joins with the members the link named, and says which account connects', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		expect(fake.joined[0]).toEqual([INVITE, [MEMBER]]);
		await vi.waitFor(() => expect(testid('connect-account')?.textContent).toContain('as 0x7099'));
		expect(testid('connect-wrong-account')).toBeNull();
		expect(button('external.connect')?.disabled).toBe(false);
		await screen.unmount();
	});

	it('refuses an account that is not one of them: aloud, with what to do, Connect off', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		await vi.waitFor(() => expect(testid('connect-wrong-account')).not.toBeNull());
		const alert = testid('connect-wrong-account')!;
		expect(alert.getAttribute('role')).toBe('alert');
		expect(alert.textContent).toContain('0x3C44');
		expect(alert.textContent).toContain('0x7099');
		expect(button('external.connect')?.disabled).toBe(true);
		await screen.unmount();
	});

	it('connected as a stranger: the refusal, never "keep this page open"', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.approved = true;
		fake.bridge.prompt = undefined;
		fake.bridge.dispatchEvent(new Event('change'));
		await tick();
		await vi.waitFor(() => expect(testid('connect-wrong-account')).not.toBeNull());
		expect(testid('connect-connected')).toBeNull();
		// The wallet switches to the member: the page carries on.
		fake.bridge.signer.accounts.splice(0, 1, MEMBER);
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(() => expect(testid('connect-connected')).not.toBeNull());
		expect(testid('connect-wrong-account')).toBeNull();
		await screen.unmount();
	});

	it('an older link names no one: nothing to refuse', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}`);
		expect(fake.joined[0]).toEqual([INVITE, []]);
		await tick();
		expect(testid('connect-wrong-account')).toBeNull();
		expect(button('external.connect')?.disabled).toBe(false);
		await screen.unmount();
	});
});

describe('the connect page says where it stands (r4)', () => {
	it('r4D-16: a spinner and "Connecting to Vela…" before anything is found', async () => {
		let release!: () => void;
		fake.discover = new Promise<void>((r) => (release = r));
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}`, { wait: false });
		const booting = testid('connect-booting');
		expect(booting?.textContent).toContain('external.connectingVela');
		expect(booting?.querySelector('.spinner')).not.toBeNull();
		release();
		await screen.unmount();
	});

	it('r4C-1: the relay refusing this page is said, with Retry', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}`);
		fake.bridge.relay = { kind: 'failed', why: 'refused', detail: 'HTTP 4403' };
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(() =>
			expect(testid('connect-relay')?.textContent).toContain(
				"Can't connect to the signing relay (it refused this device's pass)."
			)
		);
		button('external.retry')!.click();
		expect(fake.retried).toBe(1);
		await screen.unmount();
	});

	it("r4C-7: Vela's request is reviewed before the wallet is asked — the multisig, network, what, number, hashes", async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.bridge.prompt = undefined;
		fake.approved = true;
		let answered: boolean | null = null;
		const entry = {
			id: 'r1',
			at: Date.now(),
			session: 's',
			method: 'eth_signTypedData_v4',
			account: MEMBER,
			safeTx: { chainId: 5042002, safe: SAFE, tx: TX },
			stage: 'review'
		};
		fake.bridge.activity = [entry];
		fake.bridge.request = { entry, answer: (go: boolean) => (answered = go) };
		fake.bridge.dispatchEvent(new Event('prompt'));
		await vi.waitFor(() => expect(testid('connect-review')).not.toBeNull());
		const hashes = safeTxHashes(5042002, SAFE, TX);
		expect(testid('review-safe')?.textContent).toBe(SAFE);
		expect(testid('review-network')?.textContent).toContain('Arc');
		expect(testid('review-recipient')?.textContent).toBe(TX.to);
		await vi.waitFor(() => expect(testid('review-title')?.textContent).toContain('Send 0.01'));
		expect(testid('review-nonce')?.textContent).toContain('3');
		expect(testid('hash-safe-tx')?.textContent).toBe(hashes.safeTxHash);
		expect(testid('hash-domain')?.textContent).toBe(hashes.domainHash);
		expect(testid('hash-message')?.textContent).toBe(hashes.messageHash);
		// The wallet is asked on the tap, not before.
		expect(answered).toBeNull();
		button('Sign with MetaMask')!.click();
		expect(answered).toBe(true);
		await screen.unmount();
	});

	it('r4C-3 / r4C-12: the wait in the wallet is timed; then "Signed ✓" and each outcome', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.bridge.prompt = undefined;
		fake.approved = true;
		const waiting = {
			id: 'r1',
			at: Date.now() - 25_000,
			walletAt: Date.now() - 25_000,
			session: 's',
			method: 'eth_signTypedData_v4',
			account: MEMBER,
			stage: 'wallet'
		};
		fake.bridge.activity = [waiting];
		fake.bridge.dispatchEvent(new Event('activity'));
		await vi.waitFor(() =>
			expect(testid('connect-in-wallet')?.textContent).toMatch(/Confirm in MetaMask · 0:2\d/)
		);
		expect(testid('connect-still-waiting')).not.toBeNull();
		fake.bridge.activity = [
			{ ...waiting, stage: 'answered', outcome: 'done', form: 'typedData' },
			{ ...waiting, id: 'r0', stage: 'answered', outcome: 'withdrawn' },
			{
				...waiting,
				id: 'r-1',
				stage: 'answered',
				outcome: 'rejected',
				problem: { kind: 'declined' }
			}
		];
		fake.bridge.dispatchEvent(new Event('activity'));
		await vi.waitFor(() => expect(testid('connect-signed')).not.toBeNull());
		const list = testid('connect-activity')!.textContent ?? '';
		expect(list).toContain('external.outcomeDone');
		expect(list).toContain('external.outcomeWithdrawn');
		expect(list).toContain('external.walletDeclined');
		await screen.unmount();
	});
});

describe("Vela's side, said when it is not there (r4C-4, r4C-15)", () => {
	it('r4C-15: Vela leaves while a request is in the wallet: "Vela went offline — reconnecting…"', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.bridge.prompt = undefined;
		fake.approved = true;
		fake.bridge.activity = [
			{
				id: 'r1',
				at: Date.now(),
				walletAt: Date.now(),
				session: 's',
				method: 'eth_signTypedData_v4',
				account: MEMBER,
				stage: 'wallet'
			}
		];
		fake.state = 'waiting';
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(() =>
			expect(testid('connect-vela-gone')?.textContent).toContain('external.velaOffline')
		);
		await screen.unmount();
	});

	it('r4C-4: on the relay and Vela not there for a while: said, not an endless wait', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.bridge.prompt = undefined;
		fake.state = 'waiting';
		fake.stateSince = Date.now() - 15_000;
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(
			() => expect(testid('connect-vela-absent')?.textContent).toBe('external.velaNotConnected'),
			{ timeout: 3_000 }
		);
		expect(testid('connect-waiting-vela')).toBeNull();
		await screen.unmount();
	});
});

describe("the link's own transaction, from here (r4C-2, r4C-9)", () => {
	const link = `#for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.0x${'ab'.repeat(32)}`;

	function ready(approved: number, owners = [MEMBER, STRANGER], findings: unknown[] = []) {
		const hashes = safeTxHashes(5042002, SAFE, TX);
		fake.outside.load = {
			kind: 'ready',
			view: {
				chainId: 5042002,
				safe: SAFE,
				tx: TX,
				hashes,
				genesis: null,
				findings,
				members: { deployed: true, owners, threshold: 2, nonce: 3n },
				item: {
					safeTxHash: hashes.safeTxHash,
					status: approved >= 2 ? 'ready' : 'pending',
					approved,
					threshold: 2,
					nonce: 3n,
					renameUndeployed: false,
					review: { action: { kind: 'transfer' }, refusal: null },
					approvals:
						approved > 0
							? [{ owner: STRANGER, counts: true, approval: { owner: STRANGER }, createdMs: 1 }]
							: []
				}
			}
		};
	}

	it('reads it without Vela, and offers the wallet connect, then Sign', async () => {
		const screen = await open([], link);
		await vi.waitFor(() => expect(fake.outside).not.toBeNull());
		ready(0);
		await vi.waitFor(() => expect(testid('connect-outside')?.textContent).toContain('0 of 2'));
		expect(button('external.connectWallet')).toBeDefined();
		fake.bridge.signer.accounts.splice(0, 0, MEMBER);
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(() => expect(button('Sign with MetaMask')).toBeDefined());
		expect(button('Execute with MetaMask')).toBeUndefined();
		await screen.unmount();
	});

	it('with the threshold met counting the owner: Execute, its fee said as paid by the wallet', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.outside).not.toBeNull());
		fake.outside.fee = async () => ({ wei: 21000n * 1_000_000_000n, count: 1, partial: false });
		ready(1);
		await vi.waitFor(() => expect(button('Execute with MetaMask')).toBeDefined());
		await vi.waitFor(() => expect(testid('outside-fee')?.textContent).toContain('fee 0.000021'));
		expect(button('external.approveOnly')).toBeDefined();
		await screen.unmount();
	});

	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('§3.5: a genesis refused on that network — said, and nothing is offered', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.outside).not.toBeNull());
		ready(
			1,
			[MEMBER, STRANGER],
			[{ from: 'genesis', chainId: 5042002, reason: { kind: 'module', address: MODULE } }]
		);
		await vi.waitFor(() => expect(testid('authority-refused')).not.toBeNull());
		expect(button('Execute with MetaMask')).toBeUndefined();
		expect(button('Sign with MetaMask')).toBeUndefined();
		expect(button('external.approveOnly')).toBeUndefined();
		await screen.unmount();
	});

	it('§3.5: a risk on that network — said, and each action asks first', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.outside).not.toBeNull());
		let approved = 0;
		fake.outside.approve = async () => void (approved += 1);
		ready(
			0,
			[MEMBER, STRANGER],
			[{ from: 'chain', chainId: 5042002, risk: { kind: 'module', address: MODULE } }]
		);
		await vi.waitFor(() => expect(testid('authority-risk')).not.toBeNull());
		await vi.waitFor(() => expect(button('Sign with MetaMask')).toBeDefined());
		button('Sign with MetaMask')!.click();
		await vi.waitFor(() => expect(testid('outside-risk-confirm')).not.toBeNull());
		expect(approved, 'nothing asked of the wallet before the tap').toBe(0);
		button('authority.signRiskCancel')!.click();
		await vi.waitFor(() => expect(testid('outside-risk-confirm')).toBeNull());
		expect(approved).toBe(0);
		button('Sign with MetaMask')!.click();
		await vi.waitFor(() => expect(button('authority.signRiskConfirm')).toBeDefined());
		button('authority.signRiskConfirm')!.click();
		await vi.waitFor(() => expect(approved).toBe(1));
		await screen.unmount();
	});

	it('a member it is not: said, nothing offered', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.outside).not.toBeNull());
		ready(0, [STRANGER]);
		await vi.waitFor(() => expect(testid('outside-not-owner')).not.toBeNull());
		expect(button('Sign with MetaMask')).toBeUndefined();
		await screen.unmount();
	});
});
