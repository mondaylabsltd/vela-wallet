/**
 * Tests for network module.
 */

// Mock react-native transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(), save: jest.fn(), remove: jest.fn(), syncNow: jest.fn(),
}));

import { chainName, nativeSymbol, networkId, DEFAULT_NETWORKS, getAllNetworksSync } from '@/models/network';

describe('network', () => {
  describe('chainName', () => {
    test('returns correct names for all default networks', () => {
      expect(chainName(1)).toBe('Ethereum');
      expect(chainName(56)).toBe('BNB Chain');
      expect(chainName(137)).toBe('Polygon');
      expect(chainName(42161)).toBe('Arbitrum');
      expect(chainName(10)).toBe('Optimism');
      expect(chainName(8453)).toBe('Base');
      expect(chainName(43114)).toBe('Avalanche');
      expect(chainName(100)).toBe('Gnosis');
    });

    test('returns "Chain N" for unknown chain IDs', () => {
      expect(chainName(999)).toBe('Chain 999');
      expect(chainName(31337)).toBe('Chain 31337');
    });
  });

  describe('nativeSymbol', () => {
    test('returns correct symbols', () => {
      expect(nativeSymbol(1)).toBe('ETH');
      expect(nativeSymbol(56)).toBe('BNB');
      expect(nativeSymbol(137)).toBe('POL');
      expect(nativeSymbol(42161)).toBe('ETH');
      expect(nativeSymbol(10)).toBe('ETH');
      expect(nativeSymbol(8453)).toBe('ETH');
      expect(nativeSymbol(43114)).toBe('AVAX');
      expect(nativeSymbol(100)).toBe('xDAI');
    });

    test('defaults to ETH for unknown chains', () => {
      expect(nativeSymbol(999)).toBe('ETH');
    });
  });

  describe('networkId', () => {
    test('returns correct API identifiers', () => {
      expect(networkId(1)).toBe('eth-mainnet');
      expect(networkId(56)).toBe('bnb-mainnet');
      expect(networkId(137)).toBe('matic-mainnet');
      expect(networkId(42161)).toBe('arb-mainnet');
      expect(networkId(10)).toBe('opt-mainnet');
      expect(networkId(8453)).toBe('base-mainnet');
      expect(networkId(43114)).toBe('avax-mainnet');
      expect(networkId(100)).toBe('gnosis-mainnet');
    });

    test('returns chain-N for unknown', () => {
      expect(networkId(999)).toBe('chain-999');
    });
  });

  describe('DEFAULT_NETWORKS', () => {
    test('has 8 default networks', () => {
      expect(DEFAULT_NETWORKS).toHaveLength(8);
    });

    test('all networks have required fields', () => {
      for (const net of DEFAULT_NETWORKS) {
        expect(net.id).toBeTruthy();
        expect(net.displayName).toBeTruthy();
        expect(net.chainId).toBeGreaterThan(0);
        expect(net.rpcURL).toMatch(/^https:\/\//);
        expect(net.explorerURL).toMatch(/^https:\/\//);
        expect(net.bundlerURL).toMatch(/^https:\/\//);
        expect(net.logoURL).toBeTruthy();
      }
    });

    test('all chain IDs are unique', () => {
      const ids = DEFAULT_NETWORKS.map(n => n.chainId);
      expect(new Set(ids).size).toBe(ids.length);
    });
  });

  describe('getAllNetworksSync', () => {
    test('returns at least default networks', () => {
      const all = getAllNetworksSync();
      expect(all.length).toBeGreaterThanOrEqual(DEFAULT_NETWORKS.length);
    });
  });
});
