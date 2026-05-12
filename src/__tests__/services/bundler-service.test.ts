/**
 * Tests for bundler-service helper functions.
 */

// Mock transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(), save: jest.fn(), remove: jest.fn(), syncNow: jest.fn(),
}));
jest.mock('@/services/chain-registry', () => ({
  fetchChainInfo: jest.fn(async () => null),
}));
global.fetch = jest.fn();

import { formatWei, clearBundlerCache } from '@/services/bundler-service';

describe('bundler-service', () => {
  describe('formatWei', () => {
    test('formats zero', () => {
      expect(formatWei(0n)).toBe('0');
    });

    test('formats dust amounts', () => {
      expect(formatWei(1n)).toBe('< 0.000001');
      expect(formatWei(999_999_999n)).toBe('< 0.000001');
    });

    test('formats small amounts', () => {
      expect(formatWei(1_000_000_000_000n)).toBe('0.000001');
    });

    test('formats typical gas fund amounts (0.005 ETH)', () => {
      const fiveMilliEth = 5_000_000_000_000_000n;
      expect(formatWei(fiveMilliEth)).toBe('0.0050');
    });

    test('formats 1 ETH', () => {
      expect(formatWei(1_000_000_000_000_000_000n)).toBe('1.000');
    });

    test('formats large amounts', () => {
      expect(formatWei(10_000_000_000_000_000_000n)).toBe('10.000');
    });
  });

  describe('clearBundlerCache', () => {
    test('does not throw when clearing empty cache', () => {
      expect(() => clearBundlerCache(1, '0xabc')).not.toThrow();
      expect(() => clearBundlerCache(1)).not.toThrow();
    });
  });
});
