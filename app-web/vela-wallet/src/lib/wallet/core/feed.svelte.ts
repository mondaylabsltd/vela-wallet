/**
 * The `activity_feed` resident, as Svelte sees it (spec 025 Phase 4).
 *
 * The ported resident (`feed-resident.ts`) is the engine — one app-lifetime
 * session (alias memo, backlog gate, toast generation outlive any screen),
 * view dedup, reference-stable rows, the raw-record index for the detail
 * sheet. This class bridges its subscription into `$state` and gates
 * construction on the wasm (`loadCore()` first). The shell's accounts list
 * — local names win over the network — comes from the session store.
 */

import { loadCore } from '$lib/core/client';
import type { FeedView } from '$lib/core/generated/FeedView';
import { session } from '$lib/session/core/session.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import {
	activityFeedView,
	dispatchActivityFeed,
	ensureActivityFeed,
	INITIAL_VIEW,
	setActivityFeedAccount,
	subscribeActivityFeed
} from './feed-resident';

class Feed {
	view = $state<FeedView>(INITIAL_VIEW);

	#booted: Promise<void> | null = null;

	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			ensureActivityFeed(() =>
				session.view.accounts.map((row) => ({
					address: row.account.address,
					name: row.account.name
				}))
			);
			subscribeActivityFeed((view) => {
				// A transfer the scan just found moved the balances too: the hero
				// refetches past the token cache (issue 188). The core celebrates only
				// a genuinely-new incoming record, never the first pass, so this
				// fires once per arrival — the row glows, the figure follows.
				if (view.new_item_id !== null && view.new_item_id !== this.view.new_item_id) {
					balance.refresh(true);
				}
				this.view = view;
			});
			this.view = activityFeedView();
		})();
		return this.#booted;
	}

	/** Point the feed at an account (idempotent for the same address). */
	async setAccount(address: string): Promise<void> {
		await this.boot();
		setActivityFeedAccount(address);
	}

	/** The screen regained focus — the core re-reads and re-scans by its rules. */
	focusTick(): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'focus_tick' });
	}

	/** The periodic poll (and the receive watcher's nudge). */
	liveTick(): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'live_tick' });
	}

	/** Balance privacy changed — the core withholds the toast (invariant ④). */
	privacyChanged(hidden: boolean): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'privacy_changed', hidden });
	}

	chainFilter(chainId: number | null): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'chain_filter_changed', chain_id: chainId });
	}

	deleteRecord(id: string): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'delete_requested', id });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const feed = new Feed();
