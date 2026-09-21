/**
 * The shell's half of taking a free speed (issue 686): WHICH tier to price
 * beside the one in force, and WHEN to swap them.
 *
 * Every one of those decisions is the core's since 2026-09-21 (`fee_policy`'s
 * speed rules, through `feeSpeedRule`), so the native shells make the same
 * ones. This file is the wiring the wallet route runs them through — it reads
 * the shell's sessions into the core's terms and hands back the answer — kept
 * out of the route so a browser test drives the very same steps rather than a
 * copy of them.
 */
import { feeSpeedRule } from '$lib/core/kernels';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FASTEST_TIER } from '../speed-choice';
import { speedQuote } from '../speed-choice';
import type { FeeQuote } from './fee-quote.svelte';
import type { TierPreview } from './tier-preview.svelte';

/**
 * The tier this send runs at (`fee_policy::tier_in_force`): the one-shot pick,
 * else the fastest while it is being taken for free (`freeFast`), else the
 * person's stored default. An upgrade never applies to a default that already
 * IS the fastest.
 */
export function tierInForce<T extends FeeTier>(
	picked: T | null,
	preferred: T,
	freeFast: boolean
): T | typeof FASTEST_TIER {
	return feeSpeedRule<T>({ rule: 'tier_in_force', picked, preferred, free_fast: freeFast });
}

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
	return feeSpeedRule<T | null>({
		rule: 'free_speed_partner',
		picked,
		preferred,
		in_force: inForce,
		on_form: onForm
	});
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
	return feeSpeedRule<T | null>({ rule: 'pick_in_force', picked, partner, tapped });
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
	return feeSpeedRule<T[]>({
		rule: 'preview_tiers',
		open,
		offered: [...offered],
		in_force: inForce,
		partner
	});
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
 * own question refused. That — and "the partner's preview is still out" — is
 * the shell's half, because `pending` (the account-context read before a
 * dispatch) is the shell's; both reach the core as "no settled quote".
 */
export function freeSpeedSwap(args: {
	feeQuote: FeeQuote;
	tierPreview: TierPreview;
	preferred: FeeTier;
	inForce: FeeTier;
	partner: FeeTier | null;
}): boolean | null {
	const { feeQuote, tierPreview, preferred, inForce, partner } = args;
	// The reads keep the order they always had, and each "nothing to judge
	// yet" returns early, not only because the core would answer `null`
	// anyway: this runs in a tracked effect, and it must subscribe to exactly
	// what it used to — no partner means not even the session in force is
	// read. The core still re-checks every one of these. It is also what keeps the
	// route's mount (before the core has loaded; `partner` is `null` until a
	// send session exists) from calling into the core at all.
	if (partner === null) return null;
	if (feeQuote.pending || feeQuote.view.busy) return null;
	const mine = speedQuote(feeQuote.view.fee);
	const row = tierPreview.rows.find((candidate) => candidate.tier === partner);
	if (!row || row.quote.pending || row.quote.view.busy) return null;
	const theirs = speedQuote(row.quote.view.fee);
	if (mine === null || theirs === null) return null;
	return feeSpeedRule<boolean | null>({
		rule: 'free_speed_swap',
		preferred,
		in_force: inForce,
		partner,
		mine,
		theirs
	});
}
