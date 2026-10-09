/**
 * The ONE resident `display_currency` session — WEB (spec 024, research D8).
 *
 * App-resident on the Expo resident's evidence
 * (display-currency-resident.ts @ e78afdfa): two writers racing on
 * `vela.displayCurrency` with different seeding rules was a real bug
 * (spec 017), so every money-showing surface reads the same
 * atomically-committed {code, rate} from here — 025's balances included.
 */

import { DisplayCurrencyCore, loadCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { CurrencyEvent } from '$lib/core/generated/CurrencyEvent';
import type { CurrencyShellResult } from '$lib/core/generated/CurrencyShellResult';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import {
	currencyOperationFailure,
	executeCurrencyOperation,
	type CurrencyEffect
} from './currency-executor';

/** The machine's own initial view — USD/1, mirrored until the core rules.
 *  `rate: null` elsewhere is "cannot price"; the pristine USD pair is 1.
 *  Uncommitted, so no money figure is drawn in it (the core's rule), and
 *  nothing is pending: the stored choice has not been read yet. */
const INITIAL: CurrencyView = { code: 'USD', rate: 1, committed: false, pending: null };

class Currency {
	view = $state<CurrencyView>(INITIAL);

	#loop: EffectLoop<CurrencyEvent> | null = null;
	#booted: Promise<void> | null = null;

	/** Idempotent; a later surface's boot is a coalesced refresh, never a race. */
	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			this.#loop = createJsonWasmShell<
				CurrencyView,
				CurrencyEvent,
				CurrencyEffect,
				CurrencyShellResult
			>(new DisplayCurrencyCore(), {
				onView: (view) => {
					this.view = view;
				},
				execute: executeCurrencyOperation,
				toFailure: currencyOperationFailure,
				onError: (error) => console.error('[display-currency] core fault:', error)
			});
			this.#loop.start({ type: 'refresh' });
		})();
		return this.#booted;
	}

	/** The person chose a currency. The core persists and re-rates. */
	choose(code: string): void {
		this.#loop?.dispatch({ type: 'user_chose', code });
	}

	refresh(): void {
		this.#loop?.dispatch({ type: 'refresh' });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const currency = new Currency();
