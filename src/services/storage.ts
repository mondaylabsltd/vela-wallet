/**
 * Local persistence layer (AsyncStorage).
 *
 * All wallet data lives on-device only. Reads and writes go straight to
 * AsyncStorage — no network, no cross-device sync.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import type { StoredAccount, PendingUpload, CustomToken, NetworkConfig, ServiceEndpoints, PriceSource, CustomNetwork, LocalePrefs } from '@/models/types';
import { DEFAULT_SERVICE_ENDPOINTS, DEFAULT_LOCALE_PREFS } from '@/models/types';
import type { RpcProviderKeys } from '@/services/rpc-providers';
import type { StoredAssetSim } from '@/services/tx-simulation';

const KEYS = {
  accounts: 'vela.accounts',
  activeAccountIndex: 'vela.activeAccountIndex',
  pendingUploads: 'vela.pendingUploads',
  customTokens: 'vela.customTokens',
  networkConfig: 'vela.networkConfig',
  serviceEndpoints: 'vela.serviceEndpoints',
  transactionHistory: 'vela.transactionHistory',
  priceSource: 'vela.priceSource',
  customNetworks: 'vela.customNetworks',
  localePrefs: 'vela.localePrefs',
  rpcProviders: 'vela.rpcProviders',
} as const;

// ---------------------------------------------------------------------------
// Generic array helpers
// ---------------------------------------------------------------------------

async function loadArray<T>(key: string): Promise<T[]> {
  try {
    const raw = await AsyncStorage.getItem(key);
    if (raw) return JSON.parse(raw);
  } catch {
    console.log(`[Storage] Local parse error for "${key}"`);
  }
  return [];
}

async function saveArray<T>(key: string, items: T[]): Promise<void> {
  await AsyncStorage.setItem(key, JSON.stringify(items));
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

export async function saveAccount(account: StoredAccount): Promise<void> {
  const accounts = await loadAccounts();
  const filtered = accounts.filter(a => a.id !== account.id);
  filtered.push(account);
  await saveArray(KEYS.accounts, filtered);
}

export async function loadAccounts(): Promise<StoredAccount[]> {
  return loadArray<StoredAccount>(KEYS.accounts);
}

export async function findAccountByCredentialId(id: string): Promise<StoredAccount | undefined> {
  const accounts = await loadAccounts();
  return accounts.find(a => a.id === id);
}

// ---------------------------------------------------------------------------
// Active Account Index (local-only, UI preference)
// ---------------------------------------------------------------------------

export async function saveActiveAccountIndex(index: number): Promise<void> {
  await AsyncStorage.setItem(KEYS.activeAccountIndex, String(index));
}

export async function loadActiveAccountIndex(): Promise<number> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.activeAccountIndex);
    if (raw != null) return parseInt(raw, 10) || 0;
  } catch {}
  return 0;
}

// ---------------------------------------------------------------------------
// Pending Uploads
// ---------------------------------------------------------------------------

export async function savePendingUpload(upload: PendingUpload): Promise<void> {
  const uploads = await loadPendingUploads();
  const filtered = uploads.filter(u => u.id !== upload.id);
  filtered.push(upload);
  await saveArray(KEYS.pendingUploads, filtered);
}

export async function loadPendingUploads(): Promise<PendingUpload[]> {
  return loadArray<PendingUpload>(KEYS.pendingUploads);
}

export async function removePendingUpload(credentialId: string): Promise<void> {
  const uploads = await loadPendingUploads();
  await saveArray(KEYS.pendingUploads, uploads.filter(u => u.id !== credentialId));
}

export async function hasPendingUploads(): Promise<boolean> {
  const uploads = await loadPendingUploads();
  return uploads.length > 0;
}

// ---------------------------------------------------------------------------
// Custom Tokens
// ---------------------------------------------------------------------------

export async function saveCustomToken(token: CustomToken): Promise<void> {
  const tokens = await loadCustomTokens();
  const filtered = tokens.filter(t => t.id !== token.id);
  filtered.push(token);
  await saveArray(KEYS.customTokens, filtered);
}

export async function loadCustomTokens(): Promise<CustomToken[]> {
  return loadArray<CustomToken>(KEYS.customTokens);
}

export async function removeCustomToken(id: string): Promise<void> {
  const tokens = await loadCustomTokens();
  await saveArray(KEYS.customTokens, tokens.filter(t => t.id !== id));
}

// ---------------------------------------------------------------------------
// Network Config
// ---------------------------------------------------------------------------

export async function saveNetworkConfig(config: NetworkConfig): Promise<void> {
  const configs = await loadNetworkConfigs();
  const filtered = configs.filter(c => c.chainId !== config.chainId);
  filtered.push(config);
  await saveArray(KEYS.networkConfig, filtered);
}

export async function loadNetworkConfigs(): Promise<NetworkConfig[]> {
  return loadArray<NetworkConfig>(KEYS.networkConfig);
}

export async function getNetworkConfig(chainId: number): Promise<NetworkConfig | undefined> {
  const configs = await loadNetworkConfigs();
  return configs.find(c => c.chainId === chainId);
}

// ---------------------------------------------------------------------------
// Custom Networks
// ---------------------------------------------------------------------------

export async function saveCustomNetwork(network: CustomNetwork): Promise<void> {
  const networks = await loadCustomNetworks();
  const filtered = networks.filter(n => n.id !== network.id);
  filtered.push(network);
  await saveArray(KEYS.customNetworks, filtered);
}

export async function loadCustomNetworks(): Promise<CustomNetwork[]> {
  return loadArray<CustomNetwork>(KEYS.customNetworks);
}

export async function removeCustomNetwork(id: string): Promise<void> {
  const networks = await loadCustomNetworks();
  await saveArray(KEYS.customNetworks, networks.filter(n => n.id !== id));
}

// ---------------------------------------------------------------------------
// Service Endpoints
// ---------------------------------------------------------------------------

/** In-memory cache — initialised on first `loadServiceEndpoints()` call. */
let _endpointsCache: ServiceEndpoints = { ...DEFAULT_SERVICE_ENDPOINTS };

export async function loadServiceEndpoints(): Promise<ServiceEndpoints> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.serviceEndpoints);
    if (raw) {
      _endpointsCache = { ...DEFAULT_SERVICE_ENDPOINTS, ...JSON.parse(raw) };
      return _endpointsCache;
    }
  } catch {}
  return { ...DEFAULT_SERVICE_ENDPOINTS };
}

export async function saveServiceEndpoints(endpoints: ServiceEndpoints): Promise<void> {
  _endpointsCache = { ...endpoints };
  await AsyncStorage.setItem(KEYS.serviceEndpoints, JSON.stringify(endpoints));
}

/**
 * Synchronous getter for the ethereum-data base URL.
 * Returns the user-configured value if available, otherwise the default.
 * The cache is populated by `loadServiceEndpoints()` which runs at app startup.
 */
export function getEthereumDataURL(): string {
  return _endpointsCache.ethereumDataURL || DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL;
}

/**
 * Synchronous getter for the bundler service base URL.
 * Returns the user-configured value if available, otherwise the default.
 */
export function getBundlerServiceURL(): string {
  return _endpointsCache.bundlerServiceURL || DEFAULT_SERVICE_ENDPOINTS.bundlerServiceURL;
}

/**
 * Synchronous getter for the fiat exchange-rate endpoint.
 * Returns the user-configured value if available, otherwise the default.
 */
export function getFiatRatesURL(): string {
  return _endpointsCache.fiatRatesURL || DEFAULT_SERVICE_ENDPOINTS.fiatRatesURL;
}

// ---------------------------------------------------------------------------
// Localization preferences (number / date / time formats)
// ---------------------------------------------------------------------------

/** In-memory cache — initialised by `loadLocalePrefs()` at startup. */
let _localePrefsCache: LocalePrefs = { ...DEFAULT_LOCALE_PREFS };

/**
 * Subscribers notified whenever the prefs change. Number/date/time formatting is
 * read synchronously from the cache during render, so without this nothing tells
 * the already-mounted screens to re-render — a format change only showed up after
 * a reload. `useLocalePrefs()` (locale-format) bridges this to React.
 *
 * Anchored on `globalThis` so Fast Refresh re-evaluating this module during
 * development doesn't swap in a fresh empty set and orphan already-mounted
 * subscribers (which makes live format changes silently require a reload).
 * Inert in production — the module is only evaluated once.
 */
const _globalStore = globalThis as unknown as { __velaLocaleListeners?: Set<() => void> };
const _localeListeners: Set<() => void> = (_globalStore.__velaLocaleListeners ??= new Set<() => void>());

export function subscribeLocalePrefs(listener: () => void): () => void {
  _localeListeners.add(listener);
  return () => { _localeListeners.delete(listener); };
}

function notifyLocaleListeners(): void {
  for (const l of _localeListeners) l();
}

export async function loadLocalePrefs(): Promise<LocalePrefs> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.localePrefs);
    if (raw) {
      _localePrefsCache = { ...DEFAULT_LOCALE_PREFS, ...JSON.parse(raw) };
      notifyLocaleListeners();
      return _localePrefsCache;
    }
  } catch {}
  return { ...DEFAULT_LOCALE_PREFS };
}

export async function saveLocalePrefs(prefs: LocalePrefs): Promise<void> {
  // New object identity on every save so useSyncExternalStore sees a fresh
  // snapshot; notify synchronously so the UI updates before the disk write.
  _localePrefsCache = { ...prefs };
  notifyLocaleListeners();
  await AsyncStorage.setItem(KEYS.localePrefs, JSON.stringify(prefs));
}

/** Synchronous getter (cache populated by `loadLocalePrefs()` at startup). */
export function getLocalePrefs(): LocalePrefs {
  return _localePrefsCache;
}

// ---------------------------------------------------------------------------
// Price Source
// ---------------------------------------------------------------------------

export async function loadPriceSource(): Promise<PriceSource> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.priceSource);
    if (raw === 'dex') return 'dex';
  } catch {}
  return 'api';
}

export async function savePriceSource(source: PriceSource): Promise<void> {
  await AsyncStorage.setItem(KEYS.priceSource, source);
}

// ---------------------------------------------------------------------------
// RPC Provider Keys (Alchemy / dRPC / Ankr)
// ---------------------------------------------------------------------------

/**
 * In-memory cache of the per-provider API keys, populated by
 * `loadRpcProviders()` at startup. The RPC pool reads it synchronously while
 * building each chain's endpoint list (see services/rpc-pool.ts), so it must be
 * available without an await.
 *
 * Keys are stored as plaintext, consistent with how per-network RPC overrides
 * already persist URLs that can embed credentials (`?apikey=`).
 */
let _rpcProvidersCache: RpcProviderKeys = {};

const _rpcProviderStore = globalThis as unknown as { __velaRpcProviderListeners?: Set<() => void> };
const _rpcProviderListeners: Set<() => void> = (_rpcProviderStore.__velaRpcProviderListeners ??= new Set<() => void>());

export function subscribeRpcProviders(listener: () => void): () => void {
  _rpcProviderListeners.add(listener);
  return () => { _rpcProviderListeners.delete(listener); };
}

function notifyRpcProviderListeners(): void {
  for (const l of _rpcProviderListeners) l();
}

export async function loadRpcProviders(): Promise<RpcProviderKeys> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.rpcProviders);
    if (raw) {
      _rpcProvidersCache = JSON.parse(raw);
      notifyRpcProviderListeners();
    }
  } catch {}
  return _rpcProvidersCache;
}

export async function saveRpcProviders(keys: RpcProviderKeys): Promise<void> {
  // Drop empty entries so a cleared key fully removes the provider.
  const cleaned: RpcProviderKeys = {};
  for (const [id, key] of Object.entries(keys)) {
    const trimmed = (key ?? '').trim();
    if (trimmed) cleaned[id as keyof RpcProviderKeys] = trimmed;
  }
  // Fresh object identity so useSyncExternalStore sees a new snapshot; notify
  // synchronously so the UI updates before the disk write resolves.
  _rpcProvidersCache = cleaned;
  notifyRpcProviderListeners();
  await AsyncStorage.setItem(KEYS.rpcProviders, JSON.stringify(cleaned));
}

/** Synchronous getter (cache populated by `loadRpcProviders()` at startup). */
export function getRpcProviderKeys(): RpcProviderKeys {
  return _rpcProvidersCache;
}

// ---------------------------------------------------------------------------
// Transaction History (local recording)
// ---------------------------------------------------------------------------

/**
 * History entry type.
 *
 * On-chain (have txHash, viewable on explorer):
 *   - send:        User-initiated transfer from the Send screen
 *   - dapp_tx:     dApp-initiated transaction (eth_sendTransaction)
 *
 * Off-chain (signature only, no txHash):
 *   - sign_message:    personal_sign (e.g. login, verify ownership)
 *   - sign_typed_data: eth_signTypedData_v4 (e.g. Permit, order, gasless approval)
 */
export type TransactionType =
  | 'send'
  | 'receive'
  | 'dapp_tx'
  | 'sign_message'
  | 'sign_typed_data';

export interface LocalTransaction {
  id: string;
  userOpHash: string;
  /** On-chain tx hash. Empty string for off-chain signatures. */
  txHash: string;
  from: string;
  to: string;
  /** Resolved identity name of the recipient (e.g. "vitalik.eth"). */
  toName?: string;
  value: string;
  symbol: string;
  decimals: number;
  /**
   * Ordered token-logo URL candidates, captured at send time so the detail sheet
   * can show the real token logo instead of a letter glyph. Older records / most
   * receives lack it (the logo falls back to the letter circle).
   */
  logoUrls?: string[];
  chainId: number;
  timestamp: number;
  status: 'pending' | 'confirmed' | 'failed';
  /** Operation type. Defaults to 'send' for backwards compatibility with old records. */
  type?: TransactionType;
  /** dApp name or domain (e.g. "Uniswap", "walletpair.org"). */
  dappOrigin?: string;
  /** ERC-7730 clear signing intent (e.g. "Swap", "Approve", "Permit"). */
  intent?: string;
  /** USD value at the time of the event, pre-formatted (e.g. "$1.00"). Optional. */
  usd?: string;
  /**
   * Raw signed content, captured at sign time so the detail view can show
   * exactly what was authorized. For sign_message it's the decoded message
   * text; for sign_typed_data the typed-data JSON; for dapp_tx the calldata.
   * Capped in length to keep stored history small. Older records lack it.
   */
  signedContent?: string;
  /**
   * The original dApp request (method + params), captured so the exact signing
   * panel can be re-rendered read-only from history (Connections → tap → replay).
   * Capped; `requestTruncated` marks when a large payload (e.g. a deploy's
   * bytecode) was clipped. Older records lack it (they fall back to the metadata
   * detail view instead of a full replay).
   */
  signedRequest?: { method: string; params: unknown[] };
  requestTruncated?: boolean;
  /**
   * Sign-time asset-change simulation (predicted +/− balance changes), captured at
   * approve so the detail/replay view can show "what moved" the same way Send and
   * the live signing sheet do. Stored JSON-safe (bigint deltas as decimal strings,
   * see StoredAssetSim). A prediction, not the confirmed on-chain result; only set
   * for transactions/batches, and only when an engine could compute it. Older
   * records lack it.
   */
  assetChanges?: StoredAssetSim;
}

/** Cap stored signed payloads so a huge typed-data blob can't bloat history. */
export const MAX_SIGNED_CONTENT = 8000;

/**
 * Serialize every transaction-history mutation. Each save/update/merge/delete does
 * a read-modify-write over the whole array; without a lock, two concurrent writers
 * both read the same snapshot and the last write clobbers the other. That silently
 * dropped the sibling records of a batch send (split = 1 token → N recipients,
 * multiSelect = N tokens → 1 recipient) — which are written together via
 * `Promise.all` — collapsing the batch to a single line in Activity. Chaining every
 * mutation through one promise makes them atomic with respect to each other.
 */
let _txWriteLock: Promise<unknown> = Promise.resolve();
function withTxLock<T>(fn: () => Promise<T>): Promise<T> {
  const run = _txWriteLock.then(fn, fn);
  // Keep the chain alive whether the mutation resolves or rejects.
  _txWriteLock = run.then(() => undefined, () => undefined);
  return run;
}

export async function saveTransaction(tx: LocalTransaction): Promise<void> {
  // De-dupe by id: a resubmitted UserOp (e.g. two identical sends sharing a
  // nonce) yields the same userOpHash, which is the record id — without this it
  // would persist twice and surface as a React duplicate-key warning in the feed.
  return withTxLock(async () => {
    const txs = (await loadTransactions()).filter((t) => t.id !== tx.id);
    txs.unshift(tx); // newest first
    // Keep max 200 transactions
    if (txs.length > 200) txs.length = 200;
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
  });
}

/**
 * Persist several records in ONE atomic read-modify-write. Preferred over
 * `Promise.all(records.map(saveTransaction))` for batch sends: fewer disk writes
 * and no reliance on the write-lock to serialize the siblings. De-duped by id,
 * newest-first, capped at 200.
 */
export async function saveTransactions(incoming: LocalTransaction[]): Promise<void> {
  if (incoming.length === 0) return;
  return withTxLock(async () => {
    const ids = new Set(incoming.map((t) => t.id));
    const txs = (await loadTransactions()).filter((t) => !ids.has(t.id));
    txs.unshift(...incoming); // newest first, in the given order
    if (txs.length > 200) txs.length = 200;
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
  });
}

export async function loadTransactions(): Promise<LocalTransaction[]> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.transactionHistory);
    if (raw) return JSON.parse(raw);
  } catch {}
  return [];
}

/**
 * Patch an existing transaction by id — e.g. flip a 'pending' send to 'confirmed'
 * once its on-chain hash resolves. No-op if the id isn't present.
 */
export async function updateTransaction(id: string, patch: Partial<LocalTransaction>): Promise<void> {
  return withTxLock(async () => {
    const txs = await loadTransactions();
    const idx = txs.findIndex((t) => t.id === id);
    if (idx === -1) return;
    txs[idx] = { ...txs[idx], ...patch };
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
  });
}

/**
 * Apply the same patch to several records in ONE atomic read-modify-write —
 * e.g. flip every sibling of a batch send to 'confirmed' once its shared on-chain
 * hash lands. Missing ids are skipped.
 */
export async function updateTransactions(ids: string[], patch: Partial<LocalTransaction>): Promise<void> {
  if (ids.length === 0) return;
  const idSet = new Set(ids);
  return withTxLock(async () => {
    const txs = await loadTransactions();
    let changed = false;
    for (let i = 0; i < txs.length; i++) {
      if (idSet.has(txs[i].id)) { txs[i] = { ...txs[i], ...patch }; changed = true; }
    }
    if (changed) await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
  });
}

/**
 * Merge new transactions into the store, de-duplicated by `id`. Used by the
 * received-transfer monitor so discovered deposits persist (and survive the
 * monitor's incremental block checkpoint advancing past them). Newest-first,
 * capped at 200. Returns the count of genuinely-new records added.
 */
export async function mergeTransactions(incoming: LocalTransaction[]): Promise<number> {
  if (incoming.length === 0) return 0;
  return withTxLock(async () => {
    const existing = await loadTransactions();
    const ids = new Set(existing.map((t) => t.id));
    const fresh = incoming.filter((t) => !ids.has(t.id));
    if (fresh.length === 0) return 0;
    const merged = [...fresh, ...existing].sort((a, b) => b.timestamp - a.timestamp);
    if (merged.length > 200) merged.length = 200;
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(merged));
    return fresh.length;
  });
}

/** Remove a single transaction by id (e.g. swipe-to-delete in Connections). */
export async function deleteTransaction(id: string): Promise<void> {
  return withTxLock(async () => {
    const txs = await loadTransactions();
    const next = txs.filter((t) => t.id !== id);
    if (next.length === txs.length) return;
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(next));
  });
}

/**
 * Clear all dApp connection-activity records (signatures + dApp txs) for an
 * address. Leaves value-transfer history (send / receive) untouched.
 */
export async function deleteConnectionEvents(address: string): Promise<void> {
  const lc = address.toLowerCase();
  const dappTypes = new Set<TransactionType>(['dapp_tx', 'sign_message', 'sign_typed_data']);
  return withTxLock(async () => {
    const txs = await loadTransactions();
    const next = txs.filter(
      (t) => !(t.from.toLowerCase() === lc && dappTypes.has((t.type ?? 'send') as TransactionType)),
    );
    await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(next));
  });
}

// ---------------------------------------------------------------------------
// Clear All (for logout)
// ---------------------------------------------------------------------------

export async function clearAll(): Promise<void> {
  for (const key of Object.values(KEYS)) {
    await AsyncStorage.removeItem(key);
  }
}
