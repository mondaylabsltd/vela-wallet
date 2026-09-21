/**
 * The speed control's words and figures — for the send form AND the dApp
 * signing sheet (spec 069).
 *
 * Every decision in it is the `fee_speed` core's: the tier in force, the free
 * upgrade, the one-speed statement, which option is measuring, each gas bid
 * as text. What this file adds is the words, and each option's fee through
 * the caller's own fee line — so the option in force and the fee row above it
 * are formatted by the same hand. Two surfaces, one builder: the design sheet
 * says of the fee card that Send and signing "must not drift".
 */
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeSpeedModel } from './model';

/** A tier this build offers — `rapid` is dead (spec 068) and nothing names it. */
export type OfferedTier = Exclude<FeeTier, 'rapid'>;

/** The words the control is drawn in, resolved by the surface's own catalog. */
export interface SpeedWords {
	label: string;
	once: string;
	free: string;
	single: string;
	gasPriceLabel: string;
	/** What each speed is CALLED — the speed itself, never a number. */
	names: Record<OfferedTier, string>;
	/** …and what each one buys, the line under the name. */
	hints: Record<OfferedTier, string>;
}

/** The dead `rapid` reads as the factory `fast` — the core's own answer for it. */
export function offeredTier(tier: FeeTier): OfferedTier {
	return tier === 'rapid' ? 'fast' : tier;
}

/**
 * The folded control from the core's view. `feeText` formats one option's own
 * settled quote at its own tier — `null` when nothing can be said about it.
 */
export function speedControlModel(
	view: FeeSpeedView,
	words: SpeedWords,
	feeText: (quote: FeeEstimateView, tier: FeeTier) => { coin: string; fiat: string | null } | null
): FeeSpeedModel {
	return {
		label: words.label,
		// THEIR default (or their pick for this one), never a hardcoded one.
		value: words.names[offeredTier(view.tier)],
		open: view.open,
		onceNote: words.once,
		// Why the tier above is not the person's default (issue 686). The core
		// never sets it beside the one-speed statement: a speed that buys
		// nothing is not an upgrade.
		freeNote: view.free_note ? words.free : undefined,
		singleNote: view.single ? words.single : undefined,
		gasPriceLabel: words.gasPriceLabel,
		gasPriceLine: view.gas_price_line,
		options: view.options.map((option) => {
			const tier = offeredTier(option.tier);
			const line = option.fee ? feeText(option.fee, option.tier) : null;
			return {
				id: tier,
				label: words.names[tier],
				detail: words.hints[tier],
				// "…" while this tier's own quote is out, "—" when there is none
				// to be had. Never another tier's figure wearing this tier's name.
				value: line ? line.coin : option.measuring ? '…' : '—',
				valueFiat: line?.fiat ?? undefined,
				gasPrice: option.gas_price ?? undefined,
				selected: option.selected
			};
		})
	};
}
