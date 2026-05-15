/**
 * 7-day historical balance for a token.
 *
 * Estimates block numbers for each day's local midnight using actual
 * on-chain timestamps, then queries historical balances.
 *
 * Archive RPC discovery: tries all endpoints in the RPC pool for each chain,
 * remembers which ones support archive queries, and uses those for subsequent calls.
 */

import { rpcCall } from './rpc-adapter';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface BalancePoint {
  /** Local date string, e.g. "May 14" */
  label: string;
  /** Balance as a float (human-readable units, not wei). -1 = no data available. */
  balance: number;
}

// ---------------------------------------------------------------------------
// Archive RPC discovery + cache
// ---------------------------------------------------------------------------

/** Cache: chainId → archive-capable RPC URL (null = no archive RPC found) */
const archiveRpcCache = new Map<number, string | null>();

/**
 * Direct JSON-RPC call to a specific URL (bypass rpc-pool).
 */
async function directRpcCall(
  url: string,
  method: string,
  params: any[],
): Promise<{ result?: any; error?: any }> {
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  return res.json();
}

/**
 * Find an RPC endpoint that supports archive queries for this chain.
 * Tests by querying eth_getBalance at a block ~1 day old.
 * Result is cached permanently (archive support doesn't change).
 */
async function findArchiveRpc(chainId: number, testAddress: string, oldBlockHex: string): Promise<string | null> {
  if (archiveRpcCache.has(chainId)) return archiveRpcCache.get(chainId) ?? null;

  // Get all RPC URLs for this chain from the pool
  const urls = await getChainRpcUrls(chainId);

  for (const url of urls) {
    try {
      const res = await directRpcCall(url, 'eth_getBalance', [testAddress, oldBlockHex]);
      if (res.result && !res.error) {
        console.log(`[BalanceHistory] Archive RPC found for chain ${chainId}: ${url}`);
        archiveRpcCache.set(chainId, url);
        return url;
      }
    } catch { /* try next */ }
  }

  console.warn(`[BalanceHistory] No archive RPC found for chain ${chainId}`);
  archiveRpcCache.set(chainId, null);
  return null;
}

/**
 * Get all known RPC URLs for a chain by peeking into the rpc-pool internals.
 * Falls back to a few well-known public archive RPCs.
 */
async function getChainRpcUrls(chainId: number): Promise<string[]> {
  // Try to get the pool's current best URL first
  const { getChainRpcUrl } = await import('./rpc-pool');
  const poolUrl = await getChainRpcUrl(chainId);

  // Well-known RPCs that often support archive (drpc, llamarpc, etc.)
  const knownArchive: Record<number, string[]> = {
    1:     ['https://eth.drpc.org', 'https://rpc.ankr.com/eth'],
    56:    ['https://bsc.drpc.org', 'https://bsc-mainnet.public.blastapi.io'],
    137:   ['https://polygon.drpc.org', 'https://polygon-mainnet.public.blastapi.io'],
    42161: ['https://arbitrum.drpc.org'],
    10:    ['https://optimism.drpc.org'],
    8453:  ['https://base.drpc.org'],
    43114: ['https://avalanche.drpc.org'],
    100:   ['https://gnosis.drpc.org'],
  };

  const urls = new Set<string>();
  if (poolUrl) urls.add(poolUrl);
  for (const u of knownArchive[chainId] ?? []) urls.add(u);

  return [...urls];
}

// ---------------------------------------------------------------------------
// Block estimation — uses actual on-chain timestamps
// ---------------------------------------------------------------------------

async function getBlockInfo(chainId: number, blockTag: string): Promise<{ number: number; timestamp: number } | null> {
  const res = await rpcCall('eth_getBlockByNumber', [blockTag, false], chainId);
  if (res.error || !res.result) return null;
  const block = res.result as { number?: string; timestamp?: string };
  if (!block.number || !block.timestamp) return null;
  return {
    number: parseInt(block.number, 16),
    timestamp: parseInt(block.timestamp, 16),
  };
}

async function estimateBlocks(chainId: number): Promise<{
  currentBlock: number;
  currentTimestamp: number;
  avgBlockTime: number;
} | null> {
  const latest = await getBlockInfo(chainId, 'latest');
  if (!latest) return null;

  const sampleBlock = Math.max(0, latest.number - 1000);
  const sample = await getBlockInfo(chainId, '0x' + sampleBlock.toString(16));
  if (!sample) return null;

  const blockDiff = latest.number - sample.number;
  const timeDiff = latest.timestamp - sample.timestamp;
  if (blockDiff <= 0 || timeDiff <= 0) return null;

  return {
    currentBlock: latest.number,
    currentTimestamp: latest.timestamp,
    avgBlockTime: timeDiff / blockDiff,
  };
}

function blockAtTime(chain: { currentBlock: number; currentTimestamp: number; avgBlockTime: number }, targetTs: number): number {
  const secondsAgo = chain.currentTimestamp - targetTs;
  const blocksAgo = Math.floor(secondsAgo / chain.avgBlockTime);
  return Math.max(0, chain.currentBlock - blocksAgo);
}

// ---------------------------------------------------------------------------
// Balance queries (via archive RPC)
// ---------------------------------------------------------------------------

const BALANCE_OF = '0x70a08231';

function encodeBalanceOf(address: string): string {
  return BALANCE_OF + '000000000000000000000000' + address.toLowerCase().slice(2);
}

async function queryBalance(
  archiveUrl: string,
  address: string,
  tokenAddress: string | null,
  decimals: number,
  blockHex: string,
): Promise<number | null> {
  try {
    let res;
    if (!tokenAddress) {
      res = await directRpcCall(archiveUrl, 'eth_getBalance', [address, blockHex]);
    } else {
      const data = encodeBalanceOf(address);
      res = await directRpcCall(archiveUrl, 'eth_call', [{ to: tokenAddress, data }, blockHex]);
    }
    if (res.error || !res.result || res.result === '0x') return null;
    return Number(BigInt(res.result)) / Math.pow(10, decimals);
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Fetch 7-day balance history for a token.
 * Returns up to 8 data points (7 past midnights + current balance).
 * Automatically discovers archive-capable RPCs and caches the result.
 */
export async function fetch7DayHistory(params: {
  address: string;
  chainId: number;
  tokenAddress: string | null;
  decimals: number;
  currentBalance: number;
}): Promise<BalancePoint[]> {
  const { address, chainId, tokenAddress, decimals, currentBalance } = params;

  const chain = await estimateBlocks(chainId);
  if (!chain) return [];

  const now = new Date();

  // Generate midnight timestamps for the past 7 days
  const midnights: { date: Date; targetTs: number }[] = [];
  for (let i = 7; i >= 1; i--) {
    const d = new Date(now);
    d.setDate(d.getDate() - i);
    d.setHours(0, 0, 0, 0);
    midnights.push({ date: d, targetTs: Math.floor(d.getTime() / 1000) });
  }

  // Estimate a test block (~1 day ago) for archive RPC discovery
  const testBlock = blockAtTime(chain, chain.currentTimestamp - 86400);
  const testBlockHex = '0x' + testBlock.toString(16);

  // Find an archive-capable RPC for this chain
  const archiveUrl = await findArchiveRpc(chainId, address, testBlockHex);
  if (!archiveUrl) {
    // No archive RPC — return only today's balance
    const todayLabel = now.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    return [{ label: todayLabel, balance: currentBalance }];
  }

  // Query all 7 days in parallel via the archive RPC
  const queries = midnights.map(async ({ date, targetTs }): Promise<BalancePoint> => {
    const label = date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    try {
      const block = blockAtTime(chain, targetTs);
      const blockHex = '0x' + block.toString(16);

      // Verify block timestamp is within ±1 hour of target
      const blockInfo = await getBlockInfo(chainId, blockHex);
      if (!blockInfo || Math.abs(blockInfo.timestamp - targetTs) > 3600) {
        return { label, balance: -1 };
      }

      const balance = await queryBalance(archiveUrl, address, tokenAddress, decimals, blockHex);
      return { label, balance: balance ?? -1 };
    } catch {
      return { label, balance: -1 };
    }
  });

  return Promise.all(queries);
}
