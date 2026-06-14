/**
 * Received-transfer monitor (client-side, RPC log polling).
 *
 * Reuses the existing RPC pool (`poolRpcCall`) so it inherits the user's
 * configured endpoints + the ethereum-data.awesometools.dev fallback and
 * automatic failover — no new networking.
 *
 * For each chain it runs one `eth_getLogs` for the ERC-20 `Transfer` event with
 * the wallet address as the *recipient* topic. EIP-7708 makes native ETH/gas
 * transfers emit the same `Transfer` event, so the single topic-filtered query
 * captures both ERC-20 and native incoming payments (native logs are recognised
 * by a sentinel contract address). Scanning is incremental: a per-(chain,address)
 * last-scanned block is persisted so each poll only fetches new logs.
 *
 * Limitation (by design): on chains WITHOUT EIP-7708 — i.e. all of them today — a
 * plain native send emits no log, so it can't be discovered this way. We do NOT
 * try to infer it from balance diffs: that heuristic silently missed internal-call
 * deposits (CEX withdrawals, disperse/multisig/router sends), missed fast-chain
 * windows, was masked by concurrent gas spend, and produced inconsistent results.
 * The received amount is still reflected correctly in the balance (that comes from
 * `eth_getBalance` in the asset list); only the Activity *row* is absent on those
 * chains, and the UI points the user at the block explorer for full native history.
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import { poolRpcCall, getLogsRangeCap } from '@/services/rpc-pool';

/** keccak256("Transfer(address,address,uint256)") */
const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

/**
 * Contract-address sentinels that mark a log as a *native* (ETH/gas) transfer
 * under EIP-7708. The exact magic address is still being finalised across
 * clients, so we match a small set and treat everything else as ERC-20.
 */
const NATIVE_LOG_ADDRESSES = new Set<string>([
  '0x0000000000000000000000000000000000000000',
  '0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
]);

/** Block window for the very first scan (no stored checkpoint yet). */
const INITIAL_LOOKBACK = 3000;
/** Max block span per query — keeps us under public-RPC getLogs limits. */
const MAX_RANGE = 3000;

/**
 * Approx block time per chain (ms), used to translate a re-scan time window into
 * a block count. Deliberately on the low side so we cover *at least* the asked
 * window (an extra chunk or two is cheap; missing a recent payment is not).
 */
const BLOCK_TIME_MS: Record<number, number> = {
  1: 12_000, 56: 1_500, 137: 2_000, 42161: 250, 10: 2_000, 8453: 2_000, 43114: 2_000, 100: 5_000,
};
const DEFAULT_BLOCK_TIME_MS = 2_500;
/**
 * Cap on getLogs chunks per chain for a deep re-scan, so a long window on a
 * fast chain can't fan out into hundreds of calls and trip rate limits. When a
 * window exceeds this, we keep the most *recent* portion (what "I missed a
 * payment" needs) and drop the oldest.
 */
const MAX_RESCAN_CHUNKS = 25;

export interface DeepScanProgress {
  chainId: number;
  chunk: number;
  totalChunks: number;
}

export interface IncomingTransfer {
  /** Stable id: `${chainId}-${txHash}-${logIndex}`. */
  id: string;
  chainId: number;
  /** Token contract address (lowercased), or null for native. */
  token: string | null;
  isNative: boolean;
  from: string;
  /** Raw on-chain amount (not yet divided by decimals). */
  value: bigint;
  txHash: string;
  blockNumber: number;
  logIndex: number;
  /** Unix seconds (resolved from the block; falls back to now). */
  timestamp: number;
}

function lastScanKey(chainId: number, address: string): string {
  return `vela.lastScan.${chainId}.${address.toLowerCase()}`;
}

function addressTopic(address: string): string {
  return '0x' + '0'.repeat(24) + address.slice(2).toLowerCase();
}

function hexToNumber(hex: string): number {
  return parseInt(hex, 16);
}

/** Max recursion when splitting a too-wide getLogs range (1 → 2 → 4 … chunks). */
const MAX_SPLIT_DEPTH = 8;

/**
 * Fetch ERC-20 + EIP-7708 native `Transfer` logs for `[from, to]` on one chain,
 * automatically shrinking the block span when an endpoint rejects the request
 * for being too wide ("…limited to a 100 range", "block range too large", "more
 * than N results", …). The generous {@link MAX_RANGE} default is kept for fast
 * endpoints; only strict ones pay the extra round-trips. When the error states a
 * max span we split into chunks of that size; otherwise we halve and recurse.
 *
 * Throws on any *non-range* error (network down, all endpoints failed) so callers
 * can mark the chain failed — same contract as the previous single call.
 */
async function getTransferLogs(
  address: string,
  chainId: number,
  from: number,
  to: number,
  depth = 0,
): Promise<any[]> {
  if (from > to) return [];

  const res = await poolRpcCall('eth_getLogs', [{
    fromBlock: '0x' + from.toString(16),
    toBlock: '0x' + to.toString(16),
    topics: [TRANSFER_TOPIC, null, addressTopic(address)],
  }], chainId);

  if (res.error) {
    const cap = getLogsRangeCap(res.error);
    const span = to - from + 1;
    // Not a range error, can't shrink further, or too deep → surface it.
    if (cap === null || span <= 1 || depth >= MAX_SPLIT_DEPTH) {
      throw new Error(res.error.message || 'eth_getLogs failed');
    }
    // Split by the server's stated cap when given (guaranteeing real shrinkage),
    // else halve. Recurse so a still-too-wide chunk shrinks again.
    const chunk = Math.max(1, cap > 0 ? Math.min(cap, span - 1) : Math.floor(span / 2));
    const out: any[] = [];
    for (let lo = from; lo <= to; lo += chunk) {
      const hi = Math.min(lo + chunk - 1, to);
      out.push(...await getTransferLogs(address, chainId, lo, hi, depth + 1));
    }
    return out;
  }

  return Array.isArray(res.result) ? res.result : [];
}

/** Scan one chain for incoming transfers since the last checkpoint. */
async function scanChain(address: string, chainId: number): Promise<IncomingTransfer[]> {
  // Latest block (anchor + checkpoint).
  const bn = await poolRpcCall('eth_blockNumber', [], chainId);
  if (bn.error || typeof bn.result !== 'string') return [];
  const latest = hexToNumber(bn.result);
  if (!Number.isFinite(latest) || latest <= 0) return [];

  const storedRaw = await AsyncStorage.getItem(lastScanKey(chainId, address)).catch(() => null);
  const stored = storedRaw ? parseInt(storedRaw, 10) : null;
  const from = stored != null
    ? Math.max(stored + 1, latest - MAX_RANGE)
    : Math.max(0, latest - INITIAL_LOOKBACK);

  // Always advance the checkpoint, even when nothing new is found.
  const advance = () => AsyncStorage.setItem(lastScanKey(chainId, address), String(latest)).catch(() => {});

  if (from > latest) { await advance(); return []; }

  let rawLogs: any[];
  try {
    rawLogs = await getTransferLogs(address, chainId, from, latest);
  } catch {
    // Endpoint(s) unreachable — advance anyway (incremental poll retries next tick).
    await advance();
    return [];
  }

  await advance();

  const out = decodeTransferLogs(rawLogs, address, chainId);
  await resolveTimestamps(out, chainId);
  return out;
}

/**
 * Decode raw `eth_getLogs` Transfer entries into IncomingTransfer records.
 * Never trusts the RPC's topic filter alone: only accepts logs whose recipient
 * (topics[2]) is actually this wallet, so a buggy/caching/malicious endpoint in
 * the failover pool can't surface someone else's transfer as a fake "Received".
 */
function decodeTransferLogs(rawLogs: any[], address: string, chainId: number): IncomingTransfer[] {
  const want = addressTopic(address);
  const out: IncomingTransfer[] = [];
  for (const log of rawLogs) {
    try {
      // Standard ERC-20 Transfer: topics = [sig, from, to], data = value.
      const topics: string[] = log.topics ?? [];
      if (topics.length < 3) continue;
      if ((topics[2] ?? '').toLowerCase() !== want) continue;
      const fromAddr = '0x' + topics[1].slice(26);
      const value = BigInt(log.data && log.data !== '0x' ? log.data : '0x0');
      if (value === 0n) continue;
      const contract = String(log.address).toLowerCase();
      const isNative = NATIVE_LOG_ADDRESSES.has(contract);
      const logIndex = hexToNumber(log.logIndex ?? '0x0');
      out.push({
        id: `${chainId}-${log.transactionHash}-${logIndex}`,
        chainId,
        token: isNative ? null : contract,
        isNative,
        from: fromAddr,
        value,
        txHash: log.transactionHash,
        blockNumber: hexToNumber(log.blockNumber ?? '0x0'),
        logIndex,
        timestamp: 0,
      });
    } catch {
      // skip malformed log
    }
  }
  return out;
}

/** Resolve block timestamps for matched logs (distinct blocks, capped). */
async function resolveTimestamps(transfers: IncomingTransfer[], chainId: number): Promise<void> {
  const blocks = [...new Set(transfers.map((o) => o.blockNumber))].slice(0, 25);
  const tsByBlock = new Map<number, number>();
  await Promise.allSettled(blocks.map(async (b) => {
    const r = await poolRpcCall('eth_getBlockByNumber', ['0x' + b.toString(16), false], chainId);
    if (!r.error && r.result?.timestamp) tsByBlock.set(b, hexToNumber(r.result.timestamp));
  }));
  const nowSec = Math.floor(Date.now() / 1000);
  for (const o of transfers) o.timestamp = tsByBlock.get(o.blockNumber) ?? nowSec;
}

/**
 * Deep historical re-scan of a time window on one chain (chunked `eth_getLogs`).
 * Unlike the incremental `scanChain`, this ignores the checkpoint and queries
 * back `minutes` worth of blocks, so a user who missed a payment can pull recent
 * transfers on demand. Throws if the chain is unreachable or a chunk errors, so
 * the caller can report that chain as failed (→ explorer / fix-RPC fallback).
 *
 * Note: only covers log-emitting transfers (ERC-20 + EIP-7708 native). Plain
 * native sends emit no log, so those remain explorer-only.
 */
export async function deepScanChain(
  address: string,
  chainId: number,
  minutes: number,
  onProgress?: (p: DeepScanProgress) => void,
): Promise<IncomingTransfer[]> {
  const bn = await poolRpcCall('eth_blockNumber', [], chainId);
  if (bn.error || typeof bn.result !== 'string') throw new Error('eth_blockNumber failed');
  const latest = hexToNumber(bn.result);
  if (!Number.isFinite(latest) || latest <= 0) throw new Error('invalid block number');

  const blockTime = BLOCK_TIME_MS[chainId] ?? DEFAULT_BLOCK_TIME_MS;
  const wanted = Math.ceil((minutes * 60 * 1000) / blockTime);
  const capped = Math.min(wanted, MAX_RESCAN_CHUNKS * (MAX_RANGE + 1));
  const from = Math.max(0, latest - capped);

  // Split [from, latest] into <= MAX_RANGE chunks (most-recent kept on cap).
  const ranges: [number, number][] = [];
  for (let start = from; start <= latest; start += MAX_RANGE + 1) {
    ranges.push([start, Math.min(start + MAX_RANGE, latest)]);
  }

  const out: IncomingTransfer[] = [];
  let done = 0;
  for (const [lo, hi] of ranges) {
    // Throws on a hard error → caller marks the chain failed (explorer fallback).
    // A too-wide chunk is split transparently, so progress stays MAX_RANGE-grained.
    const chunkLogs = await getTransferLogs(address, chainId, lo, hi);
    out.push(...decodeTransferLogs(chunkLogs, address, chainId));
    onProgress?.({ chainId, chunk: ++done, totalChunks: ranges.length });
  }

  await resolveTimestamps(out, chainId);
  return out;
}

/**
 * Fetch incoming transfers across the given chains. Each chain is scanned
 * independently; a failing chain yields no items rather than failing the batch.
 * Results are newest-first.
 */
export async function fetchIncomingTransfers(address: string, chainIds: number[]): Promise<IncomingTransfer[]> {
  if (!address) return [];
  const results = await Promise.allSettled(chainIds.map((id) => scanChain(address, id)));
  const all: IncomingTransfer[] = [];
  for (const r of results) {
    if (r.status === 'fulfilled') all.push(...r.value);
  }
  return all.sort((a, b) => b.blockNumber - a.blockNumber || b.logIndex - a.logIndex);
}

/** Clear all scan checkpoints for an address (e.g. on account switch / sign-out). */
export async function resetTransferCheckpoints(address: string, chainIds: number[]): Promise<void> {
  const lc = address.toLowerCase();
  await Promise.all(chainIds.flatMap((id) => [
    AsyncStorage.removeItem(lastScanKey(id, address)).catch(() => {}),
    // Also clear keys written by the now-removed native balance-diff fallback.
    AsyncStorage.removeItem(`vela.nativeBal.${id}.${lc}`).catch(() => {}),
    AsyncStorage.removeItem(`vela.nativeBlk.${id}.${lc}`).catch(() => {}),
  ]));
}
