/**
 * Activity feed model + adapters.
 *
 * Payment-first split:
 *   - Activity  = value transfers (received / sent). The home's hero feed.
 *   - Connection events = dApp signing / tx-request events (shown under Connections).
 *
 * The feed reads from the LOCAL transaction store, which is the durable source of
 * truth. Outgoing transfers are written by the Send flow;
 * incoming transfers are discovered by the RPC transfer-monitor and PERSISTED
 * here (`syncReceivedTransfers`) so they survive the monitor's incremental block
 * checkpoint advancing past them.
 */

import { loadTransactions, mergeTransactions, type LocalTransaction } from '@/services/storage';
import { chainName, nativeSymbol } from '@/models/network';
import { shortAddress } from '@/models/wallet-state';
import { tokenChainId, tokenLogoURLsByAddress, nativeLogoURLs, type APIToken } from '@/models/types';
import { fetchTokens } from '@/services/wallet-api';
import { fetchIncomingTransfers, type IncomingTransfer } from '@/services/transfer-monitor';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { formatTokenAmount, formatDate } from '@/services/locale-format';
import i18n from '@/i18n';

export interface ActivityItem {
  id: string;
  direction: 'in' | 'out';
  /** Action only — "Received" / "Sent" (token lives in `amount`). */
  title: string;
  /** Counterparty only — e.g. "from 0xd4…ec9a" (network shown via the chain badge). */
  subtitle: string;
  /** Signed amount with token, e.g. "+1 USDT" / "-45 USDT". */
  amount: string;
  /** USD value, pre-formatted, e.g. "$1.00" ("$0.00" when unknown). */
  usd: string;
  /** USD value as a number (0 when unknown) — used to convert to the display currency. */
  usdValue: number;
  token: string;
  chainId: number;
  timestamp: number;
  txHash?: string;
  /** Counterparty address (sender for received, recipient for sent). */
  address?: string;
  /** Resolved name for `address` (local/ENS/Vela/etc.), if known. */
  alias?: string;
  /**
   * Set when this row collapses a batch send (split = 1 token → N recipients,
   * multiSelect = N tokens → 1 recipient) that shared one UserOp. Carries the
   * per-line breakdown so the row + detail sheet can show every transfer instead
   * of one (misleading) line. Undefined for plain single sends/receives.
   */
  batch?: ActivityBatch;
}

/** One line in a batch send breakdown (mirror of a stored per-line record). */
export interface ActivityBatchTransfer {
  to: string;
  toName?: string;
  value: string;
  symbol: string;
  decimals: number;
  usdValue: number;
  logoUrls?: string[];
}

/** A batch send (split / multiSelect) summarized from its per-line records. */
export interface ActivityBatch {
  kind: 'split' | 'multiSelect';
  count: number;
  totalUsd: number;
  transfers: ActivityBatchTransfer[];
  /** Stored record ids of the siblings — for live status reconciliation. */
  ids: string[];
  // Shared header fields (identical across the siblings) for the detail sheet.
  from: string;
  chainId: number;
  timestamp: number;
  status: LocalTransaction['status'];
  txHash: string;
  userOpHash: string;
  /** split only: the single token symbol + its logo. */
  symbol?: string;
  logoUrls?: string[];
  /** multiSelect only: the single recipient. */
  to?: string;
  toName?: string;
}

export interface ConnectionEvent {
  id: string;
  /** "Signature request" | "Token approval" | "dApp Transaction" … */
  label: string;
  subtitle: string;
  timestamp: number;
  /** Lifecycle of the underlying record — drives the pending/failed row badge. */
  status: LocalTransaction['status'];
  /** Source record — powers the tap-through detail sheet. */
  tx: LocalTransaction;
}

/**
 * Token amount for the (glanceable) feed: large balances abbreviate
 * (12,345,678 → "12.3M") and the fraction is capped at 4 digits. The detail
 * sheet shows the exact value — "glance = compact, detail = exact".
 */
function compactAmount(n: number): string {
  return formatTokenAmount(n, { compact: true });
}

/** Compact relative time: "now", "2m", "3h", "Mon", "Jun 3". */
export function relativeTime(tsSeconds: number, nowMs: number = Date.now()): string {
  const diff = Math.max(0, Math.floor(nowMs / 1000) - tsSeconds);
  // Localized via the i18n singleton (relativeTime is called at render, so it
  // re-evaluates with the current language when the user switches).
  if (diff < 45) return i18n.t('time.now');
  if (diff < 3600) return i18n.t('time.minutesShort', { n: Math.round(diff / 60) });
  if (diff < 86400) return i18n.t('time.hoursShort', { n: Math.round(diff / 3600) });
  const d = new Date(tsSeconds * 1000);
  if (diff < 7 * 86400) {
    // Short weekday in the active language; fall back to the locale date preset.
    try { return d.toLocaleDateString(i18n.language, { weekday: 'short' }); } catch { return formatDate(d); }
  }
  return formatDate(d);
}

/** Local-midnight ms for a timestamp — the key that groups feed items by day. */
export function dayStartMs(tsSeconds: number): number {
  const d = new Date(tsSeconds * 1000);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Date-group header for the activity feed: "Today" / "Yesterday", else the
 *  user's date preset ("04/07/2026"). Localized + re-evaluated at render. */
export function dayGroupLabel(tsSeconds: number, nowMs: number = Date.now()): string {
  const now = new Date(nowMs);
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const diffDays = Math.round((todayStart - dayStartMs(tsSeconds)) / 86_400_000);
  if (diffDays <= 0) return i18n.t('time.today');
  if (diffDays === 1) return i18n.t('time.yesterday');
  return formatDate(tsSeconds * 1000);
}

/** Format a USD number as "$1.00"; "$0.00" for unknown/zero. */
function formatUsd(n: number): string {
  if (!isFinite(n) || n <= 0) return '$0.00';
  return '$' + n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/** Symbols treated as ≈ $1 so stablecoin transfers aren't shown as "$0.00". */
const STABLE_SYMBOLS = new Set([
  'USDT', 'USDT0', 'USDC', 'USDC.E', 'DAI', 'BUSD', 'TUSD', 'FDUSD', 'USDE', 'PYUSD', 'USDP', 'GUSD', 'LUSD', 'FRAX', 'USDD',
]);

/**
 * Normalise a symbol for stablecoin matching: upper-case and fold the Tether
 * glyph "₮" to "T" so the on-chain symbol "USD₮0" matches "USDT0".
 */
function stableKey(symbol: string): string {
  return (symbol || '').toUpperCase().replace(/₮/g, 'T');
}

/** True if `symbol` denotes a ≈ $1 stablecoin. */
export function isStable(symbol: string): boolean {
  return STABLE_SYMBOLS.has(stableKey(symbol));
}

/**
 * Numeric USD value for a transaction. Prefers the value stored at event time;
 * if that's missing/zero but the token is a known stablecoin, falls back to the
 * token amount (≈ $1 each) so e.g. a received USDT shows a real fiat value.
 * Exported so the detail sheet can convert into the display currency too.
 */
export function txUsdValue(tx: LocalTransaction): number {
  const stored = tx.usd ? parseFloat(tx.usd.replace(/[^0-9.]/g, '')) : 0;
  if (isFinite(stored) && stored > 0) return stored;
  if (isStable(tx.symbol || '')) {
    const amt = parseFloat(tx.value || '0');
    return isFinite(amt) && amt > 0 ? amt : 0;
  }
  return 0;
}

/** Map a local "send"-type transaction into an outgoing ActivityItem. */
function sendTxToActivity(tx: LocalTransaction): ActivityItem {
  const amt = compactAmount(parseFloat(tx.value || '0'));
  return {
    id: tx.id,
    direction: 'out',
    title: 'Sent',
    subtitle: `to ${shortAddress(tx.to)}`,
    amount: `-${amt} ${tx.symbol}`,
    usd: tx.usd ?? '$0.00',
    usdValue: txUsdValue(tx),
    token: tx.symbol,
    chainId: tx.chainId,
    timestamp: tx.timestamp,
    txHash: tx.txHash || undefined,
    address: tx.to,
    alias: tx.toName,
  };
}

/**
 * Classify + summarize a group of 'send' records that shared one UserOp.
 *   split       — one token, many recipients (一币多人)
 *   multiSelect — many tokens, one recipient (多币一人)
 * The siblings share from/chainId/timestamp/status/txHash (written together and
 * patched together), so the first one carries the header fields.
 */
export function buildBatchView(group: LocalTransaction[]): ActivityBatch {
  const first = group[0];
  const symbols = new Set(group.map((g) => g.symbol));
  const recipients = new Set(group.map((g) => g.to.toLowerCase()));
  const kind: 'split' | 'multiSelect' =
    symbols.size <= 1 && recipients.size > 1 ? 'split'
    : recipients.size <= 1 && symbols.size > 1 ? 'multiSelect'
    : symbols.size <= 1 ? 'split' : 'multiSelect';
  const totalUsd = group.reduce((s, g) => s + txUsdValue(g), 0);
  return {
    kind,
    count: group.length,
    totalUsd,
    transfers: group.map((g) => ({ to: g.to, toName: g.toName, value: g.value, symbol: g.symbol, decimals: g.decimals, usdValue: txUsdValue(g), logoUrls: g.logoUrls })),
    ids: group.map((g) => g.id),
    from: first.from,
    chainId: first.chainId,
    timestamp: first.timestamp,
    status: first.status,
    txHash: first.txHash,
    userOpHash: first.userOpHash,
    symbol: kind === 'split' ? first.symbol : undefined,
    logoUrls: kind === 'split' ? first.logoUrls : undefined,
    to: kind === 'multiSelect' ? first.to : undefined,
    toName: kind === 'multiSelect' ? first.toName : undefined,
  };
}

/** Map a batch send group into one ActivityItem with a per-line breakdown. */
function batchSendToActivity(group: LocalTransaction[]): ActivityItem {
  const b = buildBatchView(group);
  // split sums to one token figure ("-30 USDC"); multiSelect can't sum mixed
  // tokens, so the figure is the asset count and the fiat total leads instead.
  const amount = b.kind === 'split'
    ? `-${compactAmount(b.transfers.reduce((s, x) => s + (parseFloat(x.value) || 0), 0))} ${b.symbol ?? ''}`
    : i18n.t('componentsTx.receipt.assetsCount', { n: b.count });
  return {
    id: b.userOpHash || group[0].id,
    direction: 'out',
    title: 'Sent',
    // Used by the home row only when there's no single recipient (split). The
    // multiSelect row uses `address` to render "to <recipient>".
    subtitle: i18n.t('componentsTx.receipt.recipientsCount', { n: b.count }),
    amount,
    usd: formatUsd(b.totalUsd),
    usdValue: b.totalUsd,
    token: b.symbol ?? '',
    chainId: b.chainId,
    timestamp: b.timestamp,
    txHash: b.txHash || undefined,
    address: b.kind === 'multiSelect' ? b.to : undefined,
    alias: b.kind === 'multiSelect' ? b.toName : undefined,
    batch: b,
  };
}

interface TokenMeta { symbol: string; decimals: number; priceUsd: number | null }

/** Index the user's known tokens by `${chainId}:${contract}` (or `:native`). */
function buildTokenIndex(tokens: APIToken[]): Map<string, TokenMeta> {
  const m = new Map<string, TokenMeta>();
  for (const t of tokens) {
    const cid = tokenChainId(t);
    const key = t.tokenAddress ? `${cid}:${t.tokenAddress.toLowerCase()}` : `${cid}:native`;
    m.set(key, { symbol: t.symbol, decimals: t.decimals, priceUsd: t.priceUsd });
  }
  return m;
}

/** Map a discovered incoming transfer into a persistable 'receive' record. */
function incomingToRecord(tx: IncomingTransfer, address: string, index: Map<string, TokenMeta>): LocalTransaction {
  const key = tx.isNative ? `${tx.chainId}:native` : `${tx.chainId}:${(tx.token ?? '').toLowerCase()}`;
  const meta = index.get(key);
  const symbol = meta?.symbol ?? (tx.isNative ? nativeSymbol(tx.chainId) : 'tokens');
  const decimals = meta?.decimals ?? 18;
  const amount = Number(tx.value) / 10 ** decimals;
  // Prefer a real price; otherwise treat ≈$1 stablecoins as their token amount
  // (covers on-chain-resolved tokens with no price feed), else unknown ($0.00).
  const usd = meta?.priceUsd != null
    ? formatUsd(amount * meta.priceUsd)
    : isStable(symbol)
      ? formatUsd(amount)
      : '$0.00';
  // Capture the token's logo URLs now, while we still hold the contract address
  // (tx.token) — the persisted record keeps only `symbol`, so the detail sheet
  // can't reconstruct an ERC-20 logo later. Mirrors what the send path stores,
  // so a received token shows the same logo as when it's sent (not a letter).
  const logoUrls = tx.isNative
    ? nativeLogoURLs(tx.chainId, symbol)
    : tx.token
      ? tokenLogoURLsByAddress(tx.chainId, tx.token)
      : [];
  return {
    id: tx.id,
    userOpHash: '',
    txHash: tx.txHash,
    from: tx.from,
    to: address,
    value: String(amount),
    symbol,
    decimals,
    ...(logoUrls.length ? { logoUrls } : {}),
    chainId: tx.chainId,
    timestamp: tx.timestamp,
    status: 'confirmed',
    type: 'receive',
    usd,
  };
}

/**
 * Fill the lookup with metadata for incoming tokens the user doesn't already
 * hold. Without this, an unknown token falls back to 18 decimals + "tokens" in
 * {@link incomingToRecord}, so a 6-decimal stablecoin shows as "+0 tokens".
 * Resolves real symbol/decimals on-chain (batched per chain, cached) and folds
 * them into `index`. Best-effort: unresolved tokens keep the conservative
 * fallback, so a metadata-RPC hiccup never blocks persisting the receipt.
 */
async function enrichTokenIndex(
  incoming: IncomingTransfer[],
  index: Map<string, TokenMeta>,
): Promise<void> {
  const byChain = new Map<number, Set<string>>();
  for (const tx of incoming) {
    if (tx.isNative || !tx.token) continue;
    const addr = tx.token.toLowerCase();
    if (index.has(`${tx.chainId}:${addr}`)) continue;
    let set = byChain.get(tx.chainId);
    if (!set) byChain.set(tx.chainId, (set = new Set()));
    set.add(addr);
  }
  if (byChain.size === 0) return;

  await Promise.all([...byChain].map(async ([chainId, addrs]) => {
    const metas = await resolveTokenMetadata(chainId, [...addrs]).catch((e) => {
      // Distinguish a transient metadata-RPC outage (these tokens will be retried
      // next sync) from genuine unreadable spam — without a log the two look
      // identical and an incoming stablecoin can vanish from Activity silently.
      console.warn(
        `[Activity] token metadata unresolved for ${addrs.size} token(s) on chain ${chainId} ` +
        `(${e instanceof Error ? e.message : String(e)}) — kept out of feed, will retry next sync`,
      );
      return null;
    });
    if (!metas) return;
    for (const [addr, meta] of metas) {
      index.set(`${chainId}:${addr}`, { symbol: meta.symbol, decimals: meta.decimals, priceUsd: null });
    }
    const stillUnresolved = [...addrs].filter((a) => !index.has(`${chainId}:${a}`));
    if (stillUnresolved.length) {
      console.warn(
        `[Activity] ${stillUnresolved.length} incoming token(s) on chain ${chainId} returned no ` +
        `readable symbol/decimals — skipped this sync (retried next sync)`,
      );
    }
  }));
}

/** Map a stored 'receive' record into an incoming ActivityItem. */
function receiveRecordToActivity(tx: LocalTransaction): ActivityItem {
  const amt = compactAmount(parseFloat(tx.value || '0'));
  return {
    id: tx.id,
    direction: 'in',
    title: 'Received',
    subtitle: `from ${shortAddress(tx.from)}`,
    amount: `+${amt} ${tx.symbol}`,
    usd: tx.usd ?? '$0.00',
    usdValue: txUsdValue(tx),
    token: tx.symbol,
    chainId: tx.chainId,
    timestamp: tx.timestamp,
    txHash: tx.txHash || undefined,
    address: tx.from,
  };
}

/**
 * Discover new received transfers via the RPC monitor and persist them to the
 * local store (de-duped). Best-effort: any failure is swallowed so the cached
 * feed still renders. Returns the number of new receipts persisted.
 */
/** Main payment chains to monitor when the wallet has no balances yet. */
const DEFAULT_MONITOR_CHAINS = [1, 56, 137, 42161, 8453, 100];

export async function syncReceivedTransfers(address: string): Promise<number> {
  if (!address) return 0;
  try {
    // Only monitor chains the wallet actually uses (has a balance on) to keep
    // RPC load low and avoid rate limits; fall back to the main payment chains
    // for a brand-new wallet so its first receipt is still caught.
    const tokens = await fetchTokens(address).catch(() => [] as APIToken[]);
    const active = [...new Set(tokens.map(tokenChainId))];
    const chainIds = active.length ? active : DEFAULT_MONITOR_CHAINS;
    const incoming = await fetchIncomingTransfers(address, chainIds);
    if (incoming.length === 0) return 0;
    const index = buildTokenIndex(tokens);
    await enrichTokenIndex(incoming, index);
    // Skip non-native tokens whose metadata we couldn't resolve: persisting them
    // would store a misleading "+0 tokens" (the 18-decimal fallback zeroes out a
    // real 6-decimal amount). They remain in the recent-blocks scan window and
    // are retried next sync once metadata resolves; genuine spam with no readable
    // symbol/decimals simply never clutters the feed.
    const records = incoming
      .filter((tx) => tx.isNative || index.has(`${tx.chainId}:${(tx.token ?? '').toLowerCase()}`))
      .map((tx) => incomingToRecord(tx, address, index));
    if (records.length === 0) return 0;
    return await mergeTransactions(records);
  } catch {
    return 0;
  }
}

/**
 * Load the value-transfer Activity feed from the local store (sent + received).
 * Pure read — call `syncReceivedTransfers` first (separately) to discover and
 * persist new receipts, so the caller can react to the returned new-count.
 */
export async function loadActivityItems(address: string): Promise<ActivityItem[]> {
  if (!address) return [];

  const lc = address.toLowerCase();
  const txs = await loadTransactions().catch(() => [] as LocalTransaction[]);

  // Batch sends write one record per line, each with a DISTINCT id (`<hash>-<i>`)
  // but a shared userOpHash. Group those siblings so a split/multiSelect shows as
  // ONE row with a breakdown instead of N look-alike rows. Dedupe by id first so a
  // legacy same-id duplicate (a resubmitted single send) is NOT mistaken for a
  // batch — only genuinely distinct lines count toward the group size.
  const sendGroups = new Map<string, LocalTransaction[]>();
  const groupSeenIds = new Map<string, Set<string>>();
  for (const t of txs) {
    if ((t.type ?? 'send') !== 'send') continue;
    if (t.from.toLowerCase() !== lc || !t.userOpHash) continue;
    let ids = groupSeenIds.get(t.userOpHash);
    if (!ids) groupSeenIds.set(t.userOpHash, (ids = new Set()));
    if (ids.has(t.id)) continue; // same-id duplicate — count the line once
    ids.add(t.id);
    const arr = sendGroups.get(t.userOpHash) ?? [];
    arr.push(t);
    sendGroups.set(t.userOpHash, arr);
  }

  const items: ActivityItem[] = [];
  // `item.id` is the FlatList key (HomeScreen). Guard against any legacy
  // duplicate-id records already in the store so a row can't render twice, and
  // skip every member of a batch once its grouped row has been emitted.
  const seen = new Set<string>();
  for (const t of txs) {
    if (seen.has(t.id)) continue;
    const type = t.type ?? 'send';
    let item: ActivityItem | null = null;
    if (type === 'receive' && t.to.toLowerCase() === lc) {
      item = receiveRecordToActivity(t);
    } else if (type === 'send' && t.from.toLowerCase() === lc) {
      const group = t.userOpHash ? sendGroups.get(t.userOpHash) : undefined;
      if (group && group.length > 1) {
        item = batchSendToActivity(group);
        for (const g of group) seen.add(g.id);
      } else {
        item = sendTxToActivity(t);
      }
    }
    if (!item) continue;
    seen.add(t.id);
    items.push(item);
  }
  return items.sort((a, b) => b.timestamp - a.timestamp);
}

/** Raw transfer transactions (sent + received) for this account — for the detail sheet. */
export async function loadActivityTransactions(address: string): Promise<LocalTransaction[]> {
  if (!address) return [];
  const lc = address.toLowerCase();
  const txs = await loadTransactions().catch(() => [] as LocalTransaction[]);
  return txs.filter((t) => {
    const type = t.type ?? 'send';
    if (type === 'receive') return t.to.toLowerCase() === lc;
    if (type === 'send') return t.from.toLowerCase() === lc;
    return false;
  });
}

/** dApp signing / tx-request events that belong under Connections. */
export async function loadConnectionEvents(address: string): Promise<ConnectionEvent[]> {
  const txs = await loadTransactions();
  const dappTypes = new Set(['dapp_tx', 'sign_message', 'sign_typed_data']);
  return txs
    .filter((t) => t.from.toLowerCase() === address.toLowerCase())
    .filter((t) => dappTypes.has(t.type ?? ''))
    .map((t) => ({
      id: t.id,
      label: connectionEventLabel(t),
      subtitle: [t.dappOrigin, chainName(t.chainId)].filter(Boolean).join(' · '),
      timestamp: t.timestamp,
      status: t.status,
      tx: t,
    }))
    .sort((a, b) => b.timestamp - a.timestamp);
}

function connectionEventLabel(tx: LocalTransaction): string {
  // tx.intent comes from the dApp (already human-readable) — keep it; localize the fallbacks.
  switch (tx.type) {
    case 'sign_message': return i18n.t('activity.signatureRequest');
    case 'sign_typed_data': return tx.intent || i18n.t('activity.typedDataSignature');
    case 'dapp_tx': return tx.intent || i18n.t('activity.dappTransaction');
    default: return i18n.t('activity.activity');
  }
}
