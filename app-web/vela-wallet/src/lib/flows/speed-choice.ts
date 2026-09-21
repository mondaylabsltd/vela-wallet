/**
 * What picking a speed actually buys on this network (issue 686).
 *
 * Two owner decisions, and they are two faces of one question, so they are
 * answered here, in one place, from the same numbers:
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
 * tier outright (`fees.md` §5: `TempoSignRequest` carries no priority fee), so
 * there all three options are the same fee AND have no gas-price range to tell
 * them apart. Three rows offering a choice that does nothing is the complaint
 * the picker was reopened for. So when — and ONLY when — the tiers are
 * indistinguishable on everything the screen can show, the rows give way to
 * one statement. Equal fees alone are NOT that: the owner ruled against
 * collapsing a floor-clamped picker, because there the gas-price range still
 * differs and is exactly what makes the choice mean something.
 *
 * **Precedence: B before A.** If nothing tells the speeds apart, there is
 * nothing to upgrade to — a line saying "Fast costs no more, so this send goes
 * Fast" over a speed that buys nothing would be a small lie of its own. That
 * rule lives in {@link speedIsFree}, which asks {@link indistinguishable}
 * first, so the two decisions cannot drift apart.
 *
 * Decided from the NUMBERS, never from a list of chains: an unseen custom
 * network with no priority fee gets the same treatment the day it is added.
 * And only from SETTLED core amounts — never formatted strings, since two
 * different wei amounts can print alike, and never a row still measuring or
 * failed, which is not evidence either way.
 */
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { gasPriceWei } from './gas-price';

/** The fastest speed anybody is offered — what a free upgrade goes to. */
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
 * Two quotes that charge the person exactly the same thing: the same chain,
 * the same wei, in the same coin (and, for a coin fee, the same token amount).
 * Compared as the core's own decimal strings, which are exact — nothing here
 * rounds or converts.
 */
function sameCharge(a: FeeEstimateView, b: FeeEstimateView): boolean {
	return (
		a.chain_id === b.chain_id &&
		a.total_wei === b.total_wei &&
		JSON.stringify(a.fee_asset) === JSON.stringify(b.fee_asset)
	);
}

/**
 * Whether a quote carries a gas-price range the picker can draw (issue 684/
 * 685). The same test `live-send.ts` draws by: no parsable bid, no figure.
 */
function hasGasPrice(quote: FeeEstimateView): boolean {
	return gasPriceWei(quote.effective_gas_price) !== null;
}

/**
 * Nothing the screen can show tells these tiers apart: every one settled, all
 * charging the same, and none with a gas-price range. False for fewer than two
 * rows (there is nothing to compare) and for any row without a settled quote.
 */
export function indistinguishable(rows: readonly SpeedEvidence[]): boolean {
	if (rows.length < 2) return false;
	const quotes = rows.map((row) => row.quote);
	const first = quotes[0];
	if (first === null) return false;
	return quotes.every((quote) => quote !== null && sameCharge(quote, first) && !hasGasPrice(quote));
}

/**
 * B: this network has one speed. Only once EVERY offered tier has settled —
 * a picker still measuring keeps its rows, so the statement never arrives
 * early and never flickers in over a row that is about to differ.
 */
export function oneSpeed(rows: readonly SpeedEvidence[], offered: readonly FeeTier[]): boolean {
	if (!offered.every((tier) => rows.some((row) => row.tier === tier))) return false;
	return indistinguishable(rows.filter((row) => offered.includes(row.tier)));
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
	if (preferred === FASTEST_TIER) return false;
	const mine = rows.find((row) => row.tier === preferred)?.quote ?? null;
	const fastest = rows.find((row) => row.tier === FASTEST_TIER)?.quote ?? null;
	if (mine === null || fastest === null) return false;
	const pair: SpeedEvidence[] = [
		{ tier: preferred, quote: mine },
		{ tier: FASTEST_TIER, quote: fastest }
	];
	if (indistinguishable(pair)) return false;
	return sameCharge(mine, fastest);
}
