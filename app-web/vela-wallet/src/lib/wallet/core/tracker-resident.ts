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
 *   core's — so the ticker here is a dumb 3 s interval that exists only while
 *   some op is inside its 120 s wait window. Once every entry is post-window
 *   (fee-held, accepted-not-landed, unreachable) the interval stops and those
 *   entries converge on focus/resume at the core's 12 s reconcile pace, which is
 *   exactly the cadence `waitForReceipt` + the Home-focus reconciler had.
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
import { createTxTrackerSession } from './tracker-session';

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

/** A surface waiting on one hash — the send screen's receipt, today's only one. */
interface OutcomeWatcher {
	notify: (outcome: SendReceiptOutcome) => void;
	/** A fee-hold is not terminal: it may still confirm, so it is sent once. */
	feeHeldSent: boolean;
}

const watchers = new Map<string, OutcomeWatcher>();

function normalize(hash: string): string {
	return hash.toLowerCase();
}

/**
 * The verdict a receipt-watching surface understands, or `null` while the op is
 * still in flight. `unreachable` and `accepted_not_landed` deliberately produce
 * NOTHING: a slow or unreachable poll leaves the payment submitted, which is
 * invariant ① on the send core's side too (`SendReceiptOutcome`'s doc).
 */
function outcomeOf(entry: TrackEntryView): SendReceiptOutcome | null {
	// Deliberately exhaustive, with no `default`: a new verdict in the core must
	// break this build rather than default into silence on a money surface.
	switch (entry.status) {
		case 'confirmed':
			return entry.tx_hash ? { type: 'confirmed', tx_hash: entry.tx_hash } : null;
		case 'dropped':
			// A definitive `success === false` receipt — dropped or reverted.
			return { type: 'failed', rejected: false };
		case 'rejected':
			// The relay refused it before any block; nothing was sent.
			return { type: 'failed', rejected: true };
		case 'fee_held':
			return { type: 'fee_held' };
		case 'pending':
		case 'unreachable':
		case 'accepted_not_landed':
			// Invariant ①: still in flight, or genuinely unknown. The surface keeps
			// showing "submitted" — it is never told a timeout was a failure.
			return null;
	}
}

function deliver(view: TrackView): void {
	if (watchers.size === 0) return;
	for (const entry of view.entries) {
		const watcher = watchers.get(entry.user_op_hash);
		if (!watcher) continue;
		const outcome = outcomeOf(entry);
		if (!outcome) continue;
		if (outcome.type === 'fee_held') {
			// Still pending, only the wording changes (invariant ②) — so the watcher
			// stays registered for the confirmation that may follow.
			if (watcher.feeHeldSent) continue;
			watcher.feeHeldSent = true;
			watcher.notify(outcome);
			continue;
		}
		watchers.delete(entry.user_op_hash);
		watcher.notify(outcome);
	}
}

/**
 * Run the interval only while something is inside its wait window. Everything
 * else converges on `HomeFocused` / `AppResumed`, at the core's own 12 s pace.
 */
function syncTicker(view: TrackView): void {
	const wanted = view.entries.some((entry) => entry.polling && entry.status === 'pending');
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
				current = view;
				deliver(view);
				syncTicker(view);
				listeners.forEach((listener) => listener(view));
			},
			onError: (error) => console.error('[tx_tracker] core fault:', error),
			ports: {
				feedReconciled: (count: number) => {
					if (count > 0) feed.liveTick();
				},
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
	watch?: (outcome: SendReceiptOutcome) => void
): void {
	if (!userOpHash) return;
	const key = normalize(userOpHash);
	if (watch) watchers.set(key, { notify: watch, feeHeldSent: false });
	dispatchTxTracker({
		type: 'submitted',
		user_op_hash: key,
		record_ids: recordIds,
		chain_id: chainId
	});
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
