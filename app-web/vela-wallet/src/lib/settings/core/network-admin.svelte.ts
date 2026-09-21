/**
 * The ONE resident `network_admin` session — WEB (spec 024, research D8).
 *
 * App-resident on the Expo resident's rationale
 * (src/services/wallet-state-core/network-admin-resident.ts @ e78afdfa): the
 * machine is the app-lifetime mirror of the network stores, and a second
 * entry point (spec 025's EIP-681 scan recovery) must share the same ledger
 * or the core-owned duplicate-chain gate reads the wrong one. Svelte 5 shape
 * per `$lib/session/core/session.svelte.ts`: `$state` view, idempotent
 * `boot()`, one-liner dispatch methods.
 */

import { loadCore, NetworkAdminCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { NetEvent } from '$lib/core/generated/NetEvent';
import type { NetShellResult } from '$lib/core/generated/NetShellResult';
import type { NetView } from '$lib/core/generated/NetView';
import type { NetEffect } from './network-admin-types';
import {
	executeNetworkAdminOperation,
	networkAdminOperationFailure
} from './network-admin-executor';

/** The machine's own initial projection — rendered until the first view lands.
 *  A hand-written constant so prerender and the pre-boot frame never touch
 *  wasm (the session store's BOOTING precedent). */
const EMPTY_VIEW: NetView = {
	loaded: false,
	networks: [],
	wizard: {
		phase: 'idle',
		query: '',
		custom_rpc: '',
		suggestions: [],
		chain_info: null,
		compat: null,
		error: null,
		can_add: false
	},
	endpoints: [],
	providers: [],
	last_added_chain_id: null
};

class NetworkAdmin {
	view = $state<NetView>(EMPTY_VIEW);

	#loop: EffectLoop<NetEvent> | null = null;
	#booted: Promise<void> | null = null;
	/** Screen events raised before the stores were read — replayed, in order, once they are. */
	#early: NetEvent[] = [];

	/**
	 * Load the core and hydrate from the stores. Idempotent — every screen that
	 * needs the ledger may call it; `started` re-reads, so a second boot from a
	 * later surface is a redundant read, never a write.
	 */
	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			this.#loop = createJsonWasmShell<NetView, NetEvent, NetEffect, NetShellResult>(
				new NetworkAdminCore(),
				{
					onView: (view) => {
						this.view = view;
						if (view.loaded) this.#replayEarly();
					},
					execute: (effect) => executeNetworkAdminOperation(effect),
					toFailure: networkAdminOperationFailure,
					onError: (error) => console.error('[network-admin] core fault:', error)
				}
			);
			this.#loop.start({ type: 'started' });
		})();
		return this.#booted;
	}

	/**
	 * Every screen event, verbatim — the core decides what each one means.
	 *
	 * One raised before the stores are read waits for them. The machine seeds
	 * from what it has loaded: "the providers page opened", answered over an
	 * empty ledger, left the saved keys showing with no draft behind them, and
	 * the first field to lose focus then saved an empty key over the one it
	 * showed (spec 072, P0). Dropped outright before `boot()` had finished,
	 * it did the same.
	 */
	dispatch(event: NetEvent): void {
		if (this.#loop === null || !this.view.loaded || this.#early.length > 0) {
			this.#early.push(event);
			return;
		}
		this.#loop.dispatch(event);
	}

	#replayEarly(): void {
		const early = this.#early;
		this.#early = [];
		for (const event of early) this.#loop?.dispatch(event);
	}
}

/**
 * Only ever touched in the browser: `boot()` loads wasm, which prerender must
 * not do, so callers guard on mount rather than at import.
 */
export const networkAdmin = new NetworkAdmin();
