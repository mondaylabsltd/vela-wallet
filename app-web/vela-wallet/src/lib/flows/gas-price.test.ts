/**
 * The gas-price formatter (issue 684).
 *
 * The wallet has never shown a gas price to anybody, so this is the first one
 * and it has to be right for chains nobody has added yet. The table below is
 * seven REAL measured chains spanning eight orders of magnitude — it is the
 * whole reason the unit is picked by the number instead of being fixed at
 * `gwei`, which would have printed `0.000000` on four of these rows.
 */
import { describe, expect, it } from 'vitest';
import type { NumberFormatKey } from '$lib/services/preferences.svelte';
import { gasPriceRangeTexts, gasPriceTexts } from './gas-price';

/** The person's number preset is named, so no test depends on the machine's. */
const EN: NumberFormatKey = 'comma_dot';

const one = (wei: bigint | null, key: NumberFormatKey = EN) => gasPriceTexts([wei], key)[0];

describe('gasPriceTexts — the unit is chosen by the number', () => {
	it('reads every measured chain the way a person would say it', () => {
		// chain          | wei           | renders as
		expect(one(300_000_000_000n)).toBe('300 gwei'); // Polygon
		expect(one(25_000_000_000n)).toBe('25 gwei'); // Kaia
		expect(one(50_000_000n)).toBe('0.05 gwei'); // Ethereum / BSC
		expect(one(5_000_000n)).toBe('0.005 gwei'); // Base
		expect(one(3_000n)).toBe('3,000 wei'); // Optimism
		expect(one(10n)).toBe('10 wei'); // Gnosis
	});

	it('switches units at 0.001 gwei and nowhere else, one price at a time', () => {
		// One wei below the boundary is still wei — and still grouped, which is
		// the only thing that makes a six-digit wei figure readable.
		expect(one(999_999n)).toBe('999,999 wei');
		expect(one(1_000_000n)).toBe('0.001 gwei');
		expect(one(1_000_001n)).toBe('0.001 gwei');
	});

	it('says zero only when the chain really charges zero, and one wei when it charges one', () => {
		expect(one(0n)).toBe('0 wei');
		expect(one(1n)).toBe('1 wei');
	});

	it('has nothing to say when there is no honest number', () => {
		// Tempo's sign request carries no priority-fee field at all. A
		// fabricated "0 wei" there would claim the three tiers are equal.
		expect(one(null)).toBeNull();
		expect(gasPriceTexts([null, 10n, null], EN)).toEqual([null, '10 wei', null]);
	});

	it('writes the digits in the number preset the person chose', () => {
		// The separators are the product's presets, never the browser's
		// (`locale-format.ts`) — but `gwei` and `wei` are proper nouns and stay.
		expect(one(50_000_000n, 'dot_comma')).toBe('0,05 gwei');
		expect(one(3_000n, 'dot_comma')).toBe('3.000 wei');
		expect(one(3_000n, 'space_comma')).toBe('3 000 wei');
	});
});

/**
 * The three tiers are shown side by side and the ONLY job of this figure is to
 * tell them apart, so the precision is decided over the set.
 */
describe('gasPriceTexts — precision across the tiers being compared', () => {
	// Measured on Polygon: three real receipts, `base + signed tip` per tier.
	const POLYGON = [270_164_477_149n, 282_464_783_233n, 299_589_817_385n];

	it('shows three significant digits when that already tells the tiers apart', () => {
		expect(gasPriceTexts(POLYGON, EN)).toEqual(['270 gwei', '282 gwei', '300 gwei']);
	});

	it('widens only as far as it must to keep two different prices different', () => {
		// 0.5% apart: three digits would print both as "270 gwei".
		expect(gasPriceTexts([270_100_000_000n, 270_400_000_000n], EN)).toEqual([
			'270.1 gwei',
			'270.4 gwei'
		]);
		// And no further than it must — the third row is already distinct at
		// four digits, so nothing gains a fifth.
		expect(gasPriceTexts([270_100_000_000n, 270_400_000_000n, 299_000_000_000n], EN)).toEqual([
			'270.1 gwei',
			'270.4 gwei',
			'299 gwei'
		]);
	});

	it('prints equal prices equally, because on a tipless chain they ARE equal', () => {
		// Every OP-stack L2 answers eth_maxPriorityFeePerGas with 0, so all
		// three tiers sign the same tip and buy the same effective price.
		// Inventing a difference here is the lie this row exists to prevent.
		expect(gasPriceTexts([5_000_000n, 5_000_000n, 5_000_000n], EN)).toEqual([
			'0.005 gwei',
			'0.005 gwei',
			'0.005 gwei'
		]);
	});

	it('lets the wei branch stay exact, and keeps it out of the precision vote', () => {
		// Optimism's real underlying tiers — all under a cent, all clamped to
		// the same $0.01 fee, and all distinguishable here to the wei.
		expect(gasPriceTexts([1_937n, 2_377n, 3_244n], EN)).toEqual([
			'1,937 wei',
			'2,377 wei',
			'3,244 wei'
		]);
	});

	/**
	 * A set that straddles 0.001 gwei. Chosen per value, the unit split the
	 * rows — "0.0012 gwei" above "750,000 wei" — and on a picker drawn Fast
	 * first the dearest tier read as the smallest number. Reachable: tiers
	 * sign 1.00 / 1.25 / 2.00 × the market tip, so an L2 with a near-zero base
	 * fee and a tip around 0.0006 gwei lands exactly here.
	 */
	it('puts every tier in ONE unit, even when the set straddles 0.001 gwei', () => {
		expect(gasPriceTexts([1_200_000n, 750_000n, 600_000n], EN)).toEqual([
			'0.0012 gwei',
			'0.00075 gwei',
			'0.0006 gwei'
		]);
		expect(gasPriceTexts([999_999n, 1_000_000n, 1_000_001n], EN)).toEqual([
			'0.001 gwei',
			'0.001 gwei',
			'0.001 gwei'
		]);
		// All of it under the boundary: wei, exact, for every row.
		expect(gasPriceTexts([999_999n, 875_000n, 800_000n], EN)).toEqual([
			'999,999 wei',
			'875,000 wei',
			'800,000 wei'
		]);
		// A tier with no figure takes no part in choosing the unit.
		expect(gasPriceTexts([null, 1_200_000n, 750_000n], EN)).toEqual([
			null,
			'0.0012 gwei',
			'0.00075 gwei'
		]);
	});

	/**
	 * Widening stops at five digits. Two tiers 100 wei apart at 0.05 gwei
	 * (two parts per million) used to buy "0.0500001 gwei" beside "0.05 gwei"
	 * — a long tail distinguishing numbers that distinguish no choice.
	 */
	it('never grows a long tail to split prices no choice turns on', () => {
		expect(gasPriceTexts([50_000_100n, 50_000_000n, 50_000_000n], EN)).toEqual([
			'0.05 gwei',
			'0.05 gwei',
			'0.05 gwei'
		]);
		// Five is still reached when five is what it takes.
		expect(gasPriceTexts([270_110_000_000n, 270_140_000_000n], EN)).toEqual([
			'270.11 gwei',
			'270.14 gwei'
		]);
		// The Ethereum-shaped near miss: a 0.1 gwei tip on a 20 gwei base.
		expect(gasPriceTexts([20_200_000_000n, 20_125_000_000n, 20_100_000_000n], EN)).toEqual([
			'20.2 gwei',
			'20.13 gwei',
			'20.1 gwei'
		]);
	});

	it('rounds to nearest rather than truncating, since nobody is billed this rate', () => {
		expect(one(299_589_817_385n)).toBe('300 gwei');
		expect(one(999_999_999n)).toBe('1 gwei');
	});
});

/**
 * Issue 685 — each tier as a range: what it bids now ~ how high it will go.
 *
 * The owner, on an ETH L2: fees of 0.00006 / 0.00004 / 0.000033 ETH — 1.8×
 * apart — over gas prices of 0.02021 / 0.02013 / 0.02011 gwei, 0.5% apart. A
 * person paying 80% more for a figure 0.5% higher concludes they are being
 * cheated. The bid moves only by the tip; the cap is where the tiers differ.
 */
describe('gasPriceRangeTexts — the gas price as a range', () => {
	it('makes the owner’s L2 tiers differ the way their fees do', () => {
		// base 0.0201 gwei; the lows are the owner's measured figures, the
		// highs each tier's cap (1.5 / 2 / 3 × base + tip).
		const texts = gasPriceRangeTexts(
			[
				{ low: 20_210_000n, high: 60_410_000n }, // fast
				{ low: 20_130_000n, high: 40_230_000n }, // standard
				{ low: 20_110_000n, high: 30_160_000n } // slow
			],
			EN
		);
		expect(texts).toEqual([
			'0.02021 ~ 0.06041 gwei',
			'0.02013 ~ 0.04023 gwei',
			'0.02011 ~ 0.03016 gwei'
		]);
		// The tops run 2 : 1.33 : 1, in step with the fees — the point of it.
		const tops = texts.map((text) => Number(text?.split(' ~ ')[1].split(' ')[0]));
		expect(tops[0] / tops[2]).toBeGreaterThan(1.9);
		expect(tops[1] / tops[2]).toBeGreaterThan(1.3);
	});

	it('reads the three mined Polygon receipts as their ranges', () => {
		expect(
			gasPriceRangeTexts(
				[
					{ low: 299_589_817_385n, high: 805_065_222_658n },
					{ low: 282_464_783_233n, high: 527_288_291_674n },
					{ low: 270_164_477_149n, high: 390_302_745_042n }
				],
				EN
			)
		).toEqual(['300 ~ 805 gwei', '282 ~ 527 gwei', '270 ~ 390 gwei']);
	});

	/**
	 * ONE unit across all six numbers. A set whose bids sit under 0.001 gwei
	 * while its caps reach past it would, chosen per number, read
	 * `750,000 wei ~ 0.0015 gwei` — a range nobody can read as one. The
	 * largest number decides, for every end of every row.
	 */
	it('writes both ends of every range in ONE unit, even when the bids alone are wei', () => {
		const texts = gasPriceRangeTexts(
			[
				{ low: 750_000n, high: 1_500_000n },
				{ low: 650_000n, high: 1_050_000n },
				{ low: 600_000n, high: 900_000n }
			],
			EN
		);
		expect(texts).toEqual([
			'0.00075 ~ 0.0015 gwei',
			'0.00065 ~ 0.00105 gwei',
			'0.0006 ~ 0.0009 gwei'
		]);
		for (const text of texts) {
			// The unit once, at the end — never after each number.
			expect(text?.match(/wei/g)).toHaveLength(1);
			expect(text?.endsWith(' gwei')).toBe(true);
		}
	});

	it('stays in exact wei when every end of every range is under 0.001 gwei', () => {
		expect(
			gasPriceRangeTexts(
				[
					{ low: 3_244n, high: 5_000n },
					{ low: 1_937n, high: 2_900n }
				],
				EN
			)
		).toEqual(['3,244 ~ 5,000 wei', '1,937 ~ 2,900 wei']);
	});

	it('writes a range whose ends meet as the single figure, never "x ~ x"', () => {
		// A chain with no base fee: the cap is `m × 0 + tip`, the tip itself.
		expect(gasPriceRangeTexts([{ low: 50_000_000n, high: 50_000_000n }], EN)).toEqual([
			'0.05 gwei'
		]);
		// Ends 100 wei apart at 0.05 gwei still print alike at five digits,
		// and a spread nobody can see is not drawn either.
		expect(
			gasPriceRangeTexts(
				[
					{ low: 50_000_000n, high: 50_000_100n },
					{ low: 50_000_000n, high: 50_000_000n }
				],
				EN
			)
		).toEqual(['0.05 gwei', '0.05 gwei']);
		// In wei too.
		expect(gasPriceRangeTexts([{ low: 10n, high: 10n }], EN)).toEqual(['10 wei']);
	});

	/**
	 * A cap a few wei over its bid — a Linea-shaped chain, base fee 7 wei
	 * beside a gwei tip — is one price, and it must vote as one. Counting its
	 * own two ends as a "collision" widened the whole set to five digits for a
	 * difference no precision here can show, and printed longer figures than
	 * #684 did for the very same bids.
	 */
	it('does not widen the set for a range whose ends differ by a few wei', () => {
		const bids = [51_234_567n, 64_043_209n];
		expect(
			gasPriceRangeTexts(
				[
					{ low: bids[0], high: bids[0] + 3n },
					{ low: bids[1], high: bids[1] + 3n }
				],
				EN
			)
		).toEqual(gasPriceTexts(bids, EN));
		expect(gasPriceTexts(bids, EN)).toEqual(['0.0512 gwei', '0.064 gwei']);
		// And beside an ordinary tier with a real spread, the set still reads
		// at three digits.
		expect(
			gasPriceRangeTexts(
				[
					{ low: 51_234_567n, high: 51_234_574n },
					{ low: 64_043_209n, high: 128_086_411n }
				],
				EN
			)
		).toEqual(['0.0512 gwei', '0.064 ~ 0.128 gwei']);
	});

	it('widens over all six numbers, so a range never collapses a spread it has', () => {
		// 270.1 and 270.4 print alike at three digits; the whole set widens.
		expect(
			gasPriceRangeTexts(
				[
					{ low: 270_100_000_000n, high: 270_400_000_000n },
					{ low: 250_000_000_000n, high: 390_000_000_000n }
				],
				EN
			)
		).toEqual(['270.1 ~ 270.4 gwei', '250 ~ 390 gwei']);
	});

	it('has nothing to say for a tier with no bid, and one figure for a bid with no top', () => {
		expect(
			gasPriceRangeTexts(
				[null, { low: 270_164_477_149n }, { low: 282_464_783_233n, high: 527_288_291_674n }],
				EN
			)
		).toEqual([null, '270 gwei', '282 ~ 527 gwei']);
	});

	it('writes the digits in the number preset the person chose', () => {
		expect(gasPriceRangeTexts([{ low: 750_000n, high: 1_500_000n }], 'dot_comma')).toEqual([
			'0,00075 ~ 0,0015 gwei'
		]);
		expect(gasPriceRangeTexts([{ low: 1_937n, high: 3_000n }], 'space_comma')).toEqual([
			'1 937 ~ 3 000 wei'
		]);
	});
});

// `gasPriceWei` — which strings are a gas price at all — moved into the core
// with the rest of the speed rules (`fee_policy::gas_price_range_of`); its
// vectors are `a_gas_price_is_decimal_wei_or_nothing` in
// `rust/crates/vela-core/tests/app_fee_policy.rs`.
