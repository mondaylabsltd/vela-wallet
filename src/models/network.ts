/**
 * Network configuration model.
 * Matches iOS WalletState.swift Network struct.
 */

// ---------------------------------------------------------------------------
// Custom network support
// ---------------------------------------------------------------------------

import { CHAINS, chainIdToApiNetwork, chainMeta } from '@/models/chains';
import type { APIToken, CustomNetwork } from '@/models/types';
import { nativeCoinLogoChainId, tokenBadgeChainId } from '@/models/types';
import { loadCustomNetworks } from '@/services/storage';

export interface Network {
  id: string;
  displayName: string;
  chainId: number;
  iconLabel: string;
  iconColor: string;
  iconBg: string;
  /** Network logo URL from ethereum-data API */
  logoURL: string;
  isL2: boolean;
  rpcURL: string;
  explorerURL: string;
  bundlerURL: string;
}

/** Base URL for chain logos from ethereum-data.awesometools.dev */
const CHAIN_LOGO_BASE = 'https://ethereum-data.awesometools.dev/chainlogos';
/** Base URL of Vela's per-chain ERC-4337 bundler. */
const BUNDLER_BASE = 'https://vela-bundler.getvela.app';

/**
 * Built-in networks, derived from the canonical {@link CHAINS} table so chainId,
 * api id and native symbol can never drift apart. logoURL and bundlerURL follow
 * a fixed per-chain URL shape. To add a network, add one entry to CHAINS.
 */
export const DEFAULT_NETWORKS: Network[] = CHAINS.map((c) => ({
  id: c.id,
  displayName: c.displayName,
  chainId: c.chainId,
  iconLabel: c.iconLabel,
  iconColor: c.iconColor,
  iconBg: c.iconBg,
  logoURL: `${CHAIN_LOGO_BASE}/eip155-${c.chainId}.png`,
  isL2: c.isL2,
  rpcURL: c.rpcURL,
  explorerURL: c.explorerURL,
  bundlerURL: `${BUNDLER_BASE}/${c.chainId}`,
}));

/** Lookup chain display name by ID. */
export function chainName(chainId: number): string {
  return DEFAULT_NETWORKS.find(n => n.chainId === chainId)?.displayName
    ?? _customNetworkCache.find(n => n.chainId === chainId)?.displayName
    ?? `Chain ${chainId}`;
}

/** Lookup a full network (default or custom) by chain ID; null if unknown. */
export function networkForChainId(chainId: number): Network | null {
  const def = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
  if (def) return def;
  const custom = _customNetworkCache.find(n => n.chainId === chainId);
  return custom ? customToNetwork(custom) : null;
}

// ---------------------------------------------------------------------------
// Block-explorer links
//
// The `${explorerURL ?? 'https://etherscan.io'}/tx/${hash}` idiom was hand-written
// (with subtly inconsistent trailing-slash handling) in ~8 screens/components.
// These are the single source of truth: `explorerBaseURL` returns null for an
// unknown chain (callers in a security context — e.g. the signing sheet — should
// show NO link rather than a misleading etherscan.io link), while the tx/address/
// token builders fall back to etherscan.io to match the historical display behavior.
// ---------------------------------------------------------------------------

/** Etherscan as the fallback explorer when a chain has no configured explorer. */
const FALLBACK_EXPLORER = 'https://etherscan.io';

/**
 * Block-explorer base URL for a chain (trailing slash stripped), or null when the
 * chain is unknown. Use this when an unknown chain should yield no link at all.
 */
export function explorerBaseURL(chainId: number): string | null {
  const url = networkForChainId(chainId)?.explorerURL;
  return url ? url.replace(/\/$/, '') : null;
}

/** Block-explorer link for a transaction hash (etherscan.io fallback). */
export function explorerTxURL(chainId: number, txHash: string): string {
  return `${explorerBaseURL(chainId) ?? FALLBACK_EXPLORER}/tx/${txHash}`;
}

/** Block-explorer link for an account/contract address (etherscan.io fallback). */
export function explorerAddressURL(chainId: number, address: string): string {
  return `${explorerBaseURL(chainId) ?? FALLBACK_EXPLORER}/address/${address}`;
}

/**
 * Block-explorer link for a token contract, optionally highlighting a holder
 * (the `?a=` query the token-detail screen uses). Etherscan.io fallback.
 */
export function explorerTokenURL(chainId: number, contract: string, holder?: string): string {
  const base = `${explorerBaseURL(chainId) ?? FALLBACK_EXPLORER}/token/${contract}`;
  return holder ? `${base}?a=${holder}` : base;
}

/**
 * The network to badge a token's logo with, or null when no badge is needed.
 * A native coin shown on its own logo chain (ETH on Ethereum) returns null so
 * the badge doesn't merely duplicate the main logo; everything else badges the
 * chain it sits on. `isNative` lets call sites that only have a symbol+chainId
 * (not a full APIToken) reuse the same rule.
 */
export function badgeNetworkFor(symbol: string, chainId: number, isNative: boolean): Network | null {
  if (isNative && nativeCoinLogoChainId(symbol, chainId) === chainId) return null;
  return networkForChainId(chainId);
}

/** Convenience wrapper of {@link badgeNetworkFor} for a full APIToken. */
export function tokenBadgeNetwork(t: APIToken): Network | null {
  const cid = tokenBadgeChainId(t);
  return cid != null ? networkForChainId(cid) : null;
}

/** Lookup native token symbol by chain ID (falls back to custom networks). */
export function nativeSymbol(chainId: number): string {
  const meta = chainMeta(chainId);
  if (meta) return meta.nativeSymbol;
  const custom = _customNetworkCache.find(n => n.chainId === chainId);
  return custom?.nativeSymbol ?? 'ETH';
}

/**
 * Logo URL for a chain's native coin, by the COIN's identity not the chain it
 * sits on (ETH on Base → Ethereum's logo, not Base's). Mirrors the native-coin
 * branch of `tokenLogoURLs` so surfaces that only have a chainId (the signing
 * sheet, balance-change rows) can show a real coin logo instead of a "?".
 */
export function nativeCoinLogoURL(chainId: number): string {
  const logoChain = nativeCoinLogoChainId(nativeSymbol(chainId), chainId);
  return `${CHAIN_LOGO_BASE}/eip155-${logoChain}.png`;
}

/**
 * Lookup network API identifier by chain ID. Inverse of `tokenChainId`
 * (models/types.ts); both are derived from CHAINS so they stay in sync.
 */
export function networkId(chainId: number): string {
  return chainIdToApiNetwork(chainId);
}

/** In-memory cache of custom networks for synchronous lookups. */
let _customNetworkCache: CustomNetwork[] = [];

/** Refresh the custom network cache from storage. Call on app start and after adding/removing. */
export async function refreshCustomNetworks(): Promise<void> {
  _customNetworkCache = await loadCustomNetworks();
}

/** Convert a CustomNetwork to the Network interface. */
export function customToNetwork(cn: CustomNetwork): Network {
  return {
    id: cn.id,
    displayName: cn.displayName,
    chainId: cn.chainId,
    iconLabel: cn.iconLabel,
    iconColor: cn.iconColor,
    iconBg: cn.iconBg,
    logoURL: cn.logoURL,
    isL2: cn.isL2,
    rpcURL: cn.rpcURL,
    explorerURL: cn.explorerURL,
    bundlerURL: cn.bundlerURL,
  };
}

/**
 * Get all networks synchronously (uses in-memory cache).
 * Call refreshCustomNetworks() at app start to populate the cache.
 */
export function getAllNetworksSync(): Network[] {
  return [...DEFAULT_NETWORKS, ..._customNetworkCache.map(customToNetwork)];
}

/** Get all networks: default + custom (refreshes cache first). */
export async function getAllNetworks(): Promise<Network[]> {
  await refreshCustomNetworks();
  return getAllNetworksSync();
}
