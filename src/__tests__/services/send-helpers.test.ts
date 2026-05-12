/**
 * Tests for SendScreen helper functions.
 *
 * These are pure utility functions extracted/mirrored from SendScreen
 * for testability. Tests cover edge cases in amount conversion and validation.
 */

// Mirror the functions from SendScreen (they're not exported, so we re-implement them here
// to verify the logic is correct)

function isValidAddress(addr: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(addr);
}

function amountToWeiHex(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const intPart = parts[0] || '0';
  let fracPart = parts[1] || '';
  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, '0');
  }
  const weiStr = (intPart + fracPart).replace(/^0+/, '') || '0';
  let n = BigInt(weiStr);
  return n.toString(16);
}

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

describe('SendScreen helpers', () => {
  describe('isValidAddress', () => {
    test('accepts valid checksummed addresses', () => {
      expect(isValidAddress('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toBe(true);
    });

    test('accepts valid lowercase addresses', () => {
      expect(isValidAddress('0x0000000000000000000000000000000000000001')).toBe(true);
    });

    test('rejects short addresses', () => {
      expect(isValidAddress('0x1234')).toBe(false);
    });

    test('rejects addresses without 0x prefix', () => {
      expect(isValidAddress('d8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toBe(false);
    });

    test('rejects empty string', () => {
      expect(isValidAddress('')).toBe(false);
    });

    test('rejects addresses with non-hex characters', () => {
      expect(isValidAddress('0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG')).toBe(false);
    });

    test('rejects addresses that are too long', () => {
      expect(isValidAddress('0x' + '0'.repeat(42))).toBe(false);
    });
  });

  describe('amountToWeiHex', () => {
    test('converts 1 ETH (18 decimals)', () => {
      const result = amountToWeiHex('1', 18);
      expect(BigInt('0x' + result)).toBe(10n ** 18n);
    });

    test('converts 0.1 ETH', () => {
      const result = amountToWeiHex('0.1', 18);
      expect(BigInt('0x' + result)).toBe(10n ** 17n);
    });

    test('converts 1.5 USDC (6 decimals)', () => {
      const result = amountToWeiHex('1.5', 6);
      expect(BigInt('0x' + result)).toBe(1_500_000n);
    });

    test('converts whole number', () => {
      const result = amountToWeiHex('100', 6);
      expect(BigInt('0x' + result)).toBe(100_000_000n);
    });

    test('truncates excess decimals', () => {
      // 1.1234567 with 6 decimals → 1.123456 (truncated, not rounded)
      const result = amountToWeiHex('1.1234567', 6);
      expect(BigInt('0x' + result)).toBe(1_123_456n);
    });

    test('handles zero', () => {
      const result = amountToWeiHex('0', 18);
      expect(result).toBe('0');
    });

    test('handles 0 decimals', () => {
      const result = amountToWeiHex('42', 0);
      expect(BigInt('0x' + result)).toBe(42n);
    });

    test('handles very small amounts', () => {
      const result = amountToWeiHex('0.000000000000000001', 18);
      expect(BigInt('0x' + result)).toBe(1n);
    });

    test('handles large amounts without overflow', () => {
      // 1 billion tokens with 18 decimals
      const result = amountToWeiHex('1000000000', 18);
      expect(BigInt('0x' + result)).toBe(1_000_000_000n * 10n ** 18n);
    });

    test('handles input like "0.0"', () => {
      const result = amountToWeiHex('0.0', 18);
      expect(result).toBe('0');
    });
  });

  describe('formatUsd', () => {
    test('formats typical values', () => {
      expect(formatUsd(1234.56)).toBe('$1,234.56');
    });

    test('formats zero', () => {
      expect(formatUsd(0)).toBe('$0.00');
    });

    test('formats small values', () => {
      expect(formatUsd(0.01)).toBe('$0.01');
    });
  });
});
