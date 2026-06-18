/**
 * Adding a custom EVM network by chain ID.
 *
 * Factors the ChainInfo → CustomNetwork conversion + persistence shared by the
 * "Add network" tab in AddTokenPanel and the EIP-681 scan recovery flow in
 * SendScreen (when a scanned request names a network Vela doesn't yet support).
 */
import { refreshCustomNetworks } from '@/models/network';
import type { CompatibilityResult, CustomNetwork } from '@/models/types';
import { fetchChainInfo, type ChainInfo } from '@/services/chain-registry';
import { checkNetworkCompatibility } from '@/services/network-checker';
import { getBundlerServiceURL, saveCustomNetwork } from '@/services/storage';

/** Convert resolved chain metadata + the chosen RPC into a CustomNetwork record. */
export function chainInfoToCustomNetwork(info: ChainInfo, bestRpcUrl?: string | null): CustomNetwork {
  return {
    id: `custom-${info.chainId}`,
    displayName: info.name,
    chainId: info.chainId,
    iconLabel: (info.nativeCurrency?.symbol ?? 'ETH').slice(0, 4),
    iconColor: '#888888',
    iconBg: '#F0F0F0',
    logoURL: info.logoURL ?? '',
    isL2: false,
    rpcURL: bestRpcUrl ?? info.rpcUrl ?? '',
    explorerURL: info.explorerUrl ?? '',
    bundlerURL: `${getBundlerServiceURL()}/${info.chainId}`,
    nativeSymbol: info.nativeCurrency?.symbol ?? 'ETH',
    addedAt: new Date().toISOString(),
  };
}

export type AddNetworkResult =
  | { ok: true; network: CustomNetwork }
  | { ok: false; reason: 'not-found' | 'not-compatible'; error?: string };

/**
 * Resolve a chain ID against the chain registry, verify ERC-4337 / P256
 * compatibility, and persist it as a custom network. Refreshes the in-memory
 * network cache so synchronous lookups (networkForChainId) see it immediately.
 */
export async function addCustomNetworkByChainId(chainId: number): Promise<AddNetworkResult> {
  const info = await fetchChainInfo(chainId);
  if (!info) return { ok: false, reason: 'not-found' };

  const compat: CompatibilityResult = await checkNetworkCompatibility(info.rpcUrls, chainId);
  if (!compat.compatible) return { ok: false, reason: 'not-compatible', error: compat.error };

  const network = chainInfoToCustomNetwork(info, compat.bestRpcUrl);
  await saveCustomNetwork(network);
  await refreshCustomNetworks();
  return { ok: true, network };
}
