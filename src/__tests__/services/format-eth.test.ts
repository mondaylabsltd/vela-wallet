/**
 * Tests for the consolidated wei→ETH display formatter.
 *
 * This is the single source of truth that replaced byte-identical copies in
 * safe-transaction, bundler-service, SettingsScreen and BundlerFundingModal
 * (and a 4-decimal variant in deployer-api). The threshold ladder is locked here
 * so those surfaces can never silently drift.
 */
import { formatWeiToEth } from '@/services/format-eth';

const ETH = 1_000_000_000_000_000_000n; // 1e18 wei

describe('formatWeiToEth', () => {
  test('exactly zero renders as "0"', () => {
    expect(formatWeiToEth(0n)).toBe('0');
  });

  test('dust below 0.000001 collapses to a "< 0.000001" marker', () => {
    expect(formatWeiToEth(1n)).toBe('< 0.000001');
    expect(formatWeiToEth(999_999_999_999n)).toBe('< 0.000001'); // < 1e12 wei
  });

  test('[0.000001, 0.001) shows 6 decimals', () => {
    expect(formatWeiToEth(1_000_000_000_000n)).toBe('0.000001'); // 1e12 wei
    expect(formatWeiToEth(ETH / 1000n - 1n)).toBe('0.001000'); // just under 0.001 rounds up at 6dp
  });

  test('[0.001, 1) shows 4 decimals', () => {
    expect(formatWeiToEth(ETH / 1000n)).toBe('0.0010'); // 0.001
    expect(formatWeiToEth(5n * ETH / 1000n)).toBe('0.0050'); // 0.005
    expect(formatWeiToEth(ETH / 2n)).toBe('0.5000'); // 0.5
  });

  test('>= 1 shows 3 decimals (the app-wide precision)', () => {
    expect(formatWeiToEth(ETH)).toBe('1.000');
    expect(formatWeiToEth(10n * ETH)).toBe('10.000');
    expect(formatWeiToEth(3n * ETH / 2n)).toBe('1.500'); // 1.5
  });

  // The boundaries are the parts most likely to regress when a caller re-rounds.
  // (ETH - 1n exceeds float64 precision and rounds to exactly 1.0, so we use a
  // cleanly-representable 0.9999 for the just-under-1 case.)
  test('boundary values land on the correct precision tier', () => {
    expect(formatWeiToEth(9999n * ETH / 10000n)).toBe('0.9999'); // just under 1 → 4dp tier
    expect(formatWeiToEth(ETH)).toBe('1.000'); // exactly 1 → 3dp tier
  });
});
