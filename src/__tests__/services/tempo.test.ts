import {
  isTempoChain,
  attoToTokenUnits,
  tempoFeeTokenUnits,
  tempoReimbursement,
  tempoSettlementSplit,
  tempoCallGasLimit,
  tempoExpectedGas,
  tempoSplitSafetyGas,
  TEMPO_DEFAULT_FEE_TOKEN,
  TEMPO_FEE_TOKEN_DECIMALS,
  TEMPO_BASE_FEE_ATTO,
  TEMPO_OUTER_OVERHEAD_GAS,
  TEMPO_CALL_GAS_PER_SUBCALL,
  TEMPO_DEPLOYED_GAS_EST,
  TEMPO_DEPLOY_GAS_EST,
  TEMPO_PER_SUBCALL_GAS_EST,
  TEMPO_COST_BUFFER_GAS,
  TEMPO_SPLIT_SAFETY_GAS,
  TEMPO_SPLIT_SAFETY_BPS,
} from '@/services/tempo';

/** Bundler's accept-check cost basis: ceilDiv((simGas + COST_BUFFER) × price → fee units). */
function bundlerCostUnits(simGas: bigint, price: bigint, decimals = 6): bigint {
  const atto = (simGas + TEMPO_COST_BUFFER_GAS) * price;
  const num = atto * 10n ** BigInt(decimals);
  return (num + 10n ** 18n - 1n) / 10n ** 18n; // ceilDiv — matches vela-bundler tempoCostInFeeToken
}

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

  describe('tempoExpectedGas', () => {
    it('prices a deployed simple send (2 sub-calls) near the measured ~420k on-chain gas', () => {
      const gas = tempoExpectedGas(true, 2);
      expect(gas).toBe(TEMPO_DEPLOYED_GAS_EST + 2n * TEMPO_PER_SUBCALL_GAS_EST);
      // Sanity: within a reasonable band of the real ~420k so the 2× charge ≈ 2× actual.
      expect(gas).toBeGreaterThan(380_000n);
      expect(gas).toBeLessThan(520_000n);
    });
    it('includes the Safe-deploy cost for an undeployed sender', () => {
      expect(tempoExpectedGas(false, 2)).toBeGreaterThan(TEMPO_DEPLOY_GAS_EST);
    });
  });

  describe('tempoReimbursement', () => {
    it('charges 2× (100% margin) the realistic cost — NOT the padded limits', () => {
      const gas = 500_000n;
      const raw = attoToTokenUnits(gas * TEMPO_BASE_FEE_ATTO, 6);
      expect(tempoReimbursement(gas, TEMPO_BASE_FEE_ATTO, 6)).toBe(raw * 2n);
    });
    it('is never zero (transfer must move a non-zero amount)', () => {
      expect(tempoReimbursement(0n, 0n, 6)).toBeGreaterThan(0n);
    });
  });

  describe('tempoSettlementSplit', () => {
    const price = TEMPO_BASE_FEE_ATTO;
    const gas = 500_000n;

    it('floors the EOA at the bundler cost (expectedGas + buffer + safety) and gives treasury the surplus', () => {
      const reimbursement = tempoReimbursement(gas, price, 6); // = 2× base
      const split = tempoSettlementSplit(reimbursement, gas, price, 6);
      const expectedFloor = attoToTokenUnits((gas + TEMPO_COST_BUFFER_GAS + TEMPO_SPLIT_SAFETY_GAS) * price, 6);
      expect(split.eoa).toBe(expectedFloor);
      expect(split.treasury).toBe(reimbursement - expectedFloor);
    });

    it('conserves the total (eoa + treasury == reimbursement)', () => {
      const reimbursement = tempoReimbursement(gas, price, 6);
      const split = tempoSettlementSplit(reimbursement, gas, price, 6);
      expect(split.eoa + split.treasury).toBe(reimbursement);
    });

    it("the EOA share always clears the bundler's cost (realGas + buffer)", () => {
      const reimbursement = tempoReimbursement(gas, price, 6);
      const split = tempoSettlementSplit(reimbursement, gas, price, 6);
      // Bundler requires reimbursed_to_EOA >= (gasUsed + TEMPO_COST_BUFFER_GAS) * price.
      // With gasUsed ~= expectedGas, the floor beats it by the safety cushion.
      const bundlerCost = attoToTokenUnits((gas + TEMPO_COST_BUFFER_GAS) * price, 6);
      expect(split.eoa).toBeGreaterThanOrEqual(bundlerCost);
      expect(split.treasury).toBeGreaterThan(0n);
    });

    it('keeps everything on the EOA (treasury 0) when the margin is too thin — never a rejection', () => {
      const floor = attoToTokenUnits((gas + TEMPO_COST_BUFFER_GAS + TEMPO_SPLIT_SAFETY_GAS) * price, 6);
      const thin = floor - 1n; // reimbursement below the floor
      const split = tempoSettlementSplit(thin, gas, price, 6);
      expect(split.eoa).toBe(thin);
      expect(split.treasury).toBe(0n);
    });

    // Regression for the reported Tempo deploy rejection (reimbursed=89700 < cost=90025).
    // The OLD tests fed the SAME gas to both the wallet floor and the bundler cost, so they
    // could never catch the real failure: the wallet prices its floor off a realistic-gas
    // ESTIMATE while the bundler prices its cost off a HIGHER simulated gas. Here the wallet
    // estimate is deliberately BELOW the bundler's simGas — the proportional cushion must still
    // carry the EOA floor over the bundler cost.
    it('EOA floor clears the bundler cost when the wallet estimate is BELOW the bundler simGas (deploy failure mode)', () => {
      const walletGas = 4_385_000n; // wallet's realistic model for a 3-sub-call undeployed send
      const bundlerSimGas = 4_421_208n; // the bundler's actual simulated gas — 36,208 higher
      const reimbursement = tempoReimbursement(walletGas, price, 6);
      const split = tempoSettlementSplit(reimbursement, walletGas, price, 6);
      expect(split.eoa).toBeGreaterThanOrEqual(bundlerCostUnits(bundlerSimGas, price));
      // And with the exact numbers from the incident, the floor must beat the 90,025 it was rejected under.
      expect(split.eoa).toBeGreaterThan(90_025n);
      expect(split.treasury).toBeGreaterThan(0n); // still routes surplus to the treasury
    });

    it('clears the bundler cost across a wide range of estimate error, up to +5% simGas drift', () => {
      for (const walletGas of [500_000n, 1_500_000n, 4_385_000n, 6_000_000n]) {
        const reimbursement = tempoReimbursement(walletGas, price, 6);
        const split = tempoSettlementSplit(reimbursement, walletGas, price, 6);
        // Bundler simGas up to 3% above the wallet estimate (proportional cushion is 3%).
        const simGas = walletGas + (walletGas * 3n) / 100n;
        expect(split.eoa).toBeGreaterThanOrEqual(bundlerCostUnits(simGas, price));
      }
    });
  });

  describe('tempoSplitSafetyGas', () => {
    it('is the flat minimum for small ops (proportional share below the floor)', () => {
      // 500k × 3% = 15k < 20k flat → flat wins. Keeps small-send behaviour identical.
      expect(tempoSplitSafetyGas(500_000n)).toBe(TEMPO_SPLIT_SAFETY_GAS);
    });
    it('scales with the op for large ops (a flat 20k is a rounding error next to a ~4.4M deploy)', () => {
      const gas = 4_385_000n;
      expect(tempoSplitSafetyGas(gas)).toBe((gas * TEMPO_SPLIT_SAFETY_BPS) / 10_000n);
      expect(tempoSplitSafetyGas(gas)).toBeGreaterThan(130_000n); // >> the 36k estimate error
    });
  });

  describe('tempoCallGasLimit', () => {
    it('scales the callGasLimit floor per sub-call (TIP-20 transfers meter ~308k each)', () => {
      // A simple send = 1 user transfer + 1 reimbursement transfer = 2 sub-calls.
      expect(tempoCallGasLimit(2)).toBe(2n * TEMPO_CALL_GAS_PER_SUBCALL);
      expect(tempoCallGasLimit(3)).toBe(3n * TEMPO_CALL_GAS_PER_SUBCALL);
    });
    it('comfortably exceeds the measured ~308k cost of a single TIP-20 transfer', () => {
      // Regression: the old 100k floor caused the atomic batch to revert "out of gas".
      expect(tempoCallGasLimit(1)).toBeGreaterThan(308_000n);
    });
    it('never returns 0 (at least one sub-call budget)', () => {
      expect(tempoCallGasLimit(0)).toBe(TEMPO_CALL_GAS_PER_SUBCALL);
    });
  });

  it('default fee token is pathUSD in the reserved 0x20c0 range', () => {
    expect(TEMPO_DEFAULT_FEE_TOKEN.toLowerCase()).toBe(
      '0x20c0000000000000000000000000000000000000',
    );
  });
});
