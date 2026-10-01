// Ported from src/services/wallet-state-core/feed-resident.ts @ c13e89d4 (spec 025).
/**
 * The one `activity_feed` core the web app has — WEB only, and APP-RESIDENT.
 *
 * The machine keeps three things that outlive a mount and would be wrong to
 * rebuild per screen: the session-lived alias memo (`alias_map` /
 * `alias_attempted` — an address→name fact is account-agnostic, and re-asking
 * costs a network round trip per counterparty), the first-pass backlog gate that
 * stops an existing backlog from celebrating, and the toast generation that
 * makes a superseded timer inert. So it is a module-level singleton, the
 * `session-resident.web.ts` pattern.
 *
 * Reference stability, for the same reason the balance resident needs it: the
 * feed is a `FlatList` and `HomeScreen`'s header carries one-shot entrance
 * animations. Equal views are dropped, and `rows` is re-projected only when the
 * rows themselves change — so an alias resolving, a toast expiring or a
 * best-effort delete settling does not re-render the whole table.
 *
 * Imported by explicit `.web` specifier on every side: `tsc` resolves a
 * `.web.ts` file's own imports to the base `.ts` variant, so a bare specifier
 * would type-check against a native module that does not exist.
 */

import { createActivityFeedSession } from './feed-session';
import type { FeedEvent } from '$lib/core/generated/FeedEvent';
import type { FeedOwnAccount } from './feed-types';
import type { FeedRow } from '$lib/core/generated/FeedRow';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { LocalTransaction } from '$lib/services/transactions-model';

/** The machine's own initial projection — mirrored until the first view lands. */
export const INITIAL_VIEW: FeedView = {
	rows: [],
	transactions: [],
	new_item_id: null,
	toast: null,
	// The core's own choice for "no chain filter" (spec 082 RG5), mirrored
	// until the first view lands.
	history_empty_key: 'history.emptyTitle',
	home_empty_key: 'home.emptyNoActivity'
};

let current: FeedView = INITIAL_VIEW;
/** Structural key of `current`, so an unchanged view never re-renders Home. */
let currentKey = JSON.stringify(INITIAL_VIEW);
let currentRows: FeedRow[] = [];
let currentRowsKey = '[]';

/**
 * The raw rows behind the last `StoreLoaded`, indexed by id — the `txByIdRef`
 * port. The detail sheet needs fields `FeedTxRecord` deliberately drops
 * (`dappOrigin`, `intent`, `signedRequest`, `assetSim`), so they are kept whole
 * rather than reconstructed from the wire shape.
 */
let rawById = new Map<string, LocalTransaction>();

/**
 * The account the core was last told about. `AccountSwitched` bumps the attempt
 * and restarts the pipeline unconditionally, so the shell must not repeat it —
 * this is the `[address]` dependency of the TS reset effect, made explicit.
 */
let account: string | null = null;

const listeners = new Set<(view: FeedView) => void>();
let session: ReturnType<typeof createActivityFeedSession> | null = null;

export function ensureActivityFeed(ownAccounts: () => FeedOwnAccount[]) {
	if (!session) {
		session = createActivityFeedSession({
			onView: (view: FeedView) => {
				const key = JSON.stringify(view);
				if (key === currentKey) return;
				currentKey = key;
				const rowsKey = JSON.stringify(view.rows);
				if (rowsKey !== currentRowsKey) {
					currentRowsKey = rowsKey;
					currentRows = view.rows;
				}
				current = view;
				listeners.forEach((listener) => listener(view));
			},
			onError: (error) => console.error('[activity-feed] core fault:', error),
			// The shell owns the accounts list; a counterparty that is one of the
			// user's own accounts resolves to its local name without any network.
			ownAccounts,
			records: {
				storeLoaded: (records) => {
					// `new Map(rawTxs.map(...))`: a legacy duplicate id keeps the LAST
					// row, exactly as the TS map did.
					rawById = new Map(records.map((tx) => [tx.id, tx]));
				}
			}
		});
		// `FocusTick` with no account yet is a whole no-op in the core — it exists
		// only to give `start()` an event so the pristine view is committed (the
		// frame `INITIAL_VIEW` already mirrors).
		session.start({ type: 'focus_tick' });
	}
	return session;
}

/** Set by the first `ensureActivityFeed`; a dispatch before boot is a no-op. */
export function dispatchActivityFeed(event: FeedEvent): void {
	session?.dispatch(event);
}

/** Point the feed at an account. Idempotent: the same address is a no-op. */
export function setActivityFeedAccount(address: string): void {
	if (address === account) return;
	account = address;
	dispatchActivityFeed({ type: 'account_switched', address });
}

/** The latest committed view. */
export function activityFeedView(): FeedView {
	return current;
}

/** Date headers + items in render order, reference-stable while unchanged. */
export function activityFeedRows(): FeedRow[] {
	return currentRows;
}

/** The raw stored record behind a feed row — what the detail sheet renders. */
export function activityFeedTx(id: string): LocalTransaction | undefined {
	return rawById.get(id);
}

/** Subscribe to every committed view. Returns the unsubscribe. */
export function subscribeActivityFeed(listener: (view: FeedView) => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

/**
 * The `tx_tracker` seam, now wired.
 *
 * The core re-reads the store when pending submissions converged, and it never
 * celebrates those (`useHomeController.ts:284-295`). Deciding *that* they
 * converged is `tx_tracker`'s job, not this machine's: its executor calls this
 * with the number of records its `UpdateTxRecords` just patched
 * (`tx-tracker-executor.web.ts`). The TypeScript `reconcileFeedPending` that
 * stood in for it — and with it web's second caller of
 * `reconcilePendingTransactions` — is gone; native still runs that reconciler
 * from `useHomeController.ts`.
 *
 * A count of 0 is a whole no-op in the core, so it is always safe to call.
 */
export function notifyFeedReconciled(resolvedCount: number): void {
	dispatchActivityFeed({ type: 'reconcile_completed', resolved_count: resolvedCount });
}
