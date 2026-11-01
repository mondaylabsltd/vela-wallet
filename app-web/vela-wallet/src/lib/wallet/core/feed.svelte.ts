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

import { accountName } from '$lib/multisig/core/account-name';
import { multisigAccountsForExt } from '$lib/multisig/core/ext';
import { loadCore } from '$lib/core/client';
import type { FeedView } from '$lib/core/generated/FeedView';
import { session } from '$lib/session/core/session.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import { spaceNetworksSync, subscribeNetworks } from '$lib/services/networks';
import { balanceReadPlan } from '$lib/core/kernels';
import { decimalToBase, onNativeBalanceMoved, setShownNative } from '$lib/services/native-watch';
import {
	activityFeedView,
	dispatchActivityFeed,
	ensureActivityFeed,
	INITIAL_VIEW,
	setActivityFeedAccount,
	subscribeActivityFeed
} from './feed-resident';

/** Whether the balances read a coin of `chainId`'s own (Tempo has none). */
/** A memo, never drawn: no reactivity wanted. */
const nativeChains: Record<number, boolean> = {};
function readsNative(chainId: number): boolean {
	let reads = nativeChains[chainId];
	if (reads === undefined) {
		try {
			reads = balanceReadPlan(chainId, [], null, []).some((slot) => slot.kind === 'native');
		} catch {
			reads = false;
		}
		nativeChains[chainId] = reads;
	}
	return reads;
}

/**
 * What the home shows of `address`'s coin on `chainId`, in base units — `0n`
 * for no row — once a round has settled for that account; `undefined`
 * otherwise (another account shown, nothing settled yet, no coin there).
 */
export function shownNative(address: string, chainId: number): bigint | undefined {
	const view = balance.view;
	if (view.address === null || view.address.toLowerCase() !== address.toLowerCase()) {
		return undefined;
	}
	if (!balance.settledFor(view.address) || !readsNative(chainId)) return undefined;
	const row = view.tokens.find((t) => t.chain_id === chainId && t.token_address === null);
	return row ? decimalToBase(row.balance, row.decimals) : 0n;
}

class Feed {
	view = $state<FeedView>(INITIAL_VIEW);
	/**
	 * Spec 093: the contact the core was last told is open — whose rows
	 * `view.contact_rows` holds. A page draws them only under that contact,
	 * so the rows of the one just closed never stand under the next.
	 */
	contactAddress = $state<string | null>(null);

	#booted: Promise<void> | null = null;

	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			// Every account here names itself on a row, multisigs among them.
			ensureActivityFeed(() => [
				...session.view.accounts.map((row) => ({
					address: row.account.address,
					name: accountName(row.account.address, row.account.name)
				})),
				...multisigAccountsForExt().accounts.map((a) => ({ address: a.address, name: a.name }))
			]);
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
			// Spec 105: only the space in front's records are drawn; the
			// network set changes with every switch of space.
			const space = () =>
				dispatchActivityFeed({
					type: 'space_chains_changed',
					chain_ids: spaceNetworksSync().map((n) => n.chainId)
				});
			space();
			subscribeNetworks(space);
			// home-5: a native coin arrived where the scan finds no log (an
			// account with no code yet) — its balance moved, so the hero and
			// the rows read again past the token cache, as for a found receipt.
			onNativeBalanceMoved((address) => {
				if (balance.view.address?.toLowerCase() === address.toLowerCase()) {
					balance.refresh(true);
				}
			});
			// r4a-17: the watch holds each reading against what the home shows,
			// so a coin that landed before the watch's first reading, or whose
			// re-read did not reach the network, is still asked for.
			setShownNative((address, chainId) => shownNative(address, chainId));
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

	/**
	 * Records were written or patched outside the feed — a dApp request's
	 * pending row, a tracker verdict (spec 082 RG3). The core's own
	 * `ReconcileCompleted`: it re-reads the store without celebrating, and `0`
	 * is a whole no-op.
	 */
	reconciled(count: number): void {
		if (!this.#booted || count <= 0) return;
		dispatchActivityFeed({ type: 'reconcile_completed', resolved_count: count });
	}

	/**
	 * Settings' Device storage cleared the transaction history (agent-23): the
	 * core reads its store again — the store is the source of truth even
	 * about emptiness — so neither Activity nor the agent's
	 * `recent_transactions` go on showing what was just removed.
	 */
	historyCleared(): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'reconcile_completed', resolved_count: 1 });
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

	/**
	 * A contact's page opened (`address`) or closed (`null`): the core then
	 * hands `view.contact_rows` — what passed between the account and that
	 * address, worded as Activity words it (spec 093). Waits for the core,
	 * so a page opened before it booted is not lost.
	 */
	async contactFilter(address: string | null): Promise<void> {
		await this.boot();
		dispatchActivityFeed({ type: 'contact_filter_changed', address });
		this.contactAddress = address;
	}

	deleteRecord(id: string): void {
		if (!this.#booted) return;
		dispatchActivityFeed({ type: 'delete_requested', id });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const feed = new Feed();
