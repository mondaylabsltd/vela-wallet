/**
 * What the relay's own estimate said about the operation a sheet is pricing
 * (spec 082 RJ19, G57).
 *
 * The web runs no simulation of its own (RG6), so the relay's
 * `eth_estimateUserOperationGas` is the one voice that can say "this will
 * revert" before the confirm. The quote's simulation (`simulateUserOpGas`)
 * records a revert here, keyed by chain and account, and clears it at the start
 * of every new question; the signing sheet reads it to draw the danger line
 * (`simWillFail` / `simWillFailReason`). A warning informs and never blocks
 * (L-D5): the confirm stays live.
 *
 * A module of its own, with no imports, so the sheet's pure model can read it
 * without pulling the send path in, and a test can plant a verdict.
 */

/** A revert the relay's estimate reported: its reason, already cleaned by the core. */
export interface EstimateReverts {
	reason: string | null;
}

const verdicts = new Map<string, EstimateReverts>();

function keyOf(chainId: number, account: string): string {
	return `${chainId}:${account.toLowerCase()}`;
}

/** The relay's estimate of this account's operation on this chain says it reverts. */
export function recordEstimateReverts(
	chainId: number,
	account: string,
	reason: string | null
): void {
	verdicts.set(keyOf(chainId, account), { reason });
}

/** A new question about this account's operation: the old verdict is not its answer. */
export function clearEstimateReverts(chainId: number, account: string): void {
	verdicts.delete(keyOf(chainId, account));
}

/** The revert the last estimate reported for this account on this chain, or `null`. */
export function estimateRevertsFor(
	chainId: number,
	account: string | null | undefined
): EstimateReverts | null {
	if (!account) return null;
	return verdicts.get(keyOf(chainId, account)) ?? null;
}
