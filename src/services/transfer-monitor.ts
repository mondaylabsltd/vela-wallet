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
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import { poolRpcCall } from '@/services/rpc-pool';

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
/** Max blocks to scan for native deposits per detected balance increase. */
const MAX_NATIVE_BLOCKS = 30;

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

  const logs = await poolRpcCall('eth_getLogs', [{
    fromBlock: '0x' + from.toString(16),
    toBlock: '0x' + latest.toString(16),
    topics: [TRANSFER_TOPIC, null, addressTopic(address)],
  }], chainId);

  await advance();

  if (logs.error || !Array.isArray(logs.result)) return [];

  const out: IncomingTransfer[] = [];
  for (const log of logs.result) {
    try {
      // Standard ERC-20 Transfer: topics = [sig, from, to], data = value.
      const topics: string[] = log.topics ?? [];
      if (topics.length < 3) continue;
      // Never trust the RPC's topic filter alone: only accept logs whose
      // recipient (topics[2]) is actually this wallet. A buggy/caching/malicious
      // endpoint in the failover pool could otherwise surface transfers
      // addressed to someone else as if we'd received them (fake "Received").
      if ((topics[2] ?? '').toLowerCase() !== addressTopic(address)) continue;
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

  // Resolve block timestamps for the matched logs (distinct blocks, capped).
  const blocks = [...new Set(out.map((o) => o.blockNumber))].slice(0, 25);
  const tsByBlock = new Map<number, number>();
  await Promise.allSettled(blocks.map(async (b) => {
    const r = await poolRpcCall('eth_getBlockByNumber', ['0x' + b.toString(16), false], chainId);
    if (!r.error && r.result?.timestamp) tsByBlock.set(b, hexToNumber(r.result.timestamp));
  }));
  const nowSec = Math.floor(Date.now() / 1000);
  for (const o of out) o.timestamp = tsByBlock.get(o.blockNumber) ?? nowSec;

  // Native (non-EIP-7708) incoming fallback.
  try {
    const native = await scanNativeIncoming(address, chainId, latest);
    out.push(...native);
  } catch {
    // best effort
  }

  return out;
}

/**
 * Detect incoming *native* transfers on chains without EIP-7708 (i.e. all of
 * them today). Native sends emit no log, so we use a balance-diff trigger: a
 * cheap `eth_getBalance` each poll, and only when the balance *increases* do we
 * scan the new blocks' transactions for value transfers to the wallet. The first
 * run only records a baseline (no history) so we "listen from app open".
 */
async function scanNativeIncoming(address: string, chainId: number, latest: number): Promise<IncomingTransfer[]> {
  const lc = address.toLowerCase();
  const balKey = `vela.nativeBal.${chainId}.${lc}`;
  const blkKey = `vela.nativeBlk.${chainId}.${lc}`;

  const balRes = await poolRpcCall('eth_getBalance', [address, 'latest'], chainId);
  if (balRes.error || typeof balRes.result !== 'string') return [];
  const curBal = BigInt(balRes.result);

  const [prevBalRaw, prevBlkRaw] = await Promise.all([
    AsyncStorage.getItem(balKey).catch(() => null),
    AsyncStorage.getItem(blkKey).catch(() => null),
  ]);
  const persist = () => Promise.all([
    AsyncStorage.setItem(balKey, curBal.toString()).catch(() => {}),
    AsyncStorage.setItem(blkKey, String(latest)).catch(() => {}),
  ]);

  if (prevBalRaw == null) { await persist(); return []; } // baseline only

  const prevBal = BigInt(prevBalRaw);
  const prevBlk = prevBlkRaw ? parseInt(prevBlkRaw, 10) : latest - 1;
  if (curBal <= prevBal) { await persist(); return []; } // no deposit

  const from = Math.max(prevBlk + 1, latest - MAX_NATIVE_BLOCKS + 1);
  const blockNums: number[] = [];
  for (let b = from; b <= latest; b++) blockNums.push(b);

  const blocks = await Promise.allSettled(
    blockNums.map((b) => poolRpcCall('eth_getBlockByNumber', ['0x' + b.toString(16), true], chainId)),
  );

  const found: IncomingTransfer[] = [];
  for (const r of blocks) {
    if (r.status !== 'fulfilled' || r.value.error) continue;
    const block = r.value.result;
    const txs = block?.transactions;
    if (!Array.isArray(txs)) continue;
    const ts = hexToNumber(block.timestamp ?? '0x0') || Math.floor(Date.now() / 1000);
    const blockNumber = hexToNumber(block.number ?? '0x0');
    for (const t of txs) {
      if (!t || !t.to || String(t.to).toLowerCase() !== lc) continue;
      const value = BigInt(t.value && t.value !== '0x' ? t.value : '0x0');
      if (value === 0n) continue;
      found.push({
        id: `${chainId}-${t.hash}-native`,
        chainId,
        token: null,
        isNative: true,
        from: String(t.from).toLowerCase(),
        value,
        txHash: t.hash,
        blockNumber,
        logIndex: 0,
        timestamp: ts,
      });
    }
  }

  await persist();
  return found;
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
  await Promise.all(chainIds.map((id) => AsyncStorage.removeItem(lastScanKey(id, address)).catch(() => {})));
}
