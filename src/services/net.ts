/**
 * Network resilience primitives — the single place timeouts, backoff, and error
 * classification live so they aren't scattered as magic numbers across services.
 *
 * Every outbound `fetch` to an external service should go through
 * `fetchWithTimeout` so no request can hang forever. A request that exceeds its
 * budget rejects with a {@link TimeoutError} (distinct from a caller abort or a
 * network failure) so callers can tell "the server is slow / we don't know the
 * outcome" apart from "the request definitively failed" — the difference that
 * matters for non-idempotent writes (see bundler sponsorship / UserOp submit).
 *
 * Note: the JSON-RPC + bundler pool (`rpc-pool.ts`) already wraps its own fetches
 * with the same AbortController pattern and sources its timeouts from here; this
 * module is for everything else (REST APIs, price/FX feeds, chain metadata,
 * descriptors, the key-index backend, dApp transports).
 */

// ---------------------------------------------------------------------------
// Central timeout config (ms). Safe defaults, one per external-service class.
// Override per call by passing a different `timeoutMs`.
// ---------------------------------------------------------------------------

export const NET_TIMEOUTS = {
  /** Read-only JSON-RPC (eth_call / getBalance / getLogs) — fail over fast. */
  rpcRead: 8_000,
  /** Bundler JSON-RPC (sendUserOp / estimate) — submission can be legitimately slow. */
  bundlerRpc: 15_000,
  /** Parallel RPC ping race for the fastest-endpoint pick. */
  rpcPing: 3_000,
  /** Bundler REST account lookup (/v1/account). */
  bundlerRest: 10_000,
  /** Bundler REST sponsorship (/v1/sponsor) — treasury write, give it room. */
  bundlerSponsor: 20_000,
  /** ethereum-data chain info / token lists / search index. */
  ethereumData: 5_000,
  /** Fiat FX rates endpoint. */
  fiatRates: 8_000,
  /** Clear-signing ERC-7730 descriptor fetch. */
  descriptor: 5_000,
  /** Public-key index reads (query / queryByWalletRef). */
  keyIndexRead: 8_000,
  /** Public-key index writes (create) — an on-chain write sits behind it. */
  keyIndexWrite: 15_000,
  /** dApp transport response POST (relay). */
  dappPost: 10_000,
  /** SSE / EventSource initial connection open. */
  sseOpen: 10_000,
  /** Custom-network RPC validation probe. */
  networkCheck: 10_000,
  /** Deployer receipt poll — per single attempt. */
  deployerPoll: 10_000,
} as const;

export type NetTimeoutKey = keyof typeof NET_TIMEOUTS;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/**
 * Thrown by {@link fetchWithTimeout} when the per-attempt budget elapses before
 * a response is received. Distinct from an `AbortError` (caller cancelled) and
 * from a plain network error: a timeout on a non-idempotent write means the
 * outcome is *unknown*, not failed.
 */
export class TimeoutError extends Error {
  readonly isTimeout = true;
  constructor(
    readonly timeoutMs: number,
    readonly url?: string,
  ) {
    super(`Request timed out after ${timeoutMs}ms${url ? ` (${shortenUrl(url)})` : ''}`);
    this.name = 'TimeoutError';
  }
}

/** True for our {@link TimeoutError}. */
export function isTimeoutError(e: unknown): e is TimeoutError {
  return e instanceof Error && e.name === 'TimeoutError';
}

/** True when a request was cancelled via an AbortSignal (caller navigated away, etc.). */
export function isAbortError(e: unknown): boolean {
  return e instanceof Error && e.name === 'AbortError';
}

export type NetErrorKind = 'timeout' | 'aborted' | 'network';

/**
 * Coarse classification for UX/logging. Note this only covers transport-level
 * failures; HTTP status and JSON-RPC error bodies are classified by the caller
 * (they encode permanent business failures vs. transient server errors).
 */
export function classifyNetError(e: unknown): NetErrorKind {
  if (isTimeoutError(e)) return 'timeout';
  if (isAbortError(e)) return 'aborted';
  return 'network';
}

// ---------------------------------------------------------------------------
// fetch with timeout + caller-cancellation
// ---------------------------------------------------------------------------

export interface FetchTimeoutOptions {
  /** Per-attempt timeout in ms. Required — pick a value from {@link NET_TIMEOUTS}. */
  timeoutMs: number;
  /** Optional caller signal — aborting it cancels the in-flight request too. */
  signal?: AbortSignal;
}

/**
 * `fetch` that can never hang: it aborts after `timeoutMs` and rejects with a
 * {@link TimeoutError}. If a caller `signal` is supplied, aborting it also
 * cancels the request (and surfaces as an `AbortError`, not a timeout).
 *
 * Implemented with a single internal AbortController that listens to both the
 * timer and the caller's signal, so it works on runtimes without
 * `AbortSignal.any` / `AbortSignal.timeout` (Hermes / RN).
 */
export async function fetchWithTimeout(
  input: string,
  init: RequestInit = {},
  opts: FetchTimeoutOptions,
): Promise<Response> {
  const { timeoutMs, signal: callerSignal } = opts;
  const controller = new AbortController();
  let timedOut = false;

  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, timeoutMs);

  const onCallerAbort = () => controller.abort();
  if (callerSignal) {
    if (callerSignal.aborted) controller.abort();
    else callerSignal.addEventListener('abort', onCallerAbort);
  }

  try {
    return await fetch(input, { ...init, signal: controller.signal });
  } catch (err) {
    // Our timer firing surfaces as an AbortError from fetch — remap it so the
    // caller sees a TimeoutError and can treat the outcome as *unknown*.
    if (timedOut && isAbortError(err)) throw new TimeoutError(timeoutMs, input);
    throw err;
  } finally {
    clearTimeout(timer);
    if (callerSignal) callerSignal.removeEventListener('abort', onCallerAbort);
  }
}

// ---------------------------------------------------------------------------
// Backoff
// ---------------------------------------------------------------------------

/**
 * Exponential backoff with full jitter, capped. `attempt` is 0-based.
 * Used for retrying *idempotent* operations only — never blind-retry a write.
 *
 * full jitter (AWS): delay = random(0, min(cap, base * 2^attempt))
 */
export function backoffWithJitter(attempt: number, baseMs = 500, capMs = 8_000): number {
  const ceil = Math.min(capMs, baseMs * 2 ** Math.max(0, attempt));
  return Math.floor(Math.random() * ceil);
}

/** Parse a `Retry-After` header (delta-seconds or HTTP-date) into ms, or null. */
export function parseRetryAfterMs(headerValue: string | null | undefined): number | null {
  if (!headerValue) return null;
  const secs = Number(headerValue);
  if (Number.isFinite(secs)) return Math.max(0, secs * 1000);
  const date = Date.parse(headerValue);
  if (Number.isFinite(date)) return Math.max(0, date - Date.now());
  return null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Host + path of a URL for logs — never the query string (may carry API keys). */
export function shortenUrl(url: string): string {
  try {
    const u = new URL(url);
    return u.hostname + u.pathname;
  } catch {
    return url.split('?')[0].slice(0, 60);
  }
}

/** Redact `key`/`apikey`/`token`/`secret` query params from a URL for safe logging. */
export function redactUrl(url: string): string {
  return url.replace(/([?&](?:api[-_]?key|key|token|secret|access[-_]?token)=)[^&#]+/gi, '$1***');
}
