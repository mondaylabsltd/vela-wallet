/* eslint-disable @typescript-eslint/no-explicit-any -- a stand-in machine host and wallet */
/**
 * The connect page in a browser, drawing the core's page machine's view (a
 * stand-in host, `fake-host.svelte.ts`: the machine itself is pinned by the
 * Rust suite).
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
 *
 * Round 6: a new link in the same tab is read at once (r5reg-2); one
 * transaction card and one message for where the page stands (r5reg-7);
 * nothing to sign once the approvals are enough (r4C-14); Execute steps back
 * for a wallet that can't pay (r5reg-13); refusals in plain words, what was
 * signed kept (r5reg-12); each request named by what it was (r4C-12); the
 * relay's trouble without hosts (r4C-1); the review numbered "#3" and named
 * (r5reg-23).
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { MULTISIG_SHAPE, type MultisigMessages } from '$lib/multisig/messages';
import {
	cancelTx,
	multisigReview,
	safeTxHashes,
	type MultisigReviewOut,
	type SafeTx
} from '$lib/multisig/core/kernel';
import type { OutsideActivityRow } from '$lib/multisig/generated/OutsideActivityRow';
import type { OutsideTxShown } from '$lib/multisig/generated/OutsideTxShown';
import type { OutsideTxTitle } from '$lib/multisig/generated/OutsideTxTitle';
import type { OutsideActivityEntry } from '$lib/multisig/generated/OutsideActivityEntry';
import type { OutsideActions } from '$lib/multisig/generated/OutsideActions';
import type { OutsideGate } from '$lib/multisig/generated/OutsideGate';
import type { OutsideSessionRow } from '$lib/multisig/generated/OutsideSessionRow';
import type { OutsideTxView } from '$lib/multisig/generated/OutsideTxView';
import type { FakeHost } from './fake-host.svelte';

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
	host: null as any,
	discover: null as null | Promise<void>,
	/** Every link the page handed the machine, in order. */
	links: [] as any[],
	refreshed: 0,
	/** An execution's fee and the wallet's coin, as the machine reads them (`null`: unknown). */
	fee: null as null | { wei: string; count: number; partial: boolean; maxWei: string },
	balance: null as null | bigint,
	/** §3.5: the machine asks about the risk before an action. */
	risky: false,
	/** Actions the machine carried out (after any risk question). */
	acted: [] as string[]
}));

vi.mock('$lib/ui/history', () => ({ replaceUrl: () => {} }));
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
vi.mock('$lib/multisig/shell/outside-page.svelte', async () => {
	const actual = await vi.importActual<typeof import('$lib/multisig/shell/outside-page.svelte')>(
		'$lib/multisig/shell/outside-page.svelte'
	);
	const { FakeHost } = await import('./fake-host.svelte');
	return {
		...actual,
		OutsidePageHost: class extends FakeHost {
			constructor() {
				super((host, event) => {
					switch (event.type) {
						case 'join':
							// As PeerConnect reads an invite: one changed on the way does not open.
							if (event.invite.includes('DAMAGED')) {
								host.set({ joinFailed: 'invalid invite' });
								return;
							}
							fake.joined.push([event.invite, event.members]);
							host.set({
								joinFailed: null,
								sessions: [session()],
								connectPrompt: { session: 's', dapp: { name: 'Vela' } }
							});
							return;
						case 'link':
							fake.links.push(event.link);
							host.set({ link: event.link, load: { kind: 'loading' }, tx: null });
							return;
						case 'refresh':
							fake.refreshed += 1;
							return;
						case 'estimate_fee':
							host.set({ fee: { account: event.account, fee: fake.fee } });
							return;
						case 'balance':
							host.set({
								balance: {
									account: event.account,
									wei: fake.balance === null ? null : String(fake.balance)
								}
							});
							return;
						case 'approve':
						case 'execute':
						case 'reject':
							if (!fake.risky) {
								fake.acted.push(event.type);
								return;
							}
							host.risk = {
								chainId: 5042002,
								findings: (host.view.tx?.gate as any)?.findings ?? [],
								answer: (signs) => {
									host.risk = null;
									if (signs) fake.acted.push(event.type);
								}
							};
					}
				});
				fake.host = this;
			}
		}
	};
});

const { default: Page } = await import('./+page.svelte');

const host = () => fake.host as FakeHost;

/** The live session, as the machine lists it. */
function session(over: Partial<OutsideSessionRow> = {}): OutsideSessionRow {
	return {
		id: 's',
		state: 'connected',
		dapp: { name: 'Vela' },
		approved: false,
		declined: false,
		created: 1,
		stateSince: Date.now(),
		relay: { kind: 'up' },
		...over
	};
}

/** Paired with Vela: its connection approved, nothing asking to connect. */
function pair(over: Partial<OutsideSessionRow> = {}) {
	host().set({ connectPrompt: null, sessions: [session({ approved: true, ...over })] });
}

function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	// §12 and r8-1: the review's nested words, each its own key.
	for (const nested of ['state', 'reading'] as const) {
		out.review[nested] = Object.fromEntries(
			Object.keys(MULTISIG_SHAPE.review[nested]).map((k) => [k, `review.${nested}.${k}`])
		);
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
	out.external.executeFeeMax = 'fee {{fee}} up to {{max}} by {{account}}';
	out.external.executeConfirms = '{{total}} prompts';
	out.status.progress = '{{approved}} of {{threshold}}';
	out.action.send = 'Send {{amount}} {{symbol}}';
	out.external.tokenNative = '{{symbol}} native on {{network}}';
	out.action.numberValue = '#{{nonce}}';
	out.external.rowApprove = 'Approve {{number}} · {{what}}';
	out.external.rowApproveTo = 'Approve {{number}} · {{what}} to {{to}}';
	out.external.rowExecute = 'Execute {{number}}';
	out.external.rowReject = 'Reject {{number}}';
	out.external.executeNeedsGas = 'needs {{fee}}, holds {{balance}}';
	out.change.rule = 'Approvals: {{before}} → {{after}}';
	out.create.thresholdValue = '{{m}} of {{n}}';
	out.change.add = 'Add {{member}}';
	out.external.relayRetrying = "Can't connect ({{reason}}) — in {{seconds}} s";
	return out;
}

const signing = new Proxy({}, { get: (_, key) => String(key) });

async function open(accounts: string[], hash: string, { wait = true } = {}) {
	fake.accounts = accounts;
	fake.joined = [];
	history.replaceState(null, '', `${location.pathname}${hash}`);
	const screen = render(Page, {
		props: {
			data: { multisigMessages: words(), signingMessages: signing, locale: 'en' } as any
		}
	});
	if (wait) {
		await vi.waitFor(() => expect(fake.host?.walletUsed).toBe(true), { timeout: 15_000 });
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
const answers = () => host().events.filter((e) => e.type === 'answer_request') as any[];

beforeAll(async () => {
	await loadCore();
});

afterEach(() => {
	history.replaceState(null, '', location.pathname);
	fake.host = null;
	fake.discover = null;
	fake.links = [];
	fake.refreshed = 0;
	fake.fee = null;
	fake.balance = null;
	fake.risky = false;
	fake.acted = [];
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
		pair();
		await tick();
		await vi.waitFor(() => expect(testid('connect-wrong-account')).not.toBeNull());
		expect(testid('connect-connected')).toBeNull();
		// The wallet switches to the member: the page carries on.
		host().set({ accounts: [MEMBER] });
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

/** A request Vela sent through the pairing, as the machine logs it. */
function asked(over: Partial<OutsideActivityEntry> = {}): OutsideActivityEntry {
	return {
		id: `r-${Math.random()}`,
		at: Date.now(),
		session: 's',
		origin: null,
		method: 'eth_signTypedData_v4',
		account: MEMBER,
		safeTx: { chainId: 5042002, safe: SAFE, tx: TX },
		send: null,
		stage: 'review',
		walletAt: null,
		outcome: null,
		problem: null,
		form: null,
		result: null,
		...over
	};
}

/** TX, as the core titles it: a send of the network coin. */
const SEND: OutsideTxTitle = {
	kind: 'send',
	to: TX.to,
	amount: '10000000000000000',
	token: { token: null, symbol: null, decimals: 18, known: true }
};

/** A SafeTx as the core page machine shows it for review (`TxShown`). */
function shownOf(tx: SafeTx, over: Partial<OutsideTxShown> = {}): OutsideTxShown {
	return {
		chainId: 5042002,
		safe: SAFE,
		nonce: BigInt(tx.nonce).toString(),
		hashes: safeTxHashes(5042002, SAFE, tx),
		name: null,
		title: SEND,
		// §12: read — the core's own review, as the page machine gathers it.
		review: multisigReview({
			chainId: 5042002,
			safe: SAFE,
			tx,
			owners: [MEMBER, STRANGER, ALSO],
			threshold: 2,
			deployed: true
		}),
		reviewState: { kind: 'ready' },
		chainNonce: '3',
		waitsFor: [],
		...over
	};
}

/** A row of the list, as the core says it. */
function row(over: Partial<OutsideActivityRow> = {}): OutsideActivityRow {
	return {
		id: `row-${Math.random()}`,
		at: Date.now(),
		label: { kind: 'approveTo', chainId: 5042002, nonce: '3', title: SEND, to: TX.to },
		outcome: { kind: 'waiting' },
		bad: false,
		...over
	};
}

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
		host().set({ relay: { kind: 'failed', why: 'refused', detail: 'HTTP 4403' } });
		await vi.waitFor(() =>
			expect(testid('connect-relay')?.textContent).toContain(
				"Can't connect to the signing relay (it refused this device's pass)."
			)
		);
		button('external.retry')!.click();
		expect(host().retried).toBe(1);
		await screen.unmount();
	});

	it("r4C-7: Vela's request is reviewed before the wallet is asked — the multisig, network, what, number, hashes", async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const entry = asked({ id: 'r1' });
		host().set({ activity: [entry], request: entry, requestReview: shownOf(TX) });
		await vi.waitFor(() => expect(testid('connect-review')).not.toBeNull());
		const hashes = safeTxHashes(5042002, SAFE, TX);
		expect(testid('review-safe')?.textContent).toBe(SAFE);
		expect(testid('review-network')?.textContent).toContain('Arc');
		expect(testid('review-recipient')?.textContent).toBe(TX.to);
		// §12: the core's own review is read — its reading heads it.
		await vi.waitFor(() => expect(testid('review-title')?.textContent).toContain('0.01'));
		expect(testid('review-nonce')?.textContent).toContain('3');
		// The review's own hashes (§12: read before the wallet is asked).
		const drawn = testid('review-hashes')?.textContent ?? '';
		expect(drawn).toContain(hashes.safeTxHash);
		expect(drawn).toContain(hashes.domainHash);
		expect(drawn).toContain(hashes.messageHash);
		// The wallet is asked on the tap, not before.
		expect(answers()).toHaveLength(0);
		button('Sign with MetaMask')!.click();
		expect(answers()).toEqual([{ type: 'answer_request', session: 's', id: 'r1', go: true }]);
		await screen.unmount();
	});

	it('§12: a request whose review is read is signed only then — shut while read, blocked when it failed, Try again', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const entry = asked({ id: 'r1' });
		const sign = () => button('Sign with MetaMask') as HTMLButtonElement | null;
		host().set({
			activity: [entry],
			request: entry,
			requestReview: shownOf(TX, { review: null, reviewState: { kind: 'loading' } })
		});
		await vi.waitFor(() => expect(testid('review-loading')).not.toBeNull());
		expect(testid('review-loading')?.textContent).toContain('review.state.loading');
		expect(sign()?.disabled).toBe(true);
		host().set({
			requestReview: shownOf(TX, {
				review: null,
				reviewState: { kind: 'failed', reason_key: 'home.multisig.review.state.timedOut' }
			})
		});
		await vi.waitFor(() => expect(testid('review-failed')).not.toBeNull());
		expect(testid('review-failed')?.textContent).toContain('review.state.timedOut');
		expect(testid('review-failed')?.textContent).toContain('review.state.blocked');
		expect(sign()?.disabled).toBe(true);
		sign()?.click();
		expect(answers()).toHaveLength(0);
		button('review.state.tryAgain')!.click();
		expect(host().events.filter((e) => e.type === 'review_again')).toHaveLength(1);
		host().set({ requestReview: shownOf(TX) });
		await vi.waitFor(() => expect(testid('review-failed')).toBeNull());
		expect(sign()?.disabled).toBe(false);
		sign()!.click();
		expect(answers()).toEqual([{ type: 'answer_request', session: 's', id: 'r1', go: true }]);
		await screen.unmount();
	});

	it('r4C-3 / r4C-12: the wait in the wallet is timed; then "Signed ✓" and each outcome', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const waiting = asked({
			id: 'r1',
			at: Date.now() - 25_000,
			walletAt: Date.now() - 25_000,
			safeTx: null,
			stage: 'wallet'
		});
		host().set({ activity: [waiting] });
		await vi.waitFor(() =>
			expect(testid('connect-in-wallet')?.textContent).toMatch(/Confirm in MetaMask · 0:2\d/)
		);
		expect(testid('connect-still-waiting')).not.toBeNull();
		host().set({
			activity: [
				{ ...waiting, stage: 'answered', outcome: 'done', form: 'typedData' },
				{ ...waiting, id: 'r0', stage: 'answered', outcome: 'withdrawn' },
				{
					...waiting,
					id: 'r-1',
					stage: 'answered',
					outcome: 'rejected',
					problem: { kind: 'declined' }
				}
			]
		});
		host().set({
			rows: [
				row({ outcome: { kind: 'done' } }),
				row({ outcome: { kind: 'withdrawn' } }),
				row({ outcome: { kind: 'walletDeclined' } })
			]
		});
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
		host().set({
			activity: [asked({ id: 'r1', walletAt: Date.now(), safeTx: null, stage: 'wallet' })]
		});
		pair({ state: 'waiting' });
		await vi.waitFor(() =>
			expect(testid('connect-vela-gone')?.textContent).toContain('external.velaOffline')
		);
		await screen.unmount();
	});

	it('r4C-4: on the relay and Vela not there for a while: said, not an endless wait', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		host().set({
			connectPrompt: null,
			sessions: [session({ state: 'waiting', stateSince: Date.now() - 15_000 })]
		});
		await vi.waitFor(
			() => expect(testid('connect-vela-absent')?.textContent).toBe('external.velaNotConnected'),
			{ timeout: 3_000 }
		);
		expect(testid('connect-waiting-vela')).toBeNull();
		await screen.unmount();
	});
});

const ALSO = '0x90f79bf6eb2c4f870365e785982e1f101e93b906';
const approval = (owner: string) => ({ owner, counts: true, approval: { owner }, createdMs: 1 });

/** The gate the core gives for these findings on the transaction's network. */
function gateOf(findings: any[]): OutsideGate {
	const refused = findings.filter((f) => f.from === 'genesis');
	if (refused.length > 0) return { kind: 'refused', findings: refused };
	const risks = findings.filter((f) => f.from === 'chain');
	return risks.length > 0 ? { kind: 'risk', findings: risks } : { kind: 'open' };
}

/** What `account` may do, as the core says it for this view (`OutsideTxView::actions`). */
function actionsFor(account: string | undefined, tx: OutsideTxView): OutsideActions | null {
	if (!account) return null;
	const owner = tx.members.owners.includes(account);
	const has = tx.item.approvals.some((a) => a.counts && a.owner === account);
	const open = tx.item.status !== 'done' && (tx.gate.kind === 'open' || tx.gate.kind === 'risk');
	const enough = tx.item.approved >= tx.item.threshold;
	const canApprove = open && owner && !has && !enough;
	const own = owner && !has ? 1 : 0;
	return {
		account,
		isOwner: owner,
		hasApproved: has,
		open,
		enough,
		canApprove,
		canApproveOnChain: canApprove,
		canExecute: open && tx.item.approved + own >= tx.item.threshold,
		canReject: open && owner
	};
}

/** The link's transaction, read: `approved` of 2, these approvals, on these owners. */
function readyView(
	approved: number,
	approvals: any[] = [],
	owners = [MEMBER, STRANGER, ALSO],
	findings: unknown[] = []
) {
	const hashes = safeTxHashes(5042002, SAFE, TX);
	const tx: OutsideTxView = {
		chainId: 5042002,
		safe: SAFE,
		tx: TX,
		hashes,
		genesis: null,
		done: null,
		findings,
		gate: gateOf(findings),
		// §12: read — the core's own review, as the page machine gathers it.
		review: multisigReview({
			chainId: 5042002,
			safe: SAFE,
			tx: TX,
			owners,
			threshold: 2,
			deployed: true
		}),
		reviewState: { kind: 'ready' },
		title: SEND,
		name: null,
		members: { deployed: true, owners, threshold: 2, nonce: '3' },
		item: {
			chainId: 5042002,
			safeTxHash: hashes.safeTxHash,
			status: approved >= 2 ? 'ready' : 'pending',
			approved,
			threshold: 2,
			nonce: '3',
			proposer: STRANGER,
			renameUndeployed: false,
			waitsFor: [],
			review: { action: { kind: 'transfer' }, refusal: null },
			approvals
		}
	};
	host().set({ load: { kind: 'ready' }, tx, actions: actionsFor(host().view.accounts[0], tx) });
}

/** As `readyView`, `approved` of 2 by the stranger, on these owners. */
function ready(approved: number, owners = [MEMBER, STRANGER], findings: unknown[] = []) {
	readyView(approved, approved > 0 ? [approval(STRANGER)] : [], owners, findings);
}

describe("the link's own transaction, from here (r4C-2, r4C-9)", () => {
	const link = `#for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.0x${'ab'.repeat(32)}`;

	it('reads it without Vela, and offers the wallet connect, then Sign', async () => {
		const screen = await open([], link);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		ready(0);
		await vi.waitFor(() => expect(testid('connect-outside')?.textContent).toContain('0 of 2'));
		expect(button('external.connectWallet')).toBeDefined();
		host().set({ accounts: [MEMBER] });
		ready(0);
		await vi.waitFor(() => expect(button('Sign with MetaMask')).toBeDefined());
		expect(button('Execute with MetaMask')).toBeUndefined();
		await screen.unmount();
	});

	it('with the threshold met counting the owner: Execute, its fee said as paid by the wallet', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		fake.fee = {
			wei: String(21000n * 1_000_000_000n),
			count: 1,
			partial: false,
			maxWei: String(2n * 21000n * 1_000_000_000n)
		};
		ready(1);
		await vi.waitFor(() => expect(button('Execute with MetaMask')).toBeDefined());
		await vi.waitFor(() => expect(testid('outside-fee')?.textContent).toContain('fee 0.000021'));
		expect(button('external.approveOnly')).toBeDefined();
		await screen.unmount();
	});

	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('§3.5: a genesis refused on that network — said, and nothing is offered', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
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
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		fake.risky = true;
		ready(
			0,
			[MEMBER, STRANGER],
			[{ from: 'chain', chainId: 5042002, risk: { kind: 'module', address: MODULE } }]
		);
		await vi.waitFor(() => expect(testid('authority-risk')).not.toBeNull());
		await vi.waitFor(() => expect(button('Sign with MetaMask')).toBeDefined());
		button('Sign with MetaMask')!.click();
		await vi.waitFor(() => expect(testid('outside-risk-confirm')).not.toBeNull());
		expect(fake.acted, 'nothing asked of the wallet before the tap').toEqual([]);
		button('authority.signRiskCancel')!.click();
		await vi.waitFor(() => expect(testid('outside-risk-confirm')).toBeNull());
		expect(fake.acted).toEqual([]);
		button('Sign with MetaMask')!.click();
		await vi.waitFor(() => expect(button('authority.signRiskConfirm')).toBeDefined());
		button('authority.signRiskConfirm')!.click();
		await vi.waitFor(() => expect(fake.acted).toEqual(['approve']));
		await screen.unmount();
	});

	it('a member it is not: said, nothing offered', async () => {
		const screen = await open([MEMBER], link);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		ready(0, [STRANGER]);
		await vi.waitFor(() => expect(testid('outside-not-owner')).not.toBeNull());
		expect(button('Sign with MetaMask')).toBeUndefined();
		await screen.unmount();
	});
});

const txHash = () => safeTxHashes(5042002, SAFE, TX).safeTxHash;

describe('round 6: a new link in the same tab (r5reg-2)', () => {
	it("reads the new link's transaction — never keeps showing the one before", async () => {
		const other = `0x${'cd'.repeat(32)}`;
		const screen = await open(
			[MEMBER],
			`#for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.${txHash()}`
		);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(0);
		await vi.waitFor(() => expect(testid('connect-outside')?.textContent).toContain('0 of 2'));
		location.hash = `#for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.${other}`;
		await vi.waitFor(() => expect(fake.links).toHaveLength(2));
		expect(fake.links[1].safeTxHash).toBe(other);
		await vi.waitFor(() =>
			expect(testid('connect-outside')?.textContent).toContain('external.loadingTx')
		);
		expect(testid('connect-outside')?.textContent).not.toContain('0 of 2');
		await screen.unmount();
	});

	it('a new invite in the same tab joins its room', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		const next = `wss://relay.getvela.app/r/def#${'B'.repeat(86)}`;
		location.hash = `#${encodeURIComponent(next)}&for=${MEMBER}`;
		await vi.waitFor(() => expect(fake.joined).toHaveLength(2));
		expect(fake.joined[1]).toEqual([next, [MEMBER]]);
		await screen.unmount();
	});
});

describe("round 6: a damaged invite is the link's, never the wallet's", () => {
	/** Reads as an invite; PeerConnect refuses it ("invalid invite"). */
	const DAMAGED = `wss://relay.getvela.app/r/DAMAGED#${'C'.repeat(86)}`;
	/** Cut short on the way: does not even read as one. */
	const CUT = `wss://relay.getvela.app/r/abc#${'A'.repeat(40)}`;
	const damaged = () => testid('connect-link-damaged')?.textContent;

	for (const [what, invite] of [
		['refused by PeerConnect', DAMAGED],
		['cut short', CUT]
	] as const) {
		it(`${what}: says the link is damaged — not "couldn't sign", not "Connecting to Vela…"`, async () => {
			const screen = await open([MEMBER], `#${encodeURIComponent(invite)}&for=${MEMBER}`, {
				wait: false
			});
			await vi.waitFor(() => expect(fake.host?.walletUsed).toBe(true), { timeout: 15_000 });
			await vi.waitFor(() => expect(damaged()).toBe('external.linkDamaged'));
			expect(testid('connect-link-damaged')?.getAttribute('role')).toBe('alert');
			expect(fake.joined).toHaveLength(0);
			expect(document.body.textContent).not.toContain('external.walletFailed');
			expect(document.body.textContent).not.toContain('external.connectingVela');
			await screen.unmount();
		});
	}

	it('a good link opened next in the same tab joins, and the line goes', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(DAMAGED)}&for=${MEMBER}`, {
			wait: false
		});
		await vi.waitFor(() => expect(fake.host?.walletUsed).toBe(true), { timeout: 15_000 });
		await vi.waitFor(() => expect(damaged()).toBe('external.linkDamaged'));
		location.hash = `#${encodeURIComponent(INVITE)}&for=${MEMBER}`;
		await vi.waitFor(() => expect(fake.joined).toHaveLength(1));
		expect(fake.joined[0]).toEqual([INVITE, [MEMBER]]);
		await vi.waitFor(() => expect(testid('connect-link-damaged')).toBeNull());
		await screen.unmount();
	});
});

describe('round 6: one card, one message (r5reg-7)', () => {
	it('signed on a paired page: "Signed ✓" with keep-open — never "you can close this page"', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		host().set({ activity: [asked({ stage: 'answered', outcome: 'done', form: 'typedData' })] });
		await vi.waitFor(() =>
			expect(testid('connect-signed')?.textContent).toContain('external.signedPaired')
		);
		expect(testid('connect-connected')?.textContent).toContain('external.keepOpen');
		expect(document.body.textContent).not.toContain('external.signedClose');
		await screen.unmount();
	});

	it("the link's transaction, once the pairing handled it, is not a second card", async () => {
		const screen = await open(
			[MEMBER],
			`#${encodeURIComponent(INVITE)}&for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.${txHash()}`
		);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(1, [approval(STRANGER)]);
		pair();
		// Vela asks for another transaction: its review is the one card.
		const next = { ...TX, nonce: '0x4' };
		const other = asked({ safeTx: { chainId: 5042002, safe: SAFE, tx: next } });
		host().set({
			activity: [other],
			request: other,
			requestReview: shownOf(next, { chainNonce: '3' })
		});
		await vi.waitFor(() => expect(testid('connect-request')).not.toBeNull());
		expect(testid('connect-outside')).toBeNull();
		expect(document.querySelectorAll('[data-testid="connect-review"]')).toHaveLength(1);
		// The link's own transaction signed through the pairing: read again, and not shown twice.
		const refreshed = fake.refreshed;
		host().set({
			request: null,
			activity: [
				asked({ stage: 'answered', outcome: 'done', form: 'typedData' }),
				{ ...other, stage: 'answered', outcome: 'withdrawn' }
			]
		});
		await vi.waitFor(() => expect(fake.refreshed).toBeGreaterThan(refreshed));
		await vi.waitFor(() => expect(testid('connect-signed')).not.toBeNull());
		expect(testid('connect-outside')).toBeNull();
		await screen.unmount();
	});
});

describe("round 6: Vela's request already has enough approvals (r4C-14)", () => {
	it('nothing to sign: said, and only Decline is offered', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const entry = asked();
		host().set({ activity: [entry], request: entry, requestEnough: true });
		await vi.waitFor(() =>
			expect(testid('connect-request-enough')?.textContent).toBe('external.enoughApprovals')
		);
		expect(button('Sign with MetaMask')).toBeUndefined();
		expect(button('external.decline')).toBeDefined();
		await screen.unmount();
	});

	it('still needing approvals (or not in the mailbox yet): Sign is offered', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const entry = asked();
		host().set({ activity: [entry], request: entry, requestEnough: false });
		await vi.waitFor(() => expect(button('Sign with MetaMask')).toBeDefined());
		expect(testid('connect-request-enough')).toBeNull();
		await screen.unmount();
	});
});

describe('round 6: the link transaction from here', () => {
	const link = () => `#for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.${txHash()}`;

	it('r4C-14: approvals already enough — nothing to sign; Execute is what is left', async () => {
		const screen = await open([MEMBER], link());
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(2, [approval(STRANGER), approval(ALSO)]);
		await vi.waitFor(() => expect(testid('outside-enough')).not.toBeNull());
		expect(testid('outside-enough')?.textContent).toBe('external.enoughApprovals');
		expect(button('Sign with MetaMask')).toBeUndefined();
		expect(button('external.approveOnly')).toBeUndefined();
		expect(button('Execute with MetaMask')).toBeDefined();
		await screen.unmount();
	});

	it("r5reg-13: a wallet that can't pay the fee: told what it needs, Execute steps back", async () => {
		fake.fee = {
			wei: String(21000n * 1_000_000_000n),
			count: 1,
			partial: false,
			maxWei: String(2n * 21000n * 1_000_000_000n)
		};
		fake.balance = 0n;
		const screen = await open([MEMBER], link());
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(1, [approval(STRANGER)]);
		await vi.waitFor(() =>
			expect(testid('outside-cannot-pay')?.textContent).toContain('needs 0.000021')
		);
		expect(testid('outside-cannot-pay')?.textContent).toContain('holds 0');
		const execute = button('Execute with MetaMask')!;
		expect(execute.disabled).toBe(true);
		expect(execute.className).not.toContain('value');
		// Approving is the main action instead.
		expect(button('Sign with MetaMask')).toBeDefined();
		await screen.unmount();
	});

	it('r5reg-13: a wallet that can pay: Execute is the main action, its fee said', async () => {
		fake.fee = {
			wei: String(21000n * 1_000_000_000n),
			count: 1,
			partial: false,
			maxWei: String(2n * 21000n * 1_000_000_000n)
		};
		fake.balance = 10n ** 18n;
		const screen = await open([MEMBER], link());
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(1, [approval(STRANGER)]);
		await vi.waitFor(() => expect(testid('outside-fee')?.textContent).toContain('fee 0.000021'));
		expect(button('Execute with MetaMask')!.disabled).toBe(false);
		expect(testid('outside-cannot-pay')).toBeNull();
		expect(button('external.approveOnly')).toBeDefined();
		await screen.unmount();
	});

	it("r5reg-12: a refusal in plain words, never the service's text; what was signed kept, Send again", async () => {
		const screen = await open([MEMBER], link());
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		readyView(0);
		host().set({
			step: {
				kind: 'failed',
				what: 'reject',
				problem: null,
				reason: {
					kind: 'refused',
					detail: 'the proxy code of factory 0x4e1D on chain 5042002 could not be read'
				},
				refused: 'setupUnchecked',
				unsent: true
			}
		});
		await vi.waitFor(() =>
			expect(testid('outside-failed')?.textContent).toBe('external.refusedSetupUnchecked')
		);
		expect(document.body.textContent).not.toContain('proxy code');
		expect(testid('outside-kept')?.textContent).toBe('external.keptSigned');
		expect(button('external.sendAgain')).toBeDefined();
		await screen.unmount();
	});

	it('r4C-12: each row says what it was — the number, what it does, and how it ended', async () => {
		const screen = await open(
			[MEMBER],
			`#${encodeURIComponent(INVITE)}&for=${MEMBER}&ms=5042002.${SAFE.toLowerCase()}.${txHash()}`
		);
		await vi.waitFor(() => expect(fake.links).toHaveLength(1));
		pair();
		host().set({
			activity: [
				asked({
					id: 'reject',
					stage: 'answered',
					outcome: 'rejected',
					problem: { kind: 'declined' },
					safeTx: { chainId: 5042002, safe: SAFE, tx: cancelTx(SAFE, '0x8') }
				}),
				asked({ id: 'approve', stage: 'answered', outcome: 'done', form: 'typedData' })
			],
			rows: [
				row({
					id: 'reject',
					label: { kind: 'reject', nonce: '8' },
					outcome: { kind: 'walletDeclined' }
				}),
				row({ id: 'approve', outcome: { kind: 'done' } })
			]
		});
		await vi.waitFor(() =>
			expect(testid('connect-activity')?.textContent).toMatch(
				/Approve #3 · Send 0\.01 \S+ to 0x0000\S*dEaD/
			)
		);
		const list = testid('connect-activity')!.textContent ?? '';
		expect(list).toContain('Reject #8');
		expect(list).toContain('external.walletDeclined');
		expect(list).toContain('external.outcomeDone');
		expect(list).not.toContain('external.signTx');
		// What was done from here is a row too.
		readyView(0);
		host().set({
			step: { kind: 'executed', txHash: `0x${'77'.repeat(32)}` },
			rows: [
				row({ id: 'here-1', label: { kind: 'execute', nonce: '3' }, outcome: { kind: 'sent' } }),
				...host().view.rows
			]
		});
		await vi.waitFor(() => expect(testid('connect-activity')?.textContent).toContain('Execute #3'));
		await screen.unmount();
	});
});

describe('round 6: the relay and Vela, said plainly', () => {
	it('r4C-1: no pass for the relay — plain words, no hosts, with Retry', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}`);
		host().set({
			relay: {
				kind: 'retrying',
				why: 'noToken',
				detail: '127.0.0.1:5191: no token in the answer · peerconnect.dev: no token in the answer',
				at: Date.now() + 20_000
			}
		});
		await vi.waitFor(() =>
			expect(testid('connect-relay')?.textContent).toContain('external.relayNoToken')
		);
		expect(testid('connect-relay')?.textContent).not.toContain('peerconnect.dev');
		expect(testid('connect-relay')?.textContent).not.toContain('127.0.0.1');
		expect(button('external.retry')).toBeDefined();
		await screen.unmount();
	});

	it('r4C-15: "Vela went offline" clears as soon as Vela is back', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		host().set({ activity: [asked({ stage: 'wallet', walletAt: Date.now() })] });
		pair({ state: 'waiting' });
		await vi.waitFor(() => expect(testid('connect-vela-gone')).not.toBeNull());
		pair({ state: 'connected' });
		await vi.waitFor(() => expect(testid('connect-vela-gone')).toBeNull(), { timeout: 2_000 });
		await screen.unmount();
	});

	it("r5reg-23: Vela's request reviewed as the link's is — named, numbered #3", async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const entry = asked();
		// The registry names the multisig (the core read it for the request).
		host().set({
			activity: [entry],
			request: entry,
			requestReview: shownOf(TX, { name: 'R5 Team' })
		});
		await vi.waitFor(() => expect(testid('review-name')?.textContent).toBe('R5 Team'));
		expect(testid('review-name-source')?.textContent).toBe('external.nameFromRecord');
		expect(testid('review-nonce')?.textContent?.trim()).toBe('#3');
		await screen.unmount();
	});
});

describe('r7-11: the outside owner reads the member change and its rule', () => {
	it('an addOwner request shows Approvals: 2 of 3 → 2 of 4, from the core review', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		pair();
		const NEWCOMER = '0x15d34aaf54267db7d7c367839aaf71a00a2c6a65';
		const add: SafeTx = {
			...TX,
			to: SAFE,
			value: '0x0',
			data: `0x0d582f13${NEWCOMER.slice(2).padStart(64, '0')}${(2).toString(16).padStart(64, '0')}`
		};
		// The core own review, as the page machine gathers it.
		const review: MultisigReviewOut = multisigReview({
			chainId: 5042002,
			safe: SAFE,
			tx: add,
			owners: [MEMBER, STRANGER, ALSO],
			threshold: 2,
			deployed: true
		});
		const entry = asked({ safeTx: { chainId: 5042002, safe: SAFE, tx: add } });
		host().set({
			activity: [entry],
			request: entry,
			requestReview: shownOf(add, { title: { kind: 'addOwner', owner: NEWCOMER }, review })
		});
		await vi.waitFor(() =>
			expect(testid('member-change-rule')?.textContent).toBe('Approvals: 2 of 3 → 2 of 4')
		);
		expect(testid('member-change')?.textContent).toContain('Add');
		// One reading: the hashes are the review own.
		expect(testid('review-hashes')?.textContent).toContain(review.hashes.safeTxHash);
		await screen.unmount();
	});
});
