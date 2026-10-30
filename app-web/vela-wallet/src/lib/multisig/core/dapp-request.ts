/**
 * A dApp's request to a multisig, from its arrival to its answer — the part
 * of `DappRequestHost` that is the multisig's.
 *
 * The order is the whole point (docs/multisig-vault-design.md §6): the
 * person sees what the site asks — the core's reading of the transaction,
 * at the number it would take — and only their "Propose and approve" signs
 * anything. Every way out answers the site, exactly once: declined before it
 * was proposed is the person's 4001; left after it was proposed says so, and
 * the proposal stays in the mailbox (and in Activity) for the members.
 *
 * An `eth_sendTransaction` is answered with the transaction that executed
 * it — never with the safeTxHash, which is no transaction hash. Approved
 * enough by the proposer alone, it is executed there and then. Otherwise the
 * site waits while the members approve, but never past the extension's
 * request limit: before it, the site is told — a JSON-RPC error, not a
 * rejection — that it was proposed and is waiting (`leftProposed`), and the
 * proposal goes on in the wallet as any other.
 *
 * A `wallet_sendCalls` (EIP-5792) is the same review of every call, and ONE
 * proposal: the calls in one MultiSendCallOnly transaction, approved once by
 * the members and executed whole or not at all. Its site is answered the
 * batch's id — the proposal's safeTxHash — as soon as it is proposed, and
 * follows it with `wallet_getCallsStatus` (pending while the members
 * approve, then the execution's receipt).
 */
import type { DappCall } from './proposals.svelte';
import type { DappProposal } from './ui.svelte';

export interface DappError {
	code: number;
	message: string;
	data?: unknown;
}

export interface DappAnswer {
	result?: unknown;
	error?: DappError;
}

/** The person declined it before anything was proposed. */
export const DECLINED: DappError = { code: 4001, message: 'User rejected the request' };

/**
 * What a multisig answers a method it does not take; `null` for a
 * transaction (`eth_sendTransaction`) or a batch of them (`wallet_sendCalls`),
 * which become a proposal. A message is refused: a multisig's signature is
 * every approving member's, which Vela does not collect for messages yet
 * (docs/multisig-vault-design.md §6) — the window says so before it answers.
 */
export function multisigRefusal(method: string): DappError | null {
	if (method === 'eth_sendTransaction' || method === 'wallet_sendCalls') return null;
	return { code: 4200, message: 'A multisig account signs no messages yet' };
}

/** The JSON-RPC server-error class (-32000): the request was taken, its outcome is elsewhere. */
export const PROPOSED_CODE = -32000;

/**
 * How long before the request's limit (the extension's, counted from the
 * claim before the passkey) a proposed transaction is answered "proposed":
 * room for the answer to reach the page before the page gives up on it.
 */
export const ANSWER_MARGIN_MS = 20_000;

/**
 * The answer for a transaction that is proposed but not executed — the
 * person left, or the request's limit is near: not a rejection (it was not
 * rejected, and the members may still execute it, 4001 would make a site
 * offer it again), not a disconnect (4900), and not a hash (none exists
 * yet). The site is told what happened, and `data` carries the proposal, so
 * nobody proposes it twice.
 */
export function leftProposed(safeTxHash: string): DappError {
	return {
		code: PROPOSED_CODE,
		message:
			`Proposed to the multisig's members (safeTxHash ${safeTxHash}), waiting for their ` +
			'approvals. This site is not told when it is executed.',
		data: { safeTxHash, status: 'proposed' }
	};
}

export interface HandOff {
	rid: string;
	/** `eth_sendTransaction` (answered with the execution's hash) or `wallet_sendCalls` (its id). */
	method: string;
	address: string;
	chainId: number;
	/** What the core read the request as (`dappMultisigCalls`): one call, or a batch's every call. */
	calls: DappCall[];
	origin: string;
	/** Draw the review (the multisig's sheet). */
	open(request: DappProposal): void;
	/** It is in the mailbox now: the sheet shows it as one. */
	proposed(rid: string, safeTxHash: string): void;
	/** Take the sheet down. */
	closeSheet(rid: string): void;
	/** RB5: is the request still live? Asked before the passkey. */
	claim(): Promise<boolean>;
	/** The page left: the request is withdrawn, nothing answered. */
	gone(): void;
	/** Propose it (the one passkey prompt). */
	propose(
		address: string,
		chainId: number,
		calls: DappCall[],
		/** The member of this device picked to propose it (r5reg-5). */
		as?: string | null
	): Promise<{ safeTxHash: string; nonce: string; fromBlock?: string }>;
	/** Answer the site (the caller answers once). */
	settle(answer: DappAnswer): void;
	/** `eth_sendTransaction`: follow the proposal until it is executed or voided. */
	follow(safeTxHash: string): void;
	/**
	 * `eth_sendTransaction`, its limit near: the site is told it was proposed
	 * (`leftProposed`); the sheet stays, saying so.
	 */
	tellProposed(safeTxHash: string): void;
	/**
	 * How long, from the claim before the passkey, the request can still be
	 * answered — the extension's limit (`signRequestTtlMs`). `undefined`: none.
	 */
	limitMs?: number;
	/** The clock (tests). */
	now?: () => number;
	/**
	 * `wallet_sendCalls`: answer the site the batch's id now — the proposal at
	 * `nonce`, proposed around block `fromBlock` (when it was read).
	 */
	answerBatch(safeTxHash: string, nonce: string, fromBlock?: string): void;
}

/** Thrown by `propose` when the page left before the passkey: nothing was signed. */
export class DappGoneError extends Error {
	constructor() {
		super('the page left before the transaction was proposed');
		this.name = 'DappGoneError';
	}
}

/**
 * Open the review of a dApp's transaction from a multisig. Nothing is signed
 * here: `propose` runs only when the person presses "Propose and approve".
 * Returns what the site is owed if the surface goes away now.
 */
export function openMultisigReview(h: HandOff): { closing(): DappAnswer } {
	const now = h.now ?? Date.now;
	let proposed: string | null = null;
	let proposing = false;
	let limit: ReturnType<typeof setTimeout> | null = null;
	const stopLimit = () => {
		if (limit !== null) clearTimeout(limit);
		limit = null;
	};
	const request: DappProposal = {
		rid: h.rid,
		address: h.address,
		chainId: h.chainId,
		method: h.method,
		calls: h.calls,
		origin: h.origin,
		safeTxHash: null,
		async propose(as) {
			if (proposed) return proposed;
			if (proposing) throw new Error('already proposing');
			proposing = true;
			try {
				if (!(await h.claim())) {
					h.gone();
					throw new DappGoneError();
				}
				// The claim restarts the request's clock: the limit counts from here.
				const claimedAt = now();
				const { safeTxHash, nonce, fromBlock } = await h.propose(h.address, h.chainId, h.calls, as);
				proposed = safeTxHash;
				h.proposed(h.rid, safeTxHash);
				if (h.method === 'wallet_sendCalls') h.answerBatch(safeTxHash, nonce, fromBlock);
				else {
					h.follow(safeTxHash);
					if (h.limitMs !== undefined) {
						const left = claimedAt + h.limitMs - ANSWER_MARGIN_MS - now();
						limit = setTimeout(
							() => {
								limit = null;
								h.tellProposed(safeTxHash);
							},
							Math.max(0, left)
						);
					}
				}
				return safeTxHash;
			} finally {
				proposing = false;
			}
		},
		executed(txHash) {
			// The real transaction: what an `eth_sendTransaction` waits for.
			if (!proposed || h.method === 'wallet_sendCalls') return;
			stopLimit();
			h.settle({ result: txHash });
		},
		decline() {
			if (proposed) return request.close();
			h.closeSheet(h.rid);
			h.settle({ error: DECLINED });
		},
		close() {
			stopLimit();
			h.closeSheet(h.rid);
			h.settle(closing());
		}
	};
	const closing = (): DappAnswer => ({ error: proposed ? leftProposed(proposed) : DECLINED });
	h.open(request);
	return { closing };
}
