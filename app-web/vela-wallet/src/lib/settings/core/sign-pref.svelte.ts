/**
 * The ONE resident `sign_pref` session — WEB (spec 071).
 *
 * App-resident for the reason `fee-tier.svelte.ts` is: more than one surface
 * reads this preference. Settings chooses it; every signing sheet starts a
 * request at its `method`; a send with no sheet signs the way it says; and
 * the Trusted Signer opens the page it names. Readers with their own copies of
 * a preference are how they come to disagree about it.
 *
 * What it is NOT: the signing sheet's "Sign with" row. A pick there changes
 * one request and never reaches this machine (contract §6) — the next
 * request starts at the default again.
 */

import { SignPrefCore, loadCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { SignPrefEvent } from '$lib/core/generated/SignPrefEvent';
import type { SignPrefShellResult } from '$lib/core/generated/SignPrefShellResult';
import type { SignPrefView } from '$lib/core/generated/SignPrefView';
import {
	executeSignPrefOperation,
	signPrefOperationFailure,
	type SignPrefEffect
} from './sign-pref-executor';

/**
 * The machine's own initial view, mirrored until the core rules: `auto` and
 * the official page — what this wallet did before the preference existed. A
 * surface that renders before the read lands must show that, not a guess.
 */
const INITIAL: SignPrefView = {
	method: 'auto',
	method_committed: false,
	offered: ['auto', 'platform', 'hybrid', 'security_key', 'trusted_signer'],
	signer_url: 'https://sign.getvela.app/',
	signer_url_is_default: true,
	signer_url_error: null,
	signer_uses_wallet_passkeys: true
};

class SignPreference {
	view = $state<SignPrefView>(INITIAL);

	#loop: EffectLoop<SignPrefEvent> | null = null;
	#booted: Promise<void> | null = null;
	/** The stored values have been read (or could not be): the view is the person's. */
	#stored = false;
	#loaded: Promise<void>;
	#markLoaded: () => void = () => {};

	constructor() {
		this.#loaded = new Promise((resolve) => (this.#markLoaded = resolve));
	}

	/** Idempotent; a later surface's boot is a coalesced refresh, never a race. */
	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			this.#loop = createJsonWasmShell<
				SignPrefView,
				SignPrefEvent,
				SignPrefEffect,
				SignPrefShellResult
			>(new SignPrefCore(), {
				onView: (view) => {
					this.view = view;
					if (this.#stored) this.#markLoaded();
				},
				execute: async (effect) => {
					try {
						return await executeSignPrefOperation(effect);
					} finally {
						// The view the core commits next is the stored one.
						if (effect.operation.type === 'read_stored') this.#stored = true;
					}
				},
				toFailure: signPrefOperationFailure,
				onError: (error) => console.error('[sign-pref] core fault:', error)
			});
			this.#loop.start({ type: 'refresh' });
		})();
		return this.#booted;
	}

	/**
	 * Booted AND the stored values read. What a signature waits on before it
	 * decides where to go: the view before the read is the factory default,
	 * not what this person chose.
	 */
	async ready(): Promise<void> {
		await this.boot();
		await this.#loaded;
	}

	/**
	 * `ready()` when something has booted this preference, and nothing to wait
	 * for otherwise: with no surface that reads it up, there is no stored
	 * choice in play and the factory default stands.
	 */
	async settled(): Promise<void> {
		if (this.#booted) await this.ready();
	}

	/** Settings: the default "Sign with". The core ignores a name it does not offer. */
	chooseMethod(method: string): void {
		this.#loop?.dispatch({ type: 'method_chosen', method });
	}

	/** Settings: the Trusted Signer page, as typed. The core validates it. */
	submitSignerUrl(text: string): void {
		this.#loop?.dispatch({ type: 'signer_url_submitted', text });
	}

	/** Settings: back to the official page. */
	resetSignerUrl(): void {
		this.#loop?.dispatch({ type: 'signer_url_reset' });
	}

	refresh(): void {
		this.#loop?.dispatch({ type: 'refresh' });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const signPreference = new SignPreference();
