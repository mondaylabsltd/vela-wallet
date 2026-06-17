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
import { fetchChainTokens } from '@/services/chain-tokens';
import { loadCustomTokens } from '@/services/storage';

/** keccak256("Transfer(address,address,uint256)") */
const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

/**
 * Contract-address sentinels that mark a log as a *native* (ETH/gas) transfer
 * under EIP-7708. The exact magic address is still being finalised across
 * clients, so we match a small set and treat everything else as ERC-20.
 */
const NATIVE_LOG_ADDRESSES = new Set<string>([
  '0xfffffffffffffffffffffffffffffffffffffffe', // EIP-7708 native-transfer log emitter
  '0x0000000000000000000000000000000000000000',
  '0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
]);

/**
 * Incremental monitor window, in blocks: the last N blocks, polled every ~10s
 * while the user watches Activity, in a single `eth_getLogs` (probe + at most one
 * capped retry on a strict endpoint) — never a checkpoint-driven catch-up that
 * can fan a wide span out into many calls and trip public-RPC rate limits.
 *
 * Overlapping windows are fine: receipts are de-duped downstream by their stable
 * id, so there's no checkpoint to maintain. The trade-off is no catch-up after a
 * long background — anything older than the window is reflected in balances.
 */
const LIVE_SCAN_BLOCKS = 100;

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

/**
 * Scan the most-recent `lookback` blocks of one chain for incoming transfers,
 * bounded to AT MOST TWO `eth_getLogs`: probe the window, and if the endpoint
 * caps the span (e.g. Monad → "limited to a 100 range"), retry ONCE for just the
 * most-recent `cap` blocks. We never fan the window out into many chunks — that
 * is exactly what trips public-RPC rate limits. There is no checkpoint: windows
 * overlap between polls and receipts are de-duped downstream by their stable id.
 *
 * Throws on a non-range / unreachable error so the caller can decide whether to
 * surface the chain as failed.
 */
export async function scanRecentTransfers(address: string, chainId: number, lookback: number, contracts?: string[]): Promise<IncomingTransfer[]> {
  const bn = await poolRpcCall('eth_blockNumber', [], chainId);
  if (bn.error || typeof bn.result !== 'string') throw new Error('eth_blockNumber failed');
  const latest = hexToNumber(bn.result);
  if (!Number.isFinite(latest) || latest <= 0) throw new Error('invalid block number');

  let from = Math.max(0, latest - lookback);
  let res = await poolRpcCall('eth_getLogs', [transferLogsFilter(address, from, latest, contracts)], chainId);
  if (res.error) {
    const cap = getLogsRangeCap(res.error);
    if (cap === null) throw new Error(res.error.message || 'eth_getLogs failed');
    // Span cap (or result cap with no number → stay conservative): scan just the
    // most-recent `span` blocks in one more call instead of chunking the window.
    const span = cap > 0 ? cap : 100;
    from = Math.max(0, latest - (span - 1));
    res = await poolRpcCall('eth_getLogs', [transferLogsFilter(address, from, latest, contracts)], chainId);
    if (res.error) throw new Error(res.error.message || 'eth_getLogs failed');
  }

  const out = decodeTransferLogs(Array.isArray(res.result) ? res.result : [], address, chainId);
  await resolveTimestamps(out, chainId);
  return out;
}

/** Incremental monitor: the last {@link LIVE_SCAN_BLOCKS} blocks, polled while
 *  the user watches Activity. A failing chain just yields nothing this tick. */
async function scanChain(address: string, chainId: number, contracts: string[]): Promise<IncomingTransfer[]> {
  try {
    return await scanRecentTransfers(address, chainId, LIVE_SCAN_BLOCKS, contracts);
  } catch {
    return [];
  }
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
 * The Transfer-event getLogs filter (token → `address`) for a block span.
 * When `contracts` is given, the query is restricted to those emitter addresses
 * (the trusted-token allowlist), so scam/airdrop tokens that emit fake Transfer
 * events to spam the wallet are never matched — and fewer logs come back.
 */
function transferLogsFilter(address: string, from: number, to: number, contracts?: string[]) {
  const filter: Record<string, unknown> = {
    fromBlock: '0x' + from.toString(16),
    toBlock: '0x' + to.toString(16),
    topics: [TRANSFER_TOPIC, null, addressTopic(address)],
  };
  if (contracts && contracts.length) filter.address = contracts;
  return filter;
}

/**
 * The contract addresses whose Transfer logs we trust, per chain: known
 * stablecoins (per chain, from the token registry) + the user's manually-added
 * tokens + the native (EIP-7708) log sentinels. `eth_getLogs` is restricted to
 * these, so scam/airdrop tokens spamming fake Transfer events never reach the
 * feed — and the query returns far fewer logs. All lower-cased.
 *
 * Note: tokens the user holds but never added and that aren't a known stablecoin
 * are intentionally NOT watched — listening to them is exactly how spam slips in.
 */
async function transferAllowlist(chainIds: number[]): Promise<Map<number, string[]>> {
  const custom = await loadCustomTokens().catch(() => []);
  const map = new Map<number, string[]>();
  await Promise.all(chainIds.map(async (chainId) => {
    const set = new Set<string>(NATIVE_LOG_ADDRESSES);
    const data = await fetchChainTokens(chainId).catch(() => null);
    data?.stables.forEach((s) => { if (s.contract) set.add(s.contract.toLowerCase()); });
    custom.filter((t) => t.chainId === chainId).forEach((t) => {
      if (t.contractAddress) set.add(t.contractAddress.toLowerCase());
    });
    map.set(chainId, [...set]);
  }));
  return map;
}

/**
 * Fetch incoming transfers across the given chains. Each chain is scanned
 * independently (restricted to its trusted-token allowlist); a failing chain
 * yields no items rather than failing the batch. Results are newest-first.
 */
export async function fetchIncomingTransfers(address: string, chainIds: number[]): Promise<IncomingTransfer[]> {
  if (!address) return [];
  const allow = await transferAllowlist(chainIds);
  const results = await Promise.allSettled(
    chainIds.map((id) => scanChain(address, id, allow.get(id) ?? [...NATIVE_LOG_ADDRESSES])),
  );
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
