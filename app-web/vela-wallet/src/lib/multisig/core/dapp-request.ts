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

/**
 * The answer when the person leaves a transaction that is proposed but not
 * executed: not a rejection (it was not rejected, and the members may still
 * execute it), and not a hash (none exists yet). The site is told what
 * happened, so nobody proposes it twice.
 */
export function leftProposed(safeTxHash: string): DappError {
	return {
		code: -32603,
		message:
			`Proposed to the multisig's members (safeTxHash ${safeTxHash}), waiting for their ` +
			'approvals. This site is not told when it is executed.'
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
		calls: DappCall[]
	): Promise<{ safeTxHash: string; nonce: string; fromBlock?: string }>;
	/** Answer the site (the caller answers once). */
	settle(answer: DappAnswer): void;
	/** `eth_sendTransaction`: follow the proposal until it is executed or voided. */
	follow(safeTxHash: string): void;
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
	let proposed: string | null = null;
	let proposing = false;
	const request: DappProposal = {
		rid: h.rid,
		address: h.address,
		chainId: h.chainId,
		method: h.method,
		calls: h.calls,
		origin: h.origin,
		safeTxHash: null,
		async propose() {
			if (proposed) return proposed;
			if (proposing) throw new Error('already proposing');
			proposing = true;
			try {
				if (!(await h.claim())) {
					h.gone();
					throw new DappGoneError();
				}
				const { safeTxHash, nonce, fromBlock } = await h.propose(h.address, h.chainId, h.calls);
				proposed = safeTxHash;
				h.proposed(h.rid, safeTxHash);
				if (h.method === 'wallet_sendCalls') h.answerBatch(safeTxHash, nonce, fromBlock);
				else h.follow(safeTxHash);
				return safeTxHash;
			} finally {
				proposing = false;
			}
		},
		decline() {
			if (proposed) return request.close();
			h.closeSheet(h.rid);
			h.settle({ error: DECLINED });
		},
		close() {
			h.closeSheet(h.rid);
			h.settle(closing());
		}
	};
	const closing = (): DappAnswer => ({ error: proposed ? leftProposed(proposed) : DECLINED });
	h.open(request);
	return { closing };
}
