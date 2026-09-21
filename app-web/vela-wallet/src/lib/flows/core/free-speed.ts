/**
 * The shell's half of taking a free speed (issue 686): WHICH tier to price
 * beside the one in force, and WHEN to swap them.
 *
 * Whether the fastest speed is free is decided in `speed-choice.ts`, from
 * settled numbers. This file is the wiring the wallet route runs that decision
 * through, kept out of the route so a browser test drives the very same steps
 * rather than a copy of them.
 */
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { FASTEST_TIER, speedIsFree } from '../speed-choice';
import type { FeeQuote } from './fee-quote.svelte';
import type { TierPreview } from './tier-preview.svelte';

/**
 * The one tier a free upgrade is judged against, or `null` when there is no
 * such question: the fastest while the send runs at the person's default,
 * their default while it runs upgraded.
 *
 * `null` — and therefore not one extra quote — for everybody whose default is
 * already the fastest. That is the factory default, so it is most people, and
 * for them this feature must cost nothing at all. `null` too the moment the
 * person picks a tier on this send: a pick is a decision, and an upgrade may
 * never overrule one.
 *
 * And `null` once the send has left its form (`onForm` false). The question
 * is asked while the person is still choosing; the confirm is the last screen
 * before a signature, and the tier it names must not change under somebody
 * reading it because Continue's pre-check happened to re-price the pair.
 * Whatever was in force at Continue is what the confirm shows and signs.
 */
export function freeSpeedPartner<T extends FeeTier>(
	picked: T | null,
	preferred: T,
	inForce: T,
	onForm: boolean
): T | typeof FASTEST_TIER | null {
	if (!onForm || picked !== null || preferred === FASTEST_TIER) return null;
	return inForce === FASTEST_TIER ? preferred : FASTEST_TIER;
}

/**
 * What this send's one-shot pick becomes when the person taps the tier that
 * is ALREADY in force.
 *
 * While a free upgrade is in question (`partner` set), that tap is a
 * decision like any other and is recorded as one — whether it lands on an
 * upgraded Fast or on their own slower default. Left unrecorded, a tap on
 * Slow made while Fast was still being measured (or was dearer) would be
 * overruled a moment later, when Fast settled at the same fee: the person
 * chose Slow on this send and the wire would say Fast.
 *
 * With no such question (a Fast default, or a pick already made) the tap
 * changes nothing, exactly as before issue 686 — tapping the option already
 * ticked is not a new decision, and the confirm keeps saying nothing about a
 * speed nobody moved.
 */
export function pickInForce<T extends FeeTier>(
	picked: T | null,
	partner: T | null,
	tapped: T
): T | null {
	if (picked !== null) return picked;
	return partner === null ? null : tapped;
}

/**
 * The tiers to keep priced beside the one in force: every other offered tier
 * while the control is open (each option shows its own fee), only the free
 * partner while it is folded, and nothing otherwise.
 */
export function previewTiers<T extends FeeTier>(
	open: boolean,
	offered: readonly T[],
	inForce: T,
	partner: T | null
): T[] {
	if (open) return offered.filter((tier) => tier !== inForce);
	return partner === null ? [] : [partner];
}

/**
 * Whether to swap the fee in force for its free-speed partner, and to what.
 *
 * Returns the new "running upgraded" flag when the partner should take over
 * — `true` when the fastest speed turned out free, `false` when the fees have
 * parted and the person's own default should come back — or `null` to hold
 * what is on screen. A quote still measuring, failed, or of another tier is
 * not evidence either way, so any of those is `null`.
 *
 * Only READS, so an effect can call it tracked. The swap itself is the
 * caller's, and it must be a PROMOTION (issue 681) — `tierPreview.promote` —
 * never a re-quote: the partner's session already priced THIS operation at
 * that tier, so it becomes the fee in force as it stands, and the figure the
 * row shows, the quote the send machine signs and the tier named on the wire
 * are then one quote.
 *
 * Never while a measurement of the fee in force is out: it may be one the
 * `send` core is waiting on, and superseding it would make the core hear its
 * own question refused.
 */
export function freeSpeedSwap(args: {
	feeQuote: FeeQuote;
	tierPreview: TierPreview;
	preferred: FeeTier;
	inForce: FeeTier;
	partner: FeeTier | null;
}): boolean | null {
	const { feeQuote, tierPreview, preferred, inForce, partner } = args;
	if (partner === null) return null;
	if (feeQuote.pending || feeQuote.view.busy) return null;
	const mine = feeQuote.view.fee;
	const row = tierPreview.rows.find((candidate) => candidate.tier === partner);
	if (!row || row.quote.pending || row.quote.view.busy) return null;
	const theirs = row.quote.view.fee;
	if (mine === null || mine.tier !== inForce || theirs === null || theirs.tier !== partner) {
		return null;
	}
	const free = speedIsFree(preferred, [
		{ tier: inForce, quote: mine },
		{ tier: partner, quote: theirs }
	]);
	return free === (inForce === FASTEST_TIER) ? null : free;
}
