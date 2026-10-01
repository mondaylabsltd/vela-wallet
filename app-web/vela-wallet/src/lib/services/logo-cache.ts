/**
 * Logos that did not load, and until when that is believed (spec 082 RE10,
 * W20).
 *
 * Token and chain logos come from the chain-data endpoint (`ethereumDataURL`)
 * and fall back to the drawn mark — a three-letter glyph, a letter on the
 * chain's colour, a dot — when the fetch fails (spec 028 follow-up,
 * 2026-09-05). Without this memory, a URL that 404'd once would be requested
 * again by every row that re-renders (a balance refresh re-derives them all),
 * and each would flash its image slot before falling back.
 *
 * Before 082 a miss lasted the whole session, so a logo that failed during a
 * network blip stayed a letter until the wallet was reopened. How long a miss
 * lasts is the core's now (`remote_mark::miss_ttl_ms`): an `<img onerror>`
 * carries no status, which the core reads as `unknown` — 60 s — after which
 * the logo is tried again.
 */
import { markMissTtlMs } from '$lib/core/kernels';

/** url → epoch ms until which it is not asked for again (`Infinity`: this session). */
const failedUntil = new Map<string, number>();

/**
 * How long a miss with no status lasts, from the core. Before the core has
 * loaded there is nobody to ask, and the miss lasts the session — what every
 * miss did before 082 — rather than a number invented here.
 */
export function missTtlMs(): number | null {
	try {
		return markMissTtlMs('unknown');
	} catch {
		return null;
	}
}

export function hasFailed(url: string, now = Date.now()): boolean {
	const until = failedUntil.get(url);
	if (until === undefined) return false;
	if (until > now) return true;
	failedUntil.delete(url);
	return false;
}

/**
 * Remember a miss. Returns how long it lasts (ms), or `null` for the rest of
 * the session — the caller retries after it.
 */
export function markFailed(url: string, now = Date.now()): number | null {
	const ttl = missTtlMs();
	failedUntil.set(url, ttl === null ? Number.POSITIVE_INFINITY : now + ttl);
	return ttl;
}

export function resetLogoCacheForTests(): void {
	failedUntil.clear();
}
