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
}

/** Where the transaction stands, as the tracker reports it. */
export type DappReceiptState =
	| { kind: 'submitting' }
	| { kind: 'submitted'; opHash: string }
	| { kind: 'confirmed'; opHash: string; txHash: string }
	| { kind: 'failed'; opHash: string };

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
		case 'failed':
			return {
				stage: 'failed',
				title: copy.failed,
				captions: [copy.failedHint],
				hash: { label: copy.opHashLabel, value: state.opHash },
				cta: copy.done
			};
	}
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
