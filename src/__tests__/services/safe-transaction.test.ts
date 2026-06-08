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

    test('standard tier: gasPrice × 1.2 × 2.5 = 30 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'standard')).toBe(30_000_000_000n);
    });

    test('slow tier: gasPrice × 1.1 × 2.5 = 27.5 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'slow')).toBe(27_500_000_000n);
    });

    test('rapid tier: gasPrice × 1.5 × 2.5 = 37.5 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'rapid')).toBe(37_500_000_000n);
    });

    test('fast tier: gasPrice × 2.0 × 2.5 = 50 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'fast')).toBe(50_000_000_000n);
    });

    test('default tier is standard', () => {
      expect(calcMaxFeePerGas(gasPrice)).toBe(calcMaxFeePerGas(gasPrice, 'standard'));
    });

    test('floor: gasPrice=0 → 1 wei', () => {
      expect(calcMaxFeePerGas(0n)).toBe(1n);
    });

    test('margin is constant 150% across all tiers', () => {
      // margin = BUNDLER_MARGIN - 1 = 2.5 - 1 = 1.5, for ALL tiers
      const tiers: GasTier[] = ['slow', 'standard', 'rapid', 'fast'];
      for (const tier of tiers) {
        const m = GAS_TIER_MULTIPLIERS[tier];
        const outerGasPrice = (gasPrice * m.num) / m.den; // gasPrice × speedTier
        const maxFee = calcMaxFeePerGas(gasPrice, tier);
        const margin = Number(maxFee - outerGasPrice) / Number(outerGasPrice);
        expect(margin).toBeCloseTo(1.5, 5); // always 150%
      }
    });

    test('user cost scales with tier (faster = more expensive)', () => {
      const slow = calcMaxFeePerGas(gasPrice, 'slow');
      const std = calcMaxFeePerGas(gasPrice, 'standard');
      const rapid = calcMaxFeePerGas(gasPrice, 'rapid');
      const fast = calcMaxFeePerGas(gasPrice, 'fast');
      expect(slow < std).toBe(true);
      expect(std < rapid).toBe(true);
      expect(rapid < fast).toBe(true);
    });
  });
});
