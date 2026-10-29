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
 * Methods are one-liners forwarding typed events — no logic lives here, but
 * one rule (spec 105): the view is the space in front's. A switch of space
 * reads the balances again, and until that round has settled the other
 * space's holdings — and a total that counted them — are not shown.
 */

import { loadCore } from '$lib/core/client';
import { networkSpace } from '$lib/network-space/network-space.svelte';
import { ensureShownNetworks, inSpace } from '$lib/services/networks';
import { loadHoldings, saveHoldings } from '$lib/services/holdings-cache';
import { onPrivacyElsewhere } from './privacy-sync';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import {
	balanceSettledFor,
	balanceView,
	dispatchBalance,
	ensureBalanceDashboard,
	INITIAL_VIEW,
	rereadHoldings,
	subscribeBalanceDashboard
} from './balance-resident';

/** `view` with only the space in front's holdings; a total that counted any other is not shown. */
function inSpaceView(view: BalanceView): BalanceView {
	const tokens = view.tokens.filter((token) => inSpace(token.chain_id));
	if (tokens.length === view.tokens.length) return view;
	return { ...view, tokens, display_total_usd: null };
}

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
			// neither fetched nor listed until a Settings write. And the Vela
			// networks out of the person's lists (owner decision 2026-10-29)
			// are settled before the first read, so none is read and then hidden.
			await Promise.all([ensureShownNetworks(), loadCore()]);
			ensureBalanceDashboard();
			subscribeBalanceDashboard((view) => {
				this.#keepSettledRows(view);
				this.view = inSpaceView(view);
			});
			this.view = inSpaceView(balanceView());
			// home-12: the person hid or showed the balance in another tab — this
			// one follows, through the same eye the person would press.
			onPrivacyElsewhere((hidden) => {
				if (balanceView().hidden !== hidden) dispatchBalance({ type: 'privacy_toggled' });
			});
			// A switch of space starts the account over in the core (home-7):
			// nothing the other space failed or held is said here.
			networkSpace.subscribe(() => {
				this.view = inSpaceView(balanceView());
				dispatchBalance({ type: 'space_changed' });
				void this.#restoreRows();
			});
		})();
		return this.#booted;
	}

	/** The settle the rows were last kept for: `address` and its settle time. */
	#kept: { address: string | null; at: number | null } = { address: null, at: null };

	/**
	 * A round settled (`last_refreshed_at_ms` moved for the same account): the
	 * rows it left on the home are kept for the next visit (home-1, home-3).
	 */
	#keepSettledRows(view: BalanceView): void {
		const { address, last_refreshed_at_ms: at } = view;
		const settled = address !== null && address === this.#kept.address && at !== this.#kept.at;
		this.#kept = { address, at };
		if (settled && at !== null) void saveHoldings(address, networkSpace.current, view.tokens);
	}

	/**
	 * The rows the home showed last time for the account and space in front,
	 * handed to the core, which lets them stand until each chain answers.
	 */
	async #restoreRows(): Promise<void> {
		const address = balanceView().address;
		if (address === null) return;
		const tokens = await loadHoldings(address, networkSpace.current);
		if (tokens.length > 0) dispatchBalance({ type: 'holdings_restored', address, tokens });
	}

	/**
	 * Whether `address`'s holdings have settled a round (see `balanceSettledFor`).
	 * Read alongside `view`, which is what makes a caller's effect re-run.
	 */
	settledFor(address: string): boolean {
		return balanceSettledFor(address);
	}

	/**
	 * One forced re-read for `address`; resolves when its next round has ended
	 * (see `rereadHoldings`). Resolves at once before the machine has booted —
	 * there is no round to wait for.
	 */
	reread(address: string): Promise<void> {
		if (!this.#booted) return Promise.resolve();
		return rereadHoldings(address);
	}

	/** The signed-in address changed (or arrived). The core re-hydrates for it. */
	async setAccount(address: string): Promise<void> {
		await this.boot();
		const changed = balanceView().address !== address;
		dispatchBalance({ type: 'account_changed', address });
		if (changed) await this.#restoreRows();
	}

	/**
	 * The signed-in address, for a screen that draws no balance of its own
	 * (perf-10: settings, the address book). The core sets it and reads its
	 * cached total — the switcher and the privacy mask are right — but asks no
	 * network until something shows the balance: `setAccount` from the wallet
	 * home, a refresh, or the switcher opening. Opened directly, those pages
	 * read 24 networks for a figure nobody saw.
	 */
	async noteAccount(address: string): Promise<void> {
		await this.boot();
		const changed = balanceView().address !== address;
		dispatchBalance({ type: 'account_noted', address });
		if (changed) await this.#restoreRows();
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

	/**
	 * The list of networks the wallet cannot reach is on screen (spec 092): the
	 * core reads every chain again, quietly, and keeps re-reading while it stays
	 * open, so one that has come back leaves it.
	 */
	unreachableListOpened(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'unreachable_list_opened' });
	}

	/** That list closed: the re-reads stop. */
	unreachableListClosed(): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'unreachable_list_closed' });
	}

	/** An RPC fix landed for one chain (spec 028 Phase 8): the core retries that chain alone. */
	fixChainResolved(chainId: number): void {
		if (!this.#booted) return;
		dispatchBalance({ type: 'fix_chain_resolved', chain_id: chainId });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const balance = new Balance();
