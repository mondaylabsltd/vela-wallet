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
	/** Spec 079: `clearSigning.alertSignedTitle` — a message was signed ("已签名！"). */
	signed: string;
}

/** Where the transaction stands, as the tracker reports it. */
export type DappReceiptState =
	| { kind: 'submitting' }
	| { kind: 'submitted'; opHash: string }
	/** Spec 079: the wait window closed; still followed, at the core's slowing pace. */
	| { kind: 'still_confirming'; opHash: string }
	/** Spec 079: abandoned at 24 h — neither landed nor failed, and no longer asked about. */
	| { kind: 'unknown'; opHash: string }
	| { kind: 'confirmed'; opHash: string; txHash: string }
	| { kind: 'failed'; opHash: string }
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
				captions: [copy.confirmingHint],
				// The operation hash, not a transaction hash: there is no
				// transaction until it lands, and labelling one as the other is
				// how a person ends up searching an explorer for nothing.
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'confirmed': {
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
		case 'failed':
			return {
				stage: 'failed',
				title: copy.failed,
				captions: [copy.failedHint],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
		case 'signed':
			return { stage: 'confirmed', title: copy.signed, captions: [], cta: copy.done };
	}
}

/**
 * The receipt state for `opHash`, from the tracker's entry for it — or `null`
 * while the tracker holds no entry yet (the landing keeps what it shows).
 *
 * `dropped` / `rejected` are failures with a hash to look at. Everything else
 * still in flight reads the core's `outcome` (spec 079): `landing` is the
 * ringed wait, `still_confirming` the honest "not on-chain yet, Vela keeps
 * checking", `unknown` the 24-hour end. `unreachable` is NOT a failure — the
 * wallet could not ask, which is not the chain saying no — so it reads
 * whatever its outcome says, like every other unlanded op. Time alone never
 * makes a cross here: the tracker's money rule.
 */
export function landingFromEntry(
	entry: TrackEntryView | undefined,
	opHash: string
): DappReceiptState | null {
	if (!entry) return null;
	switch (entry.status) {
		case 'confirmed':
			// A confirmation with no transaction hash yet is still landing.
			return entry.tx_hash
				? { kind: 'confirmed', opHash, txHash: entry.tx_hash }
				: { kind: 'submitted', opHash };
		case 'dropped':
		case 'rejected':
			return { kind: 'failed', opHash };
		case 'pending':
		case 'fee_held':
		case 'unreachable':
		case 'accepted_not_landed':
			switch (entry.outcome) {
				case 'still_confirming':
					return { kind: 'still_confirming', opHash };
				case 'unknown':
					return { kind: 'unknown', opHash };
				case 'landing':
				case 'final':
					return { kind: 'submitted', opHash };
			}
	}
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
