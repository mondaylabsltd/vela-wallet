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
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
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
const USDC = '0x00000000000000000000000000000000000000c0';

const fake = vi.hoisted(() => ({
	chain: null as any,
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
	/** r4b-3: networks that did not answer. */
	unreadable: [] as number[],
	/** Each executor's look at the execution (r4-5, r4-7), by address. */
	previews: {} as Record<string, any>,
	/** The look each execute was handed. */
	executedWith: [] as any[],
	/** What execute throws before its passkey (r4-11). */
	executeThrows: null as any,
	/** Sheets holding the shown multisig's faster poll (r5reg-14). */
	held: 0,
	/** The person's Retry on what the machine could not read. */
	retried: 0,
	/** The machine's draft, while a review-first sheet shows it (§3.6). */
	machineDraft: null as any,
	/** What "Propose and approve" on a dApp's draft comes to (the machine's `propose` state). */
	confirmed: null as any,
	/** Told when a dApp's draft is confirmed: the host's sheet then shows the proposal. */
	onConfirm: null as null | (() => Promise<void>),
	/** ms-21: a link's transaction read only, as the machine's `linked` reads (`linkedSource`). */
	linkedSource: null as any
}));

/**
 * The shown multisig's list (the facade over the proposals machine) and the
 * machine's view the sheet reads: its reviews, each executor's look, how
 * each member signs, its story, where each action stands — stood in for
 * from the seams above; the machine's own suites drive the real one
 * (rust/crates/vela-core/tests/app_multisig_proposals.rs).
 */
const view = vi.hoisted(() => ({
	key: (hash: string) => `100:${hash.toLowerCase()}`
}));
/** The members of `rows` of these kinds, as the machine lists them. */
function of(rows: { owner: string; kind: string | null }[], kinds: string[]): string[] {
	return rows.filter((r) => r.kind !== null && kinds.includes(r.kind)).map((r) => r.owner);
}
vi.mock('../core/proposals.svelte', async () => {
	const { machineSeam, machineLook, machineTimeline } =
		await import('./proposal-sheet.testing.svelte');
	/** The shown multisig's network, or a link's read only (ms-21). */
	const chainNow = () => fake.chain ?? fake.linkedSource?.chain(100) ?? null;
	const shownItem = () =>
		(chainNow()?.items ?? []).find((i: any) => i.safeTxHash.toLowerCase() === HASH) ?? null;
	const machineView = () => {
		const item = shownItem();
		const key = view.key(HASH);
		const previews: Record<string, unknown> = {};
		for (const [owner, look] of Object.entries(fake.previews)) {
			previews[`${key}:${owner.toLowerCase()}`] = machineLook(look);
		}
		const owners: string[] = chainNow()?.members.owners ?? [];
		const kindOf = (a: string) =>
			a.toLowerCase() === ME
				? 'here'
				: fake.vela.includes(a)
					? 'vela'
					: fake.unsure.includes(a)
						? 'unsure'
						: fake.contract.includes(a)
							? 'contract'
							: 'wallet';
		const rows = owners
			.filter((o) => !(item?.approvals ?? []).some((a: any) => a.counts && a.owner === o))
			.map((owner) => ({ owner, kind: kindOf(owner) }));
		const timelines: Record<string, unknown> = {};
		if (item?.status === 'done' && typeof fake.executed === 'string') {
			timelines[key] = machineTimeline(item, fake.executed, fake.frozen);
		} else if (item?.status === 'done' && fake.executed === null) {
			const cancel = item.cancelledBy;
			timelines[key] = [
				{
					kind: 'replaced',
					by: cancel,
					cancellation: !!cancel && typeof fake.executedBy[cancel] === 'string'
				}
			];
		} else if (fake.executed === undefined && item?.status === 'done' && fake.frozen) {
			timelines[key] = [];
		}
		const outbox: Record<string, unknown> = { ...machineSeam.outbox };
		if (fake.kept) outbox[`100:${SAFE.toLowerCase()}:${HASH}:${ME}`] = fake.kept;
		return {
			followed: fake.followed,
			actions: machineSeam.actions,
			// r4b-7: a flight read back after a reload, watched by the machine.
			executions: fake.inflight
				? { ...machineSeam.executions, [key]: fake.verdict }
				: machineSeam.executions,
			outbox,
			reviews: { [key]: reviewSeam.review ?? REVIEW },
			previews,
			// The machine's partition of them (app_multisig_proposals.rs
			// `each_remaining_member_signs_as_the_one_rule_says`).
			signers: {
				[key]: {
					rows,
					outside: of(rows, ['wallet', 'unsure']),
					in_vela: of(rows, ['vela']),
					either: of(rows, ['unsure']),
					contract: of(rows, ['contract']),
					enough: false
				}
			},
			timelines,
			frozen: fake.frozen ? { [key]: fake.frozen } : {},
			history_unreadable: fake.executed === undefined ? [100] : [],
			draft: fake.machineDraft,
			last_actor: null,
			// core-ms-2: who here one press approves and executes it, as the
			// machine rules (app_multisig_proposals.rs
			// `approve_and_execute_is_offered_and_runs_in_one_press`): short of
			// one approval, theirs reaches it, their look does not say they cannot pay.
			approve_execute: (() => {
				if (!item || item.status !== 'pending') return {};
				const who = (item.canExecute ?? [])
					.map((e: any) => (typeof e === 'string' ? e : e.address))
					.filter(
						(a: string) =>
							!(item.approvals ?? []).some((x: any) => x.counts && x.owner === a) &&
							item.approved + 1 >= item.threshold
					)
					.filter((a: string) => {
						const look = fake.previews[a] as { kind?: string; canPay?: boolean } | undefined;
						// Round 5 (S2): a look that could not price or check it does
						// not say they cannot pay — Execute looks again first.
						return (
							(look?.kind === 'priced' && look.canPay === true) ||
							look?.kind === 'unpriced' ||
							look?.kind === 'unchecked'
						);
					});
				return who.length > 0 ? { [key]: who } : {};
			})()
		};
	};
	return {
		proposals: {
			loading: false,
			get view() {
				// Read so a sheet redraws when an action settles.
				void machineSeam.actions;
				void machineSeam.executions;
				void machineSeam.outbox;
				void machineSeam.reads;
				return machineView();
			},
			get unreadable() {
				return fake.unreadable;
			},
			chain: () => fake.chain,
			refresh: async () => {},
			retry: async () => {
				fake.retried += 1;
				machineSeam.reads += 1;
			},
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
		itemOf: (i: unknown) => i,
		linkedSource: () => fake.linkedSource
	};
});
/**
 * The proposals machine's host: each event the sheet sends, answered as the
 * machine would — where the action ends (`actions`), how an execution ended
 * (`executions`) — from the seams above.
 */
vi.mock('../shell/proposals-host.svelte', async () => {
	const { machineSeam, stateOf, sentLog } = await import('./proposal-sheet.testing.svelte');
	class ProposeFailedError extends Error {
		constructor(
			readonly state: unknown,
			readonly chainId: number
		) {
			super('not proposed');
		}
	}
	const settle = (key: string, state: unknown) => {
		machineSeam.actions = { ...machineSeam.actions, [key]: state as never };
	};
	/** The machine's execution of `event` (`execute`), from the seams. */
	const execute = (event: any) => {
		const key = `${event.chain_id}:${String(event.safe_tx_hash ?? '').toLowerCase()}`;
		const done = `execute:${event.safe_tx_hash.toLowerCase()}`;
		fake.executedWith.push(fake.previews[event.executor]);
		if (fake.executeThrows) return settle(done, stateOf(fake.executeThrows));
		fake.calls.push('execute');
		void (async () => {
			const verdict =
				fake.verdict ??
				(fake.refused
					? { kind: 'notSent' }
					: { kind: 'executed', txHash: await fake.execution.waitForTxHash() });
			machineSeam.executions = { ...machineSeam.executions, [key]: verdict };
			settle(done, { kind: 'done', safe_tx_hash: event.safe_tx_hash, kept: false });
		})();
	};
	const host = {
		get view() {
			return null;
		},
		send(event: any) {
			sentLog.push(event);
			const key = `${event.chain_id}:${String(event.safe_tx_hash ?? '').toLowerCase()}`;
			switch (event.type) {
				case 'approve':
					fake.calls.push('approve');
					settle(
						`approve:${event.safe_tx_hash.toLowerCase()}`,
						fake.approveError ? stateOf(fake.approveError) : { kind: 'done', kept: false }
					);
					return;
				case 'approve_outside':
					fake.calls.push('external');
					settle(`approve:${event.safe_tx_hash.toLowerCase()}`, { kind: 'done', kept: false });
					return;
				case 'preview': {
					// r5reg-21: a look that finds its number used reads what ran.
					const look = fake.previews[event.executor] as { kind?: string } | undefined;
					if (look?.kind === 'spent') {
						machineSeam.executions = {
							...machineSeam.executions,
							[key]:
								typeof fake.executed === 'string'
									? { kind: 'executedByOther', txHash: fake.executed, paid: false }
									: fake.executed === null
										? { kind: 'replaced', paid: false }
										: { kind: 'unknown' }
						};
					}
					return;
				}
				case 'execute':
					return execute(event);
				case 'approve_execute': {
					// core-ms-2: the machine's one press — stopped before any
					// signature, it approves instead and keeps it (r4-11).
					execute(event);
					const lower = event.safe_tx_hash.toLowerCase();
					const ran = machineSeam.actions[`execute:${lower}`] as
						{ kind: string; preview?: { kind?: string } } | undefined;
					const before = ['fails', 'unpriced', 'unchecked', 'priced'];
					if (ran?.kind === 'stopped' && before.includes(ran.preview?.kind ?? '')) {
						fake.calls.push('approve');
						settle(`approve:${lower}`, { kind: 'done', kept: false });
					}
					return;
				}
				case 'send_again':
					fake.sentAgain += 1;
					fake.kept = null;
					machineSeam.outbox = {};
					return;
			}
		},
		async settled(key: string) {
			for (let i = 0; i < 200; i += 1) {
				const state = machineSeam.actions[key];
				if (state && state.kind !== 'working') return state;
				await new Promise((r) => setTimeout(r, 0));
			}
			return null;
		},
		async until(pick: (v: any) => unknown) {
			for (let i = 0; i < 20; i += 1) {
				const found = pick({ actions: machineSeam.actions, items: fake.chain?.items ?? [] });
				if (found !== null && found !== undefined) return found;
				await new Promise((r) => setTimeout(r, 0));
			}
			return null;
		},
		async propose(_a: unknown, _c: number, proposal: { kind: string }, opts: { as?: string }) {
			fake.calls.push(proposal.kind === 'cancel' ? 'reject' : `propose:${opts?.as ?? ''}`);
			if (fake.approveError) throw new ProposeFailedError(stateOf(fake.approveError), 100);
			return { safeTxHash: HASH };
		},
		proposeExecute: (as: string | null) => {
			fake.calls.push(`confirm:${as}`);
			sentLog.push({ type: 'propose_execute', as_member: as });
			// The machine proposes it, then executes it by its proposer (`execute:<hash>`
			// working from the moment it is proposed).
			if (fake.confirmed) {
				const hash = String(fake.confirmed.safe_tx_hash ?? '').toLowerCase();
				settle('propose', fake.confirmed);
				if (fake.confirmed.kind === 'done') settle(`execute:${hash}`, { kind: 'working' });
				void (async () => {
					await fake.onConfirm?.();
					if (fake.confirmed.kind === 'done') {
						execute({ chain_id: 100, safe_tx_hash: fake.confirmed.safe_tx_hash, executor: as });
					}
				})();
			}
		},
		confirmDraft: (as: string | null) => {
			fake.calls.push(`confirm:${as}`);
			// A dApp's draft: proposed, then shown as the proposal it became.
			if (fake.confirmed) {
				settle('propose', fake.confirmed);
				void fake.onConfirm?.();
			}
		},
		withdrawDraft: () => void fake.calls.push('withdraw')
	};
	return { proposalsHost: host, ProposeFailedError };
});
// §3.5: the network's check — the accounts machine's verdict (`check_on`;
// app_multisig_accounts.rs reads it), as the chain would answer it.
vi.mock('../core/store.svelte', async (importOriginal) => {
	const real = await importOriginal<typeof import('../core/store.svelte')>();
	real.multisig.checkOn = () => {};
	real.multisig.verdictOn = () => ({ checked: true, checking: false, findings: fake.checked });
	return real;
});
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
		riskDeclinedError: () => new DOMException('The person did not sign for the risk', 'AbortError'),
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
	REVIEW_RETRY_MS: 60_000,
	simEndedOf: async () => null,
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
vi.mock('../core/chain', () => ({ ChainUnavailableError: class extends Error {} }));
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
	tokenInfo: async (_chain: number, token: string) =>
		token === USDC ? { symbol: 'USDC', decimals: 0 } : { symbol: 'xDAI', decimals: 18 }
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
vi.mock('$lib/services/token-provenance', () => ({
	provenanceNote: () => fake.tokenNote
}));
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	nativeSymbol: () => 'xDAI'
}));

const { default: ProposalSheet } = await import('./ProposalSheet.svelte');
const { loadCore } = await import('$lib/core/client');

// The links are the core's codec: loaded, as the wallet loads it before any sheet.
beforeAll(() => loadCore());
const { resetMachineSeam, sentLog } = await import('./proposal-sheet.testing.svelte');

/** The events of `type` the sheet sent the machine. */
const machineSent = (type: string) => sentLog.filter((e) => e.type === type);

/** The machine's draft of the person's own proposal, reviewed before its passkey (§3.6). */
const draftOf = (draft: unknown, here: string[]) => ({
	item: draft,
	review: REVIEW,
	membersHere: here,
	defaultMember: here[0] ?? null,
	// core-ms-2: the proposer's own approval makes it ready (the machine's word).
	executeNow: ((draft as { canExecute?: unknown[] }).canExecute ?? []).length > 0
});

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
	fake.unreadable = [];
	fake.previews = { [ME]: priced() };
	fake.executedWith = [];
	fake.executeThrows = null;
	fake.chain = { chainId: 100, members, items: [item()], mailbox: true };
	fake.retried = 0;
	fake.machineDraft = null;
	fake.confirmed = null;
	fake.onConfirm = null;
	fake.linkedSource = null;
	resetMachineSeam();
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

/** A dApp's request as the machine holds it (`view.dapp[rid]`). */
function held(over: Record<string, unknown> = {}) {
	return {
		id: 'r1',
		method: 'eth_sendTransaction',
		origin: 'https://app.example',
		chain_id: 100,
		safe: SAFE,
		stage: 'reviewing',
		calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
		safe_tx_hash: null,
		nonce: null,
		tell_at: null,
		told: false,
		refusal: null,
		answer: null,
		...over
	} as any;
}

/** Its sheet: the held request, the claim before the passkey (RB5). */
function dappSheet(over: Record<string, unknown> = {}, claim = async () => true) {
	return { rid: 'r1', held: held(over), claim, retry: () => {} };
}

describe("a dApp's transaction from a multisig (dapp-1)", () => {
	it('is reviewed first: what the site asks, and nothing signed until Propose and approve', async () => {
		fake.machineDraft = draftOf(item(), [ME]);
		fake.confirmed = { kind: 'done', safe_tx_hash: HASH, kept: false };
		const claim = vi.fn(async () => true);
		const screen = show({ dapp: dappSheet({}, claim), safeTxHash: '' });
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
		expect(fake.calls).toEqual([]);

		press('send.propose');
		await settle();
		// RB5: asked whether the request is still live, then the machine's draft confirmed.
		expect(claim).toHaveBeenCalledTimes(1);
		expect(fake.calls).toEqual([`confirm:${ME}`]);

		press('external.decline');
		expect(machineSent('dapp_leave')).toEqual([{ type: 'dapp_leave', id: 'r1' }]);
		await screen.unmount();
	});

	it('a page gone before the passkey: nothing is confirmed', async () => {
		fake.machineDraft = draftOf(item(), [ME]);
		const screen = show({ dapp: dappSheet({}, async () => false), safeTxHash: '' });
		await settle();
		press('send.propose');
		await settle();
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});

	it('its review could not be drawn: why, with Decline and Retry', async () => {
		resetMachineSeam();
		const { machineSeam } = await import('./proposal-sheet.testing.svelte');
		machineSeam.actions = { propose: { kind: 'mailboxUnreadable' } };
		const retry = vi.fn();
		const screen = show({ dapp: { ...dappSheet(), retry }, safeTxHash: '' });
		await settle();
		expect(document.body.textContent).toContain('proposal.mailboxDown');
		press('external.retry');
		expect(retry).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});
});

describe("a dApp's eth_sendTransaction its proposer alone makes ready (round 3, G2)", () => {
	it('is proposed and executed in one go, and the site is handed the real transaction', async () => {
		const TX = `0x${'cd'.repeat(32)}`;
		const one = { ...members, threshold: 1 };
		const mine = [{ owner: ME, counts: true, approval: {}, createdMs: 1 }];
		fake.machineDraft = draftOf(item({ threshold: 1, canExecute: [local] }), [ME]);
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
		let screen: ReturnType<typeof show> | null = null;
		const sheet = dappSheet();
		// The machine proposed it; the host's sheet then shows the proposal it became.
		fake.confirmed = { kind: 'done', safe_tx_hash: HASH, kept: false };
		fake.onConfirm = async () => {
			await screen?.rerender({
				dapp: { ...sheet, held: { ...sheet.held, stage: 'waiting', safe_tx_hash: HASH } },
				safeTxHash: HASH
			});
		};
		screen = show({ dapp: sheet, safeTxHash: '' });
		await settle();
		// One door, worded for what it does: no wait for approvals nobody else gives.
		expect(buttons()).toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('send.propose');
		press('proposal.approveAndExecute');
		await settle();
		await settle();
		// Proposed, then executed — one press, the machine's (`propose_execute`):
		// it answers the site with the transaction.
		expect(fake.calls).toEqual([`confirm:${ME}`, 'execute']);
		expect(machineSent('propose_execute')).toHaveLength(1);
		expect(machineSent('execute')).toHaveLength(0);
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
		const sheet = dappSheet({ stage: 'waiting', safe_tx_hash: HASH });
		const screen = show({ dapp: sheet, m: words2 });
		await settle();
		expect(document.body.textContent).toContain('needed 1');
		expect(document.body.textContent).toContain('proposal.siteWaits');
		await screen.rerender({ dapp: { ...sheet, held: { ...sheet.held, told: true } } });
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
		// The shown multisig's list has nothing: the sheet reads the machine's `linked`.
		fake.chain = null;
		fake.linkedSource = linked();
		const onwatch = vi.fn();
		const screen = show({
			preview: true,
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
		fake.linkedSource = linked();
		const screen = show({
			preview: true,
			watch: { added: true, onwatch: () => {} },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		expect(document.body.textContent).toContain('proposal.watching');
		expect(buttons()).not.toContain('proposal.watch');
		await screen.unmount();
	});

	it('an outside approval is the machine’s, on the link’s transaction', async () => {
		fake.chain = null;
		fake.linkedSource = linked();
		const screen = show({
			preview: true,
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
		expect(machineSent('approve_outside')).toEqual([
			{ type: 'approve_outside', chain_id: 100, safe_tx_hash: HASH }
		]);
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
		// What it carried is kept by the machine from the look it signed (Rust:
		// `remember_execution`, app_multisig_proposals.rs): the executor alone is asked.
		expect(machineSent('execute')).toEqual([
			expect.objectContaining({ executor: ME, safe_tx_hash: HASH })
		]);
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
		// The machine read its flight back: its item says it is executing.
		fake.chain.items = [{ ...ready(), executing: true }];
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
		expect(onexecuted).toHaveBeenCalledWith(expect.objectContaining({ txHash: TX }));
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
				// The machine's word: the token it is short of, and the amounts.
				why: { kind: 'short', token: USDC, have: 28n, need: 50n }
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
		const prebuilt = { chainId: 100, address: SAFE, kind: 'transfers' };
		fake.machineDraft = draftOf(item({ canApprove: [local, SECOND] }), [ME, OTHER]);
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
		expect(fake.calls).toEqual([`confirm:${OTHER}`]);
		await screen.unmount();
	});

	it('one member here: no choice to show, it proposes', async () => {
		const prebuilt = { chainId: 100, address: SAFE, kind: 'transfers' };
		fake.machineDraft = draftOf(item({ canApprove: [local] }), [ME]);
		const screen = show({ prebuilt, safeTxHash: '' });
		await settle();
		expect(document.querySelector('[data-testid="actor-choice"]')).toBeNull();
		press('send.propose');
		await settle();
		expect(fake.calls).toEqual([`confirm:${ME}`]);
		await screen.unmount();
	});
});

describe('"Approve and execute" whose execute part cannot run (r4-11)', () => {
	it('keeps the approval, and says only executing could not run', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local], canApprove: [local] })];
		const { ExecutorCannotPayError } = await import('../core/execution');
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
		// r7-8: never listed as approving in Vela — said once, either way.
		expect(document.querySelector('[data-testid="approves-in-vela"]')).toBeNull();
		const either = document.querySelector('[data-testid="either-way"]');
		const outside = document.querySelector('[data-testid="outside-invite"]');
		expect(either).not.toBeNull();
		// The outside wallet's link is still there, after the Vela one.
		expect(outside).not.toBeNull();
		expect(
			either!.compareDocumentPosition(outside!) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
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
		fake.chain = { chainId: 100, members: strangers, items: [], mailbox: true };
		fake.machineDraft = draftOf(item({ canApprove: [], canExecute: [] }), []);
		const screen = show({ dapp: dappSheet(), safeTxHash: '' });
		await settle();
		expect(buttons()).toContain('send.proposeOnly');
		expect(buttons()).not.toContain('send.propose');
		await screen.unmount();
	});
});

describe('round 6: the review’s words and where its actions stand', () => {
	/** The person's own transfer, reviewed before its passkey (`prebuilt`). */
	const own = () => {
		fake.machineDraft = draftOf(
			item({
				review: { action: { kind: 'transfer', token: null, to: TO, amount: '1' }, refusal: null }
			}),
			[ME]
		);
		return { chainId: 100, address: SAFE, kind: 'transfers' };
	};

	it('r4D-13: the person’s own proposal is withdrawn, not declined (a site’s request still is)', async () => {
		const screen = show({ prebuilt: own(), safeTxHash: '' });
		await settle();
		expect(buttons()).toContain('proposal.dontPropose');
		expect(buttons()).not.toContain('external.decline');
		press('proposal.dontPropose');
		expect(fake.calls).toEqual(['withdraw']);
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
