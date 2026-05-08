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
const CHAIN_LOGO_BASE = 'https://ethereum-data.awesometools.dev/chains';

export const DEFAULT_NETWORKS: Network[] = [
  {
    id: 'ethereum', displayName: 'Ethereum', chainId: 1,
    iconLabel: 'ETH', iconColor: '#627EEA', iconBg: '#EEF0F8',
    logoURL: `${CHAIN_LOGO_BASE}/1/logo.png`, isL2: false,
    rpcURL: 'https://eth.llamarpc.com', explorerURL: 'https://etherscan.io',
    bundlerURL: 'https://api.pimlico.io/v2/1/rpc',
  },
  {
    id: 'bnb', displayName: 'BNB Chain', chainId: 56,
    iconLabel: 'BNB', iconColor: '#F0B90B', iconBg: '#FFF8E1',
    logoURL: `${CHAIN_LOGO_BASE}/56/logo.png`, isL2: false,
    rpcURL: 'https://bsc-dataseed.binance.org', explorerURL: 'https://bscscan.com',
    bundlerURL: 'https://api.pimlico.io/v2/56/rpc',
  },
  {
    id: 'polygon', displayName: 'Polygon', chainId: 137,
    iconLabel: 'POL', iconColor: '#8247E5', iconBg: '#F0EAFF',
    logoURL: `${CHAIN_LOGO_BASE}/137/logo.png`, isL2: true,
    rpcURL: 'https://polygon-rpc.com', explorerURL: 'https://polygonscan.com',
    bundlerURL: 'https://api.pimlico.io/v2/137/rpc',
  },
  {
    id: 'arbitrum', displayName: 'Arbitrum', chainId: 42161,
    iconLabel: 'ARB', iconColor: '#28A0F0', iconBg: '#E8F4FD',
    logoURL: `${CHAIN_LOGO_BASE}/42161/logo.png`, isL2: true,
    rpcURL: 'https://arb1.arbitrum.io/rpc', explorerURL: 'https://arbiscan.io',
    bundlerURL: 'https://api.pimlico.io/v2/42161/rpc',
  },
  {
    id: 'optimism', displayName: 'Optimism', chainId: 10,
    iconLabel: 'OP', iconColor: '#FF0420', iconBg: '#FFECEC',
    logoURL: `${CHAIN_LOGO_BASE}/10/logo.png`, isL2: true,
    rpcURL: 'https://mainnet.optimism.io', explorerURL: 'https://optimistic.etherscan.io',
    bundlerURL: 'https://api.pimlico.io/v2/10/rpc',
  },
  {
    id: 'base', displayName: 'Base', chainId: 8453,
    iconLabel: 'BASE', iconColor: '#0052FF', iconBg: '#E8EEFF',
    logoURL: `${CHAIN_LOGO_BASE}/8453/logo.png`, isL2: true,
    rpcURL: 'https://mainnet.base.org', explorerURL: 'https://basescan.org',
    bundlerURL: 'https://api.pimlico.io/v2/8453/rpc',
  },
  {
    id: 'avalanche', displayName: 'Avalanche', chainId: 43114,
    iconLabel: 'AVAX', iconColor: '#E84142', iconBg: '#FFF0F0',
    logoURL: `${CHAIN_LOGO_BASE}/43114/logo.png`, isL2: false,
    rpcURL: 'https://api.avax.network/ext/bc/C/rpc', explorerURL: 'https://snowtrace.io',
    bundlerURL: 'https://api.pimlico.io/v2/43114/rpc',
  },
];

/** Lookup chain display name by ID. */
export function chainName(chainId: number): string {
  return DEFAULT_NETWORKS.find(n => n.chainId === chainId)?.displayName ?? `Chain ${chainId}`;
}

/** Lookup native token symbol by chain ID. */
export function nativeSymbol(chainId: number): string {
  switch (chainId) {
    case 1: case 42161: case 10: case 8453: return 'ETH';
    case 56: return 'BNB';
    case 137: return 'POL';
    case 43114: return 'AVAX';
    default: return 'ETH';
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
    default: return 'eth-mainnet';
  }
}
