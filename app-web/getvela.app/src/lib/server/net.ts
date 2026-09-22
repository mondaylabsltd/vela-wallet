/**
 * Server-side fetch resilience for the API routes.
 *
 * Every upstream call MUST go through one of these so a hung third-party service
 * can't tie up a Worker request indefinitely. Timeouts live here (not as magic
 * numbers scattered across +server.ts files).
 *
 * Spec 081 (FR-015) deleted the five dormant routes that were the only other
 * users of this module — `api/{wallet,transactions,nft,bundler,proxy}`. The
 * helpers that went with them (`fetchWithConnectTimeout` for the streaming
 * proxy, `safeHost` for keeping provider API keys out of the bundler's logs,
 * and the `bundler`/`nft`/`proxy` budgets) were removed at the same time; the
 * two survivors below are what `exchange-rate` and `bug-report` still call.
 */

export const UPSTREAM_TIMEOUTS = {
	/** Fiat exchange-rate provider (frankfurter). */
	exchangeRate: 8_000
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
