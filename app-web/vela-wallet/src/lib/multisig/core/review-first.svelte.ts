/**
 * §3.6: the one review before the proposer's own first signature, too — a
 * transfer from the send flow, a rename, a member change: the transaction is
 * built at its number, drawn by the proposal sheet as a dApp's is before it
 * exists (what moves, who it reaches, the state-diff guard, the hashes, the
 * AI's opinion), and the passkey is asked only after "Propose and approve".
 * Drawn by the multisig host, on top of any sheet or the send flow; where no
 * host is up to draw it, nothing is signed (as a risk confirmation).
 */
import type { ChainState, ProposalItem } from './proposals.svelte';

export interface ReviewFirstRequest {
	chainId: number;
	address: string;
	chain: ChainState;
	item: ProposalItem;
	/** "Propose and approve": the proposal goes on to its passkey. */
	confirm(): void;
	/** Declined, or the sheet closed: nothing is signed. */
	decline(): void;
}

/** What a declined review throws: a cancel (`AbortError`), never a failure. */
export const REVIEW_DECLINED = 'The person declined the proposal on its review';

class ReviewFirstHost {
	/** Raw: an answer clears the request it belongs to (identity, not a copy). */
	request = $state.raw<ReviewFirstRequest | null>(null);
	#hosts = 0;

	/** A host draws the review while mounted; the returned function unmounts it. */
	attach(): () => void {
		this.#hosts += 1;
		return () => {
			this.#hosts -= 1;
			if (this.#hosts === 0) this.request?.decline();
		};
	}

	/** Resolves on "Propose and approve"; rejects (`AbortError`) on decline or with no host. */
	ask(input: Omit<ReviewFirstRequest, 'confirm' | 'decline'>): Promise<void> {
		this.request?.decline();
		if (this.#hosts === 0) {
			return Promise.reject(new DOMException(REVIEW_DECLINED, 'AbortError'));
		}
		return new Promise<void>((resolve, reject) => {
			const request: ReviewFirstRequest = {
				...input,
				confirm: () => {
					if (this.request === request) this.request = null;
					resolve();
				},
				decline: () => {
					if (this.request === request) this.request = null;
					reject(new DOMException(REVIEW_DECLINED, 'AbortError'));
				}
			};
			this.request = request;
		});
	}
}

export const reviewFirstHost = new ReviewFirstHost();

/** The review of `item` (built at its number on `chain`), before the proposer's passkey. */
export function reviewFirst(
	address: string,
	chainId: number,
	chain: ChainState,
	item: ProposalItem
): Promise<void> {
	return reviewFirstHost.ask({ chainId, address, chain, item });
}
