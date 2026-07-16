import { describe, expect, it } from 'vitest';
import type { RecoverySettings } from './types';
import { configuredNetwork, configuredNetworks } from './networks';

const settings = {
  enabled: true,
  rpId: 'getvela.app',
  chainId: 56,
  chainNames: {
    '1': 'Ethereum',
    '56': 'BNB Smart Chain',
    '777': 'Custom Network',
  },
  rpcUrls: {
    '1': 'https://ethereum.example',
    '56': 'https://bsc.example',
    '777': 'https://custom.example',
    invalid: 'https://invalid.example',
    '-1': 'https://negative.example',
  },
  credentialIds: {},
  relayerPrivateKey: `0x${'11'.repeat(32)}`,
  localConfirmations: {},
} satisfies RecoverySettings;

describe('configuredNetworks', () => {
  it('returns preset and custom networks in display-name order', () => {
    expect(configuredNetworks(settings)).toEqual([
      {
        chainId: 56,
        chainName: 'BNB Smart Chain',
        rpcUrl: 'https://bsc.example',
        nativeSymbol: 'BNB',
      },
      {
        chainId: 777,
        chainName: 'Custom Network',
        rpcUrl: 'https://custom.example',
        nativeSymbol: 'native',
      },
      {
        chainId: 1,
        chainName: 'Ethereum',
        rpcUrl: 'https://ethereum.example',
        nativeSymbol: 'ETH',
      },
    ]);
  });

  it('finds a configured network and rejects an unknown chain', () => {
    expect(configuredNetwork(settings, 56)?.rpcUrl).toBe('https://bsc.example');
    expect(configuredNetwork(settings, 10)).toBeUndefined();
  });
});
