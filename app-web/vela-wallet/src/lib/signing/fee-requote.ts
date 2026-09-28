/**
 * The signing sheet asks for a failed fee again by itself (spec 079, FR-008).
 *
 * On the Xiaomi the row said "点击重试" with the relay down — and stayed that
 * way after the relay came back, with the slide shut and nothing on screen to
 * say that a tap was the only way forward. Every client now re-asks on the
 * CORE's schedule (`fee_policy::requote_delay_ms`: 3 s, 6 s, 12 s, then every
 * 15 s, and never for a failure a retry cannot clear), while the sheet is open
 * and nothing has been approved.
 *
 * This holds only the timer and the attempt count. Which failures are retried,
 * and how long to wait, is the core's answer (`delayMs`); whether the sheet is
 * still open and unapproved is the host's (`observe`'s `open`); what "ask
 * again" means is the fee session's (`requote`). A pure class, so the schedule
 * and the stopping rules are tested without a browser or a relay.
 */
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeeView } from '$lib/core/generated/FeeView';

export interface FeeRequoteDeps {
	/** The core's wait before `attempt` (1-based) after `failure`; `null` = stop. */
	delayMs(failure: FeeFailure, attempt: number): number | null;
	/** Ask the fee session again (the refresh control's own path). */
	requote(): void;
	/** Test seam; `setTimeout` / `clearTimeout` by default. */
	schedule?: (run: () => void, ms: number) => unknown;
	cancel?: (handle: unknown) => void;
}

/** What `observe` needs of the fee in force. */
export interface FeeRequoteInput {
	failed: FeeFailure | null;
	/** A measurement is out: its answer is awaited, not pre-empted. */
	busy: boolean;
}

export class FeeRequoteTimer {
	readonly #deps: FeeRequoteDeps;
	readonly #schedule: (run: () => void, ms: number) => unknown;
	readonly #cancel: (handle: unknown) => void;
	#attempt = 0;
	#handle: unknown = null;
	/** Whether the host still wants the re-ask when the timer fires. */
	#open = false;

	constructor(deps: FeeRequoteDeps) {
		this.#deps = deps;
		this.#schedule = deps.schedule ?? ((run, ms) => setTimeout(run, ms));
		this.#cancel =
			deps.cancel ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>));
	}

	/** Automatic attempts made for the current failure (resets on a quote). */
	get attempt(): number {
		return this.#attempt;
	}

	/** A re-ask is waiting on its timer. */
	get scheduled(): boolean {
		return this.#handle !== null;
	}

	/**
	 * Feed every fee view, with whether the sheet is still open and unapproved.
	 *
	 * - closed or approved → stop, and forget the count (a new request starts
	 *   at attempt 1);
	 * - a measurement out → wait for its answer (a re-ask waiting on its
	 *   timer is dropped: a tap on refresh is already asking). The core
	 *   clears `failed` while it is busy, so this comes BEFORE the reset
	 *   below — a re-ask in flight is not a failure that ended, and treating
	 *   it as one kept the schedule at 3 s forever;
	 * - an answer with no failure (a quote) → the failure is over: reset;
	 * - failed, nothing scheduled → schedule attempt n+1 at the core's delay,
	 *   or nothing when the core says no retry can fix it.
	 */
	observe(fee: FeeRequoteInput | null, open: boolean): void {
		this.#open = open;
		if (!open || fee === null) {
			this.stop();
			return;
		}
		if (fee.busy) {
			this.#clear();
			return;
		}
		if (fee.failed === null) {
			this.#attempt = 0;
			this.#clear();
			return;
		}
		if (this.#handle !== null) return;
		const wait = this.#deps.delayMs(fee.failed, this.#attempt + 1);
		if (wait === null) return;
		this.#attempt += 1;
		this.#handle = this.#schedule(() => {
			this.#handle = null;
			// Checked again at the moment it fires: the person may have
			// approved or closed in between, and a re-quote under a slide that
			// already committed would re-price what they signed.
			if (this.#open) this.#deps.requote();
		}, wait);
	}

	/** The sheet went, or the person approved: nothing more is asked. */
	stop(): void {
		this.#open = false;
		this.#attempt = 0;
		this.#clear();
	}

	#clear(): void {
		if (this.#handle === null) return;
		this.#cancel(this.#handle);
		this.#handle = null;
	}
}

/**
 * The failure the fee row keeps saying across a re-ask.
 *
 * The core clears `failed` the moment a re-quote starts (`busy`), so a row
 * read straight off the view said why, then "estimating", then why again —
 * every few seconds, with the reason line (and the slide under it) jumping
 * each time, and a screen reader announcing it on every cycle. So: a failure
 * sets it, a measurement in flight keeps it, an answer (a quote, or nothing
 * asked at all) clears it.
 */
export function heldFeeFailure(
	previous: FeeFailure | null,
	fee: FeeRequoteInput | null
): FeeFailure | null {
	if (fee === null) return null;
	if (fee.failed !== null) return fee.failed;
	return fee.busy ? previous : null;
}

/**
 * The fee as the signing sheet reads it: a quote that never reached the core
 * because the chain could not be read (`FeeQuote.contextLost` — the account's
 * deployment is unknown, so nothing could be priced) is the recoverable
 * failure it is — the relay/chain out of reach — rather than an idle row.
 *
 * Idle, the row drew nothing and the slide stood OPEN on a transaction whose
 * cost nobody had been told, with nothing asking again (the extension with the
 * network down). As `quote_unavailable` the row says why, the refresh and the
 * timer ask again (the retry re-runs the whole request, context read
 * included), and the slide stays shut until a quote lands. Only while nothing
 * is being measured and nothing else is in hand.
 */
export function withLostContext(view: FeeView, contextLost: boolean): FeeView {
	if (!contextLost || view.busy || view.failed !== null || view.fee !== null) return view;
	return { ...view, failed: 'quote_unavailable', confirm_fee_ready: false };
}
