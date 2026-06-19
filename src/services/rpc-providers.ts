/**
 * Third-party RPC providers — Alchemy, dRPC, Ankr.
 *
 * Each provider works with a SINGLE global API key that unlocks every network
 * the provider serves, and none requires a credit card to obtain that key. We
 * store one key per provider (see storage.ts) and build the per-chain RPC URL
 * on demand from a static slug map.
 *
 * This module is a LEAF — it imports only the canonical chain table, so both the
 * RPC pool (services/rpc-pool.ts) and the settings UI can depend on it without a
 * require cycle.
 *
 * The slug maps below were verified live against each provider (probe
 * `eth_chainId`, confirm the reported id) across all 12 Vela chains: Alchemy and
 * dRPC serve all 12; Ankr serves 8 (no Unichain / World Chain / Monad / Tempo).
 * Correctness is still re-checked at runtime — the pool bans any URL that returns
 * an auth/access error, and the settings screen's capability test probes
 * `eth_chainId` and checks the reported id matches before showing a network as
 * supported. A wrong/missing slug therefore degrades to "unavailable", never to
 * silently serving the wrong chain.
 */

import { CHAINS } from '@/models/chains';

export type ProviderId = 'alchemy' | 'drpc' | 'ankr';

/** One stored API key per provider. */
export type RpcProviderKeys = Partial<Record<ProviderId, string>>;

/**
 * Cold-start preference order within the provider tier. All providers share the
 * same base score in the pool, so this only breaks ties until real latency data
 * exists (then the fastest provider wins automatically — see rpc-pool.ts).
 * Rationale: Alchemy best reliability/coverage; dRPC multi-provider failover;
 * Ankr broad coverage.
 */
export const PROVIDER_ORDER: ProviderId[] = ['alchemy', 'drpc', 'ankr'];

/** UI metadata for each provider. */
export interface ProviderMeta {
  id: ProviderId;
  label: string;
  /** Placeholder shown in the key input. */
  keyPlaceholder: string;
  /** Dashboard URL where the user creates/copies a key. */
  keyUrl: string;
}

export const PROVIDERS: Record<ProviderId, ProviderMeta> = {
  alchemy: {
    id: 'alchemy',
    label: 'Alchemy',
    keyPlaceholder: 'API key',
    keyUrl: 'https://dashboard.alchemy.com/apikeys',
  },
  drpc: {
    id: 'drpc',
    label: 'dRPC',
    keyPlaceholder: 'dkey',
    keyUrl: 'https://drpc.org/dashboard',
  },
  ankr: {
    id: 'ankr',
    label: 'Ankr',
    keyPlaceholder: 'API key',
    keyUrl: 'https://www.ankr.com/rpc/projects/',
  },
};

/**
 * chainId -> provider network slug. A chain absent from a provider's map means
 * we don't build a URL for that provider on that chain.
 */
const PROVIDER_CHAIN_SLUGS: Record<ProviderId, Record<number, string>> = {
  // https://{slug}.g.alchemy.com/v2/{key} — serves all 12 Vela chains.
  alchemy: {
    1: 'eth-mainnet',
    56: 'bnb-mainnet',
    137: 'polygon-mainnet',
    42161: 'arb-mainnet',
    10: 'opt-mainnet',
    8453: 'base-mainnet',
    43114: 'avax-mainnet',
    100: 'gnosis-mainnet',
    130: 'unichain-mainnet',
    4217: 'tempo-mainnet',
    143: 'monad-mainnet',
    480: 'worldchain-mainnet',
  },
  // https://lb.drpc.org/ogrpc?network={slug}&dkey={key} — serves all 12 Vela chains.
  drpc: {
    1: 'ethereum',
    56: 'bsc',
    137: 'polygon',
    42161: 'arbitrum',
    10: 'optimism',
    8453: 'base',
    43114: 'avalanche',
    100: 'gnosis',
    130: 'unichain',
    4217: 'tempo',
    143: 'monad',
    480: 'worldchain',
  },
  // https://rpc.ankr.com/{slug}/{key} — Ankr does NOT serve Unichain, World Chain,
  // Monad (mainnet) or Tempo (verified via its public endpoints), so they're omitted.
  ankr: {
    1: 'eth',
    56: 'bsc',
    137: 'polygon',
    42161: 'arbitrum',
    10: 'optimism',
    8453: 'base',
    43114: 'avalanche',
    100: 'gnosis',
  },
};

/** Build the RPC URL for a provider/chain/key, or undefined if unsupported. */
export function buildProviderRpcUrl(id: ProviderId, chainId: number, key: string): string | undefined {
  const slug = PROVIDER_CHAIN_SLUGS[id]?.[chainId];
  if (!slug || !key) return undefined;
  switch (id) {
    case 'alchemy':
      return `https://${slug}.g.alchemy.com/v2/${key}`;
    case 'drpc':
      return `https://lb.drpc.org/ogrpc?network=${slug}&dkey=${key}`;
    case 'ankr':
      return `https://rpc.ankr.com/${slug}/${key}`;
  }
}

/** Chain ids this provider can serve (i.e. we have a slug for), in canonical CHAINS display order. */
export function providerChainIds(id: ProviderId): number[] {
  const slugs = PROVIDER_CHAIN_SLUGS[id] ?? {};
  return CHAINS.filter(c => slugs[c.chainId] !== undefined).map(c => c.chainId);
}
