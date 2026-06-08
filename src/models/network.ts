/**
 * Network configuration model.
 * Matches iOS WalletState.swift Network struct.
 */

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

export const DEFAULT_NETWORKS: Network[] = [
  {
    id: 'ethereum', displayName: 'Ethereum', chainId: 1,
    iconLabel: 'ETH', iconColor: '#627EEA', iconBg: '#EEF0F8',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-1.png`, isL2: false,
    rpcURL: 'https://eth.llamarpc.com', explorerURL: 'https://etherscan.io',
    bundlerURL: 'https://vela-bundler.getvela.app/1',
  },
  {
    id: 'bnb', displayName: 'BNB Chain', chainId: 56,
    iconLabel: 'BNB', iconColor: '#F0B90B', iconBg: '#FFF8E1',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-56.png`, isL2: false,
    rpcURL: 'https://bsc-dataseed.binance.org', explorerURL: 'https://bscscan.com',
    bundlerURL: 'https://vela-bundler.getvela.app/56',
  },
  {
    id: 'polygon', displayName: 'Polygon', chainId: 137,
    iconLabel: 'POL', iconColor: '#8247E5', iconBg: '#F0EAFF',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-137.png`, isL2: true,
    rpcURL: 'https://polygon-rpc.com', explorerURL: 'https://polygonscan.com',
    bundlerURL: 'https://vela-bundler.getvela.app/137',
  },
  {
    id: 'arbitrum', displayName: 'Arbitrum', chainId: 42161,
    iconLabel: 'ARB', iconColor: '#28A0F0', iconBg: '#E8F4FD',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-42161.png`, isL2: true,
    rpcURL: 'https://arb1.arbitrum.io/rpc', explorerURL: 'https://arbiscan.io',
    bundlerURL: 'https://vela-bundler.getvela.app/42161',
  },
  {
    id: 'optimism', displayName: 'Optimism', chainId: 10,
    iconLabel: 'OP', iconColor: '#FF0420', iconBg: '#FFECEC',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-10.png`, isL2: true,
    rpcURL: 'https://mainnet.optimism.io', explorerURL: 'https://optimistic.etherscan.io',
    bundlerURL: 'https://vela-bundler.getvela.app/10',
  },
  {
    id: 'base', displayName: 'Base', chainId: 8453,
    iconLabel: 'BASE', iconColor: '#0052FF', iconBg: '#E8EEFF',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-8453.png`, isL2: true,
    rpcURL: 'https://mainnet.base.org', explorerURL: 'https://basescan.org',
    bundlerURL: 'https://vela-bundler.getvela.app/8453',
  },
  {
    id: 'avalanche', displayName: 'Avalanche', chainId: 43114,
    iconLabel: 'AVAX', iconColor: '#E84142', iconBg: '#FFF0F0',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-43114.png`, isL2: false,
    rpcURL: 'https://api.avax.network/ext/bc/C/rpc', explorerURL: 'https://snowtrace.io',
    bundlerURL: 'https://vela-bundler.getvela.app/43114',
  },
  {
    id: 'gnosis', displayName: 'Gnosis', chainId: 100,
    iconLabel: 'xDAI', iconColor: '#04795B', iconBg: '#E8F5F0',
    logoURL: `${CHAIN_LOGO_BASE}/eip155-100.png`, isL2: false,
    rpcURL: 'https://rpc.gnosischain.com', explorerURL: 'https://gnosisscan.io',
    bundlerURL: 'https://vela-bundler.getvela.app/100',
  },
];

/** Lookup chain display name by ID. */
export function chainName(chainId: number): string {
  return DEFAULT_NETWORKS.find(n => n.chainId === chainId)?.displayName
    ?? _customNetworkCache.find(n => n.chainId === chainId)?.displayName
    ?? `Chain ${chainId}`;
}

/** Lookup native token symbol by chain ID. */
export function nativeSymbol(chainId: number): string {
  switch (chainId) {
    case 1: case 42161: case 10: case 8453: return 'ETH';
    case 56: return 'BNB';
    case 137: return 'POL';
    case 43114: return 'AVAX';
    case 100: return 'xDAI';
    default: {
      const custom = _customNetworkCache.find(n => n.chainId === chainId);
      return custom?.nativeSymbol ?? 'ETH';
    }
  }
}

/** Lookup network API identifier by chain ID. */
export function networkId(chainId: number): string {
  switch (chainId) {
    case 1: return 'eth-mainnet';
    case 56: return 'bnb-mainnet';
    case 137: return 'matic-mainnet';
    case 42161: return 'arb-mainnet';
    case 10: return 'opt-mainnet';
    case 8453: return 'base-mainnet';
    case 43114: return 'avax-mainnet';
    case 100: return 'gnosis-mainnet';
    default: return `chain-${chainId}`;
  }
}

// ---------------------------------------------------------------------------
// Custom network support
// ---------------------------------------------------------------------------

import type { CustomNetwork } from '@/models/types';
import { loadCustomNetworks } from '@/services/storage';

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
