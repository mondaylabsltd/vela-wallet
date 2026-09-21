/**
 * The fee quote — ONE live `fee_policy` session per fee-showing surface.
 *
 * Ported from src/hooks/use-fee-quote.ts @ f9bcb278 (a React hook there, a
 * plain reactive class here — the refs become fields, the state becomes runes;
 * every rule and every comment below is the Expo file's).
 *
 * That "one session" is the whole point. `fee_policy` is built around a
 * session — asset selection, the TTL, the option amounts, `confirm_fee_ready` —
 * and FOUR earlier integration attempts were pulled because they drove it as a
 * one-shot promise while the fee card went on patching estimates in
 * TypeScript. The shell and the core each decided part of one number, and
 * every review found the next place they disagreed. So: everything on a
 * surface that asks about the fee asks THIS session, and the number it settles
 * on is the number displayed, gated and signed.
 *
 * What the shell owns is I/O and arithmetic-free bookkeeping: which account is
 * deployed, which passkey builds its initCode, and the promise plumbing that
 * lets the `send` core's `estimate_fee` operation be answered by a live
 * machine instead of a one-shot call.
 */
import { loadCore } from '$lib/core/client';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';
import {
	accountIsDeployed,
	invalidateFeeSignals,
	type TransactionFeeEstimate
} from '$lib/services/safe-transaction';
import { createFeeSession, type FeeSession } from './fee-session';
import { resolveFee } from './send-estimates';

/**
 * The tier a request that names none is priced at.
 *
 * `fast` is the core's own factory default (`fee_tier_pref::FACTORY_DEFAULT`)
 * and what every flow hard-coded before spec 068, so a caller that does not
 * care gets exactly today's quote. Callers that DO care — the send form and
 * the signing sheet, which show the person a speed — pass `tier` on the
 * request, and what is quoted is then what the row they are looking at says.
 */
const DEFAULT_TIER: FeeTier = 'fast';

/** The machine's own initial view, mirrored until the session commits its first. */
export const IDLE_FEE_VIEW: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false
};

export interface FeeQuoteRequest {
	chainId: number;
	account: string;
	/**
	 * The REAL calls being priced, WITHOUT the fee leg — the core appends that
	 * itself, to the recipient its own quote named, so what is simulated is
	 * what is submitted. An empty list asks for a transfer-sized preview.
	 */
	calls: FeeCall[];
	/** `null` = native. A quote PARAMETER: it changes the operation being priced. */
	feeToken: string | null;
	/**
	 * The passkey public key that builds the initCode for an undeployed Safe.
	 * Carried on the REQUEST rather than held by the session, so the key that
	 * builds the initCode belongs to the operation being priced.
	 */
	publicKeyHex: string | undefined;
	/**
	 * How fast this operation should be (spec 068). A quote PARAMETER, like
	 * `feeToken`: the core prices the tier it is given, and the submit path
	 * names the tier the settled estimate carries — so the speed on screen and
	 * the speed on the wire are the same fact, read from one place. Omitted =
	 * {@link DEFAULT_TIER}, which is today's behaviour exactly.
	 */
	tier?: FeeTier;
}

/**
 * How a quote request ended.
 *
 * `context_unavailable` is not a verdict about a fee — it says the shell could
 * not obtain an input the question requires (an indeterminate `eth_getCode`),
 * so the core was never asked. Guessing a deployment status is the one thing
 * that must not happen here: guessing "deployed" ships an op with empty
 * initCode for a fresh account, guessing "undeployed" attaches initCode to a
 * live one, and both are rejected at submit.
 *
 * `abandoned` is the surface moving on under an in-flight request.
 */
export type FeeQuoteOutcome =
	| { kind: 'ok'; estimate: FeeEstimateView }
	| { kind: 'failed'; failure: FeeFailure }
	| { kind: 'context_unavailable' }
	| { kind: 'abandoned' };

/**
 * Which sessions have had `start` called. The shared effect loop draws a real
 * distinction: `start` commits the initial view before dispatching, `dispatch`
 * does not. Calling `start` twice would republish a stale view over a live one.
 */
const started = new WeakSet<FeeSession>();

/**
 * Two requests asking about the same operation — everything but the speed.
 *
 * The tier is a quote PARAMETER and nothing else: the calls, the account, the
 * chain, the fee coin and the initCode key are what "which transaction is
 * this" means. {@link FeeQuote.adopt} needs the distinction because a
 * promotion that answers a caller waiting on one operation with a quote of a
 * DIFFERENT one would be the money bug this file is built to prevent, only
 * quieter.
 */
function sameOperation(a: FeeQuoteRequest, b: FeeQuoteRequest): boolean {
	return (
		a.chainId === b.chainId &&
		a.account === b.account &&
		a.feeToken === b.feeToken &&
		a.publicKeyHex === b.publicKeyHex &&
		JSON.stringify(a.calls) === JSON.stringify(b.calls)
	);
}

export class FeeQuote {
	/** The session's view, masked to idle while a superseded question is live. */
	view = $state<FeeView>(IDLE_FEE_VIEW);
	/**
	 * Covers the account-context read too, so no surface renders "estimate
	 * failed" in the frame between deciding to quote and the machine starting.
	 */
	pending = $state(false);
	/** Distinguishes an idle machine from a failed one — both project `fee: null`. */
	asked = $state(false);

	#session: FeeSession | null = null;
	/**
	 * Who the live core session reports to.
	 *
	 * A session's callbacks are built ONCE, when it is constructed, so closing
	 * them over `this` would nail the session to the instance that made it.
	 * Promotion (issue 681) moves a settled session from one `FeeQuote` to another,
	 * and a session still calling its old owner back would publish this
	 * operation's views onto a surface nothing is rendering. So the callbacks
	 * close over this little box instead, and {@link adopt} re-points it.
	 *
	 * `owner: null` is a session on its way out: whatever it says after
	 * `dispose()` answers a question nobody is asking any more.
	 */
	#port: { owner: FeeQuote | null } = { owner: this };
	#raw: FeeView = IDLE_FEE_VIEW;
	#latest: FeeView = IDLE_FEE_VIEW;
	/**
	 * The last request never reached the core, so the core's view answers a
	 * DIFFERENT question and must not be published as this one's. Without this
	 * the surface renders the previous request's quote — on a signing sheet
	 * that is "displayed = signed" broken in the most direct way available.
	 */
	#contextLost = $state(false);
	/** The key the LIVE request carries, read when the core asks for a simulation. */
	#publicKey: string | undefined = undefined;
	/** At most one caller awaits settlement: a new request supersedes the last inside the core. */
	#settle: ((outcome: FeeQuoteOutcome) => void) | null = null;
	/** Guards the await inside `requestQuote`: a slower deployment read must not dispatch. */
	#seq = 0;
	/**
	 * What a `requestQuote` that was superseded by a PROMOTION returns.
	 *
	 * A superseded run normally answers `abandoned`, and the route turns that
	 * into `estimate_failed` for the `send` core — which is right when the
	 * surface simply moved on, and wrong when it moved on to a settled quote of
	 * the very operation the core was asking about (issue 681). The core would
	 * hear its own question refused and raise "estimate failed" on a form the
	 * person only tapped a speed on.
	 *
	 * Keyed by the `#seq` the parked caller holds, because the caller can be
	 * anywhere: inside `await accountIsDeployed`, inside `await loadCore()`, or
	 * already waiting on {@link #settle}. Only that one caller can claim it.
	 */
	#supersededBy: { seq: number; outcome: FeeQuoteOutcome } | null = null;
	/**
	 * Reactive so the speed picker's previews follow the form: when the send
	 * re-quotes (a new amount, a different coin), the other tiers are priced
	 * for the operation that is now on screen rather than the one that was.
	 */
	#lastRequest = $state<FeeQuoteRequest | null>(null);
	/** See {@link generation}. */
	#generation = $state(0);
	#neverReachedCore = false;
	/**
	 * True only while a `quote_requested` dispatch is on the stack. `start`
	 * commits the core's PRISTINE view first — `busy: false, fee: null` — which
	 * is indistinguishable from "the run finished with nothing"; settling on it
	 * resolved every first request as abandoned, which the `send` core reads as
	 * a refused estimate and never advances to confirm.
	 */
	#dispatching = false;

	/** The settled quote in the shape the submit paths sign. */
	get estimate(): TransactionFeeEstimate | null {
		return resolveFee(this.view.fee);
	}

	/**
	 * The last operation this session was asked to price, or `null` before the
	 * first one.
	 *
	 * Exposed for the speed picker (spec 068), which prices the SAME operation
	 * at the other tiers so each row can show its own fee. It replays this
	 * request rather than rebuilding one, because the calls a send is priced
	 * against are the core's — including the fee leg it appends — and a second
	 * shell-built approximation of them would be a second answer about what
	 * this transaction costs.
	 */
	get lastRequest(): FeeQuoteRequest | null {
		return this.#lastRequest;
	}

	/**
	 * Bumped every time this session is asked to price again.
	 *
	 * The speed picker's other rows are priced by separate sessions
	 * ({@link TierPreview}), which skip re-pricing while the OPERATION is
	 * unchanged — otherwise every keystroke would spawn two more relay round
	 * trips. But "the operation is the same" is not "the answer is the same":
	 * a refresh, a 15 s window rolling over, or the relay itself re-pricing all
	 * change what a tier costs while the calls stay identical. Without a signal
	 * for that, those rows froze at whatever they were first told and sat there
	 * showing a price that no longer existed — three rows, one of them live.
	 * This counter is that signal, and it moves only for the MAIN session, so
	 * the previews cannot drive their own re-pricing.
	 */
	get generation(): number {
		return this.#generation;
	}

	/** Price this operation. Resolves when the quote settles. */
	async requestQuote(request: FeeQuoteRequest): Promise<FeeQuoteOutcome> {
		const seq = ++this.#seq;
		this.#lastRequest = request;
		this.#generation += 1;
		this.#publicKey = request.publicKeyHex;
		this.asked = true;
		// Settle the previous caller BEFORE anything else: the dispatch below
		// supersedes its run inside the core, so its promise can never be
		// answered by the machine again.
		this.#resolve({ kind: 'abandoned' });
		this.pending = true;

		// Deployment status decides whether the priced op carries initCode, and
		// `accountIsDeployed` throws rather than answer an indeterminate read.
		let deployed: boolean;
		try {
			deployed = await accountIsDeployed(request.account, request.chainId);
		} catch {
			if (seq !== this.#seq) return this.#supersededOutcome(seq, { kind: 'context_unavailable' });
			this.#neverReachedCore = true;
			this.#contextLost = true;
			this.#publish();
			this.pending = false;
			return { kind: 'context_unavailable' };
		}
		if (seq !== this.#seq) return this.#supersededOutcome(seq, { kind: 'abandoned' });
		this.#neverReachedCore = false;
		this.#contextLost = false;
		this.#publish();

		await loadCore();
		if (seq !== this.#seq) return this.#supersededOutcome(seq, { kind: 'abandoned' });
		const session = this.#ensure();
		return new Promise<FeeQuoteOutcome>((resolve) => {
			this.#settle = resolve;
			const event = {
				type: 'quote_requested' as const,
				chain_id: request.chainId,
				account: request.account,
				deployed,
				public_key_available: this.#publicKey != null,
				tier: request.tier ?? DEFAULT_TIER,
				calls: request.calls,
				fee_token: request.feeToken
			};
			this.#dispatching = true;
			try {
				if (started.has(session)) session.dispatch(event);
				else {
					started.add(session);
					session.start(event);
				}
			} finally {
				this.#dispatching = false;
			}
			// Judged once, here, against the view the dispatch produced — never
			// against the pristine one `start` publishes on its way in.
			this.#settleFrom(this.#latest);
		});
	}

	/** The person picked a fee coin. */
	selectAsset(token: string | null): void {
		this.#session?.dispatch({ type: 'select_fee_asset', token });
	}

	/** The refresh affordance. */
	requote(): void {
		// Asking again means measuring again. Every other quote run inside 15 s
		// reuses the chain's gas signals so the row holds still while the form is
		// edited (issue 212); this one is the person saying "look again", so the
		// cache is dropped BEFORE the dispatch that reads it.
		if (this.#lastRequest) invalidateFeeSignals(this.#lastRequest.chainId);
		// A refresh re-prices the other tiers too: the person asked for a fresh
		// answer, not a fresh answer for one row out of three.
		this.#generation += 1;
		// The core holds the request context and re-runs its own pipeline —
		// unless the last attempt never reached it, in which case `requote`
		// would be a no-op and the retry affordance a dead button.
		if (this.#neverReachedCore && this.#lastRequest) {
			void this.requestQuote(this.#lastRequest);
			return;
		}
		this.#session?.dispatch({ type: 'requote' });
	}

	/** Leaving the confirm step: drop the asset choice and any stale ERC-20 estimate. */
	leaveConfirm(): void {
		this.#session?.dispatch({ type: 'leave_confirm' });
	}

	/** The form now targets a different chain — every earlier quote is invalid for it. */
	chainChanged(chainId: number): void {
		this.#session?.dispatch({ type: 'chain_changed', chain_id: chainId });
	}

	/**
	 * Leaving the surface. Whatever was in flight is abandoned: leaving a
	 * caller's promise pending forever would wedge the `send` core's effect
	 * loop, which is still waiting for exactly one answer to its `estimate_fee`.
	 */
	dispose(): void {
		this.#resolve({ kind: 'abandoned' });
		// Severed before the session is torn down, so a last view on its way out
		// cannot repaint a surface that has left.
		this.#port.owner = null;
		this.#session?.dispose();
		this.#session = null;
	}

	/**
	 * Take over `donor`'s settled session, in place (issue 681).
	 *
	 * The speed picker prices every tier with a real session of its own
	 * ({@link TierPreview}), so when the person taps a row, a complete
	 * `FeeQuote` for THAT tier and THAT operation already exists. Asking the
	 * relay again threw it away: the second answer can differ (gas moves, and
	 * the 15 s fee-signal window may have rolled over while the picker sat
	 * open), so the figure they tapped was not the figure they got — which is
	 * the defect. Promotion is the answer: the quote the person tapped becomes
	 * the quote in force, with no round trip at all.
	 *
	 * IN PLACE, and this is the whole reason the method exists rather than a
	 * swapped variable: the route holds ~17 references to its `feeQuote`, some
	 * captured in the send session's executor-port closures and one in the
	 * unmount `dispose()`. Every one of them must keep pointing at the object
	 * that owns the fee in force.
	 *
	 * Returns `false` when the donor has nothing settled to give — still busy,
	 * failed, or never asked. A preview that has not landed is not a price the
	 * person can have tapped, so the caller re-quotes for real instead; showing
	 * the previous tier's figure under the new tier's name would be the same
	 * defect wearing different clothes. It also returns `false` when a
	 * measurement of this session's own is out for a DIFFERENT operation: see
	 * the guard below for why that one is refused rather than answered.
	 */
	adopt(donor: FeeQuote): boolean {
		if (donor === this) return false;
		const request = donor.#lastRequest;
		// Only a SETTLED quote is worth taking: `fee` is the number, and `busy`
		// plus `pending` together cover both halves of "a measurement is out" —
		// the core's run, and the shell's account-context read before it.
		if (donor.#session === null || request === null) return false;
		if (donor.pending || donor.view.busy || donor.view.fee === null) return false;

		// A measurement of OUR OWN may be out, and it may be one the `send` core
		// is waiting on (Continue's pre-check leaves the form — and this control —
		// on screen for its whole round trip). Superseding it is fine; answering
		// it wrongly is not. The donor priced the same operation at another
		// speed, so it can answer that caller — but only if it really is the same
		// operation. When it is not, nothing here is an answer to the question in
		// flight, so the pick is refused and the caller's measurement is left
		// alone to finish.
		const measuring = this.pending;
		const mine = this.#lastRequest;
		if (measuring && (mine === null || !sameOperation(request, mine))) return false;
		// The quote that is about to be in force, in the shape a caller awaits.
		// `donor.view.fee` is non-null — the guard above is what proved it.
		const adopted: FeeQuoteOutcome = { kind: 'ok', estimate: donor.view.fee };

		// Whatever THIS session was in the middle of is superseded. The bump is
		// what makes an `await` inside a live `requestQuote` return instead of
		// dispatching over the session we just adopted; `#supersededBy` is what
		// makes it return the tapped quote rather than a refusal, wherever in its
		// two awaits it happens to be parked.
		const superseded = this.#seq;
		const parked = this.#settle === null;
		this.#seq += 1;
		this.#resolve(adopted);
		// A caller with no `#settle` registered has not reached the core yet — it
		// is inside `accountIsDeployed` or `loadCore` — so it collects its answer
		// by seq on the way out instead.
		if (measuring && parked) this.#supersededBy = { seq: superseded, outcome: adopted };
		this.#port.owner = null;
		this.#session?.dispose();

		// The donor's session, and everything that makes its answer readable.
		// `started` is keyed by the SESSION, so it travels with it: the next
		// `requestQuote` here will `dispatch`, as it should, rather than `start`
		// and republish a pristine view over a live one.
		const port = donor.#port;
		port.owner = this;
		this.#port = port;
		this.#session = donor.#session;
		this.#raw = donor.#raw;
		this.#latest = donor.#latest;
		this.#lastRequest = request;
		this.#publicKey = donor.#publicKey;
		this.#contextLost = donor.#contextLost;
		this.#neverReachedCore = donor.#neverReachedCore;
		this.#dispatching = false;
		this.asked = true;
		this.pending = false;
		this.#publish();
		// The OTHER previews are now previews of different tiers and must be
		// re-priced. This cannot recurse into re-pricing the tier just adopted:
		// the re-quote effect compares `lastRequest.tier` against the tier in
		// force, and promotion is exactly what makes those two agree.
		this.#generation += 1;

		// The donor keeps nothing. It must NOT be disposed afterwards — the
		// session it used to hold is ours now — so it is left holding none, and
		// its own `dispose()` becomes a no-op rather than a teardown of the fee
		// in force. A fresh port so a later life of its own cannot write here.
		donor.#port = { owner: donor };
		donor.#session = null;
		donor.#seq += 1;
		donor.#resolve({ kind: 'abandoned' });
		donor.#lastRequest = null;
		donor.#raw = IDLE_FEE_VIEW;
		donor.#latest = IDLE_FEE_VIEW;
		donor.#publicKey = undefined;
		donor.#contextLost = false;
		donor.#neverReachedCore = false;
		donor.#dispatching = false;
		donor.asked = false;
		donor.pending = false;
		donor.view = IDLE_FEE_VIEW;
		return true;
	}

	/**
	 * What a run that finds itself superseded should report (issue 681).
	 *
	 * `fallback` is the honest answer when the surface simply moved on. A
	 * promotion leaves something better: the settled quote it adopted, of the
	 * operation this run was asking about, which is a real answer rather than a
	 * refusal the `send` core would alert about.
	 */
	#supersededOutcome(seq: number, fallback: FeeQuoteOutcome): FeeQuoteOutcome {
		const taken = this.#supersededBy;
		if (!taken || taken.seq !== seq) return fallback;
		this.#supersededBy = null;
		return taken.outcome;
	}

	#ensure(): FeeSession {
		if (this.#session) return this.#session;
		// A port per session, made with it: `dispose()` leaves the old one
		// pointing at nobody, and this session must not inherit that silence.
		const port: { owner: FeeQuote | null } = { owner: this };
		this.#port = port;
		this.#session = createFeeSession({
			onView: (next) => {
				const owner = port.owner;
				if (owner) owner.#onView(next);
			},
			onError: (error) => console.error('[fee-policy] core fault:', error),
			publicKeyHex: () => (port.owner ? port.owner.#publicKey : undefined)
		});
		return this.#session;
	}

	/** A view from the core session this instance currently owns. */
	#onView(next: FeeView): void {
		this.#raw = next;
		this.#latest = next;
		// A quote that settled as FAILED must not leave its inputs held: the
		// 15 s fee-signal cache (issue 212) exists to keep a good number
		// still, and a failure has no number to keep. Without this, one
		// outlier read (say a relay quote the core refuses as too high)
		// would fail every retry for the rest of the window, where each
		// retry used to take a new sample. `busy` guards the run in flight.
		if (!next.busy && next.failed && this.#lastRequest) {
			invalidateFeeSignals(this.#lastRequest.chainId);
		}
		this.#publish();
		if (!this.#dispatching) this.#settleFrom(next);
	}

	/**
	 * One masking point, so no consumer has to remember: a view that answers a
	 * superseded question is not published at all. Every derived value then
	 * falls to "no quote", which is the truth.
	 */
	#publish(): void {
		this.view = this.#contextLost ? IDLE_FEE_VIEW : this.#raw;
	}

	/**
	 * A settled view as the outcome it reports. Which of the three endings it
	 * is comes from the SAME view the surface is rendering, so this can never
	 * report something the screen disagrees with.
	 */
	#settleFrom(view: FeeView): void {
		if (!this.#settle || view.busy) return;
		if (view.fee) this.#resolve({ kind: 'ok', estimate: view.fee });
		else if (view.failed) this.#resolve({ kind: 'failed', failure: view.failed });
		else this.#resolve({ kind: 'abandoned' });
	}

	#resolve(outcome: FeeQuoteOutcome): void {
		const settle = this.#settle;
		this.#settle = null;
		if (settle) {
			this.pending = false;
			settle(outcome);
		}
	}
}
