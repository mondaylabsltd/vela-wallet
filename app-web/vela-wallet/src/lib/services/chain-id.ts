/**
 * The chain a dApp request is submitted on — one reading, shared by the submit
 * path (`dapp-submit`) and the signing sheet that draws what it will sign.
 *
 * Kept apart from `dapp-submit` so the sheet can read a chain the way the
 * submit does without importing the whole submit path. No dependencies.
 */

/**
 * Resolve the effective chain ID from request context.
 * Priority: request-embedded chainId > fallback (component-level chainId).
 *
 * Lenient, as the submit has always read it: `parseInt` takes a leading
 * number, so `"1x"` is chain 1. Anything that reads as the submit would read
 * it must go through here, not a stricter copy.
 */
export function resolveChainId(
	fallback: number,
	...candidates: (string | number | undefined | null)[]
): number {
	for (const c of candidates) {
		if (c == null) continue;
		const n = typeof c === 'string' ? (c.startsWith('0x') ? parseInt(c, 16) : parseInt(c, 10)) : c;
		if (!isNaN(n) && n > 0) return n;
	}
	return fallback;
}
