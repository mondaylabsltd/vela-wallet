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
 * A link's transaction on a device with none of its members (ms-21) is the
 * machine's too (`linked`): {@link linkedSource} is it, in the same shapes.
 */
import { untrack } from 'svelte';
import type { ChainMembers } from './chain';
import { sameAddress, type Approval, type Review, type SafeTx } from './kernel';
import { localOwners, type LocalOwner } from './owners';
import { multisig, type MultisigAccount } from './store.svelte';
import {
	MailboxRefusedError,
	MailboxUnreadableError,
	ProposalUnreadError,
	NoOutsideSignerError
} from './proposal-errors';
import { proposalsHost, ProposeFailedError } from '../shell/proposals-host.svelte';
import { actionError } from '../shell/proposal-failure';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { MultisigProposalItem } from '../generated/MultisigProposalItem';
import type { MultisigChainState } from '../generated/MultisigChainState';
import type { MultisigLinkedView } from '../generated/MultisigLinkedView';
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';

export { MailboxRefusedError, MailboxUnreadableError, NoOutsideSignerError, ProposalUnreadError };

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
	/**
	 * r11 S4: every number a queued one waits behind (the core's rule,
	 * `ProposalItem.waits_for`) — the same numbers on every surface.
	 */
	waitsFor: bigint[];
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

/** A refresh still out after this long: the person is told the list may be stale (the core's). */
const SLOW_REFRESH_MS = 8_000;
/** The poll's pace before the machine has said its own (`pace_ms`). */
const POLL_MS = 12_000;

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
		renameUndeployed: i.renameUndeployed,
		waitsFor: (i.waitsFor ?? []).map((n) => BigInt(n))
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

/**
 * Where a proposal's sheet reads its transaction from: the shown multisig's
 * (`proposals`), or a link's read only ({@link linkedSource}).
 */
export interface ProposalSource {
	loading: boolean;
	chain(chainId: number): ChainState | undefined;
	/** r4b-3: networks whose members could not be read on the last refresh. */
	unreadable: number[];
	/** Read now (the person's Retry). */
	refresh(...args: unknown[]): Promise<void>;
}

/**
 * ms-21: a link's transaction on a device with none of its members, as the
 * machine reads it (`linked`): its network's members and the one
 * transaction, re-hashed to the link's — or, not there yet, whether the
 * mailbox was read (`notInMailbox`) or not (`notFound`).
 */
export function linkedSource(linked: MultisigLinkedView | null): ProposalSource {
	const mailbox = linked?.stand !== 'notFound' && linked?.stand !== 'unreadable';
	return {
		loading: !linked || linked.stand === 'reading',
		chain: (chainId) =>
			linked && linked.chain_id === chainId && linked.members
				? {
						chainId,
						members: { ...linked.members, nonce: BigInt(linked.members.nonce) },
						items: linked.item ? [itemOf(linked.item)] : [],
						mailbox
					}
				: undefined,
		unreadable: linked?.stand === 'unreadable' ? [linked.chain_id] : [],
		refresh: () => proposals.refresh()
	};
}

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
}

export const proposals = new Proposals();

/**
 * Everyone who is a member of `account` on some network — the machine's word
 * for the multisig it follows (`members_anywhere`); one not followed has no
 * network read here, so its members are its original ones.
 */
export function membersAnywhere(account: Pick<MultisigAccount, 'address' | 'owners'>): string[] {
	if (!sameAddress(proposals.followed, account.address)) return account.owners;
	return proposals.view?.members_anywhere ?? account.owners;
}

// -- actions --------------------------------------------------------------------------

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
