import {
  isTempoChain,
  attoToTokenUnits,
  tempoFeeTokenUnits,
  tempoReimbursement,
  TEMPO_DEFAULT_FEE_TOKEN,
  TEMPO_FEE_TOKEN_DECIMALS,
  TEMPO_BASE_FEE_ATTO,
  TEMPO_OUTER_OVERHEAD_GAS,
} from '@/services/tempo';

describe('tempo gas model', () => {
  describe('isTempoChain', () => {
    it('is true for Tempo mainnet (4217) and testnet (42431)', () => {
      expect(isTempoChain(4217)).toBe(true);
      expect(isTempoChain(42431)).toBe(true);
    });
    it('is false for native-coin EVM chains', () => {
      for (const id of [1, 56, 137, 42161, 10, 8453, 100, 43114]) {
        expect(isTempoChain(id)).toBe(false);
      }
    });
  });

  describe('attoToTokenUnits', () => {
    it('converts attodollars (USD×1e-18) to 6-decimal token units', () => {
      // 1e15 attodollars = $0.001 = 1000 microdollars
      expect(attoToTokenUnits(1_000_000_000_000_000n, 6)).toBe(1000n);
    });
    it('defaults to TIP-20 6 decimals', () => {
      expect(attoToTokenUnits(10n ** 18n)).toBe(10n ** BigInt(TEMPO_FEE_TOKEN_DECIMALS));
    });
    it('clamps non-positive input to 0', () => {
      expect(attoToTokenUnits(0n)).toBe(0n);
      expect(attoToTokenUnits(-5n)).toBe(0n);
    });
  });

  describe('tempoFeeTokenUnits', () => {
    it('prices (gas + overhead) × gasPrice in fee-token units', () => {
      // (50_000 + 150_000) × 20e9 atto = 4e15 atto = $0.004 = 4000 units
      const fee = tempoFeeTokenUnits(50_000n, TEMPO_BASE_FEE_ATTO, 6);
      const expected =
        ((50_000n + TEMPO_OUTER_OVERHEAD_GAS) * TEMPO_BASE_FEE_ATTO * 10n ** 6n) /
        10n ** 18n;
      expect(fee).toBe(expected);
      expect(fee).toBe(4000n);
    });
    it('falls back to the protocol base fee when gasPrice is 0', () => {
      expect(tempoFeeTokenUnits(50_000n, 0n, 6)).toBe(
        tempoFeeTokenUnits(50_000n, TEMPO_BASE_FEE_ATTO, 6),
      );
    });
  });

  describe('tempoReimbursement', () => {
    it('applies the 1.25× safety margin over the raw fee', () => {
      const raw = tempoFeeTokenUnits(50_000n, TEMPO_BASE_FEE_ATTO, 6);
      expect(tempoReimbursement(50_000n, TEMPO_BASE_FEE_ATTO, 6)).toBe((raw * 5n) / 4n);
    });
    it('is never zero (transfer must move a non-zero amount)', () => {
      expect(tempoReimbursement(0n, 0n, 6)).toBeGreaterThan(0n);
    });
  });

  it('default fee token is pathUSD in the reserved 0x20c0 range', () => {
    expect(TEMPO_DEFAULT_FEE_TOKEN.toLowerCase()).toBe(
      '0x20c0000000000000000000000000000000000000',
    );
  });
});
