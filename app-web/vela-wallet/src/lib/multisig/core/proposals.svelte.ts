/**
 * The active multisig's transactions in flight: what the mailbox holds,
 * re-checked here against the chain, and what this device can do about each.
 *
 * Nothing is taken from the mailbox on trust. For every proposal the core
 * re-hashes the transaction, re-reads what it does (`reviewTx`) and verifies
 * every approval (`verifyApproval`); an approval counts only if it verifies
 * AND its owner is an owner on that chain now. The members and nonce come from
 * the chain through the wallet's own RPC (`readMembers`) — on every network the
 * multisig is used on, including one the service does not read: the mailbox
 * stores proposals for any chain, but there it knows only the genesis (its
 * owners, nonce 0), so the service's own word on members is never used here.
 *
 * The four actions — propose, approve, execute, cancel — are each one passkey
 * prompt, and each is safe to retry: the mailbox merges, and an execution that
 * may have been sent is followed rather than sent again.
 */
import { hasCode, readMembers, type ChainMembers } from './chain';
import {
	submitExecution,
	throwIfStopped,
	type ExecutionInput,
	type ExecutionPreview
} from './execute';
import {
	callsTx,
	cancelTx,
	missingNetworks,
	multisigNetworks,
	namedGenesis,
	renameTx,
	reviewTx,
	transfersTx,
	safeTxHash,
	isEcdsaApproval,
	isOnChainApproval,
	sameAddress,
	verifyApproval,
	verifyEcdsaApproval,
	memberChangeTx,
	type Approval,
	type MemberChangeRequest,
	type Review,
	type SafeTx
} from './kernel';
import { MemberChangeRefusedError, memberStateOf } from './member-changes';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { localOwners, signApproval, type LocalOwner } from './owners';
import { plainAccounts } from './signers.svelte';
import { lastActor, proposerOf } from './actor';
import {
	fetchAllProposals,
	fetchProposals,
	genesisServiceURL,
	probeService,
	ServiceBusyError,
	ServiceUnavailableError,
	submitApproval,
	type Proposal,
	type SubmitAnswer
} from './service';
import { serviceOutage } from './service-state.svelte';
import { multisig, type MultisigAccount } from './store.svelte';
import { recordRemoteExecutions } from './records';
import { executions } from './execution-watch.svelte';
import { networkForChainId, networkRank } from '$lib/services/networks';
import { externalSigning } from '../external/signing.svelte';
import { authorizeSigning } from './gate.svelte';
import { reviewFirst } from './review-first.svelte';
import { onChainApprovals } from '../external/onchain';

export type ProposalStatus =
	/** Enough approvals, and its nonce is next: it can be executed now. */
	| 'ready'
	/** Waiting for approvals, its nonce next. */
	| 'pending'
	/** Behind an earlier nonce that has not been executed or voided yet. */
	| 'queued'
	/** Its nonce is used: executed, or voided by another transaction at it. */
	| 'done';

export interface ApprovalRow {
	owner: string;
	/**
	 * Verified, and its owner is an owner on this chain now. The mailbox takes
	 * an approval from anyone whose signature is valid; one from an account
	 * that is not a member here is shown as not counted, never counted.
	 */
	counts: boolean;
	approval: Approval;
	createdMs: number;
	/** Signed on this device and not in the mailbox yet (kept, sent again). */
	unsent?: boolean;
}

export interface ProposalItem {
	chainId: number;
	safeTxHash: string;
	tx: SafeTx;
	review: Review;
	nonce: bigint;
	proposer: string;
	approvals: ApprovalRow[];
	/** Approvals that count. */
	approved: number;
	threshold: number;
	status: ProposalStatus;
	/** This device's owners that have not approved yet. */
	canApprove: LocalOwner[];
	/** Who on this device can execute it now (it counts as one approval more). */
	canExecute: LocalOwner[];
	/** Another proposal at the same nonce that voids this one (a cancellation). */
	cancelledBy: string | null;
	/**
	 * r4b-6: the other open proposals at its number — only one of them can
	 * run; the rest are voided by it.
	 */
	sameNonce: string[];
	/**
	 * r4b-7: this device sent its execution and it may still land: not
	 * offered again, its sheet watches it.
	 */
	executing: boolean;
	createdMs: number;
	/**
	 * Something signed on this device for it has not reached the mailbox yet
	 * (the proposal itself, or an approval): other members cannot see that.
	 */
	unsent: boolean;
	/**
	 * A rename on a network where the multisig is not deployed: it cannot be
	 * executed there — a rename never deploys it (owner, 2026-10-29).
	 */
	renameUndeployed: boolean;
}

export interface ChainState {
	chainId: number;
	members: ChainMembers;
	items: ProposalItem[];
	/**
	 * Whether the mailbox's list was read just now. `false`: `items` are the
	 * last ones read, and no new proposal may take a nonce from them.
	 */
	mailbox: boolean;
}

function sortKey(a: ProposalItem, b: ProposalItem): number {
	if (a.nonce !== b.nonce) return a.nonce < b.nonce ? -1 : 1;
	return a.createdMs - b.createdMs;
}

/**
 * What one stored proposal is, as this device reads it. `onChain`: owners
 * this device read from the chain as having approved it there
 * (`approveHash`) — never one the mailbox says did.
 */
export function readProposal(
	account: Pick<MultisigAccount, 'address'>,
	members: ChainMembers,
	p: Proposal,
	onChain: string[] = []
): ProposalItem | null {
	const hash = safeTxHash(p.chainId, account.address, p.safeTx);
	if (hash.toLowerCase() !== p.safeTxHash.toLowerCase()) return null;
	const review = reviewTx(p.chainId, account.address, p.safeTx);
	const nonce = BigInt(p.safeTx.nonce);
	const approvals: ApprovalRow[] = [];
	for (const a of p.approvals) {
		if (approvals.some((r) => sameAddress(r.owner, a.owner))) continue;
		let valid: boolean;
		try {
			// An on-chain approval is the chain's to say, read here — never the mailbox's.
			valid = isOnChainApproval(a)
				? false
				: isEcdsaApproval(a)
					? sameAddress(verifyEcdsaApproval(hash, a).owner, a.owner)
					: verifyApproval(p.chainId, hash, a);
		} catch {
			valid = false;
		}
		approvals.push({
			owner: a.owner,
			counts: valid && members.owners.some((o) => sameAddress(o, a.owner)),
			approval: a,
			createdMs: a.createdMs
		});
	}
	for (const owner of onChain) {
		const at = approvals.findIndex((r) => sameAddress(r.owner, owner));
		if (at >= 0 && approvals[at].counts) continue;
		const row: ApprovalRow = {
			owner,
			counts: members.owners.some((o) => sameAddress(o, owner)),
			approval: { owner, onChain: true },
			createdMs: p.createdMs
		};
		if (at >= 0) approvals[at] = row;
		else approvals.push(row);
	}
	const approved = approvals.filter((a) => a.counts).length;
	const local = localOwners(members.owners);
	const canApprove = local.filter(
		(o) => !approvals.some((a) => a.counts && sameAddress(a.owner, o.address))
	);
	const status: ProposalStatus =
		nonce < members.nonce
			? 'done'
			: nonce > members.nonce
				? 'queued'
				: approved >= members.threshold
					? 'ready'
					: 'pending';
	// An owner executing is `msg.sender`: one approval more if it has not
	// approved. Once the approvals alone meet the threshold, any Vela account
	// on this device can send it (a Safe imported with outside owners only).
	const owners = local.filter((o) => {
		const own = approvals.some((a) => a.counts && sameAddress(a.owner, o.address));
		return approved + (own ? 0 : 1) >= members.threshold;
	});
	const anyone =
		approved >= members.threshold
			? localOwners(loadAccounts().map((a) => a.address)).filter(
					(o) => !owners.some((x) => sameAddress(x.address, o.address))
				)
			: [];
	// A rename runs only where the multisig is deployed: executed elsewhere it
	// would deploy it there with its original members (the core refuses it too).
	const renameUndeployed = review.action?.kind === 'rename' && !members.deployed;
	const executing = status !== 'done' && !!executions.get(p.chainId, account.address, hash);
	const canExecute =
		status === 'done' ||
		status === 'queued' ||
		review.action === null ||
		renameUndeployed ||
		executing
			? []
			: [...owners, ...anyone];
	return {
		chainId: p.chainId,
		safeTxHash: hash,
		tx: p.safeTx,
		review,
		nonce,
		proposer: p.proposer,
		approvals,
		approved,
		threshold: members.threshold,
		status,
		canApprove: review.action === null || status === 'done' ? [] : canApprove,
		canExecute,
		cancelledBy: null,
		sameNonce: [],
		executing,
		createdMs: p.createdMs,
		unsent: false,
		renameUndeployed
	};
}

/**
 * The mailbox's list for one chain, with what this device signed for it and
 * the mailbox has not taken yet ({@link outbox}): a proposal it lacks is
 * there as it was signed, an approval it lacks is added to its proposal.
 * Resolves which proposals and approvals are only here.
 */
function withKept(
	account: MultisigAccount,
	chainId: number,
	list: Proposal[]
): { list: Proposal[]; unsent: Map<string, Set<string>> } {
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for one read and returned, never state
	const unsent = new Map<string, Set<string>>();
	const kept = outbox.kept(chainId, account.address);
	if (kept.length === 0) return { list, unsent };
	const out = list.map((p) => ({ ...p, approvals: [...p.approvals] }));
	for (const req of kept) {
		const hash = safeTxHash(chainId, account.address, req.safeTx).toLowerCase();
		let p = out.find((x) => x.safeTxHash.toLowerCase() === hash);
		if (!p) {
			p = {
				chainId,
				safe: account.address,
				safeTxHash: hash,
				safeTx: req.safeTx,
				proposer: req.approval.owner,
				approvals: [],
				createdMs: Date.now(),
				updatedMs: Date.now()
			};
			out.push(p);
		}
		if (p.approvals.some((a) => sameAddress(a.owner, req.approval.owner))) continue;
		p.approvals.push({ ...req.approval, createdMs: Date.now() });
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- part of the one read above
		const owners = unsent.get(hash) ?? new Set<string>();
		owners.add(req.approval.owner.toLowerCase());
		unsent.set(hash, owners);
	}
	return { list: out, unsent };
}

/**
 * One chain of `account`, read now: its members from the chain, its proposals
 * from the mailbox. `mailbox: false` when the mailbox could not be read — the
 * items are then `previous`'s (the last list read), re-read against the
 * members now, and nothing may be proposed from them (a nonce taken from a
 * list that is not there would be one an open proposal already holds).
 * Throws when the members cannot be read.
 *
 * `background`: nobody is waiting on this read (the refresh). `mailbox:
 * false` skips the mailbox — the service is away, and was asked once for
 * the whole round (`serviceOutage`).
 */
export async function readChain(
	account: MultisigAccount,
	chainId: number,
	previous: Proposal[] | null = null,
	{
		background = false,
		mailbox: ask = true,
		listed,
		members: known
	}: {
		background?: boolean;
		mailbox?: boolean;
		/** This chain's part of the cross-chain list, read for every chain at once. */
		listed?: Proposal[];
		/** Its members as last read, taken as they are (a quiet poll, r4D-18). */
		members?: ChainMembers;
	} = {}
): Promise<ChainState & { proposals: Proposal[] }> {
	const [members, answer] = await Promise.all([
		known ?? readMembers(chainId, account.address, account.genesis),
		// `false`: unread (a `null` is the mailbox knowing nothing of it).
		listed !== undefined
			? { proposals: listed }
			: ask
				? fetchProposals(chainId, account.address, { background }).catch(() => false as const)
				: (false as const)
	]);
	const mailbox = answer !== false;
	const list = answer === false ? (previous ?? []) : (answer?.proposals ?? []);
	const shown = withKept(account, chainId, list);
	const onChain = await approvedOnChain(account.address, chainId, members, shown.list);
	const items = shown.list
		.map((p) => {
			const item = readProposal(account, members, p, onChain.get(p.safeTxHash.toLowerCase()));
			const owners = shown.unsent.get(p.safeTxHash.toLowerCase());
			if (item && owners) {
				item.unsent = true;
				for (const row of item.approvals) row.unsent = owners.has(row.owner.toLowerCase());
			}
			return item;
		})
		.filter((i): i is ProposalItem => i !== null);
	markCancellations(account.address, items);
	return { chainId, members, items, mailbox, proposals: list };
}

/**
 * r4C-16: per open proposal of a deployed multisig, the members with no
 * approval in the mailbox that approved it on chain (`approveHash`).
 */
async function approvedOnChain(
	safe: string,
	chainId: number,
	members: ChainMembers,
	list: Proposal[]
): Promise<Map<string, string[]>> {
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for one read and returned, never state
	const out = new Map<string, string[]>();
	if (!members.deployed) return out;
	await Promise.all(
		list.map(async (p) => {
			let nonce: bigint;
			try {
				nonce = BigInt(p.safeTx.nonce);
			} catch {
				return;
			}
			if (nonce < members.nonce) return;
			const ask = members.owners.filter(
				(o) => !p.approvals.some((a) => sameAddress(a.owner, o) && !isOnChainApproval(a))
			);
			if (ask.length === 0) return;
			const found = await onChainApprovals(chainId, safe, p.safeTxHash, ask);
			if (found.length > 0) out.set(p.safeTxHash.toLowerCase(), found);
		})
	);
	return out;
}

/** The nonce a new proposal on `chain` takes: after every open one. */
export function nextNonceOf(chain: ChainState): bigint {
	let n = chain.members.nonce;
	for (const i of chain.items) if (i.status !== 'done' && i.nonce >= n) n = i.nonce + 1n;
	return n;
}

/** A refresh still out after this long: the person is told the list may be stale. */
export const SLOW_REFRESH_MS = 8_000;
/** The poll's pace while the multisig is shown. */
export const POLL_MS = 12_000;
/**
 * r5reg-14: the pace while a sheet on one of its waiting transactions is
 * open (`watch`) — another member's approval is seen there within about
 * five seconds, not at the next 12 s round.
 */
export const OPEN_POLL_MS = 4_000;
/**
 * r4D-18 (round 5): on a network with nothing waiting — no open proposal in
 * the mailbox's list or here, nothing of this device's in flight — the chain
 * is read every this many polls (a minute), not every poll: nothing there can
 * be executed meanwhile but a transaction from outside Vela, said within the
 * minute. Anything waiting, and every read the person or the page asks for,
 * reads the chain at once.
 */
export const QUIET_POLLS = 5;

/** Whether `list` holds a proposal at or past `nonce` (one that can still run). */
function hasOpen(list: Proposal[], nonce: bigint): boolean {
	return list.some((p) => {
		try {
			return BigInt(p.safeTx.nonce) >= nonce;
		} catch {
			return true;
		}
	});
}

/**
 * Proposals the mailbox holds for the multisig on a network this device
 * does not have (a member's own network, say): shown as that, with the way
 * to add the network — never dropped, never judged (nothing there can be
 * read), and never counted as waiting on anyone here until it is added.
 */
export interface MissingChain {
	chainId: number;
	/** How many proposals it holds there. */
	count: number;
}

/** Whether this device has the network (built in, or one the person added). */
export function hasNetwork(chainId: number): boolean {
	return networkForChainId(chainId) !== null;
}

/**
 * The cross-chain list split by chain: the networks something there puts
 * the multisig on (the core's rule, `have` this device's), and those this
 * device lacks.
 */
export function splitListed(
	listed: Proposal[],
	chainIds: number[]
): { lists: Map<number, Proposal[]>; used: number[]; missing: MissingChain[] } {
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for one read and returned, never state
	const lists = new Map<number, Proposal[]>();
	for (const p of listed) lists.set(p.chainId, [...(lists.get(p.chainId) ?? []), p]);
	const proposed = [...lists.keys()];
	const have = unique([...proposed, ...chainIds]).filter(hasNetwork);
	const used = multisigNetworks({ proposals: proposed, kept: chainIds, have });
	const missing = missingNetworks({ proposals: proposed, have }).map((chainId) => ({
		chainId,
		count: lists.get(chainId)?.length ?? 0
	}));
	return { lists, used, missing };
}

/**
 * r4b-11: the last list read for each multisig, kept across reloads — its
 * members and the mailbox's waiting proposals per chain, by service. Shown at
 * once on the next page (as the last known, said so), re-read against the
 * chain as soon as it answers; never a source of a new proposal's number.
 */
const WAITING_KEY = 'vela.multisigWaiting';
const WAITING_CAP = 20;

interface KeptChain {
	chainId: number;
	members: Omit<ChainMembers, 'nonce'> & { nonce: string };
	proposals: Proposal[];
}

interface KeptList {
	service: string;
	at: number;
	chains: KeptChain[];
}

function readWaiting(): Record<string, KeptList> {
	try {
		return JSON.parse(localStorage.getItem(WAITING_KEY) ?? '{}') as Record<string, KeptList>;
	} catch {
		return {};
	}
}

/** The list kept for `address` from `service`, or `null`. */
export function keptWaiting(address: string, service: string): KeptList | null {
	const kept = readWaiting()[address.toLowerCase()];
	return kept && kept.service === service ? kept : null;
}

/**
 * r4b-11: the waiting transactions kept for `account` from an earlier page
 * (this service's), each re-read against the members kept with it — what the
 * home shows at once, and what the switcher counts (r4D-7) before this page
 * has read it. `null`: none is kept.
 */
export function keptChains(
	account: MultisigAccount
): { chains: ChainState[]; lists: Map<number, Proposal[]>; at: number } | null {
	const kept = keptWaiting(account.address, genesisServiceURL());
	if (!kept) return null;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for one read and returned, never state
	const lists = new Map<number, Proposal[]>();
	const chains: ChainState[] = kept.chains.map((c) => {
		const members: ChainMembers = { ...c.members, nonce: BigInt(c.members.nonce) };
		const items = c.proposals
			.map((p) => readProposal(account, members, p))
			.filter((i): i is ProposalItem => i !== null);
		markCancellations(account.address, items);
		lists.set(c.chainId, c.proposals);
		return { chainId: c.chainId, members, items, mailbox: false };
	});
	return { chains, lists, at: kept.at };
}

function keepWaiting(address: string, list: KeptList): void {
	try {
		const all = { ...readWaiting(), [address.toLowerCase()]: list };
		const keys = Object.keys(all);
		for (const old of keys.slice(0, Math.max(0, keys.length - WAITING_CAP))) delete all[old];
		localStorage.setItem(WAITING_KEY, JSON.stringify(all));
	} catch {
		// best effort: the next page reads it afresh
	}
}

class Proposals {
	/** Per chain the multisig is used on. */
	chains = $state<ChainState[]>([]);
	/**
	 * r4b-11: what is shown is the list kept from an earlier page, not read
	 * yet on this one (`at`: when it was read). Said as the last known.
	 */
	cached = $state<{ at: number } | null>(null);
	/** r4b-3: networks whose members could not be read on the last refresh. */
	unreadable = $state<number[]>([]);
	/** r4b-2: networks whose history could not be read (executions elsewhere may be missing). */
	historyUnreadable = $state<number[]>([]);
	/** r4b-15: the device says it is offline. */
	offline = $state(false);
	/** When the list shown was last read in full (the offline line names it). */
	readAt = $state<number | null>(null);
	/** The networks the mailbox holds proposals of it on that this device lacks. */
	missing = $state<MissingChain[]>([]);
	loading = $state(false);
	/**
	 * The last refresh failed (offline, the service down) or is taking long:
	 * the list may be stale, and whoever shows it says so.
	 */
	stale = $state(false);
	/** New activity was written: whoever shows it refreshes. */
	onrecorded: (() => void) | null = null;
	#address: string | null = null;
	/**
	 * r5reg-17: the multisig followed, as state: a view derived from whether
	 * it is the one followed reads it again when it becomes so. A plain field
	 * read once (the home's first-run card, before the host followed it) left
	 * the card up for good — the networks it was deployed on never re-read.
	 */
	#shown = $state<string | null>(null);
	#timer: ReturnType<typeof setInterval> | null = null;
	#inflight: Promise<void> | null = null;
	/** A read asked for while another was under way: it runs next. */
	#queued: Promise<void> | null = null;
	/** The poll's rounds since the last read the person or the page asked for. */
	#ticks = 0;
	/** The last list the mailbox gave, per chain: kept while it cannot be read. */
	#lists = new Map<number, Proposal[]>();
	/** The service those lists came from: another one's say nothing of it. */
	#source: string | null = null;
	#chainIds: number[] = [];
	#account: MultisigAccount | null = null;
	#online: (() => void) | null = null;
	#offline: (() => void) | null = null;
	#visible: (() => void) | null = null;
	/** Open sheets holding the faster pace (`watch`). */
	#held = 0;
	#pace = POLL_MS;

	/** The multisig followed (`null`: none). */
	get followed(): string | null {
		return this.#shown;
	}

	get items(): ProposalItem[] {
		return this.chains.flatMap((c) => c.items).sort(sortKey);
	}

	/** Open proposals (not done), next first. */
	get open(): ProposalItem[] {
		return this.items.filter((i) => i.status !== 'done');
	}

	/** Follow `account` (or stop, `null`): refresh now and every 12 s while shown. */
	follow(account: MultisigAccount | null, chainIds: number[]): void {
		const address = account?.address ?? null;
		if (this.#timer) clearInterval(this.#timer);
		this.#timer = null;
		if (this.#online) window.removeEventListener('online', this.#online);
		if (this.#offline) window.removeEventListener('offline', this.#offline);
		if (this.#visible) document.removeEventListener('visibilitychange', this.#visible);
		this.#online = null;
		this.#offline = null;
		this.#visible = null;
		if (!sameAddress(this.#address, address)) {
			this.chains = [];
			this.missing = [];
			this.#lists.clear();
			this.stale = false;
			this.cached = null;
			this.unreadable = [];
			this.historyUnreadable = [];
			this.readAt = null;
		}
		this.#address = address;
		this.#shown = address;
		this.#account = account;
		this.#chainIds = chainIds;
		if (!account) return;
		if (this.chains.length === 0) this.#seed(account);
		void this.refresh(account, chainIds);
		if (typeof window !== 'undefined') {
			this.offline = navigator.onLine === false;
			// Back online: the outage is over as far as anyone knows — read now.
			this.#online = () => {
				this.offline = false;
				void this.retry();
			};
			this.#offline = () => (this.offline = true);
			window.addEventListener('online', this.#online);
			window.addEventListener('offline', this.#offline);
			// r4b-16: back to the tab — read now, not at the next tick.
			this.#visible = () => {
				if (document.visibilityState === 'visible') void this.refresh(account, chainIds);
			};
			document.addEventListener('visibilitychange', this.#visible);
		}
		this.#poll();
	}

	/** The poll, at the pace it is held at now. */
	#poll(): void {
		if (this.#timer) clearInterval(this.#timer);
		this.#timer = null;
		const account = this.#account;
		if (!account) return;
		const chainIds = this.#chainIds;
		this.#pace = this.#held > 0 ? OPEN_POLL_MS : POLL_MS;
		this.#timer = setInterval(() => {
			if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return;
			void this.refresh(account, chainIds, { tick: true });
		}, this.#pace);
	}

	/**
	 * r5reg-14: keep it fresh while a sheet on one of its waiting
	 * transactions is open — the poll at {@link OPEN_POLL_MS}, a read at once
	 * when the list is older than that, and one whenever the window gets the
	 * focus back. The returned function lets go (the pace returns to
	 * {@link POLL_MS} once no sheet holds it).
	 */
	watch(): () => void {
		this.#held += 1;
		if (this.#pace !== OPEN_POLL_MS) this.#poll();
		const account = this.#account;
		if (account && (this.readAt === null || Date.now() - this.readAt > OPEN_POLL_MS)) {
			void this.refresh(account, this.#chainIds);
		}
		const onFocus = () => {
			if (this.#account) void this.refresh(this.#account, this.#chainIds);
		};
		if (typeof window !== 'undefined') window.addEventListener('focus', onFocus);
		let released = false;
		return () => {
			if (released) return;
			released = true;
			this.#held -= 1;
			if (typeof window !== 'undefined') window.removeEventListener('focus', onFocus);
			if (this.#held === 0 && this.#pace !== POLL_MS) this.#poll();
		};
	}

	/** Tests: the poll's pace now. */
	get pace(): number {
		return this.#pace;
	}

	/**
	 * r4b-11: the list kept from an earlier page, shown before anything is
	 * read — each proposal re-read here against the members kept with it.
	 */
	#seed(account: MultisigAccount): void {
		try {
			const kept = keptChains(account);
			if (!kept) return;
			for (const [chainId, list] of kept.lists) this.#lists.set(chainId, list);
			this.chains = kept.chains.sort((a, b) => networkRank(a.chainId) - networkRank(b.chainId));
			this.#source = genesisServiceURL();
			this.cached = { at: kept.at };
			this.readAt = kept.at;
			this.stale = true;
		} catch {
			// a kept list this build cannot read: read afresh
			this.chains = [];
			this.#lists.clear();
		}
	}

	/**
	 * Whether the stale list is stale because the multisig service is away
	 * (rather than slow, or a chain unread): said as such, with a Retry.
	 */
	get unreachable(): boolean {
		return this.stale && serviceOutage.down;
	}

	/** The person's Retry (or `online`): ask the service now, whatever its schedule. */
	retry(): Promise<void> {
		serviceOutage.reset();
		this.historyUnreadable = [];
		const account = this.#account;
		return account ? this.refresh(account, this.#chainIds) : Promise.resolve();
	}

	/**
	 * Re-read every chain the multisig is used on. One refresh at a time.
	 * `tick`: the poll's own round — a quiet network's chain is read only
	 * every {@link QUIET_POLLS}th. `fresh`: a read begun after this call —
	 * one under way is waited out, then another made.
	 */
	refresh(
		account: MultisigAccount,
		chainIds: number[],
		{ tick = false, fresh = false }: { tick?: boolean; fresh?: boolean } = {}
	): Promise<void> {
		if (this.#inflight) {
			if (!fresh) return this.#inflight;
			// r5reg-14: a read after what this device just did (an approval)
			// is never answered by one begun before it — it runs once the one
			// under way ends (one queued, however many ask).
			const next = () => {
				this.#queued = null;
				return this.refresh(account, chainIds);
			};
			this.#queued ??= this.#inflight.then(next, next);
			return this.#queued;
		}
		const round = tick ? (this.#ticks += 1) : (this.#ticks = 0);
		// Another service (Settings): the lists the last one gave are not its.
		const source = genesisServiceURL();
		if (this.#source !== source) {
			this.#lists.clear();
			if (this.#source !== null) this.chains = [];
			this.#source = source;
		}
		this.loading = this.chains.length === 0;
		const followed = () => sameAddress(this.#address, account.address);
		// Slow is said as stale: "No activity yet" over transactions the
		// mailbox has not answered for would be a claim nobody checked.
		const slow = setTimeout(() => {
			if (followed()) this.stale = true;
		}, SLOW_REFRESH_MS);
		const run = (async () => {
			let failed = false;
			const next: ChainState[] = [];
			const unread: number[] = [];
			// While the service is away: nothing until its next step, then one
			// probe for every network — not a failed read per network per round.
			let mailbox = await serviceOutage.available(probeService);
			// One read for every chain at once (a service that has the list):
			// proposals on a network the multisig is not deployed on, or one
			// this device lacks, are found without a link. `null`: an older
			// service — asked network by network, as before.
			let split: ReturnType<typeof splitListed> | null = null;
			if (mailbox) {
				try {
					const all = await fetchAllProposals(account.address, { background: true });
					if (all) split = splitListed(all.proposals, chainIds);
				} catch {
					mailbox = false;
				}
			}
			if (split) {
				// A network something there says it is on becomes one of its
				// networks here (kept: the next refresh reads it too).
				for (const id of split.used) {
					if (!account.chainIds.includes(id)) multisig.useChain(account.address, id);
				}
				if (followed()) this.missing = split.missing;
			}
			const reading = split ? unique([...chainIds, ...split.used]) : chainIds;
			await Promise.all(
				reading.map(async (chainId) => {
					const known = followed() ? (this.#lists.get(chainId) ?? null) : null;
					const listed = split ? (split.lists.get(chainId) ?? []) : undefined;
					const previous = followed() ? this.chains.find((c) => c.chainId === chainId) : undefined;
					// r4D-18: a quiet network's members, as last read, on all
					// but every QUIET_POLLS-th poll.
					const quiet =
						tick &&
						round % QUIET_POLLS !== 0 &&
						previous !== undefined &&
						previous.mailbox &&
						listed !== undefined &&
						!hasOpen(listed, previous.members.nonce) &&
						!previous.items.some((i) => i.status !== 'done' || i.executing);
					try {
						const read = await readChain(account, chainId, known, {
							background: true,
							mailbox,
							listed,
							members: quiet && previous ? previous.members : undefined
						});
						if (!read.mailbox) failed = true;
						else if (followed()) this.#lists.set(chainId, read.proposals);
						next.push({
							chainId,
							members: read.members,
							items: read.items,
							mailbox: read.mailbox
						});
					} catch {
						// r4b-3: the network did not answer — said as that, never as
						// a transaction nobody shared.
						failed = true;
						unread.push(chainId);
						const previous = this.chains.find((c) => c.chainId === chainId);
						if (previous) next.push({ ...previous, mailbox: false });
					}
				})
			);
			clearTimeout(slow);
			if (followed()) {
				// The wallet's network order (owner decision 2026-10-29).
				this.chains = next.sort((a, b) => networkRank(a.chainId) - networkRank(b.chainId));
				this.stale = failed;
				this.unreadable = unread;
				if (!failed) {
					this.cached = null;
					this.readAt = Date.now();
					this.#keep(account, source);
				}
				// Executed on another member's device (or this one's, before a
				// reload): no longer in flight here.
				for (const item of this.items) {
					if (item.status === 'done')
						executions.end(item.chainId, account.address, item.safeTxHash);
				}
				// Executed on another member's device: into this device's activity too.
				void recordRemoteExecutions(account.address, this.items).then(
					({ wrote, unreadable }) => {
						if (followed()) this.historyUnreadable = unreadable;
						if (wrote) this.onrecorded?.();
					},
					() => {}
				);
			}
			this.loading = false;
		})();
		this.#inflight = run.finally(() => (this.#inflight = null));
		return this.#inflight;
	}

	/** r4b-11: what was read in full, kept for the next page (open proposals only). */
	#keep(account: MultisigAccount, service: string): void {
		keepWaiting(account.address, {
			service,
			at: Date.now(),
			chains: this.chains.map((c) => ({
				chainId: c.chainId,
				members: { ...c.members, nonce: c.members.nonce.toString() },
				proposals: (this.#lists.get(c.chainId) ?? []).filter(
					(p) => BigInt(p.safeTx.nonce) >= c.members.nonce
				)
			}))
		});
	}

	chain(chainId: number): ChainState | undefined {
		return this.chains.find((c) => c.chainId === chainId);
	}

	/** The nonce a new proposal on `chainId` takes: after every open one. */
	nextNonce(chainId: number): bigint {
		const c = this.chain(chainId);
		return c ? nextNonceOf(c) : 0n;
	}
}

/**
 * A proposal at the same nonce that sends nothing to the multisig itself
 * voids the others; and every open proposal knows the others waiting at its
 * number (r4b-6: only one of them can run).
 */
export function markCancellations(multisigAddress: string, items: ProposalItem[]): void {
	for (const c of items) {
		if (c.review.action?.kind !== 'cancel') continue;
		for (const i of items) {
			if (i !== c && i.nonce === c.nonce) i.cancelledBy = c.safeTxHash;
		}
	}
	for (const i of items) {
		if (i.status === 'done') continue;
		i.sameNonce = items
			.filter((o) => o !== i && o.status !== 'done' && o.nonce === i.nonce)
			.map((o) => o.safeTxHash);
	}
	void multisigAddress;
}

export const proposals = new Proposals();

/**
 * Everyone who is a member of `account` on some network: on each network it
 * is deployed on, its members there now (as read for the multisig shown),
 * and its original members wherever it is not — or for one not shown, whose
 * networks are not read here. Who may sign on a given network is that
 * network's alone (`readProposal`); this only says whether any member is.
 */
export function membersAnywhere(account: Pick<MultisigAccount, 'address' | 'owners'>): string[] {
	if (!sameAddress(proposals.followed, account.address)) return account.owners;
	const deployed = proposals.chains.filter((c) => c.members.deployed);
	const undeployed = deployed.length < proposals.chains.length || deployed.length === 0;
	return unique([
		...deployed.flatMap((c) => c.members.owners),
		...(undeployed ? account.owners : [])
	]);
}

/** Each once, in order (addresses by their case: one network's list has one case). */
function unique<T>(values: T[]): T[] {
	return values.filter((v, i) => values.indexOf(v) === i);
}

// -- actions --------------------------------------------------------------------------

/** Propose `tx` on `chainId`, approved at once by `owner` (one passkey prompt). */
export async function propose(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx,
	owner: LocalOwner,
	signal?: AbortSignal
) {
	// A delegatecall into nothing "succeeds" doing nothing and spends the
	// nonce: the batch contract (MultiSendCallOnly) must be on this chain, which
	// a network the wallet accepts for Vela accounts does not promise.
	if (Number(tx.operation) === 1 && !(await hasCode(chainId, tx.to))) {
		throw new Error(`${tx.to} has no code on chain ${chainId}`);
	}
	// §3.5: read-only where its genesis is refused; asked, every time, where
	// more than its members can act for it — before the passkey.
	await authorizeSigning(account, chainId, tx);
	const hash = safeTxHash(chainId, account.address, tx);
	stopIfWithdrawn(signal);
	const approval = await signApproval(owner.account, chainId, hash, {
		multisig: account.address,
		name: account.genesis.name ?? account.label ?? null,
		tx,
		signal
	});
	// r4b-5: withdrawn while the passkey was up: what it signed goes nowhere.
	stopIfWithdrawn(signal);
	return submitProposal(account, chainId, tx, approval);
}

/** r4b-5: the person withdrew it (the confirm dismissed): nothing more is asked or sent. */
function stopIfWithdrawn(signal: AbortSignal | undefined): void {
	if (signal?.aborted) throw new DOMException('the proposal was withdrawn', 'AbortError');
}

/**
 * Signed approvals not yet in the mailbox (ms-19): written before the POST,
 * removed once the mailbox answered (took it or refused it), and sent again
 * when a page comes up — so a reload or a closed window between the passkey
 * and the mailbox never loses what the person signed. The mailbox merges, so
 * sending one twice is harmless.
 *
 * A mailbox that is busy (`429`) or out of reach is never a failure of what
 * was signed: the approval stays here, is shown as waiting to be sent (its
 * transaction reads as any other, from what was signed), and is sent again
 * on the service's word (`Retry-After`, else a backoff) for
 * {@link OUTBOX_PATIENCE_MS}; then it is said as not sent, kept, with Send
 * again. Nothing is ever signed twice.
 */
const OUTBOX_KEY = 'vela.multisigOutbox';

type Submission = Parameters<typeof submitApproval>[0];

function readOutbox(): Record<string, Submission> {
	try {
		return JSON.parse(localStorage.getItem(OUTBOX_KEY) ?? '{}') as Record<string, Submission>;
	} catch {
		return {};
	}
}

function writeOutbox(outbox: Record<string, Submission>): void {
	try {
		if (Object.keys(outbox).length === 0) localStorage.removeItem(OUTBOX_KEY);
		else localStorage.setItem(OUTBOX_KEY, JSON.stringify(outbox));
	} catch {
		// best effort: the submission still goes now
	}
}

function outboxKey(req: Submission): string {
	return `${req.chainId}:${req.safe.toLowerCase()}:${safeTxHash(req.chainId, req.safe, req.safeTx)}:${req.approval.owner.toLowerCase()}`;
}

/** How long a kept approval is sent again on its own before it is said as not sent. */
export const OUTBOX_PATIENCE_MS = 120_000;
/** The backoff when the service names no wait: 2 s, doubling, at most this. */
const OUTBOX_BACKOFF_MAX_MS = 30_000;

/** Where one kept approval stands. */
export interface OutboxEntry {
	/** `busy`: the service said wait (429); `away`: nothing answered. */
	why: 'busy' | 'away';
	/** When it is sent again (`null`: not on its own any more — Send again). */
	retryAt: number | null;
}

class Outbox {
	/** Per kept approval (its key): why it is waiting, and until when. */
	entries = $state<Record<string, OutboxEntry>>({});
	/** When each began to wait, for the patience. */
	#since = new Map<string, number>();
	#tries = new Map<string, number>();
	#timers = new Map<string, ReturnType<typeof setTimeout>>();
	/** It reached the mailbox: whoever shows it reads again. */
	onsent: ((safe: string) => void) | null = null;

	/** The kept approvals of `safe` on `chainId`, as they were signed. */
	kept(chainId: number, safe: string): Submission[] {
		void this.entries;
		return Object.values(readOutbox()).filter(
			(r) => r.chainId === chainId && sameAddress(r.safe, safe)
		);
	}

	/** Where `safeTxHash`'s kept approvals stand (the first that waits), or `null`. */
	stateOf(chainId: number, safe: string, hash: string): OutboxEntry | null {
		const prefix = `${chainId}:${safe.toLowerCase()}:${hash.toLowerCase()}:`;
		for (const [key, entry] of Object.entries(this.entries)) {
			if (key.toLowerCase().startsWith(prefix)) return entry;
		}
		return null;
	}

	/**
	 * Hand one signed approval to the mailbox, kept until it answers.
	 * Resolves the mailbox's answer, or `null` when it is kept to be sent
	 * again (busy or away); a refusal is the mailbox's answer, and drops it.
	 */
	async send(req: Submission): Promise<SubmitAnswer | null> {
		const key = outboxKey(req);
		writeOutbox({ ...readOutbox(), [key]: req });
		return this.#attempt(key, req);
	}

	async #attempt(key: string, req: Submission): Promise<SubmitAnswer | null> {
		try {
			const answer = await submitApproval(req);
			this.#drop(key);
			if (answer.ok) this.onsent?.(req.safe);
			return answer;
		} catch (error) {
			// Only a mailbox that did not take it (busy, away, failing) keeps
			// it; anything else is this device's own fault, said at once.
			if (!(error instanceof ServiceUnavailableError)) {
				this.#drop(key);
				throw error;
			}
			this.#wait(key, req, error instanceof ServiceBusyError ? error : null);
			return null;
		}
	}

	#wait(key: string, req: Submission, busy: ServiceBusyError | null): void {
		const now = Date.now();
		const since = this.#since.get(key) ?? now;
		this.#since.set(key, since);
		const tries = (this.#tries.get(key) ?? 0) + 1;
		this.#tries.set(key, tries);
		const backoff = Math.min(OUTBOX_BACKOFF_MAX_MS, 2_000 * 2 ** (tries - 1));
		const wait = Math.max(1_000, busy?.waitMs ?? backoff);
		const why = busy ? 'busy' : 'away';
		if (now + wait > since + OUTBOX_PATIENCE_MS) {
			// Long enough: said as not sent, kept, and sent on the person's word.
			this.entries = { ...this.entries, [key]: { why, retryAt: null } };
			return;
		}
		this.entries = { ...this.entries, [key]: { why, retryAt: now + wait } };
		clearTimeout(this.#timers.get(key));
		this.#timers.set(
			key,
			setTimeout(() => {
				this.#timers.delete(key);
				const still = readOutbox()[key];
				if (still) void this.#attempt(key, still);
				else this.#drop(key);
			}, wait)
		);
	}

	#drop(key: string): void {
		const { [key]: _gone, ...rest } = readOutbox();
		void _gone;
		writeOutbox(rest);
		clearTimeout(this.#timers.get(key));
		this.#timers.delete(key);
		this.#since.delete(key);
		this.#tries.delete(key);
		if (key in this.entries) {
			const { [key]: _done, ...left } = this.entries;
			void _done;
			this.entries = left;
		}
	}

	/**
	 * Send again what is kept — every approval, or those of one transaction
	 * (`hash`) — with a fresh patience: a page coming up, or the person's
	 * Send again. Resolves the multisigs whose mailbox took something.
	 */
	async sendAgain(match?: { chainId: number; safe: string; hash: string }): Promise<string[]> {
		const sent: string[] = [];
		const prefix = match
			? `${match.chainId}:${match.safe.toLowerCase()}:${match.hash.toLowerCase()}:`
			: '';
		for (const [key, req] of Object.entries(readOutbox())) {
			if (!key.toLowerCase().startsWith(prefix)) continue;
			clearTimeout(this.#timers.get(key));
			this.#since.delete(key);
			this.#tries.delete(key);
			const answer = await this.#attempt(key, req);
			if (answer?.ok) sent.push(req.safe);
		}
		return sent;
	}
}

export const outbox = new Outbox();

/** Hand one owner's signed approval to the mailbox ({@link Outbox.send}). */
function submitKept(req: Submission) {
	return outbox.send(req);
}

/**
 * Send again what a page signed and never saw reach the mailbox (ms-19).
 * Quiet: a submission that still cannot go is kept, and sent again on its
 * own for a while. Resolves the multisigs whose mailbox took something.
 */
export function flushOutbox(): Promise<string[]> {
	return outbox.sendAgain();
}

/** Hand `tx` and one owner's approval of it to the mailbox. */
export async function submitProposal(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx,
	approval: Approval
) {
	const answer = await submitKept({
		chainId,
		safe: account.address,
		safeTx: tx,
		approval,
		genesis: namedGenesis(account.genesis)
	});
	if (answer && !answer.ok) throw new MailboxRefusedError(answer.refusal);
	multisig.useChain(account.address, chainId);
	// `null`: signed and kept, on its way to the mailbox (busy, or away) —
	// shown from what was signed until the mailbox has it.
	return {
		safeTxHash: safeTxHash(chainId, account.address, tx),
		proposal: answer?.proposal ?? null
	};
}

/**
 * An approval of `item` by an owner whose wallet is elsewhere (MetaMask,
 * Rabby, a wallet app), through Vela's connect page — any of `owners` that
 * the wallet holds. Resolves once the mailbox has it.
 */
export async function approveExternally(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx,
	owners: string[]
) {
	await authorizeSigning(account, chainId, tx);
	const hash = safeTxHash(chainId, account.address, tx);
	const approval = await externalSigning.approve({
		chainId,
		safe: account.address,
		tx,
		safeTxHash: hash,
		owners
	});
	return submitProposal(account, chainId, tx, approval);
}

/**
 * The send's confirm for a multisig: these transfers, as the review the person
 * confirmed, proposed on `chainId` at the next free nonce and approved at once
 * by this device's first member (one passkey prompt). The core reads the
 * transaction back before anything is signed.
 */
export async function proposeTransfers(
	address: string,
	chainId: number,
	transfers: { to: string; token: string | null; amount: string }[],
	/** r4b-5: stops it — no passkey, nothing proposed — once the person withdraws it. */
	signal?: AbortSignal
): Promise<{ safeTxHash: string }> {
	return proposeBuilt(
		address,
		chainId,
		(nonce) =>
			transfersTx(
				transfers.map((t) => ({ token: t.token, to: t.to, amount: t.amount })),
				`0x${nonce.toString(16)}`
			),
		signal
	);
}

/**
 * Rename the multisig's record in the registry on `chainId` — a transaction
 * of the multisig's own (`rename(name)` to the registry, which only the Safe
 * itself can call), proposed at the next free nonce and approved at once like
 * a transfer; the members approve it and one executes it there (deploying the
 * multisig on that chain first if it is not yet). The core reads it back as a
 * rename before anything is signed. Resolves its safeTxHash.
 */
export async function proposeRename(
	address: string,
	chainId: number,
	name: string
): Promise<{ safeTxHash: string }> {
	return proposeBuilt(address, chainId, (nonce) => renameTx(name, `0x${nonce.toString(16)}`));
}

/**
 * A change of members or approvals on `chainId`, built by the core against
 * that network's members as the chain holds them NOW (read again here, with
 * the open proposals: `prevOwner` comes from that network's own list) and
 * proposed at the next free nonce, approved at once like a transfer. Throws
 * {@link MemberChangeRefusedError} when the core refuses it there — the
 * members moved since the person looked, say.
 */
export async function proposeMemberChange(
	address: string,
	chainId: number,
	change: MemberChangeRequest
): Promise<{ safeTxHash: string }> {
	return proposeBuilt(address, chainId, (nonce, chain) => {
		const verdict = memberChangeTx(
			address,
			memberStateOf(chain),
			change,
			`0x${nonce.toString(16)}`
		);
		if (verdict.kind === 'refused') throw new MemberChangeRefusedError(verdict.refusal);
		return verdict.tx;
	});
}

/**
 * A dApp's call (`eth_sendTransaction`'s `to`, `value`, `data`, or one of
 * `wallet_sendCalls`' calls): `value` a quantity, `data` hex.
 */
export interface DappCall {
	to: string;
	value: string;
	data: string;
}

/**
 * A dApp's calls as ONE transaction at `nonce` (the core's `callsTx`): one is
 * a plain CALL, several a MultiSendCallOnly batch, executed together or not
 * at all. No gas is ever paid from the multisig.
 */
function dappTx(calls: DappCall[], nonce: bigint): SafeTx {
	return callsTx(
		calls.map((c) => ({ to: c.to, value: c.value || '0x0', data: c.data || '0x' })),
		`0x${nonce.toString(16)}`
	);
}

/**
 * A dApp's calls as they would be proposed now, before anything is signed:
 * the transaction at the next free nonce, read by the core (`reviewTx`) as
 * every member will read it, with no approval yet — the review the person
 * confirms with "Propose and approve". Throws `MailboxUnreadableError` when
 * the list of waiting transactions cannot be read (there is no number to
 * give it).
 */
export async function draftCalls(
	address: string,
	chainId: number,
	calls: DappCall[]
): Promise<{ chain: ChainState; item: ProposalItem }> {
	const account = multisig.find(address);
	if (!account) throw new Error('not a multisig on this device');
	const read = await readChain(account, chainId).catch(() => null);
	if (!read?.mailbox) throw new MailboxUnreadableError();
	const chain: ChainState = {
		chainId,
		members: read.members,
		items: read.items,
		mailbox: true
	};
	const tx = dappTx(calls, nextNonceOf(chain));
	const now = Date.now();
	const item = readProposal(account, chain.members, {
		chainId,
		safe: account.address,
		safeTxHash: safeTxHash(chainId, account.address, tx),
		safeTx: tx,
		proposer: '',
		approvals: [],
		createdMs: now,
		updatedMs: now
	});
	if (!item) throw new Error('the transaction does not hash as itself');
	return { chain, item };
}

/**
 * A dApp's `eth_sendTransaction` or `wallet_sendCalls` from a multisig: the
 * calls, proposed as ONE transaction on `chainId` at the next free nonce and
 * approved at once, as a transfer is — only after the person confirmed its
 * review (`draftCalls`). Resolves the proposal's safeTxHash and number.
 */
export async function proposeCalls(
	address: string,
	chainId: number,
	calls: DappCall[],
	/** The member of this device picked on its review to propose it (r5reg-5). */
	as?: string | null
): Promise<{ safeTxHash: string; nonce: string }> {
	let nonce = '0x0';
	const { safeTxHash } = await proposeBuilt(
		address,
		chainId,
		(n) => {
			nonce = `0x${n.toString(16)}`;
			return dappTx(calls, n);
		},
		undefined,
		// Its review was drawn and confirmed on its draft.
		{ reviewed: true, as }
	);
	return { safeTxHash, nonce };
}

/** `tx` as the proposal it will be, at its number on `chain`, with no approval yet. */
function draftItem(account: MultisigAccount, chain: ChainState, tx: SafeTx): ProposalItem {
	const now = Date.now();
	const item = readProposal(account, chain.members, {
		chainId: chain.chainId,
		safe: account.address,
		safeTxHash: safeTxHash(chain.chainId, account.address, tx),
		safeTx: tx,
		proposer: '',
		approvals: [],
		createdMs: now,
		updatedMs: now
	});
	if (!item) throw new Error('the transaction does not hash as itself');
	return item;
}

/**
 * Propose what `build` makes at the next free nonce, read back by the core
 * first: approved by an owner on this device (its passkey), else by an
 * owner's own wallet through the connect page.
 */
async function proposeBuilt(
	address: string,
	chainId: number,
	build: (nonce: bigint, chain: ChainState) => SafeTx,
	signal?: AbortSignal,
	/** Its review was drawn and confirmed already (a dApp's calls, on their draft). */
	opts: { reviewed?: boolean; as?: string | null } = {}
): Promise<{ safeTxHash: string }> {
	const account = multisig.find(address);
	if (!account) throw new Error('not a multisig on this device');
	multisig.useChain(account.address, chainId);
	const fresh = multisig.find(account.address) ?? account;
	// Read now, not from what is shown: the nonce comes from the mailbox's
	// list as it stands, and a list that cannot be read gives none — the
	// chain's own nonce would be one an open proposal may already hold.
	const chain = await readChain(fresh, chainId).catch(() => null);
	if (!chain?.mailbox) throw new MailboxUnreadableError();
	// The owners on that chain now (a Safe's owners may have changed since its genesis).
	const owners = chain.members.owners;
	const tx = build(nextNonceOf(chain), chain);
	const review = reviewTx(chainId, account.address, tx);
	if (review.action === null) throw new Error(review.refusal ?? 'refused');
	stopIfWithdrawn(signal);
	// §3.6: the one review before the proposer's own passkey, as every member
	// gets it: drawn at its number, signed only after "Propose and approve".
	let picked = opts.as ?? null;
	if (!opts.reviewed) {
		picked = await reviewFirst(fresh.address, chainId, chain, draftItem(fresh, chain, tx));
		stopIfWithdrawn(signal);
	}
	// r5reg-5: the member of this device the person picked on the review;
	// else the one used here last, else the first — never another silently.
	const here = localOwners(owners);
	const owner = proposerOf(here, picked, fresh.address);
	// Remembered only where there is a choice to remember.
	if (owner && here.length > 1) lastActor.note(fresh.address, owner.address);
	// No member here: an outside wallet proposes it — only a plain account's
	// can; a Vela member approves in Vela, never on a wallet page (r4C-10).
	let outside: string[] = [];
	if (!owner) {
		outside = await plainAccounts(
			chainId,
			owners,
			sameAddress(proposals.followed, fresh.address) ? proposals.items : []
		);
		if (outside.length === 0) throw new NoOutsideSignerError();
	}
	const { safeTxHash } = owner
		? await propose(fresh, chainId, tx, owner, signal)
		: await approveExternally(fresh, chainId, tx, outside);
	await proposals.refresh(fresh, fresh.chainIds);
	return { safeTxHash };
}

/** `owner` approves `item` (one passkey prompt). */
export async function approve(account: MultisigAccount, item: ProposalItem, owner: LocalOwner) {
	await authorizeSigning(account, item.chainId, item.tx);
	const approval = await signApproval(owner.account, item.chainId, item.safeTxHash, {
		multisig: account.address,
		name: account.genesis.name ?? account.label ?? null,
		tx: item.tx
	});
	const answer = await submitKept({
		chainId: item.chainId,
		safe: account.address,
		safeTx: item.tx,
		approval,
		genesis: namedGenesis(account.genesis)
	});
	if (answer && !answer.ok) throw new MailboxRefusedError(answer.refusal);
	// `null`: kept, and sent again on its own (the sheet says so).
	return answer?.proposal ?? null;
}

/** What `executor` executing `item` is, for its preview and its submit alike. */
export function executionInput(
	account: Pick<MultisigAccount, 'address' | 'genesis'>,
	item: ProposalItem,
	executor: LocalOwner,
	members: ChainMembers
): ExecutionInput {
	return {
		chainId: item.chainId,
		multisig: account.address,
		genesis: account.genesis,
		members,
		tx: item.tx,
		approvals: item.approvals.filter((a) => a.counts).map((a) => a.approval),
		executor,
		action: item.review.action,
		name: account.genesis.name ?? null
	};
}

/**
 * `executor` executes `item` (one passkey prompt); resolves once the relay
 * has it. `preview` is the one shown: its calls and fee are what is signed.
 */
export async function execute(
	account: MultisigAccount,
	item: ProposalItem,
	executor: LocalOwner,
	members: ChainMembers,
	preview?: ExecutionPreview
) {
	// §3.5: the signing gate before the executor's passkey, as every signature.
	// A look that already says it stops before the passkey (it would fail, it
	// has no price, it could not be checked in time, the executor cannot pay)
	// stops here, before the gate asks anything — the approval a caller may
	// give instead goes through the gate once, as its own signature.
	if (preview) throwIfStopped(preview);
	// §3.5, §3.6: the signing gate before the executor's passkey, as every
	// signature — the core's rule over the chain read now.
	await authorizeSigning(account, item.chainId, item.tx);
	// r7-3: one signature, the executor's own operation — on its page when it
	// signs on one (`signChallenge` hands the page the operation). It counts
	// as its approval as `msg.sender` where one is missing; no approval of
	// its own is asked first, which would be a second trip to the page for
	// nothing the chain needs.
	return submitExecution(executionInput(account, item, executor, members), preview);
}

/** A voiding transaction at `item`'s nonce, approved at once by `owner`. */
export async function proposeCancel(
	account: MultisigAccount,
	item: ProposalItem,
	owner: LocalOwner
) {
	return propose(
		account,
		item.chainId,
		cancelTx(account.address, `0x${item.nonce.toString(16)}`),
		owner
	);
}

/**
 * The mailbox's list of waiting transactions (or the chain's members) could
 * not be read, so nothing was proposed: there is no nonce to take that is
 * known to be free.
 */
export class MailboxUnreadableError extends Error {
	constructor() {
		super("the multisig's waiting transactions could not be read");
		this.name = 'MailboxUnreadableError';
	}
}

/**
 * No member is on this device, and none can sign through an outside wallet:
 * every member is a Vela account, which approves in Vela on its own device.
 * Nothing was proposed.
 */
export class NoOutsideSignerError extends Error {
	constructor() {
		super('no member can sign through an outside wallet');
		this.name = 'NoOutsideSignerError';
	}
}

/** The mailbox refused a submission (the core's reason, for the log). */
export class MailboxRefusedError extends Error {
	constructor(readonly refusal: unknown) {
		super(`the mailbox refused it: ${JSON.stringify(refusal)}`);
		this.name = 'MailboxRefusedError';
	}
}

// Kept approvals that reach the mailbox: the shown multisig's list is read again.
outbox.onsent = (safe) => {
	const shown = multisig.find(safe);
	if (shown) void proposals.refresh(shown, shown.chainIds);
};
