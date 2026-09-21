/**
 * What picking a speed actually buys on this network (issue 686).
 *
 * Two owner decisions, and they are two faces of one question, so they are
 * answered in one place, from the same numbers — since 2026-09-21 that place
 * is the core (`fee_policy`'s speed rules, reached through `feeSpeedRule`), so
 * the native shells get the very same answers. This file only hands the core
 * the settled quotes and returns its verdict:
 *
 * **A. When a faster speed costs no more, take it.** On a chain whose real
 * cost is under a cent, `fee_policy` clamps every tier to the $0.01 floor, yet
 * each tier still signs a different tip and a different cap (`vela-relay`
 * `docs/fees.md` §2a). Speed is free there. Somebody whose stored default is
 * Standard or Slow chose it to save money — and on this network it saves
 * nothing, so this send goes at the fastest speed instead. Only this send:
 * the stored preference is never touched, and an explicit pick on the send
 * screen always wins (both are the route's to enforce; see {@link speedIsFree}).
 *
 * **B. When there is no speed to choose, say so.** Tempo's relay ignores the
 * tier outright (`fees.md` §5), so there all three options are the same fee
 * AND have no gas-price range to tell them apart. Equal fees alone are NOT
 * that: the owner ruled against collapsing a floor-clamped picker, because
 * there the gas-price range still differs and is exactly what makes the
 * choice mean something.
 *
 * **Precedence: B before A**, decided inside the core so the two cannot drift.
 *
 * Decided from the NUMBERS, never from a list of chains, and only from SETTLED
 * core amounts — never formatted strings, and never a row still measuring or
 * failed, which is not evidence either way.
 */
import { feeSpeedRule } from '$lib/core/kernels';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { SpeedEvidence as CoreSpeedEvidence } from '$lib/core/generated/SpeedEvidence';
import type { SpeedQuote } from '$lib/core/generated/SpeedQuote';

/**
 * The fastest speed anybody is offered — what a free upgrade goes to. A
 * literal because modules read it at import time, before the core is loaded;
 * `speed-rules-parity.test.ts` pins it to the core's `FASTEST_TIER`.
 */
export const FASTEST_TIER = 'fast' satisfies FeeTier;

/**
 * One tier, and the quote the core settled FOR THAT TIER — `null` while it is
 * measuring, when it failed, or when all that is in hand is another tier's
 * figure. A `null` here is "no evidence", never "free" and never "equal".
 */
export interface SpeedEvidence {
	tier: FeeTier;
	quote: FeeEstimateView | null;
}

/**
 * The slice of a settled quote the speed rules read (`SpeedQuote`), and
 * nothing else — so a surface's full view crosses as exactly what is judged.
 */
export function speedQuote(quote: FeeEstimateView | null | undefined): SpeedQuote | null {
	if (!quote) return null;
	return {
		tier: quote.tier,
		chain_id: quote.chain_id,
		total_wei: quote.total_wei,
		fee_asset: quote.fee_asset,
		effective_gas_price: quote.effective_gas_price ?? null,
		max_gas_price: quote.max_gas_price ?? null
	};
}

function evidence(rows: readonly SpeedEvidence[]): CoreSpeedEvidence[] {
	return rows.map((row) => ({ tier: row.tier, quote: speedQuote(row.quote) }));
}

/**
 * Nothing the screen can show tells these tiers apart: every one settled, all
 * charging the same, and none with a gas-price range. False for fewer than two
 * rows (there is nothing to compare) and for any row without a settled quote.
 */
export function indistinguishable(rows: readonly SpeedEvidence[]): boolean {
	return feeSpeedRule<boolean>({ rule: 'indistinguishable', rows: evidence(rows) });
}

/**
 * B: this network has one speed. Only once EVERY offered tier has settled —
 * a picker still measuring keeps its rows, so the statement never arrives
 * early and never flickers in over a row that is about to differ.
 */
export function oneSpeed(rows: readonly SpeedEvidence[], offered: readonly FeeTier[]): boolean {
	return feeSpeedRule<boolean>({ rule: 'one_speed', rows: evidence(rows), offered: [...offered] });
}

/**
 * A: the fastest speed costs exactly what `preferred` costs, and is actually
 * different from it, so this send should go at the fastest speed.
 *
 * `rows` must carry both `preferred` and {@link FASTEST_TIER} settled; with
 * either missing the answer is `false` — hold, do not guess. Someone whose
 * preference already IS the fastest has nothing to upgrade, and the route asks
 * nothing extra of the relay for them. B wins: tiers nothing distinguishes
 * are not an upgrade.
 */
export function speedIsFree(preferred: FeeTier, rows: readonly SpeedEvidence[]): boolean {
	return feeSpeedRule<boolean>({ rule: 'speed_is_free', preferred, rows: evidence(rows) });
}
