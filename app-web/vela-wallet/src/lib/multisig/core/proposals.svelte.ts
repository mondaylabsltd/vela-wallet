/**
 * The active multisig's transactions in flight — what the mailbox holds,
 * re-checked against the chain, and what this device can do about each — as
 * the core's proposals machine reads them (`multisig_proposals`, through
 * `shell/proposals-host.svelte.ts`). This is its facade: the shapes the
 * screens draw (`ProposalItem`, `ChainState`), the poll at the machine's
 * pace, and the proposals the send flow, Settings and the members surface
 * make (`draft`, its review, `confirm_draft`). Nothing here reads, verifies
 * or decides: the machine re-hashes every proposal, verifies every approval
 * against the members on that chain now, keeps the outbox and takes the
 * nonce from the mailbox's list read now.
 *
 * Still here, for a link's transaction read on a device with none of its
 * members (`core/linked.svelte.ts`, moved onto the machine next): its read
 * ({@link readChain}) and an outside approval of it, and the outbox that
 * approval is submitted through.
 */
import { untrack } from 'svelte';
import { readMembers, type ChainMembers } from './chain';
import {
	reviewTx,
	safeTxHash,
	isEcdsaApproval,
	isOnChainApproval,
	namedGenesis,
	sameAddress,
	verifyApproval,
	verifyEcdsaApproval,
	type Approval,
	type MemberChangeRequest,
	type Review,
	type SafeTx
} from './kernel';
import type { ExecutionInput } from './execute';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { localOwners, type LocalOwner } from './owners';
import {
	fetchProposals,
	ServiceBusyError,
	ServiceUnavailableError,
	submitApproval,
	type Proposal,
	type SubmitAnswer
} from './service';
import { multisig, type MultisigAccount } from './store.svelte';
import { networkForChainId } from '$lib/services/networks';
import { externalSigning } from '../external/signing.svelte';
import { authorizeSigning } from './gate.svelte';
import { onChainApprovals } from '../external/onchain';
import {
	MailboxRefusedError,
	MailboxUnreadableError,
	NoOutsideSignerError
} from './proposal-errors';
import { proposalsHost, ProposeFailedError } from '../shell/proposals-host.svelte';
import { actionError, memberRefusal } from '../shell/proposal-failure';
import { MemberChangeRefusedError } from './member-changes';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { MultisigProposalItem } from '../generated/MultisigProposalItem';
import type { MultisigChainState } from '../generated/MultisigChainState';
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';

export { MailboxRefusedError, MailboxUnreadableError, NoOutsideSignerError };

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
	// A dApp's draft and a link's preview are never this device's execution in flight.
	const executing = false;
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

/** A refresh still out after this long: the person is told the list may be stale (the core's). */
export const SLOW_REFRESH_MS = 8_000;
/** The poll's pace before the machine has said its own (`pace_ms`). */
export const POLL_MS = 12_000;

/** One proposal as the machine reads it, in the shapes the screens draw. */
export function itemOf(i: MultisigProposalItem): ProposalItem {
	return {
		chainId: i.chainId,
		safeTxHash: i.safeTxHash,
		tx: i.tx as SafeTx,
		review: i.review as Review,
		nonce: BigInt(i.nonce),
		proposer: i.proposer,
		approvals: i.approvals.map((a) => ({
			owner: a.owner,
			counts: a.counts,
			approval: a.approval as Approval,
			createdMs: a.createdMs,
			unsent: a.unsent
		})),
		approved: i.approved,
		threshold: i.threshold,
		status: i.status,
		canApprove: localOwners(i.canApprove),
		canExecute: localOwners(i.canExecute),
		cancelledBy: i.cancelledBy,
		sameNonce: i.sameNonce,
		executing: i.executing,
		createdMs: i.createdMs,
		unsent: i.unsent,
		renameUndeployed: i.renameUndeployed
	};
}

/** One network's members and proposals as the machine reads them. */
export function chainOf(c: MultisigChainState): ChainState {
	return {
		chainId: c.chainId,
		members: { ...c.members, nonce: BigInt(c.members.nonce) },
		items: c.items.map(itemOf),
		mailbox: c.mailbox
	};
}

const itemKey = (chainId: number, hash: string) => `${chainId}:${hash.toLowerCase()}`;

interface Read {
	view: MultisigProposalsView | null;
	chains: ChainState[];
	items: ProposalItem[];
}

const NOTHING: Read = { view: null, chains: [], items: [] };

/**
 * The shown multisig's list — the machine's (`follow`): read now and on
 * every tick at the machine's pace while the page is visible, at once when
 * it is shown again, back online, or a sheet on a waiting transaction opens
 * (`watch`, r5reg-14).
 */
class Proposals {
	#read: Read = NOTHING;
	#stops: (() => void)[] = [];
	#timer: ReturnType<typeof setTimeout> | null = null;

	#now(): Read {
		const view = proposalsHost.view;
		if (!view) return NOTHING;
		if (this.#read.view === view) return this.#read;
		const chains = view.chains.map(chainOf);
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for one read, never state
		const byKey = new Map<string, ProposalItem>();
		for (const c of chains) for (const i of c.items) byKey.set(itemKey(i.chainId, i.safeTxHash), i);
		const items = view.items.map((i) => byKey.get(itemKey(i.chainId, i.safeTxHash)) ?? itemOf(i));
		this.#read = { view, chains, items };
		return this.#read;
	}

	/** The machine's view itself (the sheets read its reviews, looks and actions). */
	get view(): MultisigProposalsView | null {
		return proposalsHost.view;
	}

	/** The multisig followed (`null`: none). */
	get followed(): string | null {
		return proposalsHost.view?.followed ?? null;
	}

	/** Per chain the multisig is used on, in the wallet's network order. */
	get chains(): ChainState[] {
		return this.#stub ?? this.#now().chains;
	}

	/** Tests: a list as the machine would read it (`null`: the machine's again). */
	set chains(chains: ChainState[] | null) {
		this.#stub = chains;
	}

	#stub = $state.raw<ChainState[] | null>(null);

	/** Every proposal, by number then age. */
	get items(): ProposalItem[] {
		if (this.#stub) {
			return this.#stub
				.flatMap((c) => c.items)
				.sort((a, b) =>
					a.nonce !== b.nonce ? (a.nonce < b.nonce ? -1 : 1) : a.createdMs - b.createdMs
				);
		}
		return this.#now().items;
	}

	/** Open proposals (not done), next first. */
	get open(): ProposalItem[] {
		return this.items.filter((i) => i.status !== 'done');
	}

	/** r4b-11: what is shown is the list kept from an earlier page (`at`: when it was read). */
	get cached(): { at: number } | null {
		const at = proposalsHost.view?.cached_at;
		return at === null || at === undefined ? null : { at };
	}

	/** r4b-3: networks whose members could not be read on the last refresh. */
	get unreadable(): number[] {
		return proposalsHost.view?.unreadable ?? [];
	}

	/** r4b-2: networks whose history could not be read. */
	get historyUnreadable(): number[] {
		return proposalsHost.view?.history_unreadable ?? [];
	}

	/** r4b-15: the device says it is offline. */
	get offline(): boolean {
		return proposalsHost.view?.offline ?? false;
	}

	/** When the list shown was last read in full. */
	get readAt(): number | null {
		return proposalsHost.view?.read_at ?? null;
	}

	/** The networks the mailbox holds proposals of it on that this device lacks. */
	get missing(): MissingChain[] {
		return proposalsHost.view?.missing ?? [];
	}

	get loading(): boolean {
		return proposalsHost.view?.loading ?? false;
	}

	/** The last refresh failed or is taking long: whoever shows it says so. */
	get stale(): boolean {
		return proposalsHost.view?.stale ?? false;
	}

	/** Stale because the multisig service is away: said as such, with a Retry. */
	get unreachable(): boolean {
		return proposalsHost.view?.unreachable ?? false;
	}

	/** The poll's pace now (the machine's). */
	get pace(): number {
		return proposalsHost.view?.pace_ms ?? POLL_MS;
	}

	/** New activity was written: whoever shows it refreshes. */
	get onrecorded(): (() => void) | null {
		return proposalsHost.onrecorded;
	}

	set onrecorded(fn: (() => void) | null) {
		proposalsHost.onrecorded = fn;
	}

	/**
	 * Follow `account` (or stop, `null`): read now, then on every tick while
	 * it is shown. Called from a screen's effect: what it reads of the
	 * machine's view is never what that effect depends on (`untrack`).
	 */
	follow(account: MultisigAccount | null, chainIds: number[] = account?.chainIds ?? []): void {
		void chainIds;
		untrack(() => this.#follow(account));
	}

	#follow(account: MultisigAccount | null): void {
		for (const stop of this.#stops.splice(0)) stop();
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = null;
		void proposalsHost.follow(account);
		if (!account || typeof window === 'undefined') return;
		if (navigator.onLine === false) {
			void proposalsHost.boot().then(() => proposalsHost.send({ type: 'online', online: false }));
		}
		// Back online: the outage is over as far as anyone knows — read now.
		const online = () => proposalsHost.send({ type: 'online', online: true });
		const offline = () => proposalsHost.send({ type: 'online', online: false });
		// r4b-16: back to the tab — read now, not at the next tick.
		const visible = () => {
			if (document.visibilityState === 'visible') {
				proposalsHost.send({ type: 'refresh', fresh: false });
			}
		};
		window.addEventListener('online', online);
		window.addEventListener('offline', offline);
		document.addEventListener('visibilitychange', visible);
		this.#stops.push(
			() => window.removeEventListener('online', online),
			() => window.removeEventListener('offline', offline),
			() => document.removeEventListener('visibilitychange', visible)
		);
		// The poll, at the pace the machine says now (faster while a sheet watches).
		const round = () => {
			this.#timer = setTimeout(() => {
				if (document.visibilityState === 'visible') proposalsHost.send({ type: 'tick' });
				round();
			}, this.pace);
		};
		round();
		this.#stops.push(() => {
			if (this.#timer) clearTimeout(this.#timer);
			this.#timer = null;
		});
	}

	/**
	 * r5reg-14: keep it fresh while a sheet on one of its waiting
	 * transactions is open — the machine's faster pace, a read at once when
	 * the list is older than that, and one whenever the window gets the focus
	 * back. The returned function lets go.
	 */
	watch(): () => void {
		return untrack(() => this.#watch());
	}

	#watch(): () => void {
		proposalsHost.send({ type: 'watch' });
		// The pace changed: the next round is at the new one.
		this.#repace();
		const focus = () => proposalsHost.send({ type: 'refresh', fresh: false });
		if (typeof window !== 'undefined') window.addEventListener('focus', focus);
		let released = false;
		return () => {
			if (released) return;
			released = true;
			if (typeof window !== 'undefined') window.removeEventListener('focus', focus);
			untrack(() => {
				proposalsHost.send({ type: 'unwatch' });
				this.#repace();
			});
		};
	}

	#repace(): void {
		if (!this.#timer) return;
		clearTimeout(this.#timer);
		const round = () => {
			this.#timer = setTimeout(() => {
				if (typeof document === 'undefined' || document.visibilityState === 'visible') {
					proposalsHost.send({ type: 'tick' });
				}
				round();
			}, this.pace);
		};
		round();
	}

	/** The person's Retry (or `online`): ask the service now, whatever its schedule. */
	retry(): Promise<void> {
		return untrack(() => {
			const before = this.readAt;
			proposalsHost.send({ type: 'retry' });
			return this.#read_settled(before);
		});
	}

	/**
	 * Read every chain the multisig is used on again. `fresh`: a read begun
	 * after this call (one under way is waited out, then another made).
	 * Resolves once a read ended (or {@link SLOW_REFRESH_MS} twice over).
	 */
	refresh(
		account?: MultisigAccount | null,
		chainIds?: number[],
		{ fresh = false }: { tick?: boolean; fresh?: boolean } = {}
	): Promise<void> {
		void account;
		void chainIds;
		return untrack(() => {
			const before = this.readAt;
			proposalsHost.send({ type: 'refresh', fresh });
			return this.#read_settled(before);
		});
	}

	async #read_settled(before: number | null): Promise<void> {
		await proposalsHost.until(
			(view) => (!view.loading && (view.read_at !== before || view.stale) ? true : null),
			SLOW_REFRESH_MS * 2
		);
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

export const proposals = new Proposals();

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
 * Propose `proposal` on `chainId` through the machine — drawn at the next
 * free number with its review, shown first (§3.6), signed once the person
 * confirms it — and say why not in the typed errors the screens word.
 */
async function proposeThrough(
	address: string,
	chainId: number,
	proposal: MultisigProposalKind,
	signal?: AbortSignal
): Promise<{ safeTxHash: string }> {
	const account = multisig.find(address);
	if (!account) throw new Error('not a multisig on this device');
	try {
		return await proposalsHost.propose(account, chainId, proposal, { signal });
	} catch (error) {
		if (error instanceof ProposeFailedError) {
			// A member change the core refuses there (the members moved since the person looked).
			const refusal = error.state.kind === 'refused' ? memberRefusal(error.state.reason) : null;
			if (refusal) throw new MemberChangeRefusedError(refusal as never);
			throw actionError(error.state, chainId) ?? new Error(error.state.kind);
		}
		throw error;
	}
}

/**
 * The send's confirm for a multisig: these transfers, proposed on `chainId`
 * at the next free nonce — its review first, then one passkey.
 */
export function proposeTransfers(
	address: string,
	chainId: number,
	transfers: { to: string; token: string | null; amount: string }[],
	/** r4b-5: stops it — no passkey, nothing proposed — once the person withdraws it. */
	signal?: AbortSignal
): Promise<{ safeTxHash: string }> {
	return proposeThrough(
		address,
		chainId,
		{
			kind: 'transfers',
			transfers: transfers.map((t) => ({ token: t.token, to: t.to, amount: t.amount }))
		},
		signal
	);
}

/**
 * Rename the multisig's record in the registry on `chainId` — a transaction
 * of the multisig's own, proposed at the next free nonce like a transfer.
 */
export function proposeRename(
	address: string,
	chainId: number,
	name: string
): Promise<{ safeTxHash: string }> {
	return proposeThrough(address, chainId, { kind: 'rename', name });
}

/**
 * A change of members or approvals on `chainId`, built by the core against
 * that network's members as the chain holds them NOW; throws
 * `MemberChangeRefusedError` when the core refuses it there.
 */
export function proposeMemberChange(
	address: string,
	chainId: number,
	change: MemberChangeRequest
): Promise<{ safeTxHash: string }> {
	return proposeThrough(address, chainId, { kind: 'member_change', change: change as never });
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
