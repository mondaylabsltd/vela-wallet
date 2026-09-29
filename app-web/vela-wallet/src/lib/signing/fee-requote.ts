/**
 * The signing sheet asks for a failed fee again by itself (spec 079, FR-008).
 *
 * On the Xiaomi the row said "点击重试" with the relay down — and stayed that
 * way after the relay came back, with the slide shut and nothing on screen to
 * say that a tap was the only way forward. Every client now re-asks on the
 * CORE's schedule (`fee_policy::requote_delay_ms`: 3 s, 6 s, then every 8 s,
 * and never for a failure a retry cannot clear — spec 082 RJ12), each re-ask
 * bounded by `feeRequoteTimeoutMs()`, while the sheet is open and nothing has
 * been approved. Every failure and every recovery is a `fee:` log line.
 *
 * This holds only the timer and the attempt count. Which failures are retried,
 * and how long to wait, is the core's answer (`delayMs`); whether the sheet is
 * still open and unapproved is the host's (`observe`'s `open`); what "ask
 * again" means is the fee session's (`requote`). A pure class, so the schedule
 * and the stopping rules are tested without a browser or a relay.
 */
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeeView } from '$lib/core/generated/FeeView';
import { countPanelFailure } from '$lib/services/bug-report';

export interface FeeRequoteDeps {
	/** The core's wait before `attempt` (1-based) after `failure`; `null` = stop. */
	delayMs(failure: FeeFailure, attempt: number): number | null;
	/** Ask the fee session again (the refresh control's own path). */
	requote(): void;
	/**
	 * The bound on each automatic re-ask (`feeRequoteTimeoutMs()`, spec 082
	 * RJ12): one that has not answered by then is given up and the next is
	 * scheduled — a quote hung on a dead relay held the fee off for ~7 s more
	 * each time. Absent: unbounded (a harness).
	 */
	timeoutMs?: () => number;
	/** The chain the fee is for, for the `fee:` log lines. */
	chainId?: () => number | null;
	/** Test seam; `setTimeout` / `clearTimeout` by default. */
	schedule?: (run: () => void, ms: number) => unknown;
	cancel?: (handle: unknown) => void;
	/** Test seam; `console.log` by default. */
	log?: (line: string) => void;
}

/** What `observe` needs of the fee in force. */
export interface FeeRequoteInput {
	failed: FeeFailure | null;
	/** A measurement is out: its answer is awaited, not pre-empted. */
	busy: boolean;
}

/**
 * A fee failure as a log word and a counter class (spec 082 RJ12, G61):
 * the wire name, or `chain_read` / `chain_read_rate_limited` for the one
 * failure that is an object. Never a host, never a value.
 */
export function feeFailureCause(failure: FeeFailure | 'timeout'): string {
	if (typeof failure === 'string') return failure;
	return failure.chain_read.rate_limited ? 'chain_read_rate_limited' : 'chain_read';
}

export class FeeRequoteTimer {
	readonly #deps: FeeRequoteDeps;
	readonly #schedule: (run: () => void, ms: number) => unknown;
	readonly #cancel: (handle: unknown) => void;
	readonly #log: (line: string) => void;
	#attempt = 0;
	#handle: unknown = null;
	/** The bound on the automatic re-ask now out (RJ12). */
	#guard: unknown = null;
	/**
	 * The re-ask now out ran past its bound: the measurement still showing is
	 * given up, so a "busy" view no longer holds the next re-ask back.
	 */
	#abandoned = false;
	/** The failure being retried — what a timed-out re-ask is retried as. */
	#failure: FeeFailure | null = null;
	/** A failure the core will not retry, already said once (no line per view). */
	#final: string | null = null;
	/** Whether the host still wants the re-ask when the timer fires. */
	#open = false;

	constructor(deps: FeeRequoteDeps) {
		this.#deps = deps;
		this.#schedule = deps.schedule ?? ((run, ms) => setTimeout(run, ms));
		this.#cancel =
			deps.cancel ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>));
		this.#log = deps.log ?? ((line) => console.log(line));
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
	 *   it as one kept the schedule at 3 s forever. An automatic re-ask that
	 *   ran past its bound is no longer waited for (RJ12);
	 * - an answer with no failure (a quote) → the failure is over: reset,
	 *   and say so (`fee: quote back …`);
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
			if (this.#abandoned) return;
			this.#clear();
			return;
		}
		this.#abandoned = false;
		this.#clearGuard();
		if (fee.failed === null) {
			if (this.#attempt > 0) {
				this.#log(`fee: quote back chain=${this.#chain()} after ${this.#attempt} re-quotes`);
			}
			this.#attempt = 0;
			this.#failure = null;
			this.#final = null;
			this.#clear();
			return;
		}
		this.#failure = fee.failed;
		if (this.#handle !== null) return;
		const cause = feeFailureCause(fee.failed);
		if (this.#final === cause) return;
		this.#next(fee.failed, cause);
	}

	/** The sheet went, or the person approved: nothing more is asked. */
	stop(): void {
		this.#open = false;
		this.#attempt = 0;
		this.#failure = null;
		this.#final = null;
		this.#abandoned = false;
		this.#clear();
		this.#clearGuard();
	}

	/** Schedule re-ask n+1 after `failure` (said as `cause`), when the core retries it. */
	#next(failure: FeeFailure, cause: string): void {
		const wait = this.#deps.delayMs(failure, this.#attempt + 1);
		countPanelFailure(`fee.quote_failed.${cause}`);
		if (wait === null) {
			this.#final = cause;
			this.#log(`fee: quote failed chain=${this.#chain()} cause=${cause} no re-quote`);
			return;
		}
		this.#final = null;
		this.#attempt += 1;
		this.#log(
			`fee: quote failed chain=${this.#chain()} cause=${cause} re-quote #${this.#attempt} in ${wait} ms`
		);
		this.#handle = this.#schedule(() => {
			this.#handle = null;
			// Checked again at the moment it fires: the person may have
			// approved or closed in between, and a re-quote under a slide that
			// already committed would re-price what they signed.
			if (!this.#open) return;
			this.#abandoned = false;
			this.#deps.requote();
			this.#bound();
		}, wait);
	}

	/** Give the automatic re-ask just sent `timeoutMs` to answer (RJ12). */
	#bound(): void {
		const limit = this.#deps.timeoutMs?.();
		if (limit === undefined) return;
		this.#clearGuard();
		this.#guard = this.#schedule(() => {
			this.#guard = null;
			if (!this.#open) return;
			// Given up: the next re-ask supersedes it inside the fee session.
			this.#abandoned = true;
			this.#next(this.#failure ?? 'quote_unavailable', 'timeout');
		}, limit);
	}

	#chain(): string {
		return String(this.#deps.chainId?.() ?? 'unknown');
	}

	#clear(): void {
		if (this.#handle === null) return;
		this.#cancel(this.#handle);
		this.#handle = null;
	}

	#clearGuard(): void {
		if (this.#guard === null) return;
		this.#cancel(this.#guard);
		this.#guard = null;
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
 * failure it is — a chain node that did not answer — rather than an idle row.
 *
 * Idle, the row drew nothing and the slide stood OPEN on a transaction whose
 * cost nobody had been told, with nothing asking again (the extension with the
 * network down). As `ChainRead` (spec 082 RJ13 — never `quote_unavailable`,
 * which named Vela's relay for a public node's rate limit, G48) the row says
 * why in the core's words, the refresh and the timer ask again (the retry
 * re-runs the whole request, context read included), and the slide stays shut
 * until a quote lands. Only while nothing is being measured and nothing else
 * is in hand. `rateLimited`: the node refused for load, not for being away.
 */
export function withLostContext(view: FeeView, contextLost: boolean, rateLimited = false): FeeView {
	if (!contextLost || view.busy || view.failed !== null || view.fee !== null) return view;
	return {
		...view,
		failed: { chain_read: { rate_limited: rateLimited } },
		confirm_fee_ready: false
	};
}
