/**
 * The active multisig's transactions in flight: what the mailbox holds,
 * re-checked here against the chain, and what this device can do about each.
 *
 * Nothing is taken from the mailbox on trust. For every proposal the core
 * re-hashes the transaction, re-reads what it does (`reviewTx`) and verifies
 * every approval (`verifyApproval`); an approval counts only if it verifies
 * AND its owner is an owner on that chain now. The members and nonce come from
 * the chain through the wallet's own RPC (`readMembers`).
 *
 * The four actions — propose, approve, execute, cancel — are each one passkey
 * prompt, and each is safe to retry: the mailbox merges, and an execution that
 * may have been sent is followed rather than sent again.
 */
import { hasCode, readMembers, type ChainMembers } from './chain';
import { submitExecution } from './execute';
import {
	cancelTx,
	reviewTx,
	transfersTx,
	safeTxHash,
	sameAddress,
	verifyApproval,
	type PasskeyApproval,
	type Review,
	type SafeTx
} from './kernel';
import { localOwners, signApproval, type LocalOwner } from './owners';
import { fetchProposals, submitApproval, type Proposal } from './service';
import { multisig, type MultisigAccount } from './store.svelte';
import { recordRemoteExecutions } from './records';

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
	/** Verified, and its owner is an owner on this chain now. */
	counts: boolean;
	approval: PasskeyApproval;
	createdMs: number;
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
	createdMs: number;
}

export interface ChainState {
	chainId: number;
	members: ChainMembers;
	items: ProposalItem[];
}

function sortKey(a: ProposalItem, b: ProposalItem): number {
	if (a.nonce !== b.nonce) return a.nonce < b.nonce ? -1 : 1;
	return a.createdMs - b.createdMs;
}

/** What one stored proposal is, as this device reads it. */
export function readProposal(
	account: MultisigAccount,
	members: ChainMembers,
	p: Proposal
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
			valid = verifyApproval(p.chainId, hash, a);
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
	// The executor is `msg.sender`: one approval more if it has not approved.
	const canExecute =
		status === 'done' || status === 'queued' || review.action === null
			? []
			: local.filter((o) => {
					const own = approvals.some((a) => a.counts && sameAddress(a.owner, o.address));
					return approved + (own ? 0 : 1) >= members.threshold;
				});
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
		createdMs: p.createdMs
	};
}

class Proposals {
	/** Per chain the multisig is used on. */
	chains = $state<ChainState[]>([]);
	loading = $state(false);
	/** The last refresh failed (offline, the service down): the list may be stale. */
	stale = $state(false);
	/** New activity was written: whoever shows it refreshes. */
	onrecorded: (() => void) | null = null;
	#address: string | null = null;
	#timer: ReturnType<typeof setInterval> | null = null;
	#inflight: Promise<void> | null = null;

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
		if (!sameAddress(this.#address, address)) this.chains = [];
		this.#address = address;
		if (!account) return;
		void this.refresh(account, chainIds);
		this.#timer = setInterval(() => {
			if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return;
			void this.refresh(account, chainIds);
		}, 12_000);
	}

	/** Re-read every chain the multisig is used on. One refresh at a time. */
	refresh(account: MultisigAccount, chainIds: number[]): Promise<void> {
		if (this.#inflight) return this.#inflight;
		this.loading = this.chains.length === 0;
		const run = (async () => {
			let failed = false;
			const next: ChainState[] = [];
			await Promise.all(
				chainIds.map(async (chainId) => {
					try {
						const [members, answer] = await Promise.all([
							readMembers(chainId, account.address, account.genesis),
							fetchProposals(chainId, account.address).catch(() => {
								failed = true;
								return null;
							})
						]);
						const items = (answer?.proposals ?? [])
							.map((p) => readProposal(account, members, p))
							.filter((i): i is ProposalItem => i !== null);
						markCancellations(account.address, items);
						next.push({ chainId, members, items });
					} catch {
						failed = true;
						const previous = this.chains.find((c) => c.chainId === chainId);
						if (previous) next.push(previous);
					}
				})
			);
			if (sameAddress(this.#address, account.address)) {
				this.chains = next.sort((a, b) => a.chainId - b.chainId);
				this.stale = failed;
				// Executed on another member's device: into this device's activity too.
				void recordRemoteExecutions(account.address, this.items).then((wrote) => {
					if (wrote) this.onrecorded?.();
				});
			}
			this.loading = false;
		})();
		this.#inflight = run.finally(() => (this.#inflight = null));
		return this.#inflight;
	}

	chain(chainId: number): ChainState | undefined {
		return this.chains.find((c) => c.chainId === chainId);
	}

	/** The nonce a new proposal on `chainId` takes: after every open one. */
	nextNonce(chainId: number): bigint {
		const c = this.chain(chainId);
		if (!c) return 0n;
		let n = c.members.nonce;
		for (const i of c.items) if (i.status !== 'done' && i.nonce >= n) n = i.nonce + 1n;
		return n;
	}
}

/** A proposal at the same nonce that sends nothing to the multisig itself voids the others. */
function markCancellations(multisigAddress: string, items: ProposalItem[]): void {
	for (const c of items) {
		if (c.review.action?.kind !== 'cancel') continue;
		for (const i of items) {
			if (i !== c && i.nonce === c.nonce) i.cancelledBy = c.safeTxHash;
		}
	}
	void multisigAddress;
}

export const proposals = new Proposals();

// -- actions --------------------------------------------------------------------------

/** Propose `tx` on `chainId`, approved at once by `owner` (one passkey prompt). */
export async function propose(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx,
	owner: LocalOwner
) {
	// A delegatecall into nothing "succeeds" doing nothing and spends the
	// nonce: the batch contract (MultiSendCallOnly) must be on this chain, which
	// a network the wallet accepts for Vela accounts does not promise.
	if (Number(tx.operation) === 1 && !(await hasCode(chainId, tx.to))) {
		throw new Error(`${tx.to} has no code on chain ${chainId}`);
	}
	const hash = safeTxHash(chainId, account.address, tx);
	const approval = await signApproval(owner.account, chainId, hash);
	const answer = await submitApproval({
		chainId,
		safe: account.address,
		safeTx: tx,
		approval,
		genesis: account.genesis
	});
	if (!answer.ok) throw new MailboxRefusedError(answer.refusal);
	multisig.useChain(account.address, chainId);
	return { safeTxHash: hash, proposal: answer.proposal };
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
	transfers: { to: string; token: string | null; amount: string }[]
): Promise<{ safeTxHash: string }> {
	const account = multisig.find(address);
	if (!account) throw new Error('not a multisig on this device');
	const owner = localOwners(account.owners)[0];
	if (!owner) throw new Error('no member of this multisig on this device');
	multisig.useChain(account.address, chainId);
	const fresh = multisig.find(account.address) ?? account;
	await proposals.refresh(fresh, fresh.chainIds);
	const nonce = proposals.nextNonce(chainId);
	const tx = transfersTx(
		transfers.map((t) => ({ token: t.token, to: t.to, amount: t.amount })),
		`0x${nonce.toString(16)}`
	);
	const review = reviewTx(chainId, account.address, tx);
	if (review.action === null) throw new Error(review.refusal ?? 'refused');
	const { safeTxHash } = await propose(fresh, chainId, tx, owner);
	await proposals.refresh(fresh, fresh.chainIds);
	return { safeTxHash };
}

/** `owner` approves `item` (one passkey prompt). */
export async function approve(account: MultisigAccount, item: ProposalItem, owner: LocalOwner) {
	const approval = await signApproval(owner.account, item.chainId, item.safeTxHash);
	const answer = await submitApproval({
		chainId: item.chainId,
		safe: account.address,
		safeTx: item.tx,
		approval,
		genesis: account.genesis
	});
	if (!answer.ok) throw new MailboxRefusedError(answer.refusal);
	return answer.proposal;
}

/** `executor` executes `item` (one passkey prompt); resolves with the transaction hash. */
export async function execute(
	account: MultisigAccount,
	item: ProposalItem,
	executor: LocalOwner,
	members: ChainMembers
) {
	const result = await submitExecution({
		chainId: item.chainId,
		multisig: account.address,
		genesis: account.genesis,
		members,
		tx: item.tx,
		approvals: item.approvals.filter((a) => a.counts).map((a) => a.approval),
		executor
	});
	return result;
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

/** The mailbox refused a submission (the core's reason, for the log). */
export class MailboxRefusedError extends Error {
	constructor(readonly refusal: unknown) {
		super(`the mailbox refused it: ${JSON.stringify(refusal)}`);
		this.name = 'MailboxRefusedError';
	}
}
