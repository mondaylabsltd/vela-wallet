/**
 * What the other speeds cost (spec 068).
 *
 * The speed control shows each option's OWN fee, because the trade between
 * cost and speed has to be visible at the moment of choosing — a list of
 * names with no figures asks somebody to choose blind. Those figures are real
 * quotes: this opens one extra `fee_policy` session per tier that is NOT in
 * force and asks it to price the SAME operation at its own tier.
 *
 * Two rules the shape exists to keep:
 *
 * **Nothing is derived by arithmetic.** It would be easy to take the settled
 * quote and scale it by the tier multipliers. It would also be the exact bug
 * this codebase keeps paying for — the shell and the core each deciding part
 * of one number — and it would be wrong on top of that, because the relay's
 * reported tier price and its submit cap are different quantities (spec 068,
 * "Money and speed are two different levers"). Every figure here is a number
 * the core settled for that tier.
 *
 * **The tier in force is never previewed.** Its figure is the MAIN session's,
 * the very one the fee row above is showing, so the selected option and the
 * fee row can never be two different numbers for the same speed.
 *
 * Sessions exist only while the control is open, and the 15 s fee-signal
 * cache (issue 212) means the chain reads behind them are shared rather than
 * multiplied.
 */
import { FeeQuote, type FeeQuoteRequest } from './fee-quote.svelte';
import type { FeeTier } from '$lib/core/generated/FeeTier';

/** One tier being priced, and the session pricing it. */
export interface TierPreviewRow {
	tier: FeeTier;
	quote: FeeQuote;
}

export class TierPreview {
	/**
	 * The sessions in flight, in the order the picker draws them.
	 *
	 * Published for rendering and WRITTEN ONLY here. The sessions themselves
	 * are held in a plain field below, because the caller drives this from an
	 * effect: an effect that both read and wrote this field would re-run on
	 * its own write, which is `effect_update_depth_exceeded` — and the way it
	 * shows up is a blank wallet page, not a message about fees.
	 */
	rows = $state.raw<TierPreviewRow[]>([]);

	/** The same rows, unreactive, so disposal never reads a reactive source. */
	#live: TierPreviewRow[] = [];

	/**
	 * What the current sessions are pricing. An effect that re-runs on every
	 * view change would otherwise re-measure forever; comparing the OPERATION
	 * means the previews refresh when the send actually changes (a different
	 * amount, coin, chain or account) and not before.
	 */
	#stamp: string | null = null;

	/** Price `request` at each of `tiers`. Idempotent for the same operation. */
	/**
	 * Price `request` at each of `tiers`.
	 *
	 * Skipped while nothing has changed, so editing an amount does not spawn two
	 * extra relay round trips per keystroke. `generation` is what makes "nothing
	 * has changed" true: the calls alone are not enough, because a refresh, an
	 * expiring 15 s window or the relay re-pricing all move a tier's cost while
	 * the operation stays identical. Pass {@link FeeQuote.generation} from the
	 * session that owns the fee in force; without it these rows freeze at their
	 * first answer and quietly show a price that no longer exists.
	 */
	show(request: FeeQuoteRequest, tiers: FeeTier[], generation: number): void {
		// Every field of the request, not a chosen few. `generation` alone would
		// be enough only while every writer of `FeeQuote.lastRequest` also bumps
		// it — true today, and exactly the kind of coupling nobody would think to
		// preserve. Listing the operation outright makes the skip self-evidently
		// correct instead: two runs skip only when they really are the same
		// question at the same generation.
		const stamp = JSON.stringify([
			generation,
			tiers,
			request.chainId,
			request.account,
			request.feeToken,
			request.publicKeyHex ?? null,
			request.tier ?? null,
			request.calls
		]);
		if (stamp === this.#stamp) return;
		this.#stamp = stamp;
		this.#dispose();
		this.#live = tiers.map((tier) => {
			const quote = new FeeQuote();
			// The outcome is read off the session's own view, like every other
			// fee surface; the promise is only how the core answers.
			void quote.requestQuote({ ...request, tier });
			return { tier, quote };
		});
		this.rows = this.#live;
	}

	/**
	 * The person tapped this row: hand its session to `owner` (issue 681).
	 *
	 * The price somebody taps has to be the price they get. This row's session
	 * already priced the SAME operation at exactly this tier, through the same
	 * core — it is not an approximation of the answer, it IS the answer — so the
	 * session in force takes it over rather than asking the relay a second
	 * question whose answer can differ.
	 *
	 * The row then leaves these books. That matters twice over: `rows` must stop
	 * drawing a tier that is now the one in force, and {@link hide} must not
	 * dispose a session somebody else is now rendering from.
	 *
	 * `false` means the row had nothing settled to give (still measuring, or
	 * failed). The caller re-quotes for real then — see {@link FeeQuote.adopt}.
	 */
	promote(owner: FeeQuote, tier: FeeTier): boolean {
		const row = this.#live.find((candidate) => candidate.tier === tier);
		if (!row || !owner.adopt(row.quote)) return false;
		this.#live = this.#live.filter((candidate) => candidate !== row);
		this.rows = this.#live;
		// These sessions no longer cover the tiers on offer — the one in force
		// changed, so the set to preview did too. Forgetting the stamp is what
		// lets the next `show` re-price rather than skip as unchanged.
		this.#stamp = null;
		return true;
	}

	/** The control folded away, or the surface left. Nothing stays in flight. */
	hide(): void {
		if (this.#live.length === 0 && this.#stamp === null) return;
		this.#dispose();
		this.#live = [];
		this.rows = [];
		this.#stamp = null;
	}

	#dispose(): void {
		for (const row of this.#live) row.quote.dispose();
	}
}
