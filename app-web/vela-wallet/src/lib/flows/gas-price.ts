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
 * **The unit is chosen by the number, never fixed.** Measured gas prices span
 * eight orders of magnitude — Polygon ~300 gwei, Kaia 25, Ethereum/BSC 0.05,
 * Base 0.005, Optimism ~3,000 wei, Gnosis 10 wei — so a hardcoded `gwei` would
 * print `0.000000` on shipping networks. The rule is two units and no more:
 * gwei from 0.001 gwei (1e6 wei) upward, wei below it — judged on the set's
 * largest price, so the tiers never split across units. `mwei`/`kwei` exist but
 * nobody reads them, and a unit a person has to look up is worse than a long
 * number. `gwei` and `wei` are proper nouns and are NOT translated; only the
 * digits and the separators follow the person's number preset.
 *
 * **Formatting is decided over the whole SET, which is why there is no
 * single-value entry point.** The one job of this figure is to tell three
 * tiers apart, so the UNIT and the precision are both chosen once for the set.
 * The unit because rows in two units cannot be compared at a glance — a set
 * straddling the boundary printed "0.0012 gwei" above "750,000 wei", where the
 * dearer tier reads as the smaller number. The precision because three
 * significant digits reads well (`300`, `0.05`) and it widens only when two
 * genuinely different prices would otherwise print the same string. A caller
 * that could format one price alone would silently lose both properties.
 *
 * **Each tier is a RANGE since issue 685: what it bids now, and how high it
 * will go.** One effective price told the tiers apart by their tip alone, and
 * on a chain whose tip is a rounding error beside its base fee the owner saw
 * fees 1.8× apart over gas prices 0.5% apart — `0.02021 / 0.02013 / 0.02011
 * gwei` under `0.00006 / 0.00004 / 0.000033 ETH` — and read it as being
 * overcharged. Most of what a dearer tier buys is its CAP, how far the base fee
 * may spike before the operation stops being includable, so the cap is the
 * range's top: `0.02011 ~ 0.03016 gwei` / `0.02013 ~ 0.04023` / `0.02021 ~
 * 0.06041`, whose tops run 1 : 1.33 : 2 in step with the fees. The core
 * derives both ends (`fee_policy::gas_price_range`); nothing here computes a
 * price, it only writes them. Every rule above holds across BOTH ends of all
 * three ranges — one unit and one precision for all six numbers, because a
 * range written `3,000 wei ~ 0.005 gwei` cannot be read as a range at all —
 * and the unit is written once, at the end. `~` because that is how the owner
 * writes a range, and nothing the app shows writes a range any other way.
 * Mind the clash, though: elsewhere `~` PREFIXES a number to mean "about"
 * (`~0.0021 ETH` on the signing sheet and the fee-coin sheet). Here it is
 * always spaced and always BETWEEN two numbers, never before one, so the two
 * uses do not meet on a single figure — and nothing may start writing this
 * figure as `~x` for "about x". A range whose ends print alike is written as
 * the one figure, never `x ~ x`.
 */
import { groupDigits, numberSeparators } from '$lib/services/locale-format';
import type { NumberFormatKey } from '$lib/services/preferences.svelte';

/**
 * 0.001 gwei. A set whose largest price is at or above it reads in gwei;
 * otherwise it reads in wei.
 */
const GWEI_BRANCH_FLOOR_WEI = 1_000_000n;

const WEI_PER_GWEI = 1_000_000_000n;

/**
 * Where the gwei branch starts, and how far it may widen.
 *
 * Three is what reads: `300 gwei`, `0.05 gwei`, `0.005 gwei`. Five is the
 * ceiling, and it is a deliberate give-up point: two tiers that still print
 * alike at five digits differ by less than one part in ten thousand, which no
 * choice of speed turns on — and chasing them further is how a quiet
 * `0.05 gwei` became `0.0500001 gwei`, the long tail this figure must never
 * grow. It is enough for the real near-miss case, an Ethereum-shaped chain
 * whose tip is a sliver of its base fee (`20.1` / `20.13` / `20.2`).
 */
const MIN_SIGNIFICANT = 3;
const MAX_SIGNIFICANT = 5;

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
 * `null` in, `null` out, for the same reason as {@link gasPriceTexts}. The
 * unit and the precision are chosen ONCE over every end of every range, so the
 * rows compare as numbers down the column and across the `~`.
 */
export function gasPriceRangeTexts(
	ranges: readonly (GasPriceRange | null)[],
	key?: NumberFormatKey
): (string | null)[] {
	const present = ranges.filter((range): range is GasPriceRange => range !== null);
	// One unit for the set, chosen by its largest price — the top of the
	// dearest range, since issue 685: if any number reads in gwei, every one
	// does ("0.0006 ~ 0.0015 gwei"), so the rows compare as numbers. Only when
	// all of them sit under 0.001 gwei does the set read in wei — which prints
	// integers exactly and so never needs the precision vote below.
	const inGwei = present.some(
		({ low, high }) =>
			low >= GWEI_BRANCH_FLOOR_WEI || (high !== undefined && high >= GWEI_BRANCH_FLOOR_WEI)
	);
	// A range whose ends print alike even at the widest precision is ONE
	// price, not a spread — a chain whose base fee is a few wei beside a gwei
	// tip (Linea-shaped) puts its cap a few wei over its bid. It is written as
	// that one figure, and it votes on the precision as one figure: its own
	// two ends "colliding" would otherwise widen all six numbers to five
	// digits for a difference that no precision here can show, and the whole
	// picker would read longer than issue 684's did for the very same bids. In wei
	// every integer prints exactly, so only truly equal ends are one price.
	const topOf = ({ low, high }: GasPriceRange): bigint | undefined =>
		high === undefined ||
		(inGwei && gweiDigits(high, MAX_SIGNIFICANT, key) === gweiDigits(low, MAX_SIGNIFICANT, key))
			? undefined
			: high;
	const known = present.flatMap((range) => {
		const high = topOf(range);
		return high === undefined ? [range.low] : [range.low, high];
	});
	const significant = inGwei ? significantDigits(known, key) : 0;
	const digits = (wei: bigint) =>
		inGwei ? gweiDigits(wei, significant, key) : groupDigits(wei.toString(), key);
	const unit = inGwei ? 'gwei' : 'wei';
	return ranges.map((range) => {
		if (range === null) return null;
		const low = digits(range.low);
		// Equal ends are one figure. `x ~ x` would announce a spread that is
		// not there — the cap binds, or a chain with no base fee has nothing
		// for the cap to multiply — and make the person look for it.
		const top = topOf(range);
		const high = top === undefined ? low : digits(top);
		return high === low ? `${low} ${unit}` : `${low} ~ ${high} ${unit}`;
	});
}

/**
 * The fewest digits that keep every distinct price distinct.
 *
 * Equal prices print equally, which is the truth and not a collision: on an
 * OP-stack L2 the market tip really is 0, so all three tiers really do buy the
 * same effective price, and inventing a difference there would be the lie this
 * whole row exists to stop.
 */
function significantDigits(values: readonly bigint[], key?: NumberFormatKey): number {
	for (let significant = MIN_SIGNIFICANT; significant < MAX_SIGNIFICANT; significant += 1) {
		const seen = new Map<string, bigint>();
		let collided = false;
		for (const wei of values) {
			const text = gweiDigits(wei, significant, key);
			const other = seen.get(text);
			if (other !== undefined && other !== wei) {
				collided = true;
				break;
			}
			seen.set(text, wei);
		}
		if (!collided) return significant;
	}
	return MAX_SIGNIFICANT;
}

/**
 * A wei amount as gwei digits — no unit, so {@link significantDigits} compares
 * numbers rather than strings that happen to share a suffix.
 *
 * All of it is `bigint` arithmetic. A gas price is a per-gas figure and small
 * today, but it is read off a uint256 and this repo's rule about base units
 * never becoming a `number` does not have an exception for "small today".
 */
function gweiDigits(wei: bigint, significant: number, key?: NumberFormatKey): string {
	const value = roundToSignificant(wei, significant);
	const whole = groupDigits((value / WEI_PER_GWEI).toString(), key);
	const frac = (value % WEI_PER_GWEI).toString().padStart(9, '0').replace(/0+$/, '');
	return frac === '' ? whole : `${whole}${numberSeparators(key).decimal}${frac}`;
}

/**
 * Round half up to `significant` digits.
 *
 * To NEAREST, not truncated the way `feeAmountText` truncates a fee. A fee is a
 * charge and rounding one down is the safe direction; a gas price is a rate
 * nobody is billed for, so truncating it would understate every figure for no
 * benefit. Nearest is the only unbiased answer.
 */
function roundToSignificant(wei: bigint, significant: number): bigint {
	const digits = wei.toString().length;
	if (digits <= significant) return wei;
	const scale = 10n ** BigInt(digits - significant);
	const quotient = wei / scale;
	const remainder = wei % scale;
	return (remainder * 2n >= scale ? quotient + 1n : quotient) * scale;
}

/**
 * A decimal wei string from the core as a `bigint`, or `null`.
 *
 * `null` for anything that does not parse, and never `0n`: "the core published
 * nothing" and "the price is zero" are different facts, and only the second one
 * is a number this row may draw.
 */
export function gasPriceWei(value: string | null | undefined): bigint | null {
	if (value == null || !/^\d+$/.test(value)) return null;
	return BigInt(value);
}
