/**
 * Logos that did not load, remembered for the session.
 *
 * Token and chain logos come from the chain-data endpoint (`ethereumDataURL`)
 * and fall back to the drawn mark — a three-letter glyph, a letter on the
 * chain's colour, a dot — when the fetch fails (spec 028 follow-up,
 * 2026-09-05). Without this set, a URL that 404'd once would be requested
 * again by every row that re-renders (a balance refresh re-derives them all),
 * and each would flash its image slot before falling back. The second time,
 * the fallback has to be immediate.
 */
const failed = new Set<string>();

export function hasFailed(url: string): boolean {
	return failed.has(url);
}

export function markFailed(url: string): void {
	failed.add(url);
}

export function resetLogoCacheForTests(): void {
	failed.clear();
}
