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

import { formatWeiToEth, calcMaxFeePerGas } from '@/services/safe-transaction';

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

    test('fixed 2.5x markup → 150% margin', () => {
      const maxFee = calcMaxFeePerGas(gasPrice);
      expect(maxFee).toBe(25_000_000_000n); // 25 gwei = 10 × 2.5
    });

    test('margin = maxFee / gasPrice - 1 = 150%', () => {
      const maxFee = calcMaxFeePerGas(gasPrice);
      const margin = Number(maxFee - gasPrice) / Number(gasPrice);
      expect(margin).toBeCloseTo(1.5, 5); // 150%
    });

    test('floor: gasPrice=0 → 1 wei', () => {
      expect(calcMaxFeePerGas(0n)).toBe(1n);
    });

    test('works with typical chain gas prices', () => {
      // Polygon ~280 gwei
      const poly = calcMaxFeePerGas(280_000_000_000n);
      expect(poly).toBe(700_000_000_000n); // 700 gwei = 280 × 2.5

      // Ethereum ~30 gwei
      const eth = calcMaxFeePerGas(30_000_000_000n);
      expect(eth).toBe(75_000_000_000n); // 75 gwei = 30 × 2.5

      // BSC ~1 gwei
      const bsc = calcMaxFeePerGas(1_000_000_000n);
      expect(bsc).toBe(2_500_000_000n); // 2.5 gwei
    });
  });
});
