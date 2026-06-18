/**
 * Canonical chain table — the single source of truth for every built-in network.
 *
 * Everything else derives from CHAINS:
 *   - DEFAULT_NETWORKS (models/network.ts) maps these into rich Network objects.
 *   - networkId() / nativeSymbol() (models/network.ts) read from here.
 *   - tokenChainId() (models/types.ts) reverse-maps apiNetworkId -> chainId here.
 *
 * `apiNetworkId` and the chainId form an inverse pair: `chainIdToApiNetwork` and
 * `apiNetworkToChainId` must round-trip for every entry (enforced by tests).
 *
 * This module is a LEAF — it imports nothing from models/network or models/types,
 * so both can depend on it without a require cycle. To add a network, add ONE
 * entry here; no other map needs touching.
 */

export interface ChainMeta {
  /** Stable string id used in code/UI (e.g. 'ethereum'). */
  id: string;
  /** Human-readable name (e.g. 'Ethereum'). */
  displayName: string;
  chainId: number;
  /** Identifier used by the balance API and carried on APIToken.network. */
  apiNetworkId: string;
  /** Native gas-token symbol as the wallet displays it (POL, xDAI, USD, …). */
  nativeSymbol: string;
  /** Short label rendered when the logo can't load. */
  iconLabel: string;
  iconColor: string;
  iconBg: string;
  isL2: boolean;
  rpcURL: string;
  explorerURL: string;
  /**
   * How gas is settled on this chain. Defaults to `'native'` (standard ERC-4337:
   * UserOp paid in the native coin via EntryPoint). `'tempo'` marks chains with
   * no native coin where gas is paid in a USD stablecoin — see services/tempo.ts.
   */
  gasModel?: 'native' | 'tempo';
}

export const CHAINS: ChainMeta[] = [
  {
    id: 'ethereum', displayName: 'Ethereum', chainId: 1,
    apiNetworkId: 'eth-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'ETH', iconColor: '#627EEA', iconBg: '#EEF0F8', isL2: false,
    rpcURL: 'https://ethereum-rpc.publicnode.com', explorerURL: 'https://etherscan.io',
  },
  {
    id: 'bnb', displayName: 'BNB Chain', chainId: 56,
    apiNetworkId: 'bnb-mainnet', nativeSymbol: 'BNB',
    iconLabel: 'BNB', iconColor: '#F0B90B', iconBg: '#FFF8E1', isL2: false,
    rpcURL: 'https://bsc-dataseed.binance.org', explorerURL: 'https://bscscan.com',
  },
  {
    id: 'polygon', displayName: 'Polygon', chainId: 137,
    apiNetworkId: 'matic-mainnet', nativeSymbol: 'POL',
    iconLabel: 'POL', iconColor: '#8247E5', iconBg: '#F0EAFF', isL2: true,
    rpcURL: 'https://polygon-bor-rpc.publicnode.com', explorerURL: 'https://polygonscan.com',
  },
  {
    id: 'arbitrum', displayName: 'Arbitrum', chainId: 42161,
    apiNetworkId: 'arb-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'ARB', iconColor: '#28A0F0', iconBg: '#E8F4FD', isL2: true,
    rpcURL: 'https://arb1.arbitrum.io/rpc', explorerURL: 'https://arbiscan.io',
  },
  {
    id: 'optimism', displayName: 'Optimism', chainId: 10,
    apiNetworkId: 'opt-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'OP', iconColor: '#FF0420', iconBg: '#FFECEC', isL2: true,
    rpcURL: 'https://mainnet.optimism.io', explorerURL: 'https://optimistic.etherscan.io',
  },
  {
    id: 'base', displayName: 'Base', chainId: 8453,
    apiNetworkId: 'base-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'BASE', iconColor: '#0052FF', iconBg: '#E8EEFF', isL2: true,
    rpcURL: 'https://mainnet.base.org', explorerURL: 'https://basescan.org',
  },
  {
    id: 'avalanche', displayName: 'Avalanche', chainId: 43114,
    apiNetworkId: 'avax-mainnet', nativeSymbol: 'AVAX',
    iconLabel: 'AVAX', iconColor: '#E84142', iconBg: '#FFF0F0', isL2: false,
    rpcURL: 'https://api.avax.network/ext/bc/C/rpc', explorerURL: 'https://snowtrace.io',
  },
  {
    id: 'gnosis', displayName: 'Gnosis', chainId: 100,
    apiNetworkId: 'gnosis-mainnet', nativeSymbol: 'xDAI',
    iconLabel: 'xDAI', iconColor: '#04795B', iconBg: '#E8F5F0', isL2: false,
    rpcURL: 'https://rpc.gnosischain.com', explorerURL: 'https://gnosisscan.io',
  },
  {
    id: 'unichain', displayName: 'Unichain', chainId: 130,
    apiNetworkId: 'unichain-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'UNI', iconColor: '#F50DB4', iconBg: '#FDE8F6', isL2: true,
    rpcURL: 'https://mainnet.unichain.org', explorerURL: 'https://uniscan.xyz',
  },
  {
    // Tempo has NO native gas coin: gas is paid in USD stablecoins (TIP-20).
    // gasModel 'tempo' routes sends through services/tempo.ts (maxFee=0 UserOp +
    // bundler 0x76 + batched stablecoin reimbursement). See that module.
    id: 'tempo', displayName: 'Tempo', chainId: 4217,
    apiNetworkId: 'tempo-mainnet', nativeSymbol: 'USD',
    iconLabel: 'USD', iconColor: '#0B0B0B', iconBg: '#ECECEC', isL2: false,
    rpcURL: 'https://rpc.mainnet.tempo.xyz', explorerURL: 'https://explore.tempo.xyz',
    gasModel: 'tempo',
  },
  {
    id: 'monad', displayName: 'Monad', chainId: 143,
    apiNetworkId: 'monad-mainnet', nativeSymbol: 'MON',
    iconLabel: 'MON', iconColor: '#836EF9', iconBg: '#EFEBFF', isL2: false,
    rpcURL: 'https://rpc.monad.xyz', explorerURL: 'https://monadscan.com',
  },
  {
    id: 'worldchain', displayName: 'World Chain', chainId: 480,
    apiNetworkId: 'worldchain-mainnet', nativeSymbol: 'ETH',
    iconLabel: 'WLD', iconColor: '#000000', iconBg: '#ECECEC', isL2: true,
    rpcURL: 'https://worldchain.drpc.org', explorerURL: 'https://worldscan.org',
  },
];

/** Look up the canonical metadata for a chain id; undefined if not built in. */
export function chainMeta(chainId: number): ChainMeta | undefined {
  return CHAINS.find(c => c.chainId === chainId);
}

/**
 * chainId -> API network identifier. Custom (non-built-in) chains use the
 * `chain-{chainId}` form, which {@link apiNetworkToChainId} parses back.
 */
export function chainIdToApiNetwork(chainId: number): string {
  return chainMeta(chainId)?.apiNetworkId ?? `chain-${chainId}`;
}

/**
 * API network identifier -> chainId. Inverse of {@link chainIdToApiNetwork}.
 * Falls back to the `chain-{chainId}` form for custom networks, then to
 * Ethereum (1) for anything unrecognized.
 */
export function apiNetworkToChainId(network: string): number {
  const hit = CHAINS.find(c => c.apiNetworkId === network);
  if (hit) return hit.chainId;
  const m = network.match(/^chain-(\d+)$/);
  return m ? parseInt(m[1], 10) : 1;
}
