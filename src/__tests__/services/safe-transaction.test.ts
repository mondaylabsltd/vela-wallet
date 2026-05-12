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

import { formatWeiToEth } from '@/services/safe-transaction';

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
});
