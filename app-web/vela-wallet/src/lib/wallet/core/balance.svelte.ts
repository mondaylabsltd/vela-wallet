/**
 * The `balance_dashboard` resident, as Svelte sees it (spec 025 T121).
 *
 * The ported resident (`balance-resident.ts`) is the engine — one
 * app-lifetime session, view dedup by structural key, reference-stable
 * projections, the boot-time privacy hydrate. This class is the thin bridge
 * that turns its subscription into a `$state` view and gates construction on
 * the wasm being aboard (the Expo module relied on import-time init; here
 * `loadCore()` is awaited first, as every other web store does).
 *
 * Methods are one-liners forwarding typed events — no logic lives here.
 */

import { loadCore } from '$lib/core/client';
import { ensureCustomNetworks } from '$lib/services/networks';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import {
	balanceView,
	dispatchBalance,
	ensureBalanceDashboard,
	INITIAL_VIEW,
	subscribeBalanceDashboard
} from './balance-resident';

class Balance {
	view = $state<BalanceView>(INITIAL_VIEW);

	#booted: Promise<void> | null = null;

	/** Idempotent; every screen that shows money may call it. */
	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			// The stored custom networks come first (spec 038 #E9): the first
			// fetch walks `getAllNetworksSync()`, and a snapshot that has not
			// read storage yet is the builtin table — an added network would be
			// neither fetched nor listed until a Settings write.
			await Promise.all([ensureCustomNetworks(), loadCore()]);
			ensureBalanceDashboard();
			subscribeBalanceDashboard((view) => {
				this.view = view;
			});
			this.view = balanceView();
		})();
		return this.#booted;
	}

	/** The signed-in address changed (or arrived). The core re-hydrates for it. */
	async setAccount(address: string): Promise<void> {
		await this.boot();
		dispatchBalance({ type: 'account_changed', address });
	}

	refresh(force = false, pull = false): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'refresh_requested', force, pull });
	}

	/** Tap-to-hide. Persistence and first-write-wins are the core's. */
	togglePrivacy(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'privacy_toggled' });
	}

	focused(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'app_focused' });
	}

	backgrounded(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'app_backgrounded' });
	}

	/**
	 * The account switcher is on screen (spec 028 Phase 8): the core refreshes
	 * every listed account's total while it is, and `view.switcher.balances`
	 * carries the answers — Expo's `AccountSwitcherModal` sourcing, in the one
	 * machine that already owns the policy.
	 */
	openSwitcher(addresses: string[]): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'switcher_opened', addresses });
	}

	closeSwitcher(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'switcher_closed' });
	}

	/** An RPC fix landed for one chain (spec 028 Phase 8): the core retries that chain alone. */
	fixChainResolved(chainId: number): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'fix_chain_resolved', chain_id: chainId });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const balance = new Balance();
