/**
 * Issue 686 — what picking a speed actually buys on this network.
 *
 * Both decisions are made from settled core amounts, here, in one place; the
 * route and the view-model only ask. So both branches and their precedence are
 * pinned against the numbers that decide them: a Tempo-shaped network (same
 * fee, no gas price anywhere), a floor-clamped one (same fee, DIFFERENT gas
 * prices) and an ordinary one (the fees differ).
 */
import { describe, expect, it } from 'vitest';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { OFFERED_TIERS } from './live-send';
import { indistinguishable, oneSpeed, speedIsFree, type SpeedEvidence } from './speed-choice';

const BASE: FeeEstimateView = {
	chain_id: 10,
	total_wei: '0',
	max_fee_per_gas: '0',
	network_fee_per_gas: '0',
	relayer_fee_per_gas: '0',
	bundler_gas_price: '0',
	in_band_gas_basis: '0',
	effective_gas_price: null,
	max_gas_price: null,
	total_gas: '0',
	deployed: true,
	tier: 'fast',
	quoted: true,
	fee_asset: {
		type: 'erc20',
		token: '0xdead',
		decimals: 6,
		amount: '10000',
		symbol: 'USDC'
	},
	fee_recipient: '0xfee'
};

const quote = (tier: FeeTier, patch: Partial<FeeEstimateView> = {}): SpeedEvidence => ({
	tier,
	quote: { ...BASE, tier, ...patch }
});

/** Tempo: the relay ignores the tier, so every speed is the same and draws no gas price. */
const TEMPO = OFFERED_TIERS.map((tier) => quote(tier, { max_fee_per_gas: '20000000000' }));

/** Optimism's real tiers, all clamped to the $0.01 floor — but each with its own range. */
const FLOOR_CLAMPED: SpeedEvidence[] = [
	quote('fast', { effective_gas_price: '3244', max_gas_price: '9000' }),
	quote('standard', { effective_gas_price: '2377', max_gas_price: '6000' }),
	quote('slow', { effective_gas_price: '1937', max_gas_price: '4500' })
];

/** An ordinary chain: the faster speed is dearer. */
const PRICED: SpeedEvidence[] = [
	quote('fast', { effective_gas_price: '3244', max_gas_price: '9000', total_wei: '30' }),
	quote('standard', { effective_gas_price: '2377', max_gas_price: '6000', total_wei: '20' }),
	quote('slow', {
		effective_gas_price: '1937',
		max_gas_price: '4500',
		fee_asset: { ...BASE.fee_asset, amount: '9000' } as FeeEstimateView['fee_asset']
	})
];

describe('B: a network with one speed says so (issue 686)', () => {
	it('fires on Tempo’s shape — every speed the same fee, none with a gas price', () => {
		expect(oneSpeed(TEMPO, OFFERED_TIERS)).toBe(true);
	});

	it('never fires merely because the FEES are equal', () => {
		// The owner's ruling: on a floor-clamped chain the range is the choice.
		expect(oneSpeed(FLOOR_CLAMPED, OFFERED_TIERS)).toBe(false);
	});

	it('never fires where the fees differ', () => {
		expect(oneSpeed(PRICED, OFFERED_TIERS)).toBe(false);
		// Equal gas price (none) but a different charge is still a choice.
		expect(oneSpeed([TEMPO[0], TEMPO[1], quote('slow', { total_wei: '1' })], OFFERED_TIERS)).toBe(
			false
		);
	});

	it('waits for every speed — a row measuring or failed is not evidence', () => {
		expect(oneSpeed([TEMPO[0], TEMPO[1], { tier: 'slow', quote: null }], OFFERED_TIERS)).toBe(
			false
		);
		expect(oneSpeed([TEMPO[0], TEMPO[1]], OFFERED_TIERS)).toBe(false);
	});

	it('reads the charge in the coin, not just the wei', () => {
		// Same wei, but one speed costs a different token amount.
		const coin = quote('slow', {
			fee_asset: { ...BASE.fee_asset, amount: '10001' } as FeeEstimateView['fee_asset']
		});
		expect(indistinguishable([TEMPO[0], coin])).toBe(false);
	});
});

describe('A: when the fastest speed costs no more, take it (issue 686)', () => {
	it('takes it for a slower default on a floor-clamped chain', () => {
		expect(speedIsFree('slow', FLOOR_CLAMPED)).toBe(true);
		expect(speedIsFree('standard', FLOOR_CLAMPED)).toBe(true);
	});

	it('does nothing for somebody already on the fastest speed', () => {
		expect(speedIsFree('fast', FLOOR_CLAMPED)).toBe(false);
	});

	it('does not take it when the fastest speed is dearer', () => {
		expect(speedIsFree('standard', PRICED)).toBe(false);
		expect(speedIsFree('slow', PRICED)).toBe(false);
		// Same wei, dearer in the coin actually charged: only the token amount
		// differs, so this is the coin comparison and nothing else.
		const coinSlow = quote('slow', {
			effective_gas_price: '1937',
			total_wei: '30',
			fee_asset: { ...BASE.fee_asset, amount: '9000' } as FeeEstimateView['fee_asset']
		});
		expect(speedIsFree('slow', [PRICED[0], coinSlow])).toBe(false);
		expect(
			speedIsFree('slow', [
				PRICED[0],
				{ ...coinSlow, quote: { ...coinSlow.quote!, fee_asset: BASE.fee_asset } }
			])
		).toBe(true);
	});

	it('holds until both quotes have settled', () => {
		expect(speedIsFree('slow', [FLOOR_CLAMPED[0], { tier: 'slow', quote: null }])).toBe(false);
		expect(speedIsFree('slow', [{ tier: 'fast', quote: null }, FLOOR_CLAMPED[2]])).toBe(false);
		expect(speedIsFree('slow', [FLOOR_CLAMPED[2]])).toBe(false);
	});

	it('compares the core’s exact amounts, never what they would print as', () => {
		// 1 wei apart — the same "0.01 USDC" on screen, but not the same charge.
		const fast = quote('fast', { effective_gas_price: '3244', total_wei: '1000000000000000001' });
		const slow = quote('slow', { effective_gas_price: '1937', total_wei: '1000000000000000000' });
		expect(speedIsFree('slow', [fast, slow])).toBe(false);
	});
});

describe('precedence: B before A (issue 686)', () => {
	it('is no upgrade on a network whose speeds nothing tells apart', () => {
		// Equal fees — A's condition — but nothing else differs either, so the
		// "faster" speed buys nothing and a line claiming it does would lie.
		expect(speedIsFree('slow', TEMPO)).toBe(false);
		expect(speedIsFree('slow', [TEMPO[0], TEMPO[2]])).toBe(false);
	});

	it('is an upgrade when the fees match but the ranges differ', () => {
		expect(oneSpeed(FLOOR_CLAMPED, OFFERED_TIERS)).toBe(false);
		expect(speedIsFree('slow', FLOOR_CLAMPED)).toBe(true);
	});
});
