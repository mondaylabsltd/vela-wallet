/**
 * RPC & Bundler endpoint pool with automatic load balancing and failover.
 *
 * Endpoints are collected from multiple sources per chain:
 *   RPC:     user-configured > built-in (ethereum-data API) > network default > public fallback
 *   Bundler: user-configured > built-in (vela-bundler.getvela.app)
 *
 * Each endpoint tracks latency and failure stats. Calls are routed to the
 * highest-scoring endpoint first, with automatic failover on connectivity errors.
 */

import { DEFAULT_NETWORKS, getAllNetworksSync } from '@/models/network';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { fetchChainInfo } from './chain-registry';
import { getBundlerServiceURL, getNetworkConfig, loadServiceEndpoints } from './storage';
import { rpcLatencyMs, rpcShouldFail } from './dev/fault-injection';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface EndpointStats {
  url: string;
  /** 'fallback' = the rest of the chain-index RPC list, tried last. */
  source: 'user' | 'builtin' | 'default' | 'public' | 'fallback';
  avgLatencyMs: number;
  consecutiveFailures: number;
  lastFailureAt: number;
  totalCalls: number;
  totalFailures: number;
  /** Permanently banned (e.g. requires auth, API key). Never used again until pool refresh. */
  banned: boolean;
}

interface RPCResponse {
  jsonrpc: string;
  id: number;
  result?: any;
  error?: { code: number; message: string; data?: any };
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const rpcPools = new Map<number, EndpointStats[]>();
const bundlerPools = new Map<number, EndpointStats[]>();
const poolInitAt = new Map<number, number>();
const POOL_REFRESH_MS = 10 * 60 * 1000; // 10 min

/**
 * Ban system — two tiers:
 *   Temporary: rate-limited, 401/403, "exceeded" etc. Cooldown = 1 hour, then retry.
 *   Permanent: never had a single success AND failed >= 6 times. Persisted forever.
 */
const BANNED_STORAGE_KEY = 'vela.rpc.banned';
const TEMP_BAN_TTL_MS = 60 * 60 * 1000; // 1 hour
const PERMA_BAN_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours — allows recovery from transient outages
const PERMA_BAN_MIN_FAILURES = 6;

interface BanEntry { url: string; bannedAt: number; permanent: boolean }
const banMap = new Map<string, BanEntry>();
let banLoaded = false;

async function loadBans(): Promise<void> {
  if (banLoaded) return;
  try {
    const raw = await AsyncStorage.getItem(BANNED_STORAGE_KEY);
    if (raw) {
      for (const e of JSON.parse(raw) as BanEntry[]) banMap.set(e.url, e);
    }
  } catch { /* ignore */ }
  banLoaded = true;
}

async function saveBans(): Promise<void> {
  try {
    await AsyncStorage.setItem(BANNED_STORAGE_KEY, JSON.stringify([...banMap.values()]));
  } catch { /* ignore */ }
}

/** Prune all expired entries from the ban map to prevent unbounded growth. */
let lastBanPruneAt = 0;
const BAN_PRUNE_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes

function pruneExpiredBans(): void {
  const now = Date.now();
  if (now - lastBanPruneAt < BAN_PRUNE_INTERVAL_MS) return;
  lastBanPruneAt = now;
  let pruned = 0;
  for (const [url, entry] of banMap) {
    const expired = entry.permanent
      ? now - entry.bannedAt >= PERMA_BAN_TTL_MS
      : now - entry.bannedAt >= TEMP_BAN_TTL_MS;
    if (expired) {
      banMap.delete(url);
      pruned++;
    }
  }
  if (pruned > 0) {
    saveBans();
    console.log(`[RPC] Pruned ${pruned} expired ban(s), ${banMap.size} remaining`);
  }
}

/** Check whether a URL is currently banned (skipping expired temp bans). */
function isBanned(url: string): boolean {
  pruneExpiredBans();
  const entry = banMap.get(url);
  if (!entry) return false;
  if (entry.permanent) {
    // Permanent bans expire after 24h to allow recovery from transient outages
    if (Date.now() - entry.bannedAt >= PERMA_BAN_TTL_MS) {
      banMap.delete(url);
      return false;
    }
    return true;
  }
  if (Date.now() - entry.bannedAt < TEMP_BAN_TTL_MS) return true;
  // Temp ban expired — remove it
  banMap.delete(url);
  return false;
}

/** Temporarily ban a URL (1-hour cooldown). */
function tempBan(url: string): void {
  banMap.set(url, { url, bannedAt: Date.now(), permanent: false });
  saveBans();
}

/**
 * Check and apply permanent ban if warranted:
 * the endpoint has NEVER succeeded and has failed >= 6 times.
 */
function maybePermaBan(stats: EndpointStats): void {
  const successes = stats.totalCalls - stats.totalFailures;
  if (successes === 0 && stats.totalFailures >= PERMA_BAN_MIN_FAILURES) {
    banMap.set(stats.url, { url: stats.url, bannedAt: Date.now(), permanent: true });
    saveBans();
    console.warn(`[RPC] ${shorten(stats.url)} PERMA-BANNED: 0 success in ${stats.totalFailures} attempts`);
  }
}

/** Chains where ALL RPC endpoints failed on the last attempt. Cleared on success. */
const rpcFailedChains = new Set<number>();

/** Get the set of chain IDs whose RPC endpoints are all currently failing. */
export function getFailedRpcChains(): ReadonlySet<number> {
  return rpcFailedChains;
}

/** Built-in bundler base URL (reads user config, falls back to default) */
const getBuiltinBundler = () => getBundlerServiceURL();

/** Reliable public RPCs per chain (curated, known to work without auth). */
const PUBLIC_RPCS: Record<number, string[]> = {
  1:     ['https://ethereum-rpc.publicnode.com', 'https://1rpc.io/eth'],
  // 1rpc.io/bnb was dropped — it shares a global rate limit and returns -32001
  // "usage limit" under load. These three are CORS-enabled and reliable.
  56:    ['https://bsc-rpc.publicnode.com', 'https://bsc.drpc.org', 'https://bsc.meowrpc.com'],
  137:   ['https://polygon-bor-rpc.publicnode.com', 'https://1rpc.io/matic'],
  42161: ['https://arbitrum-one-rpc.publicnode.com', 'https://1rpc.io/arb'],
  10:    ['https://optimism-rpc.publicnode.com', 'https://1rpc.io/op'],
  8453:  ['https://base-rpc.publicnode.com', 'https://1rpc.io/base'],
  43114: ['https://avalanche-c-chain-rpc.publicnode.com', 'https://1rpc.io/avax/c'],
  100:   ['https://gnosis-rpc.publicnode.com', 'https://1rpc.io/gnosis'],
};

// ---------------------------------------------------------------------------
// Pool initialization
// ---------------------------------------------------------------------------

/** In-flight init promises to prevent duplicate concurrent initialization. */
const initInFlight = new Map<number, Promise<void>>();

async function ensurePool(chainId: number): Promise<void> {
  const initAt = poolInitAt.get(chainId) ?? 0;
  if (Date.now() - initAt < POOL_REFRESH_MS && rpcPools.has(chainId)) return;

  // Deduplicate concurrent init calls for the same chain
  const existing = initInFlight.get(chainId);
  if (existing) return existing;

  const promise = initPool(chainId).finally(() => initInFlight.delete(chainId));
  initInFlight.set(chainId, promise);
  return promise;
}

async function initPool(chainId: number): Promise<void> {
  await loadBans();
  // Ensure user-configured endpoints are loaded so getBuiltinBundler() returns the right URL
  await loadServiceEndpoints();

  const [rpcUrls, bundlerUrls] = await Promise.all([
    collectRpcUrls(chainId),
    collectBundlerUrls(chainId),
  ]);

  // Preserve existing stats, add new endpoints
  const existing = rpcPools.get(chainId) ?? [];
  const rpcStats = mergeEndpoints(existing, rpcUrls);
  rpcPools.set(chainId, rpcStats);

  const existingB = bundlerPools.get(chainId) ?? [];
  const bStats = mergeEndpoints(existingB, bundlerUrls);
  bundlerPools.set(chainId, bStats);

  poolInitAt.set(chainId, Date.now());
}

function mergeEndpoints(
  existing: EndpointStats[],
  newEntries: { url: string; source: EndpointStats['source'] }[],
): EndpointStats[] {
  const byUrl = new Map(existing.map(e => [e.url, e]));
  const result: EndpointStats[] = [];

  for (const entry of newEntries) {
    const prev = byUrl.get(entry.url);
    if (prev) {
      prev.source = entry.source; // update source in case it changed
      result.push(prev);
    } else {
      result.push({
        url: entry.url,
        source: entry.source,
        avgLatencyMs: 0,
        consecutiveFailures: 0,
        lastFailureAt: 0,
        totalCalls: 0,
        totalFailures: 0,
        banned: false,
      });
    }
  }
  return result;
}

// ---------------------------------------------------------------------------
// Endpoint collection
// ---------------------------------------------------------------------------

async function collectRpcUrls(chainId: number): Promise<{ url: string; source: EndpointStats['source'] }[]> {
  const entries: { url: string; source: EndpointStats['source'] }[] = [];
  const seen = new Set<string>();

  const add = (url: string, source: EndpointStats['source']) => {
    if (!url || seen.has(url) || isBanned(url)) return;
    seen.add(url);
    entries.push({ url, source });
  };

  // Chain index (configurable ethereum-data service, eip155-{id}.json) —
  // fetched once and reused for both the primary and the deep-fallback tiers.
  let indexRpcs: string[] = [];
  try {
    const info = await fetchChainInfo(chainId);
    indexRpcs = info?.rpcUrls ?? [];
  } catch { /* ignore */ }

  // 1. User-configured
  try {
    const config = await getNetworkConfig(chainId);
    const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
    if (config?.rpcURL && config.rpcURL !== defaultNet?.rpcURL) {
      add(config.rpcURL, 'user');
    }
  } catch { /* ignore */ }

  // 2. Built-in — first few from the chain index
  indexRpcs.slice(0, 5).forEach(url => add(url, 'builtin'));

  // 3. Network default
  const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
  if (defaultNet?.rpcURL) add(defaultNet.rpcURL, 'default');

  // Custom network default
  const customNet = getAllNetworksSync().find(n => n.chainId === chainId);
  if (customNet?.rpcURL) add(customNet.rpcURL, 'default');

  // 4. Public fallback (curated reliable, CORS-friendly RPCs)
  for (const url of PUBLIC_RPCS[chainId] ?? []) add(url, 'public');

  // 5. Deep fallback — the rest of the chain index (~15-20 RPCs/chain). Lowest
  //    priority, so it's only reached when everything above is rate-limited or
  //    banned. Bad/non-CORS entries get scored down or banned on first use.
  indexRpcs.slice(5, 20).forEach(url => add(url, 'fallback'));

  return entries;
}

async function collectBundlerUrls(chainId: number): Promise<{ url: string; source: EndpointStats['source'] }[]> {
  const entries: { url: string; source: EndpointStats['source'] }[] = [];
  const seen = new Set<string>();
  const defaultChainIds = new Set(DEFAULT_NETWORKS.map(n => n.chainId));

  const add = (url: string, source: EndpointStats['source']) => {
    if (!url || seen.has(url) || isBanned(url)) return;
    seen.add(url);
    entries.push({ url, source });
  };

  // 1. User-configured override (from NetworkConfig editor)
  try {
    const config = await getNetworkConfig(chainId);
    if (config?.bundlerURL) {
      const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
      // Skip if it's the unchanged default URL (user never intentionally set it)
      if (!defaultNet || config.bundlerURL !== defaultNet.bundlerURL) {
        add(config.bundlerURL, 'user');
      }
    }
  } catch { /* ignore */ }

  // 2. Custom network's own bundlerURL (set during "Add Network")
  if (!defaultChainIds.has(chainId)) {
    const net = getAllNetworksSync().find(n => n.chainId === chainId);
    if (net?.bundlerURL) add(net.bundlerURL, 'user');
  }

  // 3. Built-in vela bundler (always available as fallback)
  add(`${getBuiltinBundler()}/${chainId}`, 'builtin');

  return entries;
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

const SOURCE_PRIORITY: Record<string, number> = {
  user:     10000,
  builtin:  1000,
  default:  500,
  public:   100,
  fallback: 10,   // chain-index extras — only when everything else is exhausted
};

function endpointScore(stats: EndpointStats): number {
  // Permanently banned endpoints are never selected
  if (stats.banned) return -Infinity;

  let score = SOURCE_PRIORITY[stats.source] ?? 0;

  // Latency penalty: -1 per 10ms above 200ms (guard against NaN/Infinity)
  const latency = Number.isFinite(stats.avgLatencyMs) ? stats.avgLatencyMs : 0;
  if (latency > 200) {
    score -= Math.min((latency - 200) / 10, 200);
  }

  // Reliability bonus
  const successes = stats.totalCalls - stats.totalFailures;
  score += Math.min(successes, 50);

  // Failure penalty with cooldown
  if (stats.consecutiveFailures > 0) {
    const cooldownMs = Math.min(30_000 * 2 ** (stats.consecutiveFailures - 1), 300_000);
    if (Date.now() - stats.lastFailureAt < cooldownMs) {
      score -= 50_000; // effectively disabled during cooldown
    } else {
      score -= stats.consecutiveFailures * 200;
    }
  }

  return score;
}

/**
 * Detect RPC errors that indicate the endpoint is permanently unusable
 * (requires API key, authentication, or paid plan).
 * These are distinct from normal RPC errors like "execution reverted".
 */
function isPermanentRpcError(error: RPCResponse['error']): boolean {
  if (!error?.message) return false;
  const msg = error.message.toLowerCase();
  return (
    msg.includes('unauthorized') ||
    msg.includes('api key') ||
    msg.includes('authenticate') ||
    msg.includes('forbidden') ||
    msg.includes('payment required') ||
    msg.includes('exceeded') ||
    msg.includes('subscription') ||
    // Restricted public nodes that reject our topic-only getLogs and demand a
    // contract address / paid plan (e.g. publicnode BSC: "Please specify an
    // address … order a dedicated full node"). They can never serve this query,
    // so ban + fail over to a working endpoint instead of failing the chain.
    msg.includes('specify an address') ||
    msg.includes('dedicated full node')
  );
}

/**
 * Detect transient server errors that should trigger failover (try next endpoint)
 * but NOT ban the endpoint. These are server-side issues, not request-specific.
 * Excludes execution errors like "revert" or "gas" which are valid responses.
 */
function isTransientServerError(error: RPCResponse['error']): boolean {
  if (!error) return false;
  const msg = (error.message ?? '').toLowerCase();
  // Skip if the error is about execution (valid response, not a server issue)
  if (msg.includes('revert') || msg.includes('gas') || msg.includes('execution')) return false;
  // JSON-RPC internal error or server error codes
  if (error.code === -32603 || (error.code != null && error.code <= -32000 && error.code >= -32099)) return true;
  // Message-based detection
  return (
    msg.includes('internal error') ||
    msg.includes('server error') ||
    msg.includes('service unavailable') ||
    msg.includes('temporarily unavailable') ||
    msg.includes('too many request')
  );
}

/**
 * Classify an `eth_getLogs` error as a *range/size limit* (the request spanned
 * too many blocks, or would return too many results) rather than an endpoint
 * fault. These are request-specific: the endpoint is healthy but capped, so the
 * caller should split the block range and retry — failing over (the next
 * endpoint usually has the same cap) or banning a working endpoint is wrong.
 *
 * Returns the endpoint's stated max *block span* when the message includes one
 * ("...limited to a 100 range" → 100), `0` when it's a range/result error with
 * no usable block number (caller should just halve), or `null` when it isn't a
 * range error at all.
 */
export function getLogsRangeCap(error: RPCResponse['error']): number | null {
  if (!error?.message) return null;
  const msg = error.message.toLowerCase();

  // Result-count caps ("query returned more than 10000 results"): narrow the
  // span, but the number is a result count not a block span — signal "halve".
  if (msg.includes('result') &&
      (msg.includes('more than') || msg.includes('exceed') || msg.includes('limit') || msg.includes('too many'))) {
    return 0;
  }

  // Block-span caps, worded many different ways across providers.
  const isRangeError =
    msg.includes('block range') ||
    msg.includes('block height') ||
    msg.includes('too many blocks') ||
    msg.includes('range is too') ||
    msg.includes('range too') ||
    msg.includes('range limit') ||
    msg.includes('limited to') ||
    (msg.includes('range') &&
      (msg.includes('exceed') || msg.includes('large') || msg.includes('wide') || msg.includes('maximum')));
  if (!isRangeError) return null;

  // Recover the stated max block span if present (first integer in the message),
  // honouring a k/m suffix ("up to a 2K block range" → 2000) so we don't shrink
  // to a needlessly tiny chunk.
  const m = msg.match(/(\d[\d,_]*)\s*([km])?/);
  if (m) {
    let n = parseInt(m[1].replace(/[,_]/g, ''), 10);
    if (m[2] === 'k') n *= 1_000;
    else if (m[2] === 'm') n *= 1_000_000;
    if (Number.isFinite(n) && n > 0) return n;
  }
  return 0;
}

function getSortedEndpoints(chainId: number, type: 'rpc' | 'bundler'): EndpointStats[] {
  const pool = (type === 'rpc' ? rpcPools : bundlerPools).get(chainId) ?? [];
  return [...pool].sort((a, b) => endpointScore(b) - endpointScore(a));
}

// ---------------------------------------------------------------------------
// Stats recording
// ---------------------------------------------------------------------------

function recordSuccess(stats: EndpointStats, latencyMs: number): void {
  stats.totalCalls++;
  stats.consecutiveFailures = 0;
  // Exponential moving average for latency
  if (stats.avgLatencyMs === 0) {
    stats.avgLatencyMs = latencyMs;
  } else {
    stats.avgLatencyMs = stats.avgLatencyMs * 0.7 + latencyMs * 0.3;
  }
}

function recordFailure(stats: EndpointStats): void {
  stats.totalCalls++;
  stats.totalFailures++;
  stats.consecutiveFailures++;
  stats.lastFailureAt = Date.now();
}

// ---------------------------------------------------------------------------
// Core call function
// ---------------------------------------------------------------------------

/** Per-request timeout (ms). Prevents a hanging server from blocking failover. */
const REQUEST_TIMEOUT_MS = 15_000;
/**
 * Shorter timeout for read RPCs (balances / eth_call / chainId). A hung node
 * fails over to the next endpoint in 8s instead of 15s, halving the worst-case
 * wait when a chain's RPC is down (e.g. BSC). Bundler ops keep the longer
 * REQUEST_TIMEOUT_MS — UserOp submission can legitimately be slow.
 */
const RPC_READ_TIMEOUT_MS = 8_000;

/** Custom error to flag HTTP-level permanent failures (401, 403, 404). */
class HttpBanError extends Error {
  constructor(status: number) { super(`HTTP ${status}`); this.name = 'HttpBanError'; }
}

async function tryEndpoint(
  url: string,
  method: string,
  params: any[],
  extraHeaders?: Record<string, string>,
  timeoutMs: number = REQUEST_TIMEOUT_MS,
): Promise<RPCResponse> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...extraHeaders },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
      signal: controller.signal,
    });

    // HTTP 401/403/404 = permanent access issue → ban this endpoint
    if (res.status === 401 || res.status === 403 || res.status === 404) {
      throw new HttpBanError(res.status);
    }
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const contentType = res.headers.get('content-type') ?? '';
    if (!contentType.includes('json')) {
      throw new Error(`Non-JSON response (${contentType.split(';')[0] || 'unknown'})`);
    }
    const json = await res.json();
    if (!json || typeof json !== 'object') throw new Error('Invalid response');
    return json as RPCResponse;
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------------
// Fastest-RPC picker (for X-Rpc-Url header sent to bundler)
// ---------------------------------------------------------------------------

/** Ping timeout per endpoint — short since we race all in parallel. */
const PING_TIMEOUT_MS = 3_000;

/** Cache the winning URL per chain for 60s to avoid pinging every bundler call. */
const fastestRpcCache = new Map<number, { url: string; ts: number }>();
const FASTEST_RPC_TTL_MS = 3_600_000; // 1 hour

/**
 * Race all known RPC endpoints for `chainId` with a lightweight eth_chainId
 * call and return the URL that responds first / lowest latency.
 * Falls back to the score-sorted first endpoint if all pings fail.
 */
async function pickFastestRpcUrl(chainId: number): Promise<string | undefined> {
  const rpcEndpoints = getSortedEndpoints(chainId, 'rpc');
  if (rpcEndpoints.length === 0) return undefined;
  if (rpcEndpoints.length === 1) return rpcEndpoints[0].url;

  // Return cached winner if still fresh
  const cached = fastestRpcCache.get(chainId);
  if (cached && Date.now() - cached.ts < FASTEST_RPC_TTL_MS) return cached.url;

  const results: { url: string; ms: number }[] = [];

  await Promise.allSettled(
    rpcEndpoints.map(async (ep) => {
      const ac = new AbortController();
      const timer = setTimeout(() => ac.abort(), PING_TIMEOUT_MS);
      const t0 = Date.now();
      try {
        const res = await fetch(ep.url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
          signal: ac.signal,
        });
        if (!res.ok) return;
        const json = await res.json();
        if (!json?.result) return;
        results.push({ url: ep.url, ms: Date.now() - t0 });
      } catch { /* timeout or network error — skip */ } finally {
        clearTimeout(timer);
      }
    }),
  );

  if (results.length === 0) {
    // All pings failed — fall back to score-sorted first
    return rpcEndpoints[0].url;
  }

  results.sort((a, b) => a.ms - b.ms);
  const winner = results[0];
  console.log(`[RPC] Fastest for chain ${chainId}: ${shorten(winner.url)} (${winner.ms}ms) out of ${results.length}/${rpcEndpoints.length}`);
  fastestRpcCache.set(chainId, { url: winner.url, ts: Date.now() });
  return winner.url;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Make an RPC call with automatic load balancing and failover.
 * Tries endpoints in score order (user > built-in > default > public).
 */
export async function poolRpcCall(
  method: string,
  params: any[],
  chainId: number,
  retried = false,
): Promise<RPCResponse> {
  // Dev fault injection (no-op in production / when no faults are set).
  const injectedLatency = rpcLatencyMs();
  if (injectedLatency > 0) await new Promise(r => setTimeout(r, injectedLatency));
  if (rpcShouldFail(chainId)) {
    rpcFailedChains.add(chainId);
    throw new Error(`[fault] RPC forced to fail for chain ${chainId}`);
  }

  await ensurePool(chainId);

  let endpoints = getSortedEndpoints(chainId, 'rpc');

  // If all endpoints are banned, clear bans and rebuild pool to allow recovery
  if (endpoints.length === 0) {
    console.warn(`[RPC] All endpoints banned for chain ${chainId} — clearing bans and retrying`);
    const pool = rpcPools.get(chainId) ?? [];
    for (const ep of pool) {
      banMap.delete(ep.url);
      ep.banned = false;
      ep.consecutiveFailures = 0;
    }
    saveBans();
    endpoints = getSortedEndpoints(chainId, 'rpc');
  }
  console.log(`[RPC] ${method} chain=${chainId} endpoints=${endpoints.length} [${endpoints.map(e => `${e.source}:${shorten(e.url)}`).join(', ')}]`);

  for (const ep of endpoints) {
    const t0 = Date.now();
    try {
      const response = await tryEndpoint(ep.url, method, params, undefined, RPC_READ_TIMEOUT_MS);
      const ms = Date.now() - t0;

      // eth_getLogs range/size-limit: request-specific, not an endpoint fault.
      // Return it so the caller can split the block range — must come before the
      // permanent/transient checks since these errors often carry "exceed" or a
      // -32000 code that would otherwise (wrongly) ban or fail over the endpoint.
      if (method === 'eth_getLogs' && response.error && getLogsRangeCap(response.error) !== null) {
        recordSuccess(ep, ms);
        rpcFailedChains.delete(chainId);
        console.log(`[RPC] eth_getLogs → ${shorten(ep.url)} ${ms}ms RANGE_LIMIT: ${response.error.message?.slice(0, 60)} → caller splits`);
        return response;
      }

      // Ban endpoints that require auth/API key and failover to next
      if (response.error && isPermanentRpcError(response.error)) {
        ep.banned = true;
        recordFailure(ep);
        tempBan(ep.url);
        maybePermaBan(ep);
        console.warn(`[RPC] ${method} → ${shorten(ep.url)} BANNED: ${response.error.message?.slice(0, 80)}`);
        continue;
      }

      // Transient server errors: failover without banning
      if (response.error && isTransientServerError(response.error)) {
        recordFailure(ep);
        console.warn(`[RPC] ${method} → ${shorten(ep.url)} ${ms}ms SERVER_ERR: ${response.error.message?.slice(0, 60)} → trying next`);
        continue;
      }

      recordSuccess(ep, ms);
      rpcFailedChains.delete(chainId);
      console.log(`[RPC] ${method} → ${shorten(ep.url)} ${ms}ms ${response.error ? 'ERR:' + response.error.message?.slice(0, 60) : 'OK'}`);
      return response;
    } catch (err) {
      if (err instanceof HttpBanError) {
        ep.banned = true;
        recordFailure(ep);
        tempBan(ep.url);
        maybePermaBan(ep);
        console.warn(`[RPC] ${method} → ${shorten(ep.url)} BANNED: ${err.message}`);
      } else {
        recordFailure(ep);
        console.warn(`[RPC] ${method} → ${shorten(ep.url)} FAIL: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  // All endpoints failed on first pass — retry once after a short delay
  // to recover from transient network glitches (CDN 502s, DNS hiccups).
  if (!retried) {
    console.warn(`[RPC] All endpoints failed for chain ${chainId}, retrying in 1s...`);
    await new Promise(r => setTimeout(r, 1000));
    return poolRpcCall(method, params, chainId, true);
  }

  rpcFailedChains.add(chainId);
  throw new Error(`All RPC endpoints failed for chain ${chainId}`);
}

/**
 * Make a bundler RPC call with automatic failover.
 * Sends X-Rpc-Url header so the vela bundler knows how to reach the chain.
 */
export async function poolBundlerCall(
  method: string,
  params: any[],
  chainId: number,
  retried = false,
): Promise<RPCResponse> {
  await ensurePool(chainId);

  let endpoints = getSortedEndpoints(chainId, 'bundler');

  // If all endpoints are banned, clear bans and rebuild pool to allow recovery
  if (endpoints.length === 0) {
    console.warn(`[Bundler] All endpoints banned for chain ${chainId} — clearing bans and retrying`);
    const pool = bundlerPools.get(chainId) ?? [];
    for (const ep of pool) {
      banMap.delete(ep.url);
      ep.banned = false;
      ep.consecutiveFailures = 0;
    }
    saveBans();
    endpoints = getSortedEndpoints(chainId, 'bundler');
  }
  console.log(`[Bundler] ${method} chain=${chainId} endpoints=${endpoints.length} [${endpoints.map(e => `${e.source}:${shorten(e.url)}`).join(', ')}]`);

  // Pick the lowest-latency RPC URL — passed via X-Rpc-Url so the bundler can reach the chain
  const chainRpcUrl = await pickFastestRpcUrl(chainId);
  const extraHeaders = chainRpcUrl ? { 'X-Rpc-Url': chainRpcUrl } : undefined;

  for (const ep of endpoints) {
    const t0 = Date.now();
    try {
      const response = await tryEndpoint(ep.url, method, params, extraHeaders);
      const ms = Date.now() - t0;

      if (response.error && isPermanentRpcError(response.error)) {
        ep.banned = true;
        recordFailure(ep);
        tempBan(ep.url);
        maybePermaBan(ep);
        console.warn(`[Bundler] ${method} → ${shorten(ep.url)} BANNED: ${response.error.message?.slice(0, 80)}`);
        continue;
      }

      if (response.error && isTransientServerError(response.error)) {
        recordFailure(ep);
        console.warn(`[Bundler] ${method} → ${shorten(ep.url)} ${ms}ms SERVER_ERR: ${response.error.message?.slice(0, 60)} → trying next`);
        continue;
      }

      recordSuccess(ep, ms);
      console.log(`[Bundler] ${method} → ${shorten(ep.url)} ${ms}ms ${response.error ? 'ERR:' + response.error.message?.slice(0, 80) : 'OK'}`);
      return response;
    } catch (err) {
      if (err instanceof HttpBanError) {
        ep.banned = true;
        recordFailure(ep);
        tempBan(ep.url);
        maybePermaBan(ep);
        console.warn(`[Bundler] ${method} → ${shorten(ep.url)} BANNED: ${err.message}`);
      } else {
        recordFailure(ep);
        console.warn(`[Bundler] ${method} → ${shorten(ep.url)} FAIL: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  if (!retried) {
    console.warn(`[Bundler] All endpoints failed for chain ${chainId}, retrying in 1s...`);
    await new Promise(r => setTimeout(r, 1000));
    return poolBundlerCall(method, params, chainId, true);
  }

  throw new Error(`All bundler endpoints failed for chain ${chainId}`);
}

function shorten(url: string): string {
  try {
    const u = new URL(url);
    return u.hostname + u.pathname;
  } catch {
    return url.slice(0, 40);
  }
}

/**
 * Check whether the built-in vela bundler will be used for a given chain.
 * Returns true if no user-configured bundler is available.
 * Ensures the pool is initialized before checking.
 */
export async function isUsingBuiltinBundler(chainId: number): Promise<boolean> {
  await ensurePool(chainId);
  const pool = bundlerPools.get(chainId) ?? [];
  // Check if any healthy non-vela bundler exists.
  // User-configured endpoints pointing at the built-in bundler still count as built-in.
  const builtinHost = getBuiltinBundler();
  const externalEndpoints = pool.filter(
    e => e.source === 'user' && e.consecutiveFailures === 0 && !e.url.includes(builtinHost),
  );
  return externalEndpoints.length === 0;
}

/** Get the built-in bundler base URL (for REST API calls). */
export function getBuiltinBundlerUrl(): string {
  return getBuiltinBundler();
}


/** Get the best RPC URL for a chain (for passing to bundler via X-Rpc-Url). */
export async function getChainRpcUrl(chainId: number): Promise<string | null> {
  await ensurePool(chainId);
  const endpoints = getSortedEndpoints(chainId, 'rpc');
  return endpoints[0]?.url ?? null;
}

/**
 * Probe a single RPC URL with `eth_chainId` and return the chain id it reports
 * (decimal), or `null` if it's unreachable / not a valid JSON-RPC endpoint.
 * Used to validate a user-entered RPC before saving it as an override. Probes
 * over HTTP because the pool only ever calls endpoints via fetch — a `wss://`
 * URL that "works" here still couldn't be used by the pool.
 */
export async function probeRpcChainId(url: string, timeoutMs = 8_000): Promise<number | null> {
  if (!url) return null;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
      signal: controller.signal,
    });
    if (!res.ok) return null;
    const json = await res.json();
    if (typeof json?.result !== 'string') return null;
    const id = parseInt(json.result, 16);
    return Number.isFinite(id) && id > 0 ? id : null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/** Force refresh the endpoint pool for a chain. */
export async function refreshPool(chainId: number): Promise<void> {
  poolInitAt.delete(chainId);
  // Drop the cached "fastest RPC" winner — it may point at the endpoint the user
  // just replaced, and it's handed to the bundler via X-Rpc-Url for up to an hour.
  fastestRpcCache.delete(chainId);
  await initPool(chainId);
}

/** Invalidate all pools so they re-read config on next use. */
export function invalidateAllPools(): void {
  poolInitAt.clear();
}
