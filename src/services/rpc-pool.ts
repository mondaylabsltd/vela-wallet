/**
 * RPC & Bundler endpoint pool with automatic load balancing and failover.
 *
 * Endpoints are collected from multiple sources per chain:
 *   RPC:     user-configured > built-in (ethereum-data API) > network default > public fallback
 *   Bundler: user-configured > built-in (bundler.getvela.app)
 *
 * Each endpoint tracks latency and failure stats. Calls are routed to the
 * highest-scoring endpoint first, with automatic failover on connectivity errors.
 */

import { DEFAULT_NETWORKS, getAllNetworksSync } from '@/models/network';
import { fetchChainInfo } from './chain-registry';
import { getNetworkConfig } from './storage';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface EndpointStats {
  url: string;
  source: 'user' | 'builtin' | 'default' | 'public';
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

/** Built-in bundler base URL */
const BUILTIN_BUNDLER = 'https://bundler.getvela.app';

/** Reliable public RPCs per chain. */
const PUBLIC_RPCS: Record<number, string> = {
  1:     'https://1rpc.io/eth',
  56:    'https://1rpc.io/bnb',
  137:   'https://1rpc.io/matic',
  42161: 'https://1rpc.io/arb',
  10:    'https://1rpc.io/op',
  8453:  'https://1rpc.io/base',
  43114: 'https://1rpc.io/avax/c',
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
    if (!url || seen.has(url)) return;
    seen.add(url);
    entries.push({ url, source });
  };

  // 1. User-configured
  try {
    const config = await getNetworkConfig(chainId);
    const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
    if (config?.rpcURL && config.rpcURL !== defaultNet?.rpcURL) {
      add(config.rpcURL, 'user');
    }
  } catch { /* ignore */ }

  // 2. Built-in from ethereum-data API
  try {
    const info = await fetchChainInfo(chainId);
    if (info?.rpcUrls) {
      for (const url of info.rpcUrls.slice(0, 5)) add(url, 'builtin');
    }
  } catch { /* ignore */ }

  // 3. Network default
  const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
  if (defaultNet?.rpcURL) add(defaultNet.rpcURL, 'default');

  // Custom network default
  const customNet = getAllNetworksSync().find(n => n.chainId === chainId);
  if (customNet?.rpcURL) add(customNet.rpcURL, 'default');

  // 4. Public fallback
  if (PUBLIC_RPCS[chainId]) add(PUBLIC_RPCS[chainId], 'public');

  return entries;
}

async function collectBundlerUrls(chainId: number): Promise<{ url: string; source: EndpointStats['source'] }[]> {
  const entries: { url: string; source: EndpointStats['source'] }[] = [];
  const seen = new Set<string>();
  const defaultChainIds = new Set(DEFAULT_NETWORKS.map(n => n.chainId));

  const add = (url: string, source: EndpointStats['source']) => {
    if (!url || seen.has(url)) return;
    seen.add(url);
    entries.push({ url, source });
  };

  // 1. User-configured override (from NetworkConfig editor)
  try {
    const config = await getNetworkConfig(chainId);
    if (config?.bundlerURL) {
      const defaultNet = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
      // Skip if it's the unchanged default Pimlico URL (user never intentionally set it)
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
  add(`${BUILTIN_BUNDLER}/${chainId}`, 'builtin');

  return entries;
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

const SOURCE_PRIORITY: Record<string, number> = {
  user:    10000,
  builtin: 1000,
  default: 500,
  public:  100,
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
    msg.includes('subscription')
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

async function tryEndpoint(
  url: string,
  method: string,
  params: any[],
  extraHeaders?: Record<string, string>,
): Promise<RPCResponse> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...extraHeaders },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
      signal: controller.signal,
    });

    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const json = await res.json();
    if (!json || typeof json !== 'object') throw new Error('Invalid response');
    return json as RPCResponse;
  } finally {
    clearTimeout(timer);
  }
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
): Promise<RPCResponse> {
  await ensurePool(chainId);

  const endpoints = getSortedEndpoints(chainId, 'rpc');
  console.log(`[RPC] ${method} chain=${chainId} endpoints=${endpoints.length} [${endpoints.map(e => `${e.source}:${shorten(e.url)}`).join(', ')}]`);

  for (const ep of endpoints) {
    const t0 = Date.now();
    try {
      const response = await tryEndpoint(ep.url, method, params);
      const ms = Date.now() - t0;

      // Ban endpoints that require auth/API key and failover to next
      if (response.error && isPermanentRpcError(response.error)) {
        ep.banned = true;
        recordFailure(ep);
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
      console.log(`[RPC] ${method} → ${shorten(ep.url)} ${ms}ms ${response.error ? 'ERR:' + response.error.message?.slice(0, 60) : 'OK'}`);
      return response;
    } catch (err) {
      recordFailure(ep);
      console.warn(`[RPC] ${method} → ${shorten(ep.url)} FAIL: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

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
): Promise<RPCResponse> {
  await ensurePool(chainId);

  const endpoints = getSortedEndpoints(chainId, 'bundler');
  console.log(`[Bundler] ${method} chain=${chainId} endpoints=${endpoints.length} [${endpoints.map(e => `${e.source}:${shorten(e.url)}`).join(', ')}]`);

  // Get the chain's best RPC URL — passed via X-Rpc-Url so the bundler can reach the chain
  const rpcEndpoints = getSortedEndpoints(chainId, 'rpc');
  const chainRpcUrl = rpcEndpoints[0]?.url;
  const extraHeaders = chainRpcUrl ? { 'X-Rpc-Url': chainRpcUrl } : undefined;

  for (const ep of endpoints) {
    const t0 = Date.now();
    try {
      const response = await tryEndpoint(ep.url, method, params, extraHeaders);
      const ms = Date.now() - t0;

      if (response.error && isPermanentRpcError(response.error)) {
        ep.banned = true;
        recordFailure(ep);
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
      recordFailure(ep);
      console.warn(`[Bundler] ${method} → ${shorten(ep.url)} FAIL: ${err instanceof Error ? err.message : String(err)}`);
    }
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
  const userEndpoints = pool.filter(e => e.source === 'user' && e.consecutiveFailures === 0);
  return userEndpoints.length === 0;
}

/** Get the built-in bundler base URL (for REST API calls). */
export function getBuiltinBundlerUrl(): string {
  return BUILTIN_BUNDLER;
}

/** Get the best RPC URL for a chain (for passing to bundler via X-Rpc-Url). */
export async function getChainRpcUrl(chainId: number): Promise<string | null> {
  await ensurePool(chainId);
  const endpoints = getSortedEndpoints(chainId, 'rpc');
  return endpoints[0]?.url ?? null;
}

/** Force refresh the endpoint pool for a chain. */
export async function refreshPool(chainId: number): Promise<void> {
  poolInitAt.delete(chainId);
  await initPool(chainId);
}
