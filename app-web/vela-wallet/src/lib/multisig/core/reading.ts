/**
 * r8-1: the core's reading of a SafeTx's calls from its bytes alone
 * (`review.reading`, the link door — the same rule the one review carries
 * and every platform's parity vectors replay), for what has no review: an
 * executed proposal's Activity title, the sheet asking an outside wallet
 * before its review is in. Token facts are the registry's only here; the
 * review reads the rest.
 */
import type { MultisigReviewOut, ReviewConcern, ReviewReading, SafeTx } from './kernel';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';

/** What `review.reading` answers. */
export interface ReadingAnswer {
	reading: ReviewReading;
	concerns: ReviewConcern[];
	/** The confirm says "… anyway". */
	anyway: boolean;
	/** Nothing is signed. */
	refused: boolean;
}

/** The reading of `tx` on `chainId` for `safe`, or `null` when the core cannot read it. */
export function readTx(chainId: number, safe: string, tx: SafeTx): ReadingAnswer | null {
	try {
		return JSON.parse(
			wasm.multisigLinkCall('review.reading', JSON.stringify({ chainId, safe, tx }))
		) as ReadingAnswer;
	} catch {
		return null;
	}
}

/**
 * The part of a review the call drawing reads (`reading`, `concerns`), from
 * a reading alone — for `CallReview` before (or without) the review.
 */
export function asReview(answer: ReadingAnswer | null): MultisigReviewOut | null {
	if (!answer) return null;
	return { reading: answer.reading, concerns: answer.concerns } as unknown as MultisigReviewOut;
}
