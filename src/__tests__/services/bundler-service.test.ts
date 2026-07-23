/**
 * Tests for bundler-service helper functions.
 */

// Mock transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/services/chain-registry', () => ({
  fetchChainInfo: jest.fn(async () => null),
}));
global.fetch = jest.fn();

import { formatWei, clearBundlerCache, parseBundlerUnderfunded } from '@/services/bundler-service';

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

  describe('parseBundlerUnderfunded', () => {
    // The exact message the current bundler returns (see vela-relay
    // shared/rpc/handlers.ts). This is the wording that reaches the dApp signing
    // modal — if the server rewords it, this test must be updated in lockstep so
    // the funding modal keeps opening instead of dumping a raw error.
    const CURRENT = 'Transaction failed: Insufficient native balance on dedicated bundler gas account. Spendable: 1472335735010274, required: 2165665034747445. Deposit to: 0xac9cad86756b2fcaa44aa93b9ba5871ef43276f8';

    test('detects and extracts the current bundler wording', () => {
      const r = parseBundlerUnderfunded(CURRENT);
      expect(r).not.toBeNull();
      expect(r!.spendableWei).toBe(1472335735010274n);
      expect(r!.requiredWei).toBe(2165665034747445n);
      expect(r!.depositAddress).toBe('0xac9cad86756b2fcaa44aa93b9ba5871ef43276f8');
      expect(r!.asset).toBe('native');
    });

    test('detects the Tempo pathUSD variant', () => {
      const r = parseBundlerUnderfunded(
        'Insufficient pathUSD balance on dedicated bundler gas account. Spendable: 100, required: 200. Deposit to: 0xAC9cad86756b2fcAA44aA93b9bA5871ef43276f8',
      );
      expect(r).not.toBeNull();
      expect(r!.asset).toBe('pathUSD');
      expect(r!.requiredWei).toBe(200n);
    });

    test('stays backward-compatible with the legacy "bundler EOA" wording', () => {
      const r = parseBundlerUnderfunded('Insufficient balance on dedicated bundler EOA, required: 123');
      expect(r).not.toBeNull();
      expect(r!.requiredWei).toBe(123n);
    });

    test('ignores unrelated errors and empty input', () => {
      expect(parseBundlerUnderfunded('execution reverted: ERC20: transfer amount exceeds balance')).toBeNull();
      expect(parseBundlerUnderfunded(undefined)).toBeNull();
      expect(parseBundlerUnderfunded(null)).toBeNull();
      expect(parseBundlerUnderfunded('')).toBeNull();
    });
  });
});
