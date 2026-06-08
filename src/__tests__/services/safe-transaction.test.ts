/**
 * Tests for safe-transaction service.
 *
 * Tests the pure functions that build calldata, compute hashes, and format values.
 * RPC-dependent functions are not tested here (require network mocking).
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

import { formatWeiToEth, calcMaxFeePerGas, GAS_TIER_MULTIPLIERS } from '@/services/safe-transaction';
import type { GasTier } from '@/services/safe-transaction';

describe('safe-transaction', () => {
  describe('formatWeiToEth', () => {
    test('formats zero', () => {
      expect(formatWeiToEth(0n)).toBe('0');
    });

    test('formats very small amounts', () => {
      expect(formatWeiToEth(1n)).toBe('< 0.000001');
      expect(formatWeiToEth(999n)).toBe('< 0.000001');
    });

    test('formats small amounts with 6 decimals', () => {
      // 0.001 ETH = 1e15 wei
      const result = formatWeiToEth(1_000_000_000_000n); // 0.000001 ETH
      expect(result).toBe('0.000001');
    });

    test('formats medium amounts with 4 decimals', () => {
      // 0.5 ETH = 5e17 wei
      const result = formatWeiToEth(500_000_000_000_000_000n);
      expect(result).toBe('0.5000');
    });

    test('formats amounts >= 1 with 3 decimals', () => {
      // 1.5 ETH
      const result = formatWeiToEth(1_500_000_000_000_000_000n);
      expect(result).toBe('1.500');
    });

    test('formats large amounts', () => {
      // 100 ETH
      const result = formatWeiToEth(100_000_000_000_000_000_000n);
      expect(result).toBe('100.000');
    });

    test('handles typical gas fees (0.001-0.01 ETH)', () => {
      // 0.005 ETH = 5e15 wei
      const result = formatWeiToEth(5_000_000_000_000_000n);
      expect(result).toBe('0.0050');
    });
  });

  // --- calcMaxFeePerGas: margin = tier - 1, clamped to [1.1x, 2.0x] ---

  describe('calcMaxFeePerGas', () => {
    const gasPrice = 10_000_000_000n; // 10 gwei

    test('standard tier: 1.2x → 20% margin', () => {
      const maxFee = calcMaxFeePerGas(gasPrice, 'standard');
      expect(maxFee).toBe(12_000_000_000n); // 12 gwei
    });

    test('slow tier: clamped to 1.1x minimum', () => {
      // slow = 1.1x, already at minimum
      const maxFee = calcMaxFeePerGas(gasPrice, 'slow');
      expect(maxFee).toBe(11_000_000_000n); // 11 gwei (1.1x)
    });

    test('rapid tier: 1.5x → 50% margin', () => {
      const maxFee = calcMaxFeePerGas(gasPrice, 'rapid');
      expect(maxFee).toBe(15_000_000_000n);
    });

    test('fast tier: clamped to 2.0x maximum', () => {
      // fast = 2.0x, at maximum
      const maxFee = calcMaxFeePerGas(gasPrice, 'fast');
      expect(maxFee).toBe(20_000_000_000n); // 20 gwei (2.0x cap)
    });

    test('default tier is standard', () => {
      const maxFee = calcMaxFeePerGas(gasPrice);
      expect(maxFee).toBe(12_000_000_000n);
    });

    test('floor: never below 1 wei', () => {
      const maxFee = calcMaxFeePerGas(0n, 'standard');
      expect(maxFee).toBe(1n);
    });

    test('margin formula: margin = maxFee / gasPrice - 1', () => {
      const tiers: GasTier[] = ['slow', 'standard', 'rapid', 'fast'];
      const expectedMargins = [0.1, 0.2, 0.5, 1.0]; // 10%, 20%, 50%, 100%

      for (let i = 0; i < tiers.length; i++) {
        const maxFee = calcMaxFeePerGas(gasPrice, tiers[i]);
        const margin = Number(maxFee - gasPrice) / Number(gasPrice);
        expect(margin).toBeCloseTo(expectedMargins[i], 5);
      }
    });
  });
});
