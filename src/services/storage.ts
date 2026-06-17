/**
 * Local persistence layer (AsyncStorage).
 *
 * All wallet data lives on-device only. Reads and writes go straight to
 * AsyncStorage — no network, no cross-device sync.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import type { StoredAccount, PendingUpload, CustomToken, NetworkConfig, ServiceEndpoints, PriceSource, CustomNetwork, LocalePrefs } from '@/models/types';
import { DEFAULT_SERVICE_ENDPOINTS, DEFAULT_LOCALE_PREFS } from '@/models/types';

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

export async function loadLocalePrefs(): Promise<LocalePrefs> {
  try {
    const raw = await AsyncStorage.getItem(KEYS.localePrefs);
    if (raw) {
      _localePrefsCache = { ...DEFAULT_LOCALE_PREFS, ...JSON.parse(raw) };
      return _localePrefsCache;
    }
  } catch {}
  return { ...DEFAULT_LOCALE_PREFS };
}

export async function saveLocalePrefs(prefs: LocalePrefs): Promise<void> {
  _localePrefsCache = { ...prefs };
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
}

/** Cap stored signed payloads so a huge typed-data blob can't bloat history. */
export const MAX_SIGNED_CONTENT = 8000;

export async function saveTransaction(tx: LocalTransaction): Promise<void> {
  const txs = await loadTransactions();
  txs.unshift(tx); // newest first
  // Keep max 200 transactions
  if (txs.length > 200) txs.length = 200;
  await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
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
  const txs = await loadTransactions();
  const idx = txs.findIndex((t) => t.id === id);
  if (idx === -1) return;
  txs[idx] = { ...txs[idx], ...patch };
  await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(txs));
}

/**
 * Merge new transactions into the store, de-duplicated by `id`. Used by the
 * received-transfer monitor so discovered deposits persist (and survive the
 * monitor's incremental block checkpoint advancing past them). Newest-first,
 * capped at 200. Returns the count of genuinely-new records added.
 */
export async function mergeTransactions(incoming: LocalTransaction[]): Promise<number> {
  if (incoming.length === 0) return 0;
  const existing = await loadTransactions();
  const ids = new Set(existing.map((t) => t.id));
  const fresh = incoming.filter((t) => !ids.has(t.id));
  if (fresh.length === 0) return 0;
  const merged = [...fresh, ...existing].sort((a, b) => b.timestamp - a.timestamp);
  if (merged.length > 200) merged.length = 200;
  await AsyncStorage.setItem(KEYS.transactionHistory, JSON.stringify(merged));
  return fresh.length;
}

// ---------------------------------------------------------------------------
// Clear All (for logout)
// ---------------------------------------------------------------------------

export async function clearAll(): Promise<void> {
  for (const key of Object.values(KEYS)) {
    await AsyncStorage.removeItem(key);
  }
}
