/* eslint-disable @typescript-eslint/no-explicit-any -- stand-ins for the core's readings */
/**
 * One multisig transaction's sheet, in a browser.
 *
 * - dapp-1: a dApp's transaction is reviewed before anything is signed —
 *   "Propose and approve" is the only door to the passkey, beside Decline.
 * - ms-5: a member who would execute can approve without executing (and
 *   without paying the fee).
 * - ms-6: the proposer, approved already, can still withdraw it.
 * - ms-3: an execution the relay refused is said at once, with a retry.
 *
 * What it reads is stood in for (the core's suites drive it); what is real
 * is the sheet.
 */
import { tick } from 'svelte';
import type { SigningMessages } from '$lib/signing/messages';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const TO = '0x000000000000000000000000000000000000dEaD';
const TX = `0x${'ee'.repeat(32)}`;

const fake = vi.hoisted(() => ({
	chain: null as any,
	draft: null as any,
	calls: [] as string[],
	execution: null as any,
	refused: false,
	/** Owners the passkey index knows: Vela accounts elsewhere (ms-11). */
	vela: [] as string[],
	/** r4a-9: owners nothing says how they sign (no code, the index holds none). */
	unsure: [] as string[],
	/** r7: smart contract members (code, no index record) — another Safe. */
	contract: [] as string[],
	/** What the chain says executed a done proposal (ms-9, ms-10). */
	executed: undefined as string | null | undefined,
	/** What the chain says executed another hash (the cancellation's, ms-9). */
	executedBy: {} as Record<string, string | null | undefined>,
	/** The shown multisig's other networks, as `proposals` reads them. */
	chains: [] as any[],
	followed: null as string | null,
	/** Where an approval signed here stands, while the mailbox lacks it. */
	kept: null as any,
	sentAgain: 0,
	/** §3.5: what this session's check of the network found (`[]`: only its members act). */
	checked: [] as any[],
	/** The core's reading of a member change (r4a-1). */
	memberReading: null as any,
	/** What an executed transaction's receipt says (r4a-8). */
	frozen: null as any,
	/** What this device recorded when it executed one. */
	remembered: [] as any[],
	/** The core's ruling on a token nobody vouches for (r4a-2). */
	tokenNote: null as any,
	approveError: null as any,
	/** r4b-1: where the core says this device's execution ended. */
	verdict: null as any,
	/** r4b-7: an execution sent before a reload, still in flight. */
	inflight: null as any,
	ended: [] as string[],
	/** r4b-3: networks that did not answer. */
	unreadable: [] as number[],
	/** Each executor's look at the execution (r4-5, r4-7), by address. */
	previews: {} as Record<string, any>,
	/** The look each execute was handed. */
	executedWith: [] as any[],
	/** What execute throws before its passkey (r4-11). */
	executeThrows: null as any,
	/** Sheets holding the shown multisig's faster poll (r5reg-14). */
	held: 0
}));

vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		loading: false,
		get unreadable() {
			return fake.unreadable;
		},
		chain: () => fake.chain,
		refresh: async () => {},
		// r5reg-14: the faster pace an open sheet on a waiting transaction holds.
		watch: () => {
			fake.held += 1;
			return () => void (fake.held -= 1);
		},
		get chains() {
			return fake.chains;
		},
		get followed() {
			return fake.followed;
		},
		get items() {
			return fake.chain?.items ?? [];
		}
	},
	outbox: {
		stateOf: () => fake.kept,
		sendAgain: async () => {
			fake.sentAgain += 1;
			fake.kept = null;
			return [];
		}
	},
	approve: async () => {
		if (fake.approveError) throw fake.approveError;
		fake.calls.push('approve');
	},
	approveExternally: async () => void fake.calls.push('external'),
	execute: async (_a: unknown, _i: unknown, _o: unknown, _m: unknown, preview: unknown) => {
		fake.executedWith.push(preview);
		if (fake.executeThrows) throw fake.executeThrows;
		fake.calls.push('execute');
		return fake.execution;
	},
	executionInput: (_a: unknown, item: any, executor: any, members: unknown) => ({
		chainId: item.chainId,
		executor,
		members
	}),
	proposeCancel: async () => void fake.calls.push('reject'),
	draftCalls: async () => fake.draft,
	MailboxUnreadableError: class extends Error {},
	MailboxRefusedError: class extends Error {},
	NoOutsideSignerError: class extends Error {}
}));
// §3.5: the network's check, as the chain would answer it (`recheck.test.ts` reads it).
vi.mock('../core/recheck', () => ({
	checkOn: async () => fake.checked,
	findingsOn: () => []
}));
// §3.5, §3.6: the gate, its rule as the core's (`gate.test.ts` drives the real one).
vi.mock('../core/gate.svelte', () => {
	class SigningRefusedError extends Error {
		constructor(
			readonly chainId: number,
			readonly findings: unknown[]
		) {
			super('refused');
		}
	}
	class SigningUncheckedError extends Error {
		constructor(readonly chainId: number) {
			super('unchecked');
		}
	}
	class SigningBlockedError extends Error {
		constructor(readonly chainId: number) {
			super('blocked');
		}
	}
	const gateOf = (found: any[], chainId: number) => {
		const here = found.filter((f) => f.chainId === chainId);
		const refused = here.filter((f) => f.from === 'genesis');
		if (refused.length > 0) return { kind: 'refused', chainId, findings: refused };
		const risks = here.filter((f) => f.from === 'chain');
		return risks.length > 0 ? { kind: 'risk', chainId, findings: risks } : { kind: 'open' };
	};
	return {
		gateOf,
		signingGate: () => ({ kind: 'open' }),
		declinedRisk: (e: unknown) =>
			e instanceof DOMException && e.message === 'The person did not sign for the risk',
		SigningRefusedError,
		SigningUncheckedError,
		SigningBlockedError
	};
});
/**
 * §3.6: the one review, as the core would give it (`multisig_review`'s suites
 * and `review-blocks.test.ts` drive the real one) — `fake.review` overrides.
 */
const REVIEW = vi.hoisted(() => ({
	sign: { allowed: true, anyway: false },
	state: { source: 'trace', verdict: { kind: 'unchanged' } },
	recipients: [],
	hashes: {
		domainHash: `0x${'d0'.repeat(32)}`,
		messageHash: `0x${'e0'.repeat(32)}`,
		safeTxHash: `0x${'ab'.repeat(32)}`,
		ethSignHash: `0x${'f0'.repeat(32)}`
	},
	fields: [
		{ name: 'to', value: '0x000000000000000000000000000000000000dEaD' },
		{ name: 'operation', value: '0' },
		{ name: 'nonce', value: '0' }
	],
	typedData: { primaryType: 'SafeTx', domain: { chainId: 100 }, message: {} },
	simulation: { kind: 'not_offered', deltas: [] },
	simulationSource: 'none',
	concerns: [],
	tone: 'ok',
	adviceRequest: { id: 'multisig:100:0xab' },
	review: null,
	memberChanges: null
}));
const reviewSeam = vi.hoisted(() => ({ review: null as any, refreshed: 0 }));
vi.mock('../core/proposal-review.svelte', () => ({
	REVIEW_FRESH_MS: 60_000,
	ProposalReview: class {
		sim = null;
		failed = false;
		async refresh() {
			reviewSeam.refreshed += 1;
		}
		read() {
			return reviewSeam.review ?? REVIEW;
		}
		keepFresh() {
			return () => {};
		}
		dispose() {}
	}
}));
vi.mock('$lib/agent/advice.svelte', () => ({
	sheetBeside: { advice: null },
	proposalBeside: { publish: () => {}, withdraw: () => {} },
	openReview: () => null,
	adviceStanding: () => ({ standing: 'current' })
}));
vi.mock('../core/execute', () => {
	class ExecutorCannotPayError extends Error {}
	class ExecutionWouldFailError extends Error {
		constructor(readonly why: unknown) {
			super('would fail');
		}
	}
	class ExecutionUnpricedError extends Error {}
	class ExecutionUncheckedError extends Error {}
	class ExecutionSpentError extends Error {}
	return {
		ExecutionSpentError,
		ExecutorCannotPayError,
		ExecutionWouldFailError,
		ExecutionUnpricedError,
		ExecutionUncheckedError,
		stoppedBeforeSigning: (e: unknown) =>
			e instanceof ExecutorCannotPayError ||
			e instanceof ExecutionWouldFailError ||
			e instanceof ExecutionUnpricedError ||
			e instanceof ExecutionUncheckedError,
		relayRefused: async () => fake.refused
	};
});
vi.mock('../core/preflight.svelte', () => ({
	PREVIEW_FRESH_MS: 15_000,
	UNPRICED_FRESH_MS: 5_000,
	preflight: {
		get: (input: any) => fake.previews[input.executor.address] ?? null,
		check: async (input: any) => fake.previews[input.executor.address] ?? { kind: 'unpriced' }
	}
}));
vi.mock('../core/chain', () => ({ ChainUnavailableError: class extends Error {} }));
vi.mock('../core/service', () => ({ ServiceUnavailableError: class extends Error {} }));
vi.mock('../core/execution-watch.svelte', () => ({
	executions: {
		start: (flight: unknown) => flight,
		get: () => fake.inflight,
		end: (_c: number, _m: string, hash: string) => void fake.ended.push(hash)
	},
	watchExecution: async () => {
		if (fake.verdict) return fake.verdict;
		if (fake.refused) return { kind: 'notSent' };
		return { kind: 'executed', txHash: await fake.execution.waitForTxHash() };
	}
}));
// How each member signs: this device's, a Vela account the index knows, else a plain wallet.
vi.mock('../core/signers.svelte', () => ({
	signers: {
		kindOf: (_chain: number, a: string) =>
			a.toLowerCase() === ME
				? 'here'
				: fake.vela.includes(a)
					? 'vela'
					: fake.unsure.includes(a)
						? 'unsure'
						: fake.contract.includes(a)
							? 'contract'
							: 'wallet'
	}
}));
vi.mock('../core/owners', () => ({
	PasskeyCheckError: class extends Error {},
	localOwners: (owners: string[]) =>
		owners
			.filter((o) => o.toLowerCase() === ME)
			.map((address) => ({ address, name: 'Mine', account: { address } }))
}));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => a.slice(0, 6), isVela: (a: string) => fake.vela.includes(a) }
}));
// A rename executed here reads the record's new name: not this sheet's test.
vi.mock('../core/records', () => ({
	findExecution: async (_safe: string, _chain: number, hash: string) =>
		hash in fake.executedBy ? fake.executedBy[hash] : fake.executed
}));
vi.mock('../core/display', () => ({
	actionRecipient: (a: any) => a?.to ?? null,
	actionTitle: async () => 'Contract call',
	chainName: () => 'Gnosis',
	explorerTxUrl: () => null,
	formatUnits: (v: bigint) => String(v),
	localName: () => null,
	memberWho: (a: string) => a.slice(0, 6),
	changeLines: (_m: unknown, reading: any, network: string) => ({
		changes: reading.changes.map((c: any) =>
			c.call === null ? `change.${c.kind}` : `#${c.call} · change.${c.kind}`
		),
		rule: `change.rule ${reading.before.threshold}/${reading.before.owners} → ${reading.after.threshold}/${reading.after.owners}`,
		caution: `change.caution ${network}`,
		doesNotFit: reading.fits ? null : 'change.doesNotFit'
	}),
	shortenAddress: (a: string) => a.slice(0, 8),
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 })
}));
vi.mock('../core/kernel', () => {
	const same = (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase();
	return {
		sameAddress: same,
		transfersOf: () => [],
		// The core's rule for a number used with nothing of this device's sent (multisig_watch.rs).
		executionVerdict: async (facts: any) =>
			facts.executedIn.kind === 'found'
				? { kind: 'executedByOther', txHash: facts.executedIn.txHash, paid: false }
				: facts.executedIn.kind === 'none'
					? { kind: 'replaced', paid: false }
					: { kind: 'unknown' },
		readMemberChanges: () => fake.memberReading,
		memberDrift: (was: { owners: string[] }, is: { owners: string[] }) => ({
			same:
				was.owners.length === is.owners.length &&
				was.owners.every((o) => is.owners.some((x) => same(x, o)))
		})
	};
});
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('../core/executed', () => ({
	factsOfExecution: (
		approvers: string[],
		executor: string,
		approves: boolean,
		threshold: number
	) => ({
		executor,
		threshold,
		approvers: approves ? [...approvers, executor] : approvers,
		executorApproved: approves
	}),
	frozenFacts: async () => fake.frozen,
	rememberExecution: (...args: any[]) => void fake.remembered.push(args)
}));
vi.mock('$lib/services/token-provenance', () => ({
	provenanceNote: () => fake.tokenNote
}));
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	nativeSymbol: () => 'xDAI'
}));

const { default: ProposalSheet } = await import('./ProposalSheet.svelte');

/** Every word is its own key, so a test names what it looks for. */
function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const [key, value] of Object.entries(keys as Record<string, unknown>)) {
			out[group][key] =
				typeof value === 'object' && value !== null && !('forms' in value)
					? Object.fromEntries(Object.keys(value).map((leaf) => [leaf, `${group}.${key}.${leaf}`]))
					: `${group}.${key}`;
		}
	}
	out.proposal.siteAsks = '{{host}} asks for this transaction.';
	out.proposal.failed = 'Failed: {{reason}}';
	return out;
}

const members = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: true };
const local = { address: ME, name: 'Mine', account: { address: ME } };

function item(overrides: Record<string, unknown> = {}) {
	return {
		chainId: 100,
		safeTxHash: HASH,
		tx: {},
		review: { action: { kind: 'call', to: TO, value: '0', data: '0x095ea7b3' }, refusal: null },
		nonce: 0n,
		proposer: ME,
		approvals: [],
		approved: 0,
		threshold: 2,
		status: 'pending',
		canApprove: [local],
		canExecute: [],
		cancelledBy: null,
		sameNonce: [],
		executing: false,
		createdMs: 1,
		unsent: false,
		renameUndeployed: false,
		...overrides
	};
}

const account = { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2 } as any;

/** A look that says it runs, for `fee` wei, by an executor holding `balance`. */
function priced(over: Record<string, unknown> = {}) {
	return {
		kind: 'priced',
		calls: [],
		fee: { amount: 35n * 10n ** 15n, recipient: '0xfe' },
		balance: 1000n,
		canPay: true,
		carries: [OTHER],
		executorApproves: true,
		setup: { multisig: false, members: [], executor: false, amount: null },
		...over
	};
}

/** The signing sheet's words, each its own name (a proposal's call reads as that sheet does). */
const names = { get: (_: object, key: string | symbol) => String(key) };
const signing = new Proxy({} as SigningMessages, {
	get: (_, key) => (key === 'certain' ? new Proxy({}, names) : String(key))
});

function show(props: Record<string, unknown> = {}) {
	return render(ProposalSheet, {
		props: {
			signing,
			m: words(),
			account,
			chainId: 100,
			safeTxHash: HASH,
			locale: 'en',
			wide: true,
			closeLabel: 'Close',
			onclose: () => {},
			...props
		}
	});
}

const buttons = () =>
	[...document.body.querySelectorAll('button')].map((b) => b.textContent?.trim() ?? '');
const press = (label: string) =>
	[...document.body.querySelectorAll('button')]
		.find((b) => b.textContent?.trim() === label)
		?.click();
const settle = async () => {
	for (let i = 0; i < 5; i += 1) {
		await tick();
		await new Promise((r) => setTimeout(r, 0));
	}
};

beforeEach(() => {
	fake.calls = [];
	fake.held = 0;
	fake.refused = false;
	fake.vela = [];
	fake.unsure = [];
	fake.contract = [];
	fake.executed = undefined;
	fake.executedBy = {};
	fake.chains = [];
	fake.followed = null;
	fake.kept = null;
	fake.sentAgain = 0;
	fake.checked = [];
	fake.memberReading = null;
	fake.frozen = null;
	fake.remembered = [];
	fake.tokenNote = null;
	fake.approveError = null;
	fake.verdict = null;
	fake.inflight = null;
	fake.ended = [];
	fake.unreadable = [];
	fake.previews = { [ME]: priced() };
	fake.executedWith = [];
	fake.executeThrows = null;
	fake.chain = { chainId: 100, members, items: [item()], mailbox: true };
	fake.draft = { chain: { chainId: 100, members, items: [], mailbox: true }, item: item() };
	reviewSeam.review = null;
	reviewSeam.refreshed = 0;
});

describe('one review, every signer, every action (§3.6)', () => {
	const ACTIONS: [string, Record<string, unknown>][] = [
		['a transfer', { kind: 'transfer', token: null, to: TO, amount: '1' }],
		['a contract call', { kind: 'call', to: TO, value: '0', data: '0x095ea7b3' }],
		[
			'a batch',
			{
				kind: 'batch',
				calls: [
					{
						to: TO,
						value: '1',
						data: '0x',
						action: { kind: 'transfer', token: null, to: TO, amount: '1' }
					}
				]
			}
		],
		['a member change', { kind: 'addOwner', owner: OTHER, threshold: '2' }],
		['a rename', { kind: 'rename', name: 'Treasury' }],
		['a cancellation', { kind: 'cancel' }]
	];
	for (const [name, action] of ACTIONS) {
		it(`${name}: the guard, what moves, the three hashes and the technical details`, async () => {
			fake.chain = {
				chainId: 100,
				members,
				items: [item({ review: { action, refusal: null } })],
				mailbox: true
			};
			show();
			await settle();
			expect(document.querySelector('[data-testid="review-checks"]')).not.toBeNull();
			const hashes = [...document.querySelectorAll('[data-testid="review-hash"]')].map(
				(h) => h.textContent
			);
			expect(hashes).toEqual([REVIEW.hashes.domainHash, REVIEW.hashes.messageHash, HASH]);
			expect(document.querySelector('[data-testid="review-tech"]')).not.toBeNull();
			expect(document.body.textContent).toContain('review.hashesTitle');
		});
	}

	it('Bybit: the execution changes who controls it — red, and no approve, execute or override', async () => {
		reviewSeam.review = {
			...REVIEW,
			sign: { allowed: false, anyway: false },
			tone: 'refused',
			state: {
				source: 'probe',
				verdict: {
					kind: 'undeclared',
					writes: [{ kind: 'singleton', before: '0xSafe', after: '0xBad' }]
				}
			},
			concerns: [{ kind: 'stateChange' }]
		};
		fake.chain = {
			chainId: 100,
			members,
			items: [item({ canExecute: [local] })],
			mailbox: true
		};
		show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('review.stateTitle');
		expect(text).toContain('review.writeSingleton');
		expect(buttons()).not.toContain('proposal.approve');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons().some((b) => b.includes('anyway') || b.includes('Anyway'))).toBe(false);
	});

	it('a danger (a lookalike, a call that fails): the signing sheet’s friction — "… anyway", danger-styled', async () => {
		reviewSeam.review = { ...REVIEW, sign: { allowed: true, anyway: true }, tone: 'danger' };
		show();
		await settle();
		expect(buttons()).toContain('review.approveAnyway');
		const anyway = [...document.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === 'review.approveAnyway'
		);
		expect(anyway?.className).toContain('danger');
	});
});

describe("a dApp's transaction from a multisig (dapp-1)", () => {
	it('is reviewed first: what the site asks, and nothing signed until Propose and approve', async () => {
		const propose = vi.fn(async () => HASH);
		const decline = vi.fn();
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null,
			propose,
			decline,
			close: () => {}
		};
		const screen = show({ draft, safeTxHash: '' });
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('app.example asks for this transaction.');
		expect(text).toContain('proposal.data');
		// Owner 2026-10-28: the proposer reads the call decoded, as every member
		// will — before "Propose and approve", before any passkey.
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		expect(buttons()).toContain('send.propose');
		expect(buttons()).toContain('external.decline');
		// No approval, invite or withdrawal of a transaction that does not exist yet.
		expect(buttons()).not.toContain('proposal.approve');
		expect(buttons()).not.toContain('proposal.reject');
		expect(propose).not.toHaveBeenCalled();

		press('send.propose');
		await settle();
		expect(propose).toHaveBeenCalledTimes(1);

		press('external.decline');
		expect(decline).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});
});

describe("a dApp's eth_sendTransaction its proposer alone makes ready (round 3, G2)", () => {
	it('is proposed and executed in one go, and the site is handed the real transaction', async () => {
		const TX = `0x${'cd'.repeat(32)}`;
		const one = { ...members, threshold: 1 };
		const mine = [{ owner: ME, counts: true, approval: {}, createdMs: 1 }];
		fake.draft = {
			chain: { chainId: 100, members: one, items: [], mailbox: true },
			item: item({ threshold: 1, canExecute: [local] })
		};
		fake.chain = {
			chainId: 100,
			members: one,
			mailbox: true,
			items: [
				item({
					threshold: 1,
					approved: 1,
					approvals: mine,
					status: 'ready',
					canApprove: [],
					canExecute: [local]
				})
			]
		};
		fake.execution = { maybeSent: false, userOpHash: '0x01', waitForTxHash: async () => TX };
		const executed = vi.fn();
		const base = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null as string | null,
			decline: () => {},
			close: () => {},
			executed
		};
		let screen: ReturnType<typeof show> | null = null;
		const draft = {
			...base,
			// What the host does once it is in the mailbox: the sheet shows the proposal.
			propose: async () => {
				await screen?.rerender({ draft: { ...draft, safeTxHash: HASH }, safeTxHash: HASH });
				return HASH;
			}
		};
		screen = show({ draft, safeTxHash: '' });
		await settle();
		// One door, worded for what it does: no wait for approvals nobody else gives.
		expect(buttons()).toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('send.propose');
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['execute']);
		expect(executed).toHaveBeenCalledWith(TX);
		await screen.unmount();
	});

	it('waiting for others, says how many approvals it still needs, and that the site was told', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				approvals: [{ owner: ME, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		const words2 = words();
		words2.proposal.stillNeeded = 'needed {{missing}}';
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x' }],
			origin: 'https://app.example',
			safeTxHash: HASH,
			propose: async () => HASH,
			decline: () => {},
			close: () => {},
			executed: () => {}
		};
		const screen = show({ draft, m: words2 });
		await settle();
		expect(document.body.textContent).toContain('needed 1');
		expect(document.body.textContent).toContain('proposal.siteWaits');
		await screen.rerender({ draft: { ...draft, siteTold: true } });
		await settle();
		expect(document.body.textContent).toContain('proposal.siteTold');
		expect(document.body.textContent).not.toContain('proposal.siteWaits');
		expect(document.body.textContent).not.toContain('needed 1');
		await screen.unmount();
	});
});

describe('a member who would execute it (ms-5)', () => {
	it('can approve without executing, beside Approve and execute', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local] })];
		const screen = show();
		await settle();
		expect(buttons()).toContain('proposal.approveAndExecute');
		expect(buttons()).toContain('proposal.approve');
		press('proposal.approve');
		await settle();
		expect(fake.calls).toEqual(['approve']);
		await screen.unmount();
	});
});

describe('the proposer (ms-6)', () => {
	it('can withdraw its own proposal after approving it', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				approvals: [{ owner: ME, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		const screen = show();
		await settle();
		expect(buttons()).toContain('proposal.reject');
		press('proposal.reject');
		await settle();
		// The confirmation, then the cancellation it proposes.
		[...document.body.querySelectorAll('button')]
			.filter((b) => b.textContent?.trim() === 'proposal.reject')
			.at(-1)
			?.click();
		await settle();
		expect(fake.calls).toEqual(['reject']);
		await screen.unmount();
	});
});

describe('an execution the relay refused (ms-3)', () => {
	it('is a failure with a retry at once, not "Executing…" for three minutes', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				canExecute: [local],
				approvals: [{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		fake.refused = true;
		fake.execution = {
			maybeSent: true,
			userOpHash: '0x01',
			waitForTxHash: () => new Promise(() => {})
		};
		const screen = show();
		await settle();
		press('proposal.approveAndExecute');
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.notSent');
		expect(text).not.toContain('proposal.executed');
		// The way to try again is still there.
		expect(buttons()).toContain('proposal.approveAndExecute');
		await screen.unmount();
	});
});

describe('a batch of calls (owner, 2026-10-28)', () => {
	it('reads every call, and says first, in red, a call that changes the multisig', async () => {
		const batch = {
			kind: 'batch',
			calls: [
				{
					to: TO,
					value: '0',
					data: '0x095ea7b3',
					action: { kind: 'call', to: TO, value: '0', data: '0x095ea7b3' }
				},
				{
					to: SAFE,
					value: '0',
					data: '0x694e80c3',
					action: { kind: 'changeThreshold', threshold: '1' }
				}
			]
		};
		fake.chain.items = [item({ review: { action: batch, refusal: null } })];
		fake.memberReading = {
			changes: [
				{
					call: 2,
					kind: 'threshold',
					added: null,
					removed: null,
					before: { threshold: 2, owners: 2 },
					after: { threshold: 1, owners: 2 },
					fits: true
				}
			],
			before: { threshold: 2, owners: 2 },
			after: { threshold: 1, owners: 2 },
			fits: true
		};
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		const change = document.querySelector('[data-testid="member-change"]')?.textContent ?? '';
		expect(change).toContain('change.caution Gnosis');
		expect(change).toContain('#2 · change.threshold');
		expect(change).toContain('change.rule 2/2 → 1/2');
		// Every call's own bytes, in order.
		expect(document.body.textContent).toContain(`#2 ${SAFE} 0x694e80c3`);
		await screen.unmount();
	});
});

/**
 * A rename is called BY the Safe on the chain whose registry holds the
 * record — the index chain (Gnosis) — whatever networks the multisig is used
 * on: the review says where it runs, and that executing deploys the multisig
 * there first when it is not yet.
 */
describe('a rename on chain (registry v4)', () => {
	const rename = { review: { action: { kind: 'rename', name: 'Team' }, refusal: null } };

	it('says it runs on the registry’s chain — and never that executing deploys it', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, deployed: false },
			items: [item({ ...rename, renameUndeployed: true, canExecute: [] })],
			mailbox: true
		};
		const screen = show();
		await settle();
		const where = document.querySelector('[data-testid="rename-where"]')?.textContent ?? '';
		expect(where).toContain('proposal.renameRuns');
		expect(document.querySelector('[data-testid="rename-undeployed"]')).not.toBeNull();
		await screen.unmount();
	});

	it('says only where it runs once it is deployed there', async () => {
		fake.chain.items = [item(rename)];
		const screen = show();
		await settle();
		const where = document.querySelector('[data-testid="rename-where"]')?.textContent ?? '';
		expect(where).toContain('proposal.renameRuns');
		expect(document.querySelector('[data-testid="rename-undeployed"]')).toBeNull();
		await screen.unmount();
	});

	it('another transaction says neither', async () => {
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="rename-where"]')).toBeNull();
		await screen.unmount();
	});
});

describe('a transaction whose number is used (ms-9, ms-10)', () => {
	const done = () => item({ status: 'done', canApprove: [], cancelledBy: `0x${'cd'.repeat(32)}` });

	it('executed anywhere: "Executed", never "1/2 approved", "Waiting" or a pending cancellation', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.executed');
		expect(text).not.toContain('status.progress');
		expect(text).not.toContain('proposal.waiting');
		expect(text).not.toContain('proposal.cancelPending');
		expect(text).not.toContain('proposal.noExpiry');
		await screen.unmount();
	});

	it('r5reg-20: executed reads as a past fact — no check of what running it would do', async () => {
		const pending = show();
		await settle();
		// Waiting: the review's checks are there (the guard's verdict, what moves).
		expect(document.querySelector('[data-testid="review-checks"]')?.textContent?.trim()).not.toBe(
			''
		);
		await pending.unmount();
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="review-checks"]')?.textContent?.trim()).toBe('');
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.executed');
		expect(text).not.toContain('proposal.feeChecking');
		expect(text).not.toContain('proposal.wouldFail');
		// The hashes it was approved over are still there: facts, not checks.
		expect(text).toContain('review.hashesTitle');
		await screen.unmount();
	});

	it('replaced by its executed cancellation: one wording, nothing that says otherwise', async () => {
		fake.chain.items = [done()];
		fake.executed = null;
		fake.executedBy = { [`0x${'cd'.repeat(32)}`]: `0x${'ee'.repeat(32)}` };
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.voided');
		expect(document.querySelector('[data-testid="proposal-voided"]')?.textContent?.trim()).toBe(
			'proposal.voidedNote'
		);
		expect(text).not.toContain('proposal.voidedByOther');
		expect(text).not.toContain('proposal.executed');
		expect(text).not.toContain('proposal.doneNote');
		expect(text).not.toContain('proposal.cancelPending');
		expect(text).not.toContain('proposal.waiting');
		expect(text).not.toContain('proposal.noExpiry');
		expect(text).not.toContain('status.progress');
		await screen.unmount();
	});

	it('replaced by another transaction at its number: said as that, not as a cancellation', async () => {
		fake.chain.items = [done()];
		fake.executed = null;
		fake.executedBy = { [`0x${'cd'.repeat(32)}`]: null };
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="proposal-voided"]')?.textContent?.trim()).toBe(
			'proposal.voidedByOther'
		);
		await screen.unmount();
	});

	it('the chain cannot say: neither executed nor cancelled is claimed — said, with Retry', async () => {
		fake.chain.items = [done()];
		fake.executed = undefined;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		// r4b-2: an unreadable history is said as that, never as nothing.
		expect(text).toContain('proposal.cantCheckUsed');
		expect(text).not.toContain('proposal.voided');
		expect(text).not.toContain('proposal.executed');
		expect(buttons()).toContain('service.retry');
		// Readable again: the Retry reads it, and it says what happened.
		fake.executed = TX;
		press('service.retry');
		await settle();
		expect(document.body.textContent).toContain('proposal.executed');
		await screen.unmount();
	});
});

describe('an owner whose wallet is elsewhere (ms-11, ms-23)', () => {
	it('is offered an outside wallet — a queued transaction too — but a Vela member is not', async () => {
		fake.chain.items = [item({ status: 'queued' })];
		let screen = show();
		await settle();
		expect(buttons()).toContain('external.signWithWallet');
		await screen.unmount();

		// The same member, known to the passkey index: it approves in Vela.
		fake.vela = [OTHER];
		screen = show();
		await settle();
		expect(buttons()).not.toContain('external.signWithWallet');
		await screen.unmount();
	});
});

describe('a link’s transaction on a device with no member (ms-21)', () => {
	const THIRD = '0x00000000000000000000000000000000000000cc';
	const strangers = { owners: [OTHER, THIRD], threshold: 2, nonce: 0n, deployed: true };
	const linked = (refresh = vi.fn(async () => {})) => ({
		loading: false,
		chain: () => ({
			chainId: 100,
			members: strangers,
			items: [item({ canApprove: [], proposer: OTHER })],
			mailbox: true
		}),
		refresh,
		unreadable: [] as number[]
	});

	it('is read from its own source, says no member is here, and offers the two ways on', async () => {
		// The shown multisig's list has nothing: the sheet reads the link's source.
		fake.chain = null;
		const onwatch = vi.fn();
		const screen = show({
			source: linked(),
			watch: { added: false, onwatch },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		const preview = document.querySelector('[data-testid="proposal-preview"]');
		expect(preview?.textContent).toContain('proposal.previewNote');
		expect(document.body.textContent).not.toContain('proposal.notMember');
		expect(document.body.textContent).not.toContain('proposal.watching');
		expect(buttons()).toContain('proposal.watch');
		expect(buttons()).toContain('external.signWithWallet');
		// Nothing to approve or execute from here, and nothing was done.
		expect(buttons()).not.toContain('proposal.approve');
		press('proposal.watch');
		expect(onwatch).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});

	it('kept here: said, and no second Watch', async () => {
		fake.chain = null;
		const screen = show({
			source: linked(),
			watch: { added: true, onwatch: () => {} },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		expect(document.body.textContent).toContain('proposal.watching');
		expect(buttons()).not.toContain('proposal.watch');
		await screen.unmount();
	});

	it('an outside approval re-reads the link’s source, not the shown multisig’s', async () => {
		fake.chain = null;
		const refresh = vi.fn(async () => {});
		const screen = show({
			source: linked(refresh),
			watch: { added: false, onwatch: () => {} },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		const external = [...document.querySelectorAll('[data-testid="proposal-preview"] button')].find(
			(b) => b.textContent?.trim() === 'external.signWithWallet'
		) as HTMLButtonElement;
		external.click();
		await settle();
		expect(fake.calls).toContain('external');
		expect(refresh).toHaveBeenCalled();
		await screen.unmount();
	});
});

describe('every artwork opens the identicon viewer (founder call, 2026-09-05)', () => {
	afterEach(() => identiconViewer.close());

	it('the recipient and each member, on its own address; a row control stays beside it', async () => {
		const screen = show();
		await settle();
		document.body.querySelector<HTMLButtonElement>('.to button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(TO);

		const rows = [...document.body.querySelectorAll('.approvals li')];
		expect(rows).toHaveLength(2);
		rows[1].querySelector<HTMLButtonElement>('button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(OTHER);
		// "Approve with another wallet" is the row's own control, beside the
		// artwork's button: no button holds another.
		expect(rows[1].textContent).toContain('external.signWithWallet');
		expect(document.body.querySelector('button button')).toBeNull();
		await screen.unmount();
	});
});

const STRANGER = '0x00000000000000000000000000000000000000ee';
const transfer = { kind: 'transfer', token: null, to: TO, amount: '1' };

/** Owners are each network's: what the sheet says about who counts, and where it starts. */
describe('members per network', () => {
	it('shows an approval from an account that is no member here as not counted', async () => {
		fake.chain = {
			chainId: 100,
			members,
			mailbox: true,
			items: [
				item({
					review: { action: transfer, refusal: null },
					approvals: [{ owner: STRANGER, counts: false, approval: {}, createdMs: 1 }]
				})
			]
		};
		show();
		await settle();
		const line = document.body.querySelector('[data-testid="not-counted"]');
		expect(line?.textContent).toContain('proposal.notCounted');
		// Not among the members' rows either: only this network's members are listed.
		expect(document.body.querySelectorAll('.approvals li')).toHaveLength(2);
	});

	it('says a new network starts with the original members when another network changed', async () => {
		const undeployed = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: false };
		fake.chain = {
			chainId: 100,
			members: undeployed,
			mailbox: true,
			items: [item({ review: { action: transfer, refusal: null } })]
		};
		fake.followed = SAFE;
		fake.chains = [
			{ chainId: 100, members: undeployed, items: [] },
			{ chainId: 10, members: { owners: [ME, STRANGER], threshold: 1, deployed: true }, items: [] }
		];
		show();
		await settle();
		const notice = document.body.querySelector('[data-testid="starts-original"]');
		expect(notice?.textContent).toContain('proposal.startsOriginal');
		expect(notice?.querySelectorAll('li')).toHaveLength(2);
	});

	it('says nothing of it when every network still has the original members', async () => {
		const undeployed = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: false };
		fake.chain = { chainId: 100, members: undeployed, mailbox: true, items: [item()] };
		fake.followed = SAFE;
		fake.chains = [
			{ chainId: 10, members: { owners: [OTHER, ME], threshold: 2, deployed: true }, items: [] }
		];
		show();
		await settle();
		expect(document.body.querySelector('[data-testid="starts-original"]')).toBeNull();
	});
});

describe('a rename (registry v4)', () => {
	it('never offers to deploy: where it is not deployed, it says so and cannot be executed', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, deployed: false },
			mailbox: true,
			items: [
				item({
					review: { action: { kind: 'rename', name: 'Team treasury' }, refusal: null },
					renameUndeployed: true,
					canExecute: [],
					canApprove: []
				})
			]
		};
		show();
		await settle();
		expect(document.body.querySelector('[data-testid="rename-undeployed"]')?.textContent).toContain(
			'proposal.renameUndeployed'
		);
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
	});
});

describe('what was signed here and the mailbox has not taken', () => {
	it('says when it is sent again, then offers Send again', async () => {
		fake.chain = {
			chainId: 100,
			members,
			mailbox: true,
			items: [item({ unsent: true })]
		};
		fake.kept = { why: 'busy', retryAt: Date.now() + 24_000 };
		const words0 = words();
		words0.proposal.outboxBusy = 'busy — again in {{seconds}} s';
		show({ m: words0 });
		await settle();
		const line = document.body.querySelector('[data-testid="proposal-unsent"]');
		expect(line?.textContent).toMatch(/busy — again in 2[34] s/);
		expect(buttons()).not.toContain('proposal.outboxSendAgain');
	});

	it('kept, not sent on its own any more: Send again sends what was signed', async () => {
		fake.chain = { chainId: 100, members, mailbox: true, items: [item({ unsent: true })] };
		fake.kept = { why: 'busy', retryAt: null };
		show();
		await settle();
		expect(document.body.textContent).toContain('proposal.outboxNotSent');
		press('proposal.outboxSendAgain');
		await settle();
		expect(fake.sentAgain).toBe(1);
		// Never a second signature: no approve, no passkey.
		expect(fake.calls).toEqual([]);
	});
});

describe('who can act for it on this network (§3.5)', () => {
	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('its genesis refused here: read-only — the reasons said, nothing to sign or withdraw', async () => {
		fake.checked = [{ from: 'genesis', chainId: 100, reason: { kind: 'module', address: MODULE } }];
		fake.chain = {
			chainId: 100,
			members,
			items: [item({ canExecute: [local], approved: 1 })],
			mailbox: true
		};
		show();
		await settle();
		const notice = document.querySelector('[data-testid="authority-refused"]');
		expect(notice?.textContent).toContain('authority.refusedOn');
		expect(notice?.textContent).toContain('authority.module');
		expect(buttons()).not.toContain('proposal.approve');
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('proposal.reject');
		expect(buttons()).not.toContain('external.signWithWallet');
	});

	it('a risk found here: said on the sheet, and signing still offered (each signature asks)', async () => {
		fake.checked = [{ from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } }];
		show();
		await settle();
		expect(document.querySelector('[data-testid="authority-risk"]')?.textContent).toContain(
			'authority.riskModule'
		);
		expect(buttons()).toContain('proposal.approve');
		press('proposal.approve');
		await settle();
		expect(fake.calls).toEqual(['approve']);
	});

	it('"Don\'t sign" on the risk: back where it was — never "the passkey prompt was closed"', async () => {
		fake.checked = [{ from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } }];
		fake.approveError = new DOMException('The person did not sign for the risk', 'AbortError');
		show();
		await settle();
		press('proposal.approve');
		await settle();
		expect(document.querySelector('[data-testid="proposal-failed"]')).toBeNull();
		expect(document.body.textContent).not.toContain('reason.passkeyClosed');
		expect(buttons()).toContain('proposal.approve');
	});

	it("another network's findings say nothing here", async () => {
		fake.checked = [
			{ from: 'genesis', chainId: 8453, reason: { kind: 'module', address: MODULE } }
		];
		show();
		await settle();
		expect(document.querySelector('[data-testid="authority-refused"]')).toBeNull();
		expect(document.querySelector('[data-testid="authority-risk"]')).toBeNull();
		expect(buttons()).toContain('proposal.approve');
	});
});

/**
 * r4a-1: a member change alone reads as fully as one inside a batch — who,
 * the rule before → after, the network, the caution — and one the Safe would
 * refuse says executing it would fail.
 */
describe('a member change', () => {
	const add = { kind: 'addOwner', owner: TO, threshold: '2' };
	const reading = (fits: boolean) => ({
		changes: [
			{
				call: null,
				kind: 'add',
				added: TO,
				removed: null,
				before: { threshold: 2, owners: 2 },
				after: { threshold: 2, owners: 3 },
				fits
			}
		],
		before: { threshold: 2, owners: 2 },
		after: { threshold: 2, owners: 3 },
		fits
	});

	it('says who, the rule before and after, the network, and the caution', async () => {
		fake.chain.items = [item({ review: { action: add, refusal: null } })];
		fake.memberReading = reading(true);
		const screen = show();
		await settle();
		const block = document.querySelector('[data-testid="member-change"]');
		expect(block?.querySelector('[role="alert"]')?.textContent?.trim()).toBe(
			'change.caution Gnosis'
		);
		expect(block?.textContent).toContain('change.add');
		// The whole address of whom it adds, beside its name.
		expect(block?.textContent).toContain(TO);
		expect(document.querySelector('[data-testid="member-change-rule"]')?.textContent?.trim()).toBe(
			'change.rule 2/2 → 2/3'
		);
		expect(block?.textContent).not.toContain('change.doesNotFit');
		await screen.unmount();
	});

	it('one that does not fit the members now says executing it would fail', async () => {
		fake.chain.items = [item({ review: { action: add, refusal: null } })];
		fake.memberReading = reading(false);
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="member-change"]')?.textContent).toContain(
			'change.doesNotFit'
		);
		await screen.unmount();
	});
});

/**
 * r4b-1, r4b-7: an execution sent from here is watched to its end by the
 * core's verdict — the race's loser is told within the watch's first round,
 * never "Executing…" without end; one sent before a reload is watched again
 * and never offered as if nothing was sent.
 */
describe('an execution in flight (r4b-1, r4b-7)', () => {
	const ready = () =>
		item({
			approved: 2,
			status: 'ready',
			canApprove: [],
			canExecute: [local],
			approvals: [
				{ owner: ME, counts: true, approval: {}, createdMs: 1 },
				{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }
			]
		});

	it('another member executed it first: said so, nothing sent from here, nothing more offered', async () => {
		fake.chain.items = [ready()];
		fake.execution = {
			maybeSent: false,
			userOpHash: '0x01',
			waitForTxHash: () => new Promise(() => {})
		};
		fake.verdict = { kind: 'executedByOther', txHash: `0x${'77'.repeat(32)}`, paid: false };
		const screen = show();
		await settle();
		press('proposal.execute');
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.lostRace');
		expect(text).not.toContain('proposal.executing');
		expect(buttons()).not.toContain('proposal.execute');
		expect(fake.ended).toEqual([HASH]);
		await screen.unmount();
	});

	it('r5reg-14: open on a waiting transaction, it holds the faster poll — and lets go', async () => {
		fake.chain.items = [ready()];
		const screen = show();
		await settle();
		expect(fake.held).toBe(1);
		await screen.unmount();
		expect(fake.held).toBe(0);
		// An executed one is read no faster.
		fake.chain.items = [{ ...ready(), status: 'done', canExecute: [] }];
		const done = show();
		await settle();
		expect(fake.held).toBe(0);
		await done.unmount();
	});

	it('r5reg-21: run by another member while open — said as that, never "would fail"', async () => {
		fake.chain.items = [ready()];
		fake.previews = { [ME]: { kind: 'spent' } };
		fake.executed = `0x${'77'.repeat(32)}`;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.lostRace');
		expect(text).not.toContain('proposal.wouldFail');
		expect(document.querySelector('[data-testid="execute-would-fail"]')).toBeNull();
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).not.toContain('proposal.approve');
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});

	it('r7-3: what it counts is kept as its calls carry it — every approval given, the executor only sending it', async () => {
		const THIRD = '0x00000000000000000000000000000000000000e3';
		fake.chain.items = [
			item({
				approved: 2,
				status: 'ready',
				canApprove: [local],
				canExecute: [local],
				approvals: [
					{ owner: OTHER, counts: true, approval: {}, createdMs: 1 },
					{ owner: THIRD, counts: true, approval: {}, createdMs: 2 }
				]
			})
		];
		fake.previews = { [ME]: priced({ carries: [OTHER, THIRD], executorApproves: false }) };
		fake.execution = {
			maybeSent: false,
			userOpHash: '0x01',
			waitForTxHash: async () => `0x${'77'.repeat(32)}`
		};
		const screen = show();
		await settle();
		press('proposal.execute');
		await settle();
		// One signature: the execution, nothing approved first.
		expect(fake.calls).toEqual(['execute']);
		expect(fake.remembered).toHaveLength(1);
		expect(fake.remembered[0][3]).toMatchObject({ executor: ME, approvers: [OTHER, THIRD] });
		await screen.unmount();
	});

	it('replaced by another transaction at its number: said with the number', async () => {
		fake.chain.items = [ready()];
		fake.execution = {
			maybeSent: false,
			userOpHash: '0x01',
			waitForTxHash: () => new Promise(() => {})
		};
		fake.verdict = { kind: 'replaced', paid: false };
		const screen = show();
		await settle();
		press('proposal.execute');
		await settle();
		expect(document.body.textContent).toContain('proposal.replacedFirst');
		await screen.unmount();
	});

	it('past its time: said as not landed, and it may be tried again', async () => {
		fake.chain.items = [ready()];
		fake.execution = {
			maybeSent: false,
			userOpHash: '0x01',
			waitForTxHash: () => new Promise(() => {})
		};
		fake.verdict = { kind: 'timedOut' };
		const screen = show();
		await settle();
		press('proposal.execute');
		await settle();
		expect(document.body.textContent).toContain('proposal.notLanded');
		expect(buttons()).toContain('proposal.execute');
		await screen.unmount();
	});

	it('sent before a reload: watched again at once, not offered for execution', async () => {
		fake.chain.items = [ready()];
		fake.inflight = {
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH,
			nonce: '0',
			userOpHash: '0x01',
			by: ME,
			maybeSent: false,
			startedMs: Date.now(),
			createdMs: 1
		};
		// Still on its way.
		fake.verdict = { kind: 'waiting' };
		let screen = show();
		await settle();
		expect(document.body.textContent).toContain('proposal.executing');
		expect(buttons()).not.toContain('proposal.execute');
		expect(fake.calls).not.toContain('execute');
		await screen.unmount();
		// …and it landed: executed, from the watch alone.
		fake.verdict = { kind: 'executed', txHash: TX };
		const onexecuted = vi.fn();
		screen = show({ onexecuted });
		await settle();
		expect(onexecuted).toHaveBeenCalledWith(
			expect.objectContaining({ txHash: TX, userOpHash: '0x01' })
		);
		expect(fake.calls).not.toContain('execute');
		await screen.unmount();
	});
});

describe('the network cannot be read (r4b-3)', () => {
	it('says it cannot reach the network, with Retry — never "hasn’t been shared"', async () => {
		fake.chain = null;
		fake.unreadable = [100];
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.cantCheck');
		expect(text).not.toContain('proposal.notFound');
		expect(buttons()).toContain('service.retry');
		await screen.unmount();
	});
});

describe('why it did not go, in the corpus’ words (r4b-12)', () => {
	it.each([
		['a passkey prompt closed', 'cancelled', 'proposal.reason.passkeyClosed'],
		['a passkey prompt that timed out', 'timedOut', 'proposal.reason.passkeyTimedOut'],
		['a raw error', 'raw', 'proposal.reason.unknown']
	])('%s', async (_what, kind, words) => {
		const { PasskeyError } = await import('$lib/onboarding/core/passkey');
		fake.approveError =
			kind === 'raw'
				? new Error('genesis service unavailable: Unexpected token o')
				: new PasskeyError('cancelled', 'User cancelled the operation', kind === 'timedOut');
		const screen = show();
		await settle();
		press('proposal.approve');
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain(words);
		expect(text).not.toContain('Unexpected token');
		expect(text).not.toContain('User cancelled the operation');
		fake.approveError = null;
		await screen.unmount();
	});

	it('r4b-15 / r4D-10: Approve offline says offline — not the check that could not run', async () => {
		const { SigningUncheckedError } = await import('../core/gate.svelte');
		const online = vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(false);
		fake.approveError = new SigningUncheckedError(100);
		const screen = show();
		await settle();
		press('proposal.approve');
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.reason.offline');
		expect(text).not.toContain('authority.signUnchecked');
		// The person's own choice is still said as itself.
		const { PasskeyError } = await import('$lib/onboarding/core/passkey');
		fake.approveError = new PasskeyError('cancelled', 'closed', false);
		press('proposal.approve');
		await settle();
		expect(document.body.textContent ?? '').toContain('proposal.reason.passkeyClosed');
		online.mockRestore();
		fake.approveError = null;
		await screen.unmount();
	});
});

/** r4a-22: the whole proposal hash, to compare with another wallet's: shown and copied. */
describe('the proposal hash', () => {
	it('is there whole, behind its short form, with a copy', async () => {
		const written: string[] = [];
		Object.defineProperty(navigator, 'clipboard', {
			configurable: true,
			value: { writeText: async (text: string) => void written.push(text) }
		});
		const screen = show();
		await settle();
		const hash = document.querySelector('[data-testid="proposal-hash"]');
		expect(hash?.querySelector('summary')?.textContent).toContain('…');
		expect(hash?.querySelector('code')?.textContent).toBe(HASH);
		(document.querySelector('[data-testid="proposal-hash-copy"]') as HTMLButtonElement).click();
		await settle();
		expect(written).toEqual([HASH]);
		await screen.unmount();
	});
});

/**
 * r4a-8: an executed transaction reads the facts frozen when it ran — the
 * approvals that counted then, the threshold then, who sent it — never
 * today's owners re-counted, never "Waiting", never more than were needed.
 */
describe('an executed transaction', () => {
	const THEN = '0xcc000000000000000000000000000000000000cc';
	const done = () => item({ status: 'done', canApprove: [], approvals: [], approved: 0 });

	it('lists whose approvals counted then — a member removed since too — and who sent it', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		fake.frozen = {
			executor: ME,
			threshold: 2,
			approvers: [THEN, ME],
			executorApproved: true,
			source: 'chain'
		};
		const screen = show();
		await settle();
		const approvals = document.querySelector('section.approvals')?.textContent ?? '';
		// THEN is no member now (members are ME and OTHER): it approved it,
		// then — and OTHER, a member now, did not.
		expect(approvals).toContain(THEN.slice(0, 6));
		expect(document.querySelectorAll('section.approvals li')).toHaveLength(2);
		// The executor approved by sending it: counted once, as the executor.
		expect(approvals.match(/proposal\.executed/g)?.length).toBe(2);
		expect(approvals).not.toContain('status.progress');
		expect(approvals).not.toContain('proposal.waiting');
		expect(document.querySelector('[data-testid="approvals-count"]')?.textContent).toBe(
			'proposal.executed'
		);
		const facts = document.querySelector('[data-testid="executed-facts"]')?.textContent ?? '';
		expect(facts).toContain('proposal.executedBy');
		expect(facts).toContain('proposal.countedThen');
		await screen.unmount();
	});

	it('says so when which approvals counted is not on record', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		fake.frozen = null;
		const screen = show();
		await settle();
		expect(
			document.querySelector('[data-testid="executed-facts-unknown"]')?.textContent?.trim()
		).toBe('proposal.approvalsSigned');
		expect(document.body.textContent).not.toContain('status.progress');
		await screen.unmount();
	});
});

/** r4a-2: a transfer of a token nobody vouches for shows its contract on the sheet. */
describe('a transfer of an unverified token', () => {
	it('carries the note the core rules, with the contract', async () => {
		const FAKE = '0x57e64022d8d9a420dFE08a22d6a687caaBc6B7C6';
		fake.chain.items = [
			item({
				review: { action: { kind: 'transfer', token: FAKE, to: TO, amount: '1' }, refusal: null }
			})
		];
		fake.tokenNote = {
			text: 'Not the real USDC',
			tone: 'danger',
			contract: FAKE,
			short: '0x57e6…B7C6',
			explorerUrl: `https://explorer.example/address/${FAKE}`
		};
		const screen = show();
		await settle();
		const note = document.querySelector('[data-testid="token-provenance"]');
		expect(note?.getAttribute('data-tone')).toBe('danger');
		expect(note?.textContent).toContain('Not the real USDC');
		expect(note?.textContent).toContain('0x57e6…B7C6');
		expect(note?.querySelector('a')?.getAttribute('href')).toContain(FAKE);
		await screen.unmount();
	});
});

describe('what the sheet says of where it stands (r4b-4, r4b-6, r4b-17, r4a-16)', () => {
	it('a link’s own source is kept fresh while shown, and stops when closed', async () => {
		const stop = vi.fn();
		const watch = vi.fn(() => stop);
		const screen = show({
			source: {
				loading: false,
				unreadable: [],
				chain: () => fake.chain,
				refresh: async () => {},
				watch
			},
			watch: { added: false, onwatch: () => {} }
		});
		await settle();
		expect(watch).toHaveBeenCalledTimes(1);
		await screen.unmount();
		expect(stop).toHaveBeenCalledTimes(1);
	});

	it('never "3/2": the approvals counted up to the threshold', async () => {
		fake.chain.items = [item({ approved: 3, threshold: 2, status: 'ready', canApprove: [] })];
		const m = words();
		m.status.progress = '{{approved}}/{{threshold}}';
		const screen = show({ m });
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('2/2');
		expect(text).not.toContain('3/2');
		await screen.unmount();
	});

	it('proposals at one number say only one of them can run', async () => {
		fake.chain.items = [item({ sameNonce: [`0x${'ef'.repeat(32)}`] })];
		const screen = show();
		await settle();
		expect(document.body.textContent).toContain('proposal.conflict');
		await screen.unmount();
	});

	it('queued behind several: every number named', async () => {
		const m = words();
		m.proposal.queuedNoteMany = 'after {{nonces}}';
		fake.chain.items = [item({ nonce: 3n, status: 'queued' })];
		const screen = show({ m });
		await settle();
		expect(document.body.textContent).toContain('after #0, #1, #2');
		await screen.unmount();
	});
});

describe('an execution looked at before its passkey (r4-5, r4-7)', () => {
	const ready = (over: Record<string, unknown> = {}) =>
		item({
			approved: 1,
			status: 'pending',
			canExecute: [local],
			approvals: [{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }],
			canApprove: [local],
			...over
		});

	it('that would fail says why, and offers Approve — never Execute', async () => {
		fake.chain.items = [ready()];
		fake.previews = {
			[ME]: {
				kind: 'fails',
				why: { kind: 'short', symbol: 'USDC', decimals: 0, have: 28n, need: 50n }
			}
		};
		const m = words();
		m.proposal.wouldFail = 'Would fail: {{reason}}.';
		m.proposal.whyShort = 'holds {{have}}, needs {{need}}';
		const screen = show({ m });
		await settle();
		const said = document.querySelector('[data-testid="execute-would-fail"]');
		expect(said?.textContent?.trim()).toBe('Would fail: holds 28 USDC, needs 50 USDC.');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).toContain('proposal.approve');
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});

	it('shows the fee before the tap, with what it sets up, and signs that very look', async () => {
		fake.chain.items = [ready()];
		const look = priced({
			setup: { multisig: true, members: [OTHER], executor: false, amount: 20n * 10n ** 15n }
		});
		fake.previews = { [ME]: look };
		fake.execution = { maybeSent: false, userOpHash: '0x01', waitForTxHash: async () => '0x02' };
		const m = words();
		m.proposal.feeLine = 'Fee {{fee}} by {{name}}';
		m.proposal.feeSetup = 'Setup on {{network}} {{amount}}:';
		m.proposal.setupMember = 'activating {{name}}';
		const screen = show({ m });
		await settle();
		const fee = document.querySelector('[data-testid="execute-fee"]')?.textContent ?? '';
		expect(fee).toContain('Fee 0.035 xDAI by 0x0000');
		expect(fee).toContain('Setup on Gnosis 0.02 xDAI:');
		expect(fee).toContain('proposal.setupMultisig');
		expect(fee).toContain('activating 0x0000');
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['execute']);
		expect(fake.executedWith).toEqual([look]);
		await screen.unmount();
	});

	/**
	 * Round 5 (S2): a look that did not answer in time is said as not checked
	 * — approving goes ahead, and Execute is offered: it looks again first.
	 */
	it('a look past its cap: says it could not check, Approve and Execute both usable', async () => {
		fake.chain.items = [ready()];
		fake.previews = { [ME]: { kind: 'unchecked' } };
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="execute-unchecked"]')?.textContent).toContain(
			'proposal.feeUnchecked'
		);
		expect(document.querySelector('[data-testid="execute-checking"]')).toBeNull();
		for (const name of ['proposal.approve', 'proposal.approveAndExecute']) {
			const button = [...document.querySelectorAll('button')].find(
				(b) => b.textContent?.trim() === name
			) as HTMLButtonElement | undefined;
			expect(button?.disabled, name).toBe(false);
		}
		await screen.unmount();
	});

	it('while the look is out, says so and holds Execute', async () => {
		fake.chain.items = [ready()];
		fake.previews = {};
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="execute-checking"]')).not.toBeNull();
		const execute = [...document.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === 'proposal.approveAndExecute'
		) as HTMLButtonElement;
		expect(execute.disabled).toBe(true);
		await screen.unmount();
	});
});

describe('who executes (r4-10, r4D-1)', () => {
	const SECOND = { address: OTHER, name: 'Second', account: { address: OTHER } };

	it('lists each executor’s coin, marks who cannot pay, and picks one who can', async () => {
		fake.chain.items = [
			item({
				approved: 2,
				status: 'ready',
				canApprove: [],
				canExecute: [local, SECOND],
				approvals: [
					{ owner: ME, counts: true, approval: {}, createdMs: 1 },
					{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }
				]
			})
		];
		fake.previews = {
			[ME]: priced({ balance: 10n, canPay: false }),
			[OTHER]: priced({ balance: 900n })
		};
		const screen = show();
		await settle();
		const balances = [...document.querySelectorAll('[data-testid="actor-balance"]')].map((b) =>
			b.textContent?.replace(/\s+/g, ' ').trim()
		);
		expect(balances).toEqual(['10 xDAI · proposal.pickerCantPay', '900 xDAI']);
		const on = document.querySelector('.actor label.on')?.textContent ?? '';
		expect(on, 'the one who can pay is picked').toContain('Second');
		expect(buttons()).toContain('proposal.execute');
		await screen.unmount();
	});

	it('picked one who cannot pay: said before the tap; Approve is the action', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local], canApprove: [local] })];
		fake.previews = { [ME]: priced({ balance: 0n, canPay: false }) };
		const m = words();
		m.proposal.cannotPay = '{{name}} cannot pay {{fee}}, has {{balance}}';
		const screen = show({ m });
		await settle();
		// r5reg-15: in the wallet's fee precision, as the fee line reads it.
		expect(document.querySelector('[data-testid="execute-cant-pay"]')?.textContent?.trim()).toBe(
			'0x0000 cannot pay 0.035 xDAI, has 0 xDAI'
		);
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).toContain('proposal.approve');
		await screen.unmount();
	});
});

/**
 * r5reg-5: a device holding two members of the multisig shows who proposes
 * on the review ("As") — never a silent pick — and the one picked proposes.
 */
describe('who proposes, on the review before the passkey (r5reg-5)', () => {
	const SECOND = { address: OTHER, name: 'Second', account: { address: OTHER } };

	it('offers "As" with both members, and the one picked goes on to its passkey', async () => {
		const confirm = vi.fn();
		const prebuilt = {
			chainId: 100,
			address: SAFE,
			chain: { chainId: 100, members, items: [], mailbox: true },
			item: item({ canApprove: [local, SECOND] }),
			confirm,
			decline: vi.fn()
		};
		const screen = show({ prebuilt, safeTxHash: '' });
		await settle();
		const choice = document.querySelector('[data-testid="actor-choice"]');
		expect(choice, '"As" is shown while drafting').not.toBeNull();
		expect(choice?.textContent).toContain('Mine');
		expect(choice?.textContent).toContain('Second');
		const second = [...document.querySelectorAll<HTMLInputElement>('input[name="actor"]')].find(
			(i) => i.value === OTHER
		);
		second?.click();
		await settle();
		press('send.propose');
		await settle();
		expect(confirm).toHaveBeenCalledWith(OTHER);
		await screen.unmount();
	});

	it('one member here: no choice to show, it proposes', async () => {
		const confirm = vi.fn();
		const prebuilt = {
			chainId: 100,
			address: SAFE,
			chain: { chainId: 100, members, items: [], mailbox: true },
			item: item({ canApprove: [local] }),
			confirm,
			decline: vi.fn()
		};
		const screen = show({ prebuilt, safeTxHash: '' });
		await settle();
		expect(document.querySelector('[data-testid="actor-choice"]')).toBeNull();
		press('send.propose');
		await settle();
		expect(confirm).toHaveBeenCalledWith(ME);
		await screen.unmount();
	});
});

describe('"Approve and execute" whose execute part cannot run (r4-11)', () => {
	it('keeps the approval, and says only executing could not run', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local], canApprove: [local] })];
		const { ExecutorCannotPayError } = await import('../core/execute');
		fake.executeThrows = new ExecutorCannotPayError(1n, 0n);
		const screen = show();
		await settle();
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['approve']);
		const said = document.querySelector('[data-testid="approved-not-executed"]');
		expect(said?.textContent).toContain('proposal.approvedNotExecuted');
		expect(document.body.textContent).not.toContain('Failed:');
		await screen.unmount();
	});
});

describe('how each missing member approves (r4-9, r4D-6)', () => {
	it('a Vela member elsewhere: never an outside wallet — the link, in Vela', async () => {
		fake.vela = [OTHER];
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		expect(buttons()).not.toContain('external.signWithWallet');
		expect(buttons()).toContain('proposal.sendLink');
		expect(document.querySelector('[data-testid="approves-in-vela"]')).not.toBeNull();
		await screen.unmount();
	});

	it('one nothing says how they sign (r4a-9, r7): the Vela way first, the outside wallet beside it', async () => {
		fake.unsure = [OTHER];
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		// r7: their row offers "Send link" first AND "Approve with another
		// wallet" beside it — both paths, the Vela one first, never one alone.
		const row = [...document.querySelectorAll('section.approvals li')].find((li) =>
			li.textContent?.includes('proposal.sendLink')
		);
		if (!row) throw new Error('no row offers the Vela link');
		const controls = [...row.querySelectorAll('.state button')].map((b) => b.textContent?.trim());
		expect(controls).toEqual(['proposal.sendLink', 'external.signWithWallet']);
		const vela = document.querySelector('[data-testid="approves-in-vela"]');
		const outside = document.querySelector('[data-testid="outside-invite"]');
		expect(vela).not.toBeNull();
		// The outside wallet's link is still there, after the Vela one.
		expect(outside).not.toBeNull();
		expect(vela!.compareDocumentPosition(outside!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		// The row's live pairing is the outside wallet's own way (r7).
		[...row.querySelectorAll('button')]
			.find((b) => b.textContent?.trim() === 'external.signWithWallet')
			?.click();
		await settle();
		expect(fake.calls).toContain('external');
		await screen.unmount();
	});

	it('r7: another Safe as a member — a smart contract account: said honestly, no wallet or Vela link offered', async () => {
		fake.contract = [OTHER];
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		const row = document.querySelector('[data-testid="contract-member"]');
		expect(row?.textContent).toContain('members.kindContract');
		expect(row?.closest('li')?.querySelectorAll('.state button')).toHaveLength(0);
		expect(buttons()).not.toContain('external.signWithWallet');
		expect(buttons()).not.toContain('proposal.sendLink');
		expect(document.querySelector('[data-testid="contract-members"]')?.textContent).toContain(
			'proposal.contractMembers'
		);
		// Neither an outside wallet's link nor the Vela invite is offered for it.
		expect(document.querySelector('[data-testid="outside-invite"]')).toBeNull();
		expect(document.querySelector('[data-testid="approves-in-vela"]')).toBeNull();
		await screen.unmount();
	});

	it('enough approved: the others are "Not needed", and nothing asks them to sign', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, owners: [ME, TO, OTHER] },
			mailbox: true,
			items: [
				item({
					approved: 2,
					status: 'ready',
					canApprove: [],
					approvals: [
						{ owner: ME, counts: true, approval: {}, createdMs: 1 },
						{ owner: TO, counts: true, approval: {}, createdMs: 1 }
					]
				})
			]
		};
		const screen = show();
		await settle();
		expect(document.body.textContent).toContain('proposal.notNeeded');
		expect(buttons()).not.toContain('external.signWithWallet');
		expect(buttons()).not.toContain('proposal.sendLink');
		await screen.unmount();
	});
});

describe('Reject (r4D-14)', () => {
	it('has a way back', async () => {
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		press('proposal.reject');
		await settle();
		expect(buttons()).toContain('proposal.rejectCancel');
		press('proposal.rejectCancel');
		await settle();
		expect(buttons()).not.toContain('proposal.rejectCancel');
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});
});

describe('a dApp’s transaction on a device with no member (r4C-10)', () => {
	it('says "Propose", never "Propose and approve": nobody here approves it', async () => {
		const THIRD = '0x00000000000000000000000000000000000000cc';
		const strangers = { owners: [OTHER, THIRD], threshold: 2, nonce: 0n, deployed: true };
		fake.draft = {
			chain: { chainId: 100, members: strangers, items: [], mailbox: true },
			item: item({ canApprove: [], canExecute: [] })
		};
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null,
			propose: async () => HASH,
			decline: () => {},
			close: () => {}
		};
		const screen = show({ draft, safeTxHash: '' });
		await settle();
		expect(buttons()).toContain('send.proposeOnly');
		expect(buttons()).not.toContain('send.propose');
		await screen.unmount();
	});
});

describe('round 6: the review’s words and where its actions stand', () => {
	/** The person's own transfer, reviewed before its passkey (`prebuilt`). */
	const own = (decline = () => {}) => ({
		chainId: 100,
		address: SAFE,
		chain: { chainId: 100, members, items: [], mailbox: true },
		item: item({
			review: { action: { kind: 'transfer', token: null, to: TO, amount: '1' }, refusal: null }
		}),
		confirm: () => {},
		decline
	});

	it('r4D-13: the person’s own proposal is withdrawn, not declined (a site’s request still is)', async () => {
		const decline = vi.fn();
		const screen = show({ prebuilt: own(decline), safeTxHash: '' });
		await settle();
		expect(buttons()).toContain('proposal.withdraw');
		expect(buttons()).not.toContain('external.decline');
		press('proposal.withdraw');
		expect(decline).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});

	it('r5reg-15: a call the signing sheet reads heads itself — its verb is not said twice above it', async () => {
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		expect(document.querySelector('header.what .title')).toBeNull();
		await screen.unmount();
		// A cancellation is the multisig's own reading: its line stays.
		fake.chain.items = [
			item({ review: { action: { kind: 'cancel', nonce: '0' }, refusal: null } })
		];
		const again = show();
		await settle();
		expect(document.querySelector('header.what .title')).not.toBeNull();
		await again.unmount();
	});

	it('r4D-19: in the desktop dialog the actions ride its bottom edge; the phone sheet is unchanged', async () => {
		const wide = show({ prebuilt: own(), safeTxHash: '', wide: true });
		await settle();
		const bar = document.querySelector<HTMLElement>('[data-testid="proposal-actions"]');
		expect(bar).not.toBeNull();
		expect(getComputedStyle(bar!).position).toBe('sticky');
		// Over the scroller's own room beneath its content: nothing scrolls past under it.
		expect(parseFloat(getComputedStyle(bar!).bottom)).toBeLessThan(0);
		await wide.unmount();

		fake.chain.items = [item({ canExecute: [local] })];
		const pending = show({ wide: true });
		await settle();
		const primary = document.querySelector<HTMLElement>('[data-testid="proposal-actions"]');
		expect(primary && getComputedStyle(primary).position).toBe('sticky');
		await pending.unmount();

		const phone = show({ prebuilt: own(), safeTxHash: '', wide: false });
		await settle();
		const plain = document.querySelector<HTMLElement>('[data-testid="proposal-actions"]');
		expect(plain && getComputedStyle(plain).position).toBe('static');
		await phone.unmount();
	});
});
