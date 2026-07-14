export const SHARED_WEBAUTHN_OWNER = '0x94a4F6affBd8975951142c3999aEAB7ecee555c2' as const;
export const DEFAULT_RP_ID = 'getvela.app';
export const SAFE_ORIGINS = new Set(['https://app.safe.global']);
export const MESSAGE_CHANNEL = 'vela-safe-recovery-v1';
export const PROVIDER_UUID = '5ad1b6bc-5482-4f12-89d1-56535245434f';
export const PROVIDER_NAME = 'Vela Safe Recovery';
export const PROVIDER_RDNS = 'app.getvela.recovery';

export const DEFAULT_RPC_URLS: Record<number, string> = {
  1: 'https://eth.llamarpc.com',
  10: 'https://mainnet.optimism.io',
  56: 'https://bsc-dataseed.binance.org',
  100: 'https://rpc.gnosis.gateway.fm',
  137: 'https://polygon.drpc.org',
  324: 'https://mainnet.era.zksync.io',
  8453: 'https://mainnet.base.org',
  42161: 'https://arb1.arbitrum.io/rpc',
  43114: 'https://api.avax.network/ext/bc/C/rpc',
  59144: 'https://rpc.linea.build',
  534352: 'https://rpc.scroll.io',
};

export const DEFAULT_CHAIN_NAMES: Record<number, string> = {
  1: 'Ethereum',
  10: 'Optimism',
  56: 'BNB Smart Chain',
  100: 'Gnosis Chain',
  137: 'Polygon',
  324: 'zkSync Era',
  8453: 'Base',
  42161: 'Arbitrum One',
  43114: 'Avalanche C-Chain',
  59144: 'Linea',
  534352: 'Scroll',
};

export const STORAGE_KEY = 'velaSafeRecoverySettings';
export const REQUEST_TIMEOUT_MS = 3 * 60_000;
export const RPC_TIMEOUT_MS = 30_000;
export const MAX_RPC_RESPONSE_BYTES = 2 * 1024 * 1024;

export const PROVIDER_ICON =
  'data:image/svg+xml,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">' +
      '<rect width="128" height="128" rx="28" fill="#171916"/>' +
      '<path d="M28 31h72v18H49v15h36v17H49v16h51v18H28z" fill="#f15a35"/>' +
      '<circle cx="91" cy="72" r="18" fill="#171916" stroke="#f5f1e8" stroke-width="8"/>' +
      '<path d="M91 66v13m-6-7h12" stroke="#f5f1e8" stroke-width="5" stroke-linecap="round"/>' +
    '</svg>',
  );
