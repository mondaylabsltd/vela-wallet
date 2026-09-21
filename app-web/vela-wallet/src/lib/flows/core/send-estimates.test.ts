/**
 * The fee codec's one display-only field (issue 684).
 *
 * `effective_gas_price` is the only figure on the wire where "absent" and "0"
 * are different facts: absent draws nothing beside a speed, 0 draws "0 wei" —
 * a claim that the chain is free. `fromWireAmount` turns anything absent into
 * `0n`, which is right for every other field and wrong for this one, so the
 * pair is pinned here in both directions.
 */
import { describe, expect, it } from 'vitest';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { TransactionFeeEstimate } from '$lib/services/safe-transaction';
import { feeKey, fromFeeWire, toFeeWire } from './send-estimates';

const ESTIMATE: TransactionFeeEstimate = {
	chainId: 137,
	totalWei: 2_100_000_000_000_000n,
	maxFeePerGas: 805_065_222_658n,
	networkFeePerGas: 700_000_000_000n,
	relayerFeePerGas: 105_065_222_658n,
	bundlerGasPrice: 700_000_000_000n,
	inBandGasBasis: 700_000_000_000n,
	effectiveGasPrice: 299_589_817_385n,
	maxGasPrice: 805_065_222_658n,
	totalGas: 2_600_000n,
	deployed: true,
	tier: 'fast',
	quoted: true
};

describe('the effective gas price across the fee wire', () => {
	it('survives the round trip to the wei', () => {
		const wire = toFeeWire(ESTIMATE);
		expect(wire.effective_gas_price).toBe('299589817385');
		expect(fromFeeWire(wire).effectiveGasPrice).toBe(299_589_817_385n);
	});

	it('comes back absent when it went out absent — never as 0n', () => {
		const wire = toFeeWire({ ...ESTIMATE, effectiveGasPrice: undefined });
		expect(wire.effective_gas_price).toBeNull();
		expect(fromFeeWire(wire).effectiveGasPrice).toBeUndefined();
	});

	it('treats a view with no such key at all as absent too', () => {
		const keyless: Partial<FeeEstimateView> = toFeeWire(ESTIMATE);
		delete keyless.effective_gas_price;
		expect(fromFeeWire(keyless as FeeEstimateView).effectiveGasPrice).toBeUndefined();
	});

	it('keeps a MEASURED zero as zero', () => {
		const wire = toFeeWire({ ...ESTIMATE, effectiveGasPrice: 0n });
		expect(wire.effective_gas_price).toBe('0');
		expect(fromFeeWire(wire).effectiveGasPrice).toBe(0n);
	});
});

/**
 * The top of the range (issue 685) obeys the same rule: absent crosses as
 * absent, because `fromWireAmount` would draw a range ending at 0.
 */
describe('the gas price range’s top across the fee wire', () => {
	it('survives the round trip to the wei', () => {
		const wire = toFeeWire(ESTIMATE);
		expect(wire.max_gas_price).toBe('805065222658');
		expect(fromFeeWire(wire).maxGasPrice).toBe(805_065_222_658n);
	});

	it('comes back absent when it went out absent, or had no key — never as 0n', () => {
		const wire = toFeeWire({ ...ESTIMATE, maxGasPrice: undefined });
		expect(wire.max_gas_price).toBeNull();
		expect(fromFeeWire(wire).maxGasPrice).toBeUndefined();
		const keyless: Partial<FeeEstimateView> = toFeeWire(ESTIMATE);
		delete keyless.max_gas_price;
		expect(fromFeeWire(keyless as FeeEstimateView).maxGasPrice).toBeUndefined();
	});
});

/**
 * A PROPERTY of `feeKey`, which issue 686's fix relies on — not a regression
 * test of that fix. The wallet route stamps the fee it mirrors into the send
 * machine with `feeKey`; a free upgrade promotes a quote that charges exactly
 * what the one it replaces charged, yet signs a different gas cap and names a
 * different tier, so the stamp must see more than the charge. This pins that
 * `feeKey` does. Putting the old charge-only stamp back in the ROUTE would
 * leave this green: the guard for that is `fee-speed.e2e.ts` (the fee row
 * never falls back to "…", and the signed cap on the wire is Fast's).
 */
describe('the key the send machine is told a new quote by', () => {
	it('tells two tiers at the same charge apart', () => {
		const fast = toFeeWire(ESTIMATE);
		const slow = toFeeWire({ ...ESTIMATE, tier: 'slow', maxFeePerGas: 402_532_611_329n });
		expect(slow.total_wei).toBe(fast.total_wei);
		expect(feeKey(slow)).not.toBe(feeKey(fast));
	});
});
