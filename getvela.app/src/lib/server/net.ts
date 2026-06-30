/**
 * Server-side fetch resilience for the API/proxy routes.
 *
 * Every upstream call MUST go through one of these so a hung third-party service
 * can't tie up a Worker request indefinitely. Timeouts live here (not as magic
 * numbers scattered across +server.ts files) and logging helpers keep API keys
 * (which providers put in the URL path *or* query) out of the logs.
 */

export const UPSTREAM_TIMEOUTS = {
	/** Bundler / RPC provider (Pimlico / Alchemy) — UserOp submit can be slow. */
	bundler: 15_000,
	/** Fiat exchange-rate provider (frankfurter). */
	exchangeRate: 8_000,
	/** Alchemy NFT API, per chain. */
	nft: 10_000,
	/** Generic user-supplied proxy target. */
	proxy: 10_000
} as const;

/**
 * `fetch` with a hard overall timeout — for non-streaming requests where the
 * whole exchange (headers + body) should complete within the budget. Rejects
 * with a `TimeoutError` (DOMException) on timeout.
 */
export async function fetchWithTimeout(
	input: string | URL | Request,
	init: RequestInit = {},
	timeoutMs: number
): Promise<Response> {
	return fetch(input, { ...init, signal: AbortSignal.timeout(timeoutMs) });
}

/**
 * `fetch` with a *connect-phase* timeout only: it aborts if the upstream doesn't
 * return response headers within the budget, but once headers arrive the timer
 * is cleared so a long-lived stream (SSE) isn't killed mid-flight. Use for the
 * generic proxy where the body may legitimately stream for a long time.
 */
export async function fetchWithConnectTimeout(
	input: string | URL | Request,
	init: RequestInit = {},
	timeoutMs: number
): Promise<Response> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);
	try {
		return await fetch(input, { ...init, signal: controller.signal });
	} finally {
		clearTimeout(timer);
	}
}

/** Host of a URL for logs — never the path or query string (they carry API keys). */
export function safeHost(url: string): string {
	try {
		return new URL(url).host;
	} catch {
		return 'unknown';
	}
}
