/**
 * What a dApp transaction shows AFTER it is signed (spec 077, FR-002).
 *
 * A send ends at `SendReceipt` + `StatusHero`: a spinner, then a clock with a
 * filling ring, then a tick. A dApp transaction used to end at
 * `stage = { kind: 'done' }` and a window that shut itself 400 ms later — so
 * there was nowhere to draw the landing, and the owner reported exactly that
 * ("dapp 的签名也需要和转账一样，有上链的那个等待动画效果吧").
 *
 * This is the model behind the same `StatusHero`. It is a pure function so the
 * states can be tested without a browser, and it borrows the SEND receipt's own
 * words — no new corpus keys, and the two surfaces cannot drift into saying
 * different things about the same moment.
 *
 * It answers the request's own question and nothing else: **the answer has
 * already gone out** before any of this is drawn. Nothing here can swallow,
 * delay or duplicate one (spec 077, "The invariant this must not break").
 */
import type { ReceiptStage } from '$lib/flows/model';
import type { SignMethodKind } from '$lib/core/generated/SignMethodKind';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import type { SignEndingState } from '$lib/core/generated/SignEndingState';
import { signEndingState } from '$lib/core/kernels';
import { ringProgress } from '$lib/flows/ui/ring';

/** The send receipt's words, as the request surface already resolves them. */
export interface DappReceiptCopy {
	/** `componentsTx.receipt.confirming` — "Confirming…" */
	confirming: string;
	/** `componentsTx.receipt.confirmingHint` */
	confirmingHint: string;
	/** `componentsTx.receipt.statusSubmitted` */
	submitted: string;
	/** `componentsTx.receipt.statusConfirmed` */
	confirmed: string;
	/** `componentsTx.receipt.statusFailed` */
	failed: string;
	/** `componentsTx.receipt.failedHint` */
	failedHint: string;
	/** `componentsTx.receipt.userOpHash` */
	opHashLabel: string;
	/** `componentsTx.receipt.txHash` */
	txHashLabel: string;
	/** `componentsTx.receipt.explorer` */
	explorer: string;
	/** `componentsTx.receipt.done` */
	done: string;
	/**
	 * Spec 079: `componentsUi.signing.stillConfirming` — the wait window ran
	 * out and the op has not landed; Vela keeps checking. Never "failed": a
	 * timeout is not a failure, and saying so invites the same send twice.
	 */
	stillConfirming: string;
	/** Spec 079: `componentsUi.signing.unknownOutcome` — past the tracker's 24 h. */
	unknownOutcome: string;
	/**
	 * `send.txRelayFunding` — the relay is topping up the gas it pays with on
	 * this network and sends the operation once that lands. Said instead of
	 * the plain wait, for as long as the relay says so (098 follow-up).
	 */
	relayFunding: string;
	/**
	 * Spec 099 R6: `send.txRelaySending` — the relay has the operation and has
	 * not put it on the network yet. Said in place of the chain's wait, and the
	 * ring roams: no chain's clock runs over the relay's own queue.
	 */
	relaySending: string;
	/** Spec 079: `clearSigning.alertSignedTitle` — a message was signed ("已签名！"). */
	signed: string;
	/**
	 * Spec 082 RA10: `componentsUi.signing.maybeSent` — the relay's reply was
	 * lost after the op may have left. Never "failed — try again": that is the
	 * sentence that makes a person pay twice (G21).
	 */
	maybeSent: string;
	/** `send.txCloseBackground` — the one button while it may have been sent. */
	closeBackground: string;
	/** `send.txErrorGeneric` — provably not sent; nothing left, funds are safe. */
	notSentHint: string;
	/**
	 * Spec 082 RA10 / G56: `send.txSubmitting` — "提交至网络…", the title while
	 * the op may have been sent. "Submitted" said more than anyone knew.
	 */
	submitting: string;
	/**
	 * Spec 082 RJ3: `componentsUi.signing.refused` — the network refused it and
	 * nothing was sent. No "try again": the same op is refused the same way.
	 */
	refused: string;
	/**
	 * A refusal told by its reason (correctness batch item 3): corpus key →
	 * sentence, for each key a refusal can be told by. Absent or missing a
	 * key: the plain `refused`.
	 */
	refusals?: Readonly<Record<string, string>>;
}

/** Where the transaction stands, as the tracker reports it. */
export type DappReceiptState =
	| { kind: 'submitting' }
	/** `sending`: the relay has not reported it on the network yet (spec 099 R6). */
	| { kind: 'submitted'; opHash: string; sending?: boolean }
	/** Spec 079: the wait window closed; still followed, at the core's slowing pace. */
	| { kind: 'still_confirming'; opHash: string }
	/** Spec 079: abandoned at 24 h — neither landed nor failed, and no longer asked about. */
	| { kind: 'unknown'; opHash: string }
	/** The relay holds it while it tops up its gas on the chain (tracker `relay_funding`). */
	| { kind: 'relay_funding'; opHash: string }
	/** The tracker's confirmation; `txHash` is `''` when it named none. */
	| { kind: 'confirmed'; opHash: string; txHash: string }
	/** Spec 082: the op landed and reverted — the chain said no; its transaction is the proof. */
	| { kind: 'reverted'; opHash: string; txHash: string }
	/** Spec 082 RA4: the relay never admitted it (two `not_found` past 60 s) — nothing left. */
	| { kind: 'not_sent'; opHash: string }
	/**
	 * Spec 082 RJ3: the relay refused it — nothing was sent, and retrying sends
	 * the same refusal. `refusalKey`: the tracker entry's reason, as a corpus
	 * key (correctness batch item 3); absent, the plain refusal.
	 */
	| { kind: 'refused'; opHash: string; refusalKey?: string }
	/** Spec 082 RA2: the reply was lost; followed under the local hash. */
	| { kind: 'maybe_sent'; opHash: string }
	/** Spec 079: a message signature — nothing went to the chain; the tick, then gone. */
	| { kind: 'signed' };

export interface DappReceiptModel {
	/** The disc `StatusHero` draws. */
	stage: ReceiptStage;
	title: string;
	captions: string[];
	/** The hash worth showing, once there is one. */
	hash?: { label: string; value: string };
	/** Present only when there is something on an explorer to look at. */
	explorer?: { label: string; url: string };
	/** The one button. It closes the surface; the transaction does not need it. */
	cta: string;
}

/**
 * `explorerFor` is the wallet's own explorer link for this chain, or `null`
 * where the chain has none — a missing explorer draws no button rather than a
 * dead one.
 */
export function dappReceiptModel(
	state: DappReceiptState,
	copy: DappReceiptCopy,
	explorerFor: (txHash: string) => string | null
): DappReceiptModel {
	switch (state.kind) {
		case 'submitting':
			return { stage: 'submitting', title: copy.confirming, captions: [], cta: copy.done };
		case 'submitted':
			return {
				stage: 'submitted',
				title: copy.submitted,
				captions: [state.sending ? copy.relaySending : copy.confirmingHint],
				// The operation hash, not a transaction hash: there is no
				// transaction until it lands, and labelling one as the other is
				// how a person ends up searching an explorer for nothing.
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'confirmed': {
			if (!state.txHash) {
				return {
					stage: 'confirmed',
					title: copy.confirmed,
					captions: [],
					hash: { label: copy.opHashLabel, value: state.opHash },
					cta: copy.done
				};
			}
			const url = explorerFor(state.txHash);
			return {
				stage: 'confirmed',
				title: copy.confirmed,
				captions: [],
				hash: { label: copy.txHashLabel, value: state.txHash },
				...(url ? { explorer: { label: copy.explorer, url } } : {}),
				cta: copy.done
			};
		}
		case 'maybe_sent':
			// The clock, the op hash, and the one way out — no Retry: a second
			// send of an op that may already be queued is the double payment.
			// Titled "提交至网络…" (RA10, G56): nobody knows it was submitted.
			return {
				stage: 'submitted',
				title: copy.submitting,
				captions: [copy.maybeSent],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.closeBackground
			};
		case 'not_sent':
			return {
				stage: 'failed',
				title: copy.failed,
				captions: [copy.notSentHint],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'refused':
			// A cross, "failed", and why — the network said no and nothing left,
			// told by its reason: never every refusal as the fee sentence.
			// No explorer (there is no transaction) and no Retry words (RJ3).
			return {
				stage: 'failed',
				title: copy.failed,
				captions: [
					(state.refusalKey !== undefined ? copy.refusals?.[state.refusalKey] : undefined) ??
						copy.refused
				],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'reverted': {
			const url = state.txHash ? explorerFor(state.txHash) : null;
			return {
				stage: 'failed',
				title: copy.failed,
				captions: [copy.failedHint],
				hash: state.txHash
					? { label: copy.txHashLabel, value: state.txHash }
					: { label: copy.opHashLabel, value: state.opHash },
				...(url ? { explorer: { label: copy.explorer, url } } : {}),
				cta: copy.done
			};
		}
		case 'relay_funding':
			// Still on its way, and why it is waiting: the relay's own gas, not
			// the network and not this wallet.
			return {
				stage: 'submitted',
				title: copy.submitted,
				captions: [copy.relayFunding],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'still_confirming':
		case 'unknown':
			// The clock, not a cross: the tracker has no verdict, and the
			// sentence says what Vela is doing about it (Android's aftercare,
			// same words). The operation hash, as while submitted.
			return {
				stage: 'submitted',
				title: copy.submitted,
				captions: [state.kind === 'unknown' ? copy.unknownOutcome : copy.stillConfirming],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'signed':
			return { stage: 'confirmed', title: copy.signed, captions: [], cta: copy.done };
	}
}

/**
 * The receipt state for the core's ending (spec 082 RA8): what the sheet
 * draws is `sign_request::ending_state`'s verdict — confirmed, reverted, not
 * sent, or still being followed with the tracker's outcome — never a second
 * reading of the tracker's statuses here. `maybeSent` is the handoff's own
 * flag, for the moment before the tracker has an entry to say it.
 */
export function landingFromEnding(
	ending: SignEndingState,
	opHash: string,
	maybeSent: boolean
): DappReceiptState {
	switch (ending.type) {
		case 'signed':
			return { kind: 'signed' };
		case 'confirmed':
			return { kind: 'confirmed', opHash, txHash: ending.tx_hash };
		case 'reverted':
			return { kind: 'reverted', opHash, txHash: ending.tx_hash };
		case 'not_sent':
			return { kind: 'not_sent', opHash };
		case 'refused':
			return { kind: 'refused', opHash };
		case 'following':
			// The relay holds it and says why it waits: that, before the clock.
			if (ending.relay_funding) return { kind: 'relay_funding', opHash };
			switch (ending.outcome) {
				case 'maybe_sent':
					return { kind: 'maybe_sent', opHash };
				case 'still_confirming':
					return { kind: 'still_confirming', opHash };
				case 'unknown':
					return { kind: 'unknown', opHash };
				case 'landing':
				case 'final':
					return maybeSent ? { kind: 'maybe_sent', opHash } : { kind: 'submitted', opHash };
			}
	}
}

/**
 * The receipt state for an operation the sheet handed the tracker: the core's
 * ending for "answered by its operation hash", judged against the tracker's
 * entry (`signEndingState`). No entry yet → still being followed.
 */
export function landingFor(
	entry: TrackEntryView | undefined,
	opHash: string,
	maybeSent: boolean
): DappReceiptState {
	const ending = signEndingState({ type: 'still_confirming', user_op_hash: opHash }, entry ?? null);
	// The handoff's flag speaks only until the tracker has an entry. From then
	// on the entry's outcome is the core's word: `maybe_sent` while the relay
	// has not acknowledged the op, and the Landing / StillConfirming words once
	// it has (RA10) — a lost reply the relay turned out to hold is no longer
	// "don't send it again".
	const matched = entry !== undefined && entry.user_op_hash.toLowerCase() === opHash.toLowerCase();
	const state = landingFromEnding(ending, opHash, maybeSent && !matched);
	// Refused: why, as the core read it off the relay (`refusal_key`).
	if (state.kind === 'refused' && matched && entry?.refusal_key) {
		return { ...state, refusalKey: entry.refusal_key };
	}
	// Spec 099 R6: on its way, but not yet on the network — the relay has it.
	if (state.kind === 'submitted' && !(matched && entry?.relay_sent_at_ms != null)) {
		return { ...state, sending: true };
	}
	return state;
}

/** The tracker's entry for `opHash`, matched as the tracker keys it (lowercase). */
export function trackEntryFor(
	entries: readonly TrackEntryView[],
	opHash: string
): TrackEntryView | undefined {
	const wanted = opHash.toLowerCase();
	return entries.find((row) => row.user_op_hash.toLowerCase() === wanted);
}

/**
 * How long the signed tick stays before it goes by itself (spec 079): long
 * enough to be seen, short enough that nobody has to close a success. The
 * Android aftercare's figure for a message.
 */
export const SIGNED_TICK_MS = 1_400;

/** …and a landed transaction's beat, the Android and iOS figure (spec 079). */
export const LANDED_CLOSE_MS = 2_600;

/**
 * Whether the landing goes by itself, and when: a success does (the signed
 * tick, a confirmed transaction); everything the person must still READ does
 * not — submitting, waiting, still confirming, unknown, failed. `null` = only
 * Done closes it.
 */
export function autoCloseAfterMs(state: DappReceiptState): number | null {
	switch (state.kind) {
		case 'signed':
			return SIGNED_TICK_MS;
		case 'confirmed':
			return LANDED_CLOSE_MS;
		case 'submitting':
		case 'submitted':
		case 'relay_funding':
		case 'still_confirming':
		case 'unknown':
		case 'maybe_sent':
		case 'not_sent':
		case 'refused':
		case 'reverted':
			return null;
	}
}

/**
 * Whether an answer ends on the signed tick: a RESULT went out, for the
 * message request the sheet was showing. A transaction lands on the receipt
 * instead; a refusal, or an answer for some other request, draws nothing.
 */
export function endsOnSignedTick(
	answer: { id: string; ok: boolean },
	shown: { id: string; kind: SignMethodKind } | null
): boolean {
	if (!answer.ok || shown === null || shown.id !== answer.id) return false;
	return shown.kind === 'personal_sign' || shown.kind === 'typed_data' || shown.kind === 'eth_sign';
}

/**
 * Whether a tracker hand-off is one a landing rises for (spec 082 RJ1).
 *
 * The write-ahead hands the op to the tracker BEFORE its POST, as "may have
 * been sent" — the record must exist before the bytes leave. While it is
 * being posted the sheet itself says 提交至网络…; a landing raised then would
 * draw "don't send it again" over every payment for the second its POST
 * takes. So a landing rises once the relay took the op (`admitted`), once the
 * core names it the sheet's pending op (`pending_op_hash`, set when the
 * submit ended — a lost reply included), or for a hand-off that is not a
 * write-ahead's (`maybe_sent: false`: the relay's own hash).
 */
export function handoffLands(
	handoff: { user_op_hash: string; maybe_sent: boolean; admitted: boolean },
	pendingOp: string | null | undefined
): boolean {
	if (handoff.admitted || !handoff.maybe_sent) return true;
	return !!pendingOp && pendingOp.toLowerCase() === handoff.user_op_hash.toLowerCase();
}

/**
 * The operation a landing should be raised for, or `null` for none.
 *
 * One landing per operation. The obvious gate — "no landing is showing" — is
 * wrong in a way that only a real browser shows: the handoff STAYS in the view
 * after the receipt is dismissed, so the moment Done cleared the landing the
 * watcher raised the same receipt again, and the person could not get out.
 * Measured 2026-09-23 in the packaged extension, where neither a trusted click
 * nor a synthetic one could dismiss it.
 *
 * So the question is not "is one showing" but "has this one been shown".
 *
 * @param handoffOp the `tracker_handoff`'s operation hash, if the view has one
 * @param alreadyRaised the operation this surface has already raised a landing
 *   for — dismissed or still on screen, it makes no difference
 */
export function landingToRaise(
	handoffOp: string | null | undefined,
	alreadyRaised: string | null
): string | null {
	if (!handoffOp) return null;
	return handoffOp === alreadyRaised ? null : handoffOp;
}

/**
 * What Done on a landing tells the sheet machine (spec 097 N4).
 *
 * - `dismiss_tapped` — the core still shows the landing's request WITH a
 *   failure: a refusal (or "nothing was sent") the tracker reached after
 *   "Submitted". The core holds that answer while it shows — the rule of 096
 *   F8 — and this Done is the close that sends it, once. Before 097 the
 *   answer went with the verdict and the extension worker closed the window
 *   over the words; leaving the request behind now would answer the page the
 *   window's generic settlement instead, when the window goes.
 * - `hide` — the request is still on the core's sheet with its answer still
 *   out and nothing failed (a lost reply's landing, G37): its sheet waits
 *   hidden for that answer, which the pipeline sends.
 * - `null` — the request is over; nothing to tell.
 *
 * `view` is the core's `SignView` (its request and error); `landingRequest`
 * the request the landing was raised for.
 */
export function landingCloseAction(
	view: { request: { id: string } | null; error: unknown },
	landingRequest: string | null
): 'dismiss_tapped' | 'hide' | null {
	if (landingRequest === null || view.request?.id !== landingRequest) return null;
	return view.error !== null && view.error !== undefined ? 'dismiss_tapped' : 'hide';
}

/**
 * How much of the ring is drawn, 0–1, from when the chain accepted it and how
 * long this chain usually takes.
 *
 * The curve is `ringProgress` — the SEND receipt's own, not a second one that
 * looks similar. The owner asked for "和转账一样"; a different easing would be
 * the same picture moving at a different speed, which is the drift this whole
 * spec is about.
 *
 * `undefined` when the chain has no typical time — `StatusHero` then circles
 * instead of filling, which is the honest drawing of "no estimate".
 */
export function receiptProgress(
	submittedAtMs: number,
	typicalS: number,
	nowMs: number
): number | undefined {
	return ringProgress(Math.max(0, nowMs - submittedAtMs) / 1000, typicalS);
}
