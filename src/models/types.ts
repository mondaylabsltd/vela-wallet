/**
 * Core data models shared across the app.
 * Matches iOS WalletState.swift, WalletAPIService.swift models.
 */
import { apiNetworkToChainId } from '@/models/chains';
import { checksumAddress } from '@/services/eth-crypto';
import { getEthereumDataURL } from '@/services/storage';

// MARK: - Account

export interface Account {
  /** Passkey credential ID (hex string). */
  id: string;
  /** User-chosen display name. */
  name: string;
  /** Safe wallet address. */
  address: string;
  /** Account creation timestamp (ISO string). */
  createdAt: string;
}

// MARK: - Stored Account (with public key for signing)

export interface StoredAccount extends Account {
  /** Uncompressed P256 public key hex (04 || x || y). */
  publicKeyHex: string;
}

// MARK: - Pending Upload

export interface PendingUpload {
  /** Credential ID (hex). */
  id: string;
  name: string;
  publicKeyHex: string;
  attestationObjectHex: string;
  createdAt: string;
}

// MARK: - API Token

export interface APIToken {
  network: string;
  chainName: string;
  symbol: string;
  balance: string;
  decimals: number;
  logo: string | null;
  name: string;
  tokenAddress: string | null;
  priceUsd: number | null;
  spam: boolean;
}

/** Computed properties for APIToken. */
export function tokenId(t: APIToken): string {
  return `${t.network}_${t.tokenAddress ?? 'native'}_${t.symbol}`;
}

export function isNativeToken(t: APIToken): boolean {
  return t.tokenAddress == null;
}

export function tokenBalanceDouble(t: APIToken): number {
  return parseFloat(t.balance) || 0;
}

export function tokenUsdValue(t: APIToken): number {
  return tokenBalanceDouble(t) * (t.priceUsd ?? 0);
}

export function tokenChainId(t: APIToken): number {
  // Inverse of networkId() (models/network.ts); both derive from CHAINS, so a
  // newly added chain works in both directions from a single table entry.
  return apiNetworkToChainId(t.network);
}

/**
 * The chain whose logo represents a native coin. A native coin's logo is the
 * COIN's identity, not the chain it currently sits on: ether is native on
 * Ethereum, Arbitrum, Optimism and Base, but on all of them the coin is ETH —
 * so its logo is always Ethereum's (eip155-1), never the L2's. The same holds
 * for BNB on opBNB, POL on other Polygon chains, etc. Native coins unique to a
 * single chain (or a custom chain's own coin) fall back to that chain's logo.
 */
export function nativeCoinLogoChainId(symbol: string, fallbackChainId: number): number {
  switch (symbol.toUpperCase()) {
    case 'ETH': return 1;
    case 'BNB': return 56;
    case 'POL': case 'MATIC': return 137;
    case 'AVAX': return 43114;
    case 'XDAI': return 100;
    default: return fallbackChainId;
  }
}

/**
 * The chain a token's logo should be badged with, or null when a badge would
 * merely duplicate the main logo — i.e. a native coin shown on its own logo
 * chain (ETH on Ethereum, BNB on BNB Chain). ERC-20s, and native coins sitting
 * on a *different* chain than their logo (ETH on Base), always badge, since the
 * badge then adds real information.
 */
export function tokenBadgeChainId(t: APIToken): number | null {
  const cid = tokenChainId(t);
  if (isNativeToken(t) && nativeCoinLogoChainId(t.symbol, cid) === cid) return null;
  return cid;
}

export function tokenLogoURL(t: APIToken): string | null {
  return tokenLogoURLs(t)[0] ?? null;
}

/**
 * Return candidate logo URLs in priority order.
 * For ERC-20 tokens we try checksummed address first, then lowercase,
 * so we tolerate inconsistent casing on the data server.
 */
export function tokenLogoURLs(t: APIToken): string[] {
  if (t.logo && t.logo.length > 0) return [t.logo];
  const cid = tokenChainId(t);
  if (isNativeToken(t)) {
    return nativeLogoURLs(cid, t.symbol);
  }
  if (t.tokenAddress) {
    return tokenLogoURLsByAddress(cid, t.tokenAddress);
  }
  return [];
}

/**
 * Native-coin logo URL(s) for a raw (chainId, symbol) pair — for surfaces that
 * have no APIToken (e.g. a discovered incoming native transfer). The logo tracks
 * the COIN's identity via {@link nativeCoinLogoChainId} (ETH is always eip155-1,
 * even on Base), not the chain it currently sits on.
 */
export function nativeLogoURLs(chainId: number, symbol: string): string[] {
  const logoChain = nativeCoinLogoChainId(symbol, chainId);
  return [`${getEthereumDataURL()}/chainlogos/eip155-${logoChain}.png`];
}

/**
 * Per-chain logo URLs for a raw (chainId, address) pair — for surfaces that have
 * an address but no APIToken (e.g. the signing sheet). Mirrors `tokenLogoURLs`:
 * checksummed first, lowercase fallback. Keyed by the token's OWN chain, so a
 * Polygon/Base token resolves its logo correctly instead of assuming mainnet.
 */
export function tokenLogoURLsByAddress(chainId: number, tokenAddress: string): string[] {
  // Only a well-formed 20-byte address yields a logo URL. Never throw: this runs
  // inside the signing sheet's render, and checksumAddress() rejects a malformed
  // value — a logo lookup must never crash a security surface.
  if (!isAddress(tokenAddress)) return [];
  const base = `${getEthereumDataURL()}/assets/eip155-${chainId}`;
  const cs = checksumAddress(tokenAddress);
  const lc = tokenAddress.toLowerCase();
  const urls = [`${base}/${cs}/logo.png`];
  if (lc !== cs) urls.push(`${base}/${lc}/logo.png`);
  return urls;
}

// MARK: - API NFT

export interface APINFT {
  network: string;
  chainName: string;
  contractAddress: string;
  tokenId: string;
  name: string | null;
  description: string | null;
  image: string | null;
  tokenType: string;
  collectionName: string | null;
  collectionImage: string | null;
}

export function nftId(n: APINFT): string {
  return `${n.network}_${n.contractAddress}_${n.tokenId}`;
}

export function nftDisplayName(n: APINFT): string {
  return n.name ?? `${n.collectionName ?? 'NFT'} #${n.tokenId}`;
}

export function nftImageURL(n: APINFT): string | null {
  if (!n.image) return null;
  if (n.image.startsWith('ipfs://')) {
    return `https://ipfs.io/ipfs/${n.image.slice(7)}`;
  }
  return n.image;
}

// MARK: - Custom Token

export interface CustomToken {
  id: string; // "{chainId}_{contractAddress}"
  chainId: number;
  contractAddress: string;
  symbol: string;
  name: string;
  decimals: number;
  networkName: string;
}

// MARK: - Network Config

export interface NetworkConfig {
  chainId: number;
  rpcURL: string;
  explorerURL: string;
  bundlerURL: string;
}

// MARK: - Bundler / Deployer

export interface BundlerDeployerInfo {
  walletAddress: string;
  bundlerAddress: string;
  deployerAddress: string;
}

export type FundingStatus = 'not_funded' | 'funded' | 'low_balance';

export interface NetworkFundingStatus {
  chainId: number;
  bundlerBalance: string;
  deployerBalance: string;
  bundlerStatus: FundingStatus;
  deployerStatus: FundingStatus;
}

// MARK: - Custom Network

export interface CustomNetwork {
  id: string;
  displayName: string;
  chainId: number;
  iconLabel: string;
  iconColor: string;
  iconBg: string;
  logoURL: string;
  isL2: boolean;
  rpcURL: string;
  explorerURL: string;
  bundlerURL: string;
  nativeSymbol: string;
  addedAt: string;
}

// MARK: - Compatibility Check

export interface ContractStatus {
  name: string;
  address: string;
  deployed: boolean;
}

export interface CompatibilityResult {
  chainId: number;
  /** All required contracts deployed */
  compatible: boolean;
  /** Individual contract statuses */
  contracts: ContractStatus[];
  /** Best RPC URL (lowest latency) */
  bestRpcUrl?: string;
  /** Best RPC latency in ms */
  bestRpcLatency?: number;
  /** RIP-7212 P256 precompile available (required for passkey signatures) */
  p256Available?: boolean;
  /** True when all RPC attempts failed — result is inconclusive */
  rpcFailed?: boolean;
  error?: string;
}

// MARK: - Service Endpoints

export interface ServiceEndpoints {
  /** Ethereum data index URL */
  ethereumDataURL: string;
  /** Passkeys public key index URL */
  passkeyIndexURL: string;
  /** ERC-4337 bundler service URL */
  bundlerServiceURL: string;
  /**
   * Fiat exchange-rate endpoint returning USD-based rates. Two shapes are
   * accepted (see `normalizeRates`): Frankfurter v2's array
   * `[{base:'USD',quote:'EUR',rate:0.92}]` or an object `{rates:{EUR:0.92,…}}`
   * (open.er-api / v1). The displayed currency list is driven by whatever codes
   * this returns — the default (Frankfurter v2) covers ~160 currencies incl. VND.
   * Keep `?base=USD`: without it Frankfurter returns EUR-based rates.
   */
  fiatRatesURL: string;
}

export type PriceSource = 'api' | 'dex';

// MARK: - Localization (number / date / time formats)

/** Number grouping/decimal style. `auto` follows the system locale. */
export type NumberFormatKey = 'auto' | 'comma_dot' | 'dot_comma' | 'space_comma' | 'indian';
/** Date field order + separator. `auto` follows the system locale. */
export type DateFormatKey = 'auto' | 'ymd_slash' | 'mdy_slash' | 'dmy_slash' | 'dmy_dot' | 'iso';
/** 12- vs 24-hour clock. `auto` follows the system locale. */
export type TimeFormatKey = 'auto' | 'h24' | 'h12';

export interface LocalePrefs {
  numberFormat: NumberFormatKey;
  dateFormat: DateFormatKey;
  timeFormat: TimeFormatKey;
}

export const DEFAULT_LOCALE_PREFS: LocalePrefs = {
  numberFormat: 'auto',
  dateFormat: 'auto',
  timeFormat: 'auto',
};

export const DEFAULT_SERVICE_ENDPOINTS: ServiceEndpoints = {
  ethereumDataURL: 'https://ethereum-data.awesometools.dev',
  passkeyIndexURL: 'https://p256-index-rs.getvela.app',
  bundlerServiceURL: 'https://vela-relay.getvela.app',
  // Vela's self-hosted Frankfurter instance (github.com/mondaylabsltd/vela-currency):
  // FOSS, no key, ~160 currencies incl. VND. base=USD is required (default base is EUR).
  fiatRatesURL: 'https://vela-currency.getvela.app/v2/rates?base=USD',
};

// MARK: - BLE Message Types

export interface BLEIncomingRequest {
  id: string;
  method: string;
  params: any[];
  origin: string;
  favicon?: string;
  // Per-request binding for the Safari-extension sign path (see dapp-connection.tsx
  // beginExtensionSign). Loosely typed to avoid a types.ts → transport import
  // cycle. __transport routes the response to the owning transport (never a
  // concurrent WalletPair socket — F2); __chainId is the chain to sign against
  // (F4); __dapp is the per-request identity shown/recorded (F3). Absent for
  // ordinary bridge/WalletPair requests (which use the global transport/chain).
  __transport?: any;
  __chainId?: number;
  __dapp?: { name: string; url: string };
}

export interface BLEOutgoingResponse {
  id: string;
  result?: any;
  error?: BLEError;
}

export interface BLEError {
  code: number;
  message: string;
}

// MARK: - Transaction Result

export interface TransactionResult {
  userOpHash: string;
  txHash: string;
}

// MARK: - Utility Functions

/** Format a balance with appropriate precision. */
export function formatBalance(value: number): string {
  if (value === 0) return '0';
  if (value >= 1000) return value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (value >= 1) return value.toLocaleString('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 4 });
  return value.toPrecision(4);
}

/** Shorten an address to "0x1234...abcd" format. */
export function shortAddr(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 8)}...${address.slice(-6)}`;
}

/**
 * Anchored EVM (20-byte) address pattern. This literal was hand-copied as a
 * private `ADDR_RE` in 6 modules and inlined ~20 more times; this is the one copy.
 */
export const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

/**
 * True when `s` is a well-formed 0x-prefixed 20-byte hex address. Typed as a guard
 * so a passing check narrows `string | undefined` → `string` for the caller.
 */
export function isAddress(s: string | null | undefined): s is string {
  return !!s && ADDRESS_RE.test(s);
}

/**
 * Extract the first 20-byte address embedded in arbitrary text (a scanned QR or
 * pasted string), or null. Unanchored on purpose — the input may wrap the address
 * (e.g. "ethereum:0x…" or "...send to 0x…").
 */
export function extractAddress(s: string): string | null {
  const m = s.match(/0x[0-9a-fA-F]{40}/);
  return m ? m[0] : null;
}
