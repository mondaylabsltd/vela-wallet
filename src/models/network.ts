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
