/**
 * On-chain ERC-20 metadata (symbol + decimals) with a persistent cache.
 *
 * Why this exists: the received-transfer monitor only knew a token's
 * symbol/decimals when the user *already held* it (the data came from
 * `fetchTokens`). A first-time receipt of an unfamiliar token had no metadata,
 * and the old fallback assumed 18 decimals + a generic "tokens" symbol — so a
 * 6-decimal stablecoin (USDT/USDC/USD₮0) rendered as "+0 tokens".
 *
 * This reads the real values straight from the contract — batched per chain via
 * Multicall3 (`symbol()` + `decimals()`), memoised in-process, and persisted so
 * repeat scans never re-query. It reuses the existing `poolRpcCall` failover
 * pool, so no new networking. Best-effort throughout: any RPC/storage failure
 * yields a partial (or empty) map and callers keep their conservative fallback.
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import { poolRpcCall } from '@/services/rpc-pool';
import { MULTICALL3, SEL, encAggregate3, decAggregate3, decString, decU8, type Call3 } from '@/services/abi';

export interface TokenMetadata {
  symbol: string;
  decimals: number;
}

/**
 * Per-(chain,address) cache. A `null` value means "looked up and unresolvable"
 * so we don't re-query a dud contract within the session (positive results are
 * also persisted to AsyncStorage; negatives are session-only in case the miss
 * was a transient RPC failure).
 */
const memCache = new Map<string, TokenMetadata | null>();

/** Tokens per Multicall3 batch (2 sub-calls each) — keeps call data bounded. */
const BATCH = 40;

const memKey = (chainId: number, addr: string) => `${chainId}:${addr}`;
const storeKey = (chainId: number, addr: string) => `vela.tokenMeta.${chainId}.${addr}`;

function isValidDecimals(n: number): boolean {
  return Number.isInteger(n) && n >= 0 && n <= 36;
}

/**
 * Resolve `{symbol, decimals}` for the given ERC-20 contracts on one chain.
 * Returns a map keyed by lowercased address; tokens that couldn't be resolved
 * are simply absent. Never throws.
 */
export async function resolveTokenMetadata(
  chainId: number,
  addresses: string[],
): Promise<Map<string, TokenMetadata>> {
  const out = new Map<string, TokenMetadata>();
  const want = [...new Set(addresses.map((a) => a?.toLowerCase()).filter(Boolean) as string[])];
  if (want.length === 0) return out;

  // 1) In-memory cache (covers repeat scans within the session).
  const misses: string[] = [];
  for (const a of want) {
    const m = memCache.get(memKey(chainId, a));
    if (m === undefined) misses.push(a);
    else if (m) out.set(a, m);
  }
  if (misses.length === 0) return out;

  // 2) Persistent cache (covers app restarts).
  let toFetch = misses;
  try {
    const pairs = await AsyncStorage.multiGet(misses.map((a) => storeKey(chainId, a)));
    const next: string[] = [];
    pairs.forEach(([, val], i) => {
      const a = misses[i];
      const parsed = parseStored(val);
      if (parsed) {
        memCache.set(memKey(chainId, a), parsed);
        out.set(a, parsed);
      } else {
        next.push(a);
      }
    });
    toFetch = next;
  } catch {
    /* storage unavailable → fall through and fetch every miss */
  }
  if (toFetch.length === 0) return out;

  // 3) On-chain via Multicall3, in bounded batches.
  const toPersist: [string, string][] = [];
  for (let i = 0; i < toFetch.length; i += BATCH) {
    const batch = toFetch.slice(i, i + BATCH);
    const calls: Call3[] = [];
    for (const a of batch) {
      calls.push({ target: a, allowFailure: true, callData: '0x' + SEL.symbol });
      calls.push({ target: a, allowFailure: true, callData: '0x' + SEL.decimals });
    }

    let result: string | null = null;
    try {
      const res = await poolRpcCall('eth_call', [{ to: MULTICALL3, data: encAggregate3(calls) }, 'latest'], chainId);
      if (!res.error && typeof res.result === 'string') result = res.result;
    } catch {
      /* whole batch unreachable → leave these unresolved (no negative cache) */
    }
    if (result === null) continue;

    const decoded = decAggregate3(result);
    batch.forEach((a, j) => {
      const symRes = decoded[j * 2];
      const decRes = decoded[j * 2 + 1];
      let meta: TokenMetadata | null = null;
      if (symRes?.success && decRes?.success) {
        const symbol = decString(symRes.data);
        const decimals = decU8(decRes.data);
        if (symbol && isValidDecimals(decimals)) meta = { symbol, decimals };
      }
      memCache.set(memKey(chainId, a), meta); // null memoises a dud for the session
      if (meta) {
        out.set(a, meta);
        toPersist.push([storeKey(chainId, a), JSON.stringify(meta)]);
      }
    });
  }

  if (toPersist.length) await AsyncStorage.multiSet(toPersist).catch(() => {});
  return out;
}

function parseStored(val: string | null): TokenMetadata | null {
  if (!val) return null;
  try {
    const p = JSON.parse(val) as TokenMetadata;
    if (p && typeof p.symbol === 'string' && p.symbol && isValidDecimals(p.decimals)) return p;
  } catch {
    /* corrupt entry → treat as miss */
  }
  return null;
}

/** Drop the in-process cache (test hook / account switch). */
export function clearTokenMetadataCache(): void {
  memCache.clear();
}
