/**
 * The ring around the receipt's disc, while a transaction is on its way.
 *
 * ONE curve, in one place. It was written for the send (spec 038 Part D, #D3)
 * and lived inside `SendReceipt.svelte`; spec 077 gives a dApp transaction the
 * same receipt, and two copies of an easing curve is exactly how the two
 * surfaces would come to disagree about the same moment.
 *
 * It eases toward full and never gets there: about 70% at the chain's typical
 * time, 86% at twice it, and a ceiling of 92% after that. So a transaction that
 * takes three minutes is still visibly moving, one that takes ten seconds does
 * not sit at 100% waiting, and **only the confirmation closes the ring** — a
 * full ring beside the word "Submitted" would read as a finished transaction
 * that is not finished.
 */

/** The ceiling: reached only by the confirmation, never by waiting. */
const CEILING = 0.92;
/** How sharply it eases. 1.4 puts ~70% at the typical time. */
const EASE = 1.4;

/**
 * How much of the ring is drawn, 0–1.
 *
 * `undefined` when the chain has no typical time — `StatusHero` then circles
 * instead of filling, which is the honest drawing of "no estimate" rather than
 * a ring that would be an invented promise.
 */
export function ringProgress(elapsedS: number, typicalS: number): number | undefined {
	if (!(typicalS > 0)) return undefined;
	const elapsed = Math.max(0, elapsedS);
	return CEILING * (1 - Math.exp((-EASE * elapsed) / Math.max(1, typicalS)));
}
