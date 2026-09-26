/**
 * The ONE resident `sign_pref` session — WEB (spec 071): which Trusted Signer
 * page this device opens.
 *
 * App-resident for the reason `fee-tier.svelte.ts` is: readers with their own
 * copies of a preference are how they come to disagree about it.
 *
 * What it is NOT: where a signature goes. That is the key the account signed
 * in with (founder, 2026-09-26), read from the account record in
 * `$lib/signing/sign-challenge.ts`, never a preference.
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
 * The machine's own initial view, mirrored until the core rules: the official
 * page. A surface that renders before the read lands must show that, not a
 * guess.
 */
const INITIAL: SignPrefView = {
	signer_url: 'https://sign.getvela.app/',
	signer_url_is_default: true,
	signer_url_error: null,
	signer_uses_wallet_passkeys: true
};

class SignPreference {
	view = $state<SignPrefView>(INITIAL);

	#loop: EffectLoop<SignPrefEvent> | null = null;
	#booted: Promise<void> | null = null;
	/** The stored value has been read (or could not be): the view is the person's. */
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
	 * Booted AND the stored value read: the view before the read is the
	 * official page, not the one this person chose.
	 */
	async ready(): Promise<void> {
		await this.boot();
		await this.#loaded;
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
