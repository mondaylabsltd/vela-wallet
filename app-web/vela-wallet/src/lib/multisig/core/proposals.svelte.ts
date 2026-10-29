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
	callsTx,
	cancelTx,
	reviewTx,
	transfersTx,
	safeTxHash,
	isEcdsaApproval,
	sameAddress,
	verifyApproval,
	verifyEcdsaApproval,
	type Approval,
	type Review,
	type SafeTx
} from './kernel';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { localOwners, signApproval, type LocalOwner } from './owners';
import {
	fetchProposals,
	genesisServiceURL,
	probeService,
	submitApproval,
	type Proposal
} from './service';
import { serviceOutage } from './service-state.svelte';
import { multisig, type MultisigAccount } from './store.svelte';
import { recordRemoteExecutions } from './records';
import { networkRank } from '$lib/services/networks';
import { externalSigning } from '../external/signing.svelte';

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
	approval: Approval;
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
			valid = isEcdsaApproval(a)
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
	const canExecute =
		status === 'done' || status === 'queued' || review.action === null
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
		createdMs: p.createdMs
	};
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
	{ background = false, mailbox: ask = true }: { background?: boolean; mailbox?: boolean } = {}
): Promise<ChainState & { proposals: Proposal[] }> {
	const [members, answer] = await Promise.all([
		readMembers(chainId, account.address, account.genesis),
		// `false`: unread (a `null` is the mailbox knowing nothing of it).
		ask
			? fetchProposals(chainId, account.address, { background }).catch(() => false as const)
			: (false as const)
	]);
	const mailbox = answer !== false;
	const list = answer === false ? (previous ?? []) : (answer?.proposals ?? []);
	const items = list
		.map((p) => readProposal(account, members, p))
		.filter((i): i is ProposalItem => i !== null);
	markCancellations(account.address, items);
	return { chainId, members, items, mailbox, proposals: list };
}

/** The nonce a new proposal on `chain` takes: after every open one. */
export function nextNonceOf(chain: ChainState): bigint {
	let n = chain.members.nonce;
	for (const i of chain.items) if (i.status !== 'done' && i.nonce >= n) n = i.nonce + 1n;
	return n;
}

/** A refresh still out after this long: the person is told the list may be stale. */
export const SLOW_REFRESH_MS = 8_000;

class Proposals {
	/** Per chain the multisig is used on. */
	chains = $state<ChainState[]>([]);
	loading = $state(false);
	/**
	 * The last refresh failed (offline, the service down) or is taking long:
	 * the list may be stale, and whoever shows it says so.
	 */
	stale = $state(false);
	/** New activity was written: whoever shows it refreshes. */
	onrecorded: (() => void) | null = null;
	#address: string | null = null;
	#timer: ReturnType<typeof setInterval> | null = null;
	#inflight: Promise<void> | null = null;
	/** The last list the mailbox gave, per chain: kept while it cannot be read. */
	#lists = new Map<number, Proposal[]>();
	/** The service those lists came from: another one's say nothing of it. */
	#source: string | null = null;
	#chainIds: number[] = [];
	#account: MultisigAccount | null = null;
	#online: (() => void) | null = null;

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
		this.#online = null;
		if (!sameAddress(this.#address, address)) {
			this.chains = [];
			this.#lists.clear();
			this.stale = false;
		}
		this.#address = address;
		this.#account = account;
		this.#chainIds = chainIds;
		if (!account) return;
		void this.refresh(account, chainIds);
		// Back online: the outage is over as far as anyone knows — read now.
		if (typeof window !== 'undefined') {
			this.#online = () => this.retry();
			window.addEventListener('online', this.#online);
		}
		this.#timer = setInterval(() => {
			if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return;
			void this.refresh(account, chainIds);
		}, 12_000);
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
		const account = this.#account;
		return account ? this.refresh(account, this.#chainIds) : Promise.resolve();
	}

	/** Re-read every chain the multisig is used on. One refresh at a time. */
	refresh(account: MultisigAccount, chainIds: number[]): Promise<void> {
		if (this.#inflight) return this.#inflight;
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
			// While the service is away: nothing until its next step, then one
			// probe for every network — not a failed read per network per round.
			const mailbox = await serviceOutage.available(probeService);
			await Promise.all(
				chainIds.map(async (chainId) => {
					const known = followed() ? (this.#lists.get(chainId) ?? null) : null;
					try {
						const read = await readChain(account, chainId, known, {
							background: true,
							mailbox
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
						failed = true;
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
		return c ? nextNonceOf(c) : 0n;
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
	return submitProposal(account, chainId, tx, approval);
}

/**
 * Signed approvals not yet in the mailbox (ms-19): written before the POST,
 * removed once the mailbox answered (took it or refused it), and sent again
 * when a page comes up — so a reload or a closed window between the passkey
 * and the mailbox never loses what the person signed. The mailbox merges, so
 * sending one twice is harmless.
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

/** Hand one owner's signed approval to the mailbox, kept in the outbox until it answers. */
async function submitKept(req: Submission) {
	const key = outboxKey(req);
	writeOutbox({ ...readOutbox(), [key]: req });
	const answer = await submitApproval(req);
	const { [key]: _sent, ...rest } = readOutbox();
	void _sent;
	writeOutbox(rest);
	return answer;
}

/**
 * Send again what a page signed and never saw reach the mailbox (ms-19).
 * Quiet: a submission that still cannot go stays for the next page.
 * Resolves the multisigs whose mailbox took something.
 */
export async function flushOutbox(): Promise<string[]> {
	const sent: string[] = [];
	for (const [key, req] of Object.entries(readOutbox())) {
		try {
			const answer = await submitApproval(req);
			const { [key]: _done, ...rest } = readOutbox();
			void _done;
			writeOutbox(rest);
			if (answer.ok) sent.push(req.safe);
		} catch {
			// the mailbox is still out of reach: kept for the next page
		}
	}
	return sent;
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
		genesis: account.genesis
	});
	if (!answer.ok) throw new MailboxRefusedError(answer.refusal);
	multisig.useChain(account.address, chainId);
	return { safeTxHash: safeTxHash(chainId, account.address, tx), proposal: answer.proposal };
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
	transfers: { to: string; token: string | null; amount: string }[]
): Promise<{ safeTxHash: string }> {
	return proposeBuilt(address, chainId, (nonce) =>
		transfersTx(
			transfers.map((t) => ({ token: t.token, to: t.to, amount: t.amount })),
			`0x${nonce.toString(16)}`
		)
	);
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
	calls: DappCall[]
): Promise<{ safeTxHash: string; nonce: string }> {
	let nonce = '0x0';
	const { safeTxHash } = await proposeBuilt(address, chainId, (n) => {
		nonce = `0x${n.toString(16)}`;
		return dappTx(calls, n);
	});
	return { safeTxHash, nonce };
}

/**
 * Propose what `build` makes at the next free nonce, read back by the core
 * first: approved by an owner on this device (its passkey), else by an
 * owner's own wallet through the connect page.
 */
async function proposeBuilt(
	address: string,
	chainId: number,
	build: (nonce: bigint) => SafeTx
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
	const owner = localOwners(owners)[0];
	const tx = build(nextNonceOf(chain));
	const review = reviewTx(chainId, account.address, tx);
	if (review.action === null) throw new Error(review.refusal ?? 'refused');
	const { safeTxHash } = owner
		? await propose(fresh, chainId, tx, owner)
		: await approveExternally(fresh, chainId, tx, owners);
	await proposals.refresh(fresh, fresh.chainIds);
	return { safeTxHash };
}

/** `owner` approves `item` (one passkey prompt). */
export async function approve(account: MultisigAccount, item: ProposalItem, owner: LocalOwner) {
	const approval = await signApproval(owner.account, item.chainId, item.safeTxHash);
	const answer = await submitKept({
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

/** The mailbox refused a submission (the core's reason, for the log). */
export class MailboxRefusedError extends Error {
	constructor(readonly refusal: unknown) {
		super(`the mailbox refused it: ${JSON.stringify(refusal)}`);
		this.name = 'MailboxRefusedError';
	}
}
