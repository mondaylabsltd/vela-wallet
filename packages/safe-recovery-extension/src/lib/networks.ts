import { DEFAULT_NATIVE_SYMBOLS } from './constants';
import type { RecoverySettings } from './types';

export interface ConfiguredNetwork {
  chainId: number;
  chainName: string;
  rpcUrl: string;
  nativeSymbol: string;
}

export function configuredNetworks(settings: RecoverySettings): ConfiguredNetwork[] {
  return Object.entries(settings.rpcUrls)
    .map(([rawChainId, rpcUrl]) => {
      const chainId = Number(rawChainId);
      if (!Number.isSafeInteger(chainId) || chainId <= 0 || !rpcUrl) return undefined;
      return {
        chainId,
        chainName: settings.chainNames[rawChainId] ?? `Chain ${chainId}`,
        rpcUrl,
        nativeSymbol: DEFAULT_NATIVE_SYMBOLS[chainId] ?? 'native',
      };
    })
    .filter((network): network is ConfiguredNetwork => network !== undefined)
    .sort((left, right) => left.chainName.localeCompare(right.chainName));
}

export function configuredNetwork(
  settings: RecoverySettings,
  chainId: number,
): ConfiguredNetwork | undefined {
  return configuredNetworks(settings).find((network) => network.chainId === chainId);
}
