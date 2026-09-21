/**
 * What a speed actually buys, as text: the effective gas price (issue 684).
 *
 * The owner looked at the speed picker on Tempo, Kaia, AVAX, Optimism,
 * Unichain, XLayer and BNB and found three identical fees. The cause is almost
 * never the tier arithmetic — `fee_policy` clamps every tier to the $0.01 dust
 * floor on any chain whose real cost is under a cent, so Optimism's genuinely
 * different 1937 / 2377 / 3244 wei all round up to the same figure. The tiers
 * still buy different inclusion (each signs 1.00 / 1.25 / 2.00 × the market
 * tip, `vela-relay/docs/fees.md` §2a), but with nothing else on the row three
 * equal fees read as a broken feature. This is that something else.
 *
 * **Every decision about these figures is the core's** since 2026-09-21
 * (`fee_policy::gas_price_figures`, invariant ⓔ), so the native shells write
 * the very same digits. What it decides, for the record:
 *
 * - **The unit is chosen by the number, never fixed.** Measured gas prices
 *   span eight orders of magnitude — Polygon ~300 gwei, Kaia 25, Ethereum/BSC
 *   0.05, Base 0.005, Optimism ~3,000 wei, Gnosis 10 wei. Two units and no
 *   more: gwei from 0.001 gwei (1e6 wei) upward, wei below it — judged on the
 *   set's largest number, so the tiers never split across units.
 * - **The precision is decided over the whole SET**: three significant digits,
 *   widened (to five at most) only when two genuinely different prices would
 *   otherwise print alike; rounded to nearest, never truncated.
 * - **Each tier is a RANGE since issue 685**: what it bids now ~ its cap, the
 *   most the chain can ever charge per gas at that speed. One unit and one
 *   precision for all six numbers, and a range whose ends print alike is the
 *   one figure, never `x ~ x`.
 *
 * What stays here is the WRITING: the person's number preset (the core's
 * digits are canonical — ASCII, `.` decimal, no grouping), the unit name
 * (`gwei` and `wei` are proper nouns and are NOT translated), and the `~`.
 * `~` because that is how the owner writes a range. Mind the clash, though:
 * elsewhere `~` PREFIXES a number to mean "about" (`~0.0021 ETH` on the
 * signing sheet and the fee-coin sheet). Here it is always spaced and always
 * BETWEEN two numbers, never before one, so the two uses do not meet on a
 * single figure — and nothing may start writing this figure as `~x` for
 * "about x".
 */
import { feeSpeedRule } from '$lib/core/kernels';
import type { GasPriceFigures } from '$lib/core/generated/GasPriceFigures';
import { groupDigits, numberSeparators } from '$lib/services/locale-format';
import type { NumberFormatKey } from '$lib/services/preferences.svelte';

/**
 * One tier's gas price: what it bids now (`low`, the core's
 * `effective_gas_price`) and how high it will go (`high`, its
 * `max_gas_price` — the cap). `high` is absent when the core published no
 * top, and the tier then reads as the single figure it did before issue 685.
 */
export interface GasPriceRange {
	low: bigint;
	high?: bigint;
}

/**
 * The effective gas prices of a set of tiers, as text, in the order given —
 * each a single figure. See {@link gasPriceRangeTexts}, which this is the
 * one-ended case of.
 *
 * `null` in, `null` out: a chain that reports no priority fee at all (Tempo's
 * sign request carries no such field) has no honest number here, and a
 * fabricated `0` would claim the tiers are equal when nothing measured them.
 */
export function gasPriceTexts(
	values: readonly (bigint | null)[],
	key?: NumberFormatKey
): (string | null)[] {
	return gasPriceRangeTexts(
		values.map((low) => (low === null ? null : { low })),
		key
	);
}

/**
 * The gas-price ranges of a set of tiers, as text, in the order given:
 * `0.02011 ~ 0.03016 gwei`, or `0.005 gwei` where the two ends print alike.
 *
 * `null` in, `null` out, for the same reason as {@link gasPriceTexts}. There is
 * no single-value entry point on purpose: the unit and the precision are
 * chosen ONCE over every end of every range, and a caller that could format
 * one price alone would silently lose both properties.
 */
export function gasPriceRangeTexts(
	ranges: readonly (GasPriceRange | null)[],
	key?: NumberFormatKey
): (string | null)[] {
	const figures = feeSpeedRule<GasPriceFigures>({
		rule: 'gas_price_figures',
		ranges: ranges.map((range) =>
			range === null
				? null
				: {
						low: range.low.toString(),
						high: range.high === undefined ? null : range.high.toString()
					}
		)
	});
	return gasPriceFigureTexts(figures, key);
}

/**
 * A set of figures the core formatted together — the speed picker's, or
 * {@link gasPriceRangeTexts}' — as text: `low ~ high unit`, or `low unit`
 * where the core says the range is one figure.
 */
export function gasPriceFigureTexts(
	{ unit, figures }: GasPriceFigures,
	key?: NumberFormatKey
): (string | null)[] {
	return figures.map((figure) => {
		if (figure === null) return null;
		const low = localDigits(figure.low, key);
		return figure.high === null
			? `${low} ${unit}`
			: `${low} ~ ${localDigits(figure.high, key)} ${unit}`;
	});
}

/**
 * The core's canonical digits (`1234.5`) in the person's number preset: the
 * whole part grouped, the preset's decimal mark. Nothing is rounded here.
 */
function localDigits(canonical: string, key?: NumberFormatKey): string {
	const [whole, frac] = canonical.split('.');
	const grouped = groupDigits(whole, key);
	return frac === undefined ? grouped : `${grouped}${numberSeparators(key).decimal}${frac}`;
}
