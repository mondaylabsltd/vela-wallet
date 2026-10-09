// Ported from src/services/wallet-state-core/tx-tracker-resident.ts @ f9bcb278 —
// RN seams rewritten to the web modules; cadence, verdicts and ordering
// verbatim. Web deltas: the core is loaded asynchronously before the session is
// built, and the `sign_request` seam is bound in Phase 5.
/**
 * The one `tx_tracker` core the web app has — WEB only, and APP-RESIDENT.
 *
 * This machine's whole subject is money that is already in flight, and money in
 * flight outlives every screen: a send's receipt lands while the user is in the
 * browser tab, a dApp tx confirms after its sheet closed, and a page reload must
 * pick up whatever the last process left `pending`. So it is a module-level
 * singleton, the `session-resident.web.ts` pattern — created once, never
 * disposed, and started with an `AppResumed` sweep that IS the cross-restart
 * recovery (invariant ⑥).
 *
 * It replaces four separate pollers on web:
 *
 * | was | now |
 * | --- | --- |
 * | `send-executor.web.ts`'s `waitForReceipt` fallback (`useSendController.ts:1045-1070`) | `trackSubmitted` + the outcome listener below |
 * | `feed-resident.web.ts`'s `reconcileFeedPending` (`tx-reconciler.ts`) | `HomeFocused` → `LoadPendingTxs` |
 * | `dapp-connection.web.tsx`'s `resumedRef` startup scan | `startTxTracker()` → the same sweep |
 * | `sign-executor.web.ts`'s `autoAddFromReceipt` | `NotifyConfirmed` → `token_trust` |
 *
 * Two shell responsibilities the core deliberately refuses:
 *
 * - **Cadence.** `Tick` may arrive at any frequency — every throttle is the
 *   core's — so the ticker here is a dumb 3 s interval that exists while the
 *   core is still following ANY op (`TrackEntryView.polling`). It used to stop
 *   at the end of the 120 s wait window, and an op past it was asked about
 *   only on the next focus/resume: the sheet said "submitted" forever and never
 *   saw it land (spec 079, F13). Past the window the core paces the asking
 *   itself — 12 s, then 60 s after ten minutes, then 300 s after an hour, and
 *   nothing past 24 h (`tx_tracker::receipt_interval_ms`) — so a tick it does
 *   not want costs one wasm call and no request.
 * - **Who cares about an outcome.** The core knows hashes, not surfaces. A
 *   caller may hand `trackSubmitted` a listener; it is fired once per verdict
 *   and dropped when the entry becomes terminal.
 *
 * Nothing here is read during render — this module is never a React dependency,
 * and the send controller receives outcomes by callback (the module-level-read
 * trap in `wallet-state.web.ts` applies to the whole codebase).
 *
 * Imported by explicit `.web` specifier on every side: `tsc` resolves a
 * `.web.ts` file's own imports to the base `.ts` variant, so a bare specifier
 * would type-check against a native module that does not exist.
 */

import { balance } from '$lib/wallet/core/balance.svelte';
import { feed } from '$lib/wallet/core/feed.svelte';
import { notifyReceiptLogsConfirmed } from './token-trust-resident';
import { loadCore } from '$lib/core/client';
import { inFlightOps } from '$lib/core/kernels';
import { createTxTrackerSession } from './tracker-session';
import { trackerLogLines } from './tracker-executor';

import type { InFlightOp } from '$lib/core/generated/InFlightOp';
import type { SendReceiptOutcome } from '$lib/core/generated/SendReceiptOutcome';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import type { TrackEvent } from '$lib/core/generated/TrackEvent';
import type { TrackView } from '$lib/core/generated/TrackView';

/**
 * The dumb tick. It is NOT a policy: the core drops anything inside its own 3 s
 * receipt / 12 s status throttles, so this only has to be no slower than the
 * fastest cadence the core can want.
 */
const TICK_MS = 3_000;

/** The machine's own initial projection — mirrored until the first view lands. */
const INITIAL_VIEW: TrackView = { entries: [] };

let current: TrackView = INITIAL_VIEW;
let session: ReturnType<typeof createTxTrackerSession> | null = null;
let ticker: ReturnType<typeof setInterval> | null = null;

const listeners = new Set<(view: TrackView) => void>();

/**
 * The operations holding their account's nonce, as the core reads them from
 * the tracker's own view (`inFlightOps`, correctness batch item 3). Every
 * machine that can sign a second operation — Send and the signing sheet —
 * is told this list on every tracker render, so its confirm waits for the
 * first instead of taking the same nonce.
 */
let currentOps: InFlightOp[] = [];
const opsListeners = new Set<(ops: InFlightOp[]) => void>();

/** The core's list for this view; a fault keeps the last one (a hold is never dropped by a bug). */
function opsOf(view: TrackView): InFlightOp[] {
	try {
		return inFlightOps(view);
	} catch (error) {
		console.error('[tx_tracker] in_flight_ops failed:', error);
		return currentOps;
	}
}

/** A surface waiting on one hash — the send screen's receipt, today's only one. */
interface OutcomeWatcher {
	notify: (outcome: SendReceiptOutcome) => void;
	/**
	 * The last outcome handed over, so the same one is never sent twice. A
	 * non-terminal outcome (fee-held, acknowledged) arrives on EVERY view while
	 * it holds; the send core hears each once (spec 082: deduped on the outcome
	 * as well as the status).
	 */
	lastSent: string | null;
}

const watchers = new Map<string, OutcomeWatcher>();

function normalize(hash: string): string {
	return hash.toLowerCase();
}

/**
 * The verdict a receipt-watching surface understands, or `null` while there is
 * nothing new to say. A mirror of the core's `send::receipt_outcome_of` —
 * which has no wasm export yet (listed for the exports phase) — kept
 * exhaustive with no `default`, so a new verdict breaks this build rather than
 * defaulting into silence on a money surface.
 *
 * `unreachable` and `accepted_not_landed` with no acknowledgement produce
 * NOTHING: a slow or unreachable poll leaves the payment submitted, which is
 * invariant ① on the send core's side too (`SendReceiptOutcome`'s doc).
 */
export function outcomeOf(entry: TrackEntryView): SendReceiptOutcome | null {
	switch (entry.status) {
		case 'confirmed':
			return entry.tx_hash ? { type: 'confirmed', tx_hash: entry.tx_hash } : null;
		case 'dropped':
			// A definitive `success === false` receipt — dropped or reverted.
			return { type: 'failed', rejected: false, not_sent: false };
		case 'rejected':
			// The relay refused it before any block; nothing was sent. Why is
			// the core's reading of the relay (`refusal`): the receipt says it
			// by that reason, never every refusal as the fee sentence.
			return entry.refusal != null
				? { type: 'failed', rejected: true, not_sent: false, refusal: entry.refusal }
				: { type: 'failed', rejected: true, not_sent: false };
		case 'not_sent':
			// A may-have-been-sent op the relay never had (spec 082 RA4): not
			// sent — never the fee-rejected words.
			return { type: 'failed', rejected: false, not_sent: true };
		case 'fee_held':
			return { type: 'fee_held' };
		case 'relay_funding':
			// The relay holds it while it tops up its gas: still pending.
			return { type: 'relay_funding' };
		case 'pending':
		case 'unreachable':
		case 'accepted_not_landed':
			switch (entry.outcome) {
				case 'landing':
				case 'still_confirming':
					// The relay has it (or it was accepted outright): a receipt that
					// read "may have been sent" can drop that caption.
					return { type: 'acknowledged' };
				case 'maybe_sent':
				case 'unknown':
				case 'final':
					return null;
			}
	}
}

function deliver(view: TrackView): void {
	if (watchers.size === 0) return;
	for (const entry of view.entries) {
		const watcher = watchers.get(entry.user_op_hash);
		if (!watcher) continue;
		const outcome = outcomeOf(entry);
		if (!outcome) continue;
		const key = JSON.stringify(outcome);
		if (key === watcher.lastSent) continue;
		watcher.lastSent = key;
		// Still pending — only the wording changes (invariant ②) — so the watcher
		// stays registered for the verdict that may follow.
		if (
			outcome.type !== 'fee_held' &&
			outcome.type !== 'relay_funding' &&
			outcome.type !== 'acknowledged'
		) {
			watchers.delete(entry.user_op_hash);
		}
		watcher.notify(outcome);
	}
}

/**
 * Whether the tracker needs the clock: the core is still following some op.
 * Not "inside its wait window" — an op past it still lands, and the core, not
 * this interval, decides how often it is asked about (spec 079).
 */
export function trackerNeedsClock(view: TrackView): boolean {
	return view.entries.some((entry) => entry.polling);
}

/** Run the interval while the core still follows an op; stop it when none is left. */
function syncTicker(view: TrackView): void {
	const wanted = trackerNeedsClock(view);
	if (wanted && !ticker) {
		ticker = setInterval(() => {
			session?.dispatch({ type: 'tick' });
		}, TICK_MS);
	} else if (!wanted && ticker) {
		clearInterval(ticker);
		ticker = null;
	}
}

/**
 * Boot the machine. The core is fetched asynchronously on web, so this is a
 * promise — every caller either awaits it or fires and forgets (the dispatches
 * below queue behind the same promise, so nothing is lost).
 */
let booting: Promise<void> | null = null;

export function ensureTxTracker(): Promise<void> {
	if (booting) return booting;
	booting = (async () => {
		await loadCore();
		session = createTxTrackerSession({
			onView: (view: TrackView) => {
				// Every status change is a console line (spec 082 G61).
				for (const line of trackerLogLines(current, view)) console.log(line);
				current = view;
				deliver(view);
				syncTicker(view);
				listeners.forEach((listener) => listener(view));
				// Read from THIS view, as the resident received it: the core's
				// own JSON, `sender` and `stalled` included.
				currentOps = opsOf(view);
				opsListeners.forEach((listener) => listener(currentOps));
			},
			onError: (error) => console.error('[tx_tracker] core fault:', error),
			ports: {
				feedReconciled: (count: number) => feed.reconciled(count),
				receiptLogsConfirmed: notifyReceiptLogsConfirmed,
				// Money left: the hero total refetches past the token cache, so
				// the figure follows a send the moment its receipt lands (issue 188).
				// A dropped dispatch (balance not booted yet) costs nothing — the
				// boot's own `account_changed` fetches fresh anyway.
				confirmed: () => balance.refresh(true)
			}
		});

		// The `sign_request` seam is bound in Phase 5, where that resident lands:
		// a dApp tx that reached the relay is handed over the moment its view
		// publishes a handoff. Until then the only producer is the send flow.

		// Foregrounding is a reconcile trigger, as `AppState` 'active' was for the
		// TS reconciler. Guarded for the static render pass, which has no document.
		if (typeof document !== 'undefined' && typeof document.addEventListener === 'function') {
			document.addEventListener('visibilitychange', () => {
				if (!document.hidden) session?.dispatch({ type: 'app_resumed' });
			});
		}

		// `start` commits the pristine view first (the frame `INITIAL_VIEW`
		// mirrors), then sweeps the store: this IS the cross-restart recovery that
		// `dapp-connection.tsx`'s `resumedRef` scan and the reconciler's launch run
		// used to do separately (invariant ⑥).
		session.start({ type: 'app_resumed' });
	})();
	return booting;
}

export function dispatchTxTracker(event: TrackEvent): void {
	void ensureTxTracker().then(() => session?.dispatch(event));
}

/**
 * Boot the machine and ask for a recovery sweep. Idempotent and cheap: the
 * core throttles the sweep to one per 12 s and is single-flight, so calling it
 * from every mount is free.
 */
export function startTxTracker(): void {
	const started = session !== null;
	void ensureTxTracker().then(() => {
		if (started) session?.dispatch({ type: 'app_resumed' });
	});
}

/**
 * Hand a freshly accepted UserOp over. Idempotent by hash in the core — a
 * second consumer of the same hash joins the same entry, the same in-flight
 * request and the same 3 s cooldown (invariant ⑤) — so a resubmit, a recovery
 * sweep and a live submit can all name it without doubling any traffic.
 *
 * `watch` is fired at most once per verdict and never for a timeout, an abort
 * or an unreachable bundler.
 */
export function trackSubmitted(
	userOpHash: string,
	recordIds: string[],
	chainId: number,
	watch?: (outcome: SendReceiptOutcome) => void,
	/** The submit's reply was lost; the hash is the local one (spec 082 RA4). */
	maybeSent = false,
	/** The head read before the first POST — the find-event's start (ruling 8). */
	submitBlock: number | null = null,
	/**
	 * The relay took the op a write-ahead hand-off announced (spec 082 RJ1): it
	 * is never "may have been sent" again. Its watcher, if any, is kept.
	 */
	admitted = false,
	/**
	 * The account that signed it (the send machine's `TrackSubmitted.sender`):
	 * how the tracker knows this account now has an operation in flight on
	 * this chain, and holds its next confirm (correctness batch item 3).
	 */
	sender: string | null = null
): void {
	if (!userOpHash) return;
	const key = normalize(userOpHash);
	if (watch) watchers.set(key, { notify: watch, lastSent: null });
	dispatchTxTracker({
		type: 'submitted',
		user_op_hash: key,
		record_ids: recordIds,
		chain_id: chainId,
		maybe_sent: maybeSent,
		submit_block: submitBlock,
		admitted,
		...(sender ? { sender } : {})
	});
}

/**
 * A write-ahead op proven never sent (spec 082 RJ1): the tracker drops those
 * records (and the entry, when none is left) without patching anything, and
 * whoever watched it stops hearing about it.
 */
export function withdrawTracked(userOpHash: string, recordIds: string[]): void {
	if (!userOpHash) return;
	const key = normalize(userOpHash);
	watchers.delete(key);
	dispatchTxTracker({ type: 'withdrawn', user_op_hash: key, record_ids: recordIds });
}

/** Stop watching a hash — the surface went away, the tracking continues. */
export function unwatchTxTracker(userOpHash: string): void {
	watchers.delete(normalize(userOpHash));
}

/** The latest committed view. Synchronous — that is the whole point. */
export function txTrackerView(): TrackView {
	return current;
}

/** Subscribe to every committed view. Returns the unsubscribe. */
export function subscribeTxTracker(listener: (view: TrackView) => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

/**
 * Hear the operations holding their account's nonce — now, and after every
 * tracker render (correctness batch item 3). Boots the tracker, whose sweep of
 * the stored pending records is what lets a reload still know an operation is
 * in flight. An unchanged list is not repeated to the same listener (the
 * machines dedupe it too), so a hold never flickers. Returns the unsubscribe.
 */
export function subscribeInFlightOps(listener: (ops: InFlightOp[]) => void): () => void {
	let told: string | null = null;
	const tell = (ops: InFlightOp[]) => {
		const key = JSON.stringify(ops);
		if (key === told) return;
		told = key;
		listener(ops);
	};
	opsListeners.add(tell);
	tell(currentOps);
	void ensureTxTracker();
	return () => {
		opsListeners.delete(tell);
	};
}
