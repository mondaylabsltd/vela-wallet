/**
 * Defense-in-depth gate for dApp read-only RPC that is forwarded to the wallet.
 *
 * WalletPair v1 permits dApps to request read-only chain state over its encrypted
 * channel. A buggy or greedy dApp can flood the wallet with
 * reads — saturating the JS thread and the RPC pool, and starving the user's
 * signing confirmation. Signing requests do NOT pass through this gate, so they
 * are never throttled or delayed by read traffic.
 *
 * The gate does two cheap, always-safe things:
 *   1. Dedupe — identical in-flight reads (same method+params+chain+account)
 *      share a single underlying call. Only concurrent duplicates are collapsed;
 *      results are never cached across time, so answers are never stale.
 *   2. Concurrency cap — at most MAX_CONCURRENT_READS reads run at once; the rest
 *      queue and start as slots free up, bounding thread/RPC-pool pressure.
 */

const MAX_CONCURRENT_READS = 6;
/**
 * Upper bound on queued (not-yet-started) reads. A buggy dApp flooding with
 * DISTINCT keys (dedupe can't collapse those) would otherwise grow the waiter
 * queue without limit. Beyond this, excess reads are rejected with a retryable
 * error rather than accumulating unboundedly in memory.
 */
const MAX_QUEUED_READS = 512;

/** Error code for "too many requests" — retryable on the dApp side. */
const RATE_LIMITED_CODE = -32005;

let active = 0;
const waiters: Array<() => void> = [];
const inFlight = new Map<string, Promise<unknown>>();

function acquire(): Promise<void> {
  if (active < MAX_CONCURRENT_READS) {
    active++;
    return Promise.resolve();
  }
  if (waiters.length >= MAX_QUEUED_READS) {
    return Promise.reject(
      Object.assign(new Error('Wallet busy: too many concurrent read requests'), {
        code: RATE_LIMITED_CODE,
      }),
    );
  }
  return new Promise<void>((resolve) => waiters.push(resolve));
}

function release(): void {
  const next = waiters.shift();
  if (next) {
    // Hand the slot directly to the next waiter — active count is unchanged.
    next();
  } else {
    active--;
  }
}

/**
 * Run a read-only request behind the dedupe + concurrency gate.
 *
 * @param key  Uniquely identifies the request (method + params + chain + account).
 *             Identical concurrent keys share one execution.
 * @param task The work to run (typically `() => handleReadOnlyRPC(...)`).
 *
 * Rejects with a `code: -32005` error when the queue is saturated; the caller
 * should answer the dApp with a retryable error. A slot is released only if it
 * was acquired, so an overflow rejection never corrupts the concurrency count.
 */
export function gateReadOnly<T>(key: string, task: () => Promise<T>): Promise<T> {
  const existing = inFlight.get(key);
  if (existing) return existing as Promise<T>;

  const run = (async () => {
    await acquire(); // throws on overflow — no slot held, so no release below
    try {
      return await task();
    } finally {
      release();
    }
  })().finally(() => {
    inFlight.delete(key);
  });
  inFlight.set(key, run);
  return run;
}

/** Build a stable dedupe key for a read-only request. */
export function readOnlyKey(
  chainId: number,
  account: string,
  method: string,
  params: unknown[],
): string {
  let p: string;
  try {
    p = JSON.stringify(params);
  } catch {
    p = String(params);
  }
  return `${chainId}|${account.toLowerCase()}|${method}|${p}`;
}

/** Reset all gate state. Tests only. */
export function __resetReadOnlyGate(): void {
  active = 0;
  waiters.length = 0;
  inFlight.clear();
}
