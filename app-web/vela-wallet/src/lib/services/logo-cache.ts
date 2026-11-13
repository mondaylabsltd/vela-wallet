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
 *
 * home-20: the built-in endpoint answers a logo it does not have with `200
 * text/html` (its site's fallback page), which `<img>` reports as the same
 * status-less error as a dropped connection — so every missing logo was asked
 * for again each minute, forever. After a miss, one HEAD request per URL
 * (`probeMiss`) fetches the status the `<img>` could not see, and the core
 * classes it: a success that did not draw is `not_an_image` and a 404 is
 * `not_found` — both last the session — while a request that never got an
 * answer stays the 60 s miss it was.
 */
import { markMissTtlMs } from '$lib/core/kernels';

/** url → epoch ms until which it is not asked for again (`Infinity`: this session). */
const failedUntil = new Map<string, number>();

/**
 * How long a miss with no status lasts, from the core. Before the core has
 * loaded there is nobody to ask, and the miss lasts the session — what every
 * miss did before 082 — rather than a number invented here.
 */
export function missTtlMs(status?: number): number | null {
	try {
		return markMissTtlMs('unknown', status ?? null);
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
export function markFailed(url: string, now = Date.now(), status?: number): number | null {
	const ttl = missTtlMs(status);
	failedUntil.set(url, ttl === null ? Number.POSITIVE_INFINITY : now + ttl);
	return ttl;
}

/** One probe per URL, however many rows draw it. */
const probes = new Map<string, Promise<number | null>>();

/**
 * Ask the endpoint for the status a failed `<img>` could not report, and
 * remember the miss for as long as that status says. Resolves to the miss's
 * TTL as `markFailed` returns it. A probe that gets no answer (offline, a
 * mirror without CORS) leaves the status-less 60 s miss in place.
 */
export function probeMiss(
	url: string,
	fetchImpl: typeof fetch = fetch,
	now: () => number = Date.now
): Promise<number | null> {
	const known = probes.get(url);
	if (known) return known;
	const probe = (async () => {
		let status: number;
		try {
			const response = await fetchImpl(url, {
				method: 'HEAD',
				credentials: 'omit',
				referrerPolicy: 'no-referrer'
			});
			status = response.status;
		} catch {
			probes.delete(url);
			return missTtlMs();
		}
		const ttl = markFailed(url, now(), status);
		// A miss that heals is probed afresh next time; one for the session
		// never comes back to ask.
		if (ttl !== null) probes.delete(url);
		return ttl;
	})();
	probes.set(url, probe);
	return probe;
}

export function resetLogoCacheForTests(): void {
	failedUntil.clear();
	probes.clear();
}
