/**
 * The ONE resident `fee_tier_pref` session — WEB (spec 068).
 *
 * App-resident for the same reason `currency.svelte.ts` is: two surfaces read
 * this preference (Settings, where it is chosen, and the send form's folded
 * speed control, which shows THEIR default rather than a hardcoded one), and
 * two readers with their own copies of a preference is how the two surfaces
 * come to disagree about what the default is.
 *
 * What it is NOT: the per-transaction picker. A pick on the send screen is
 * one-shot — it changes what that one send is priced and submitted at and
 * never reaches this machine, so the next send is back at the default. A
 * setting that silently drifts is a setting nobody can trust.
 */

import { FeeTierPrefCore, loadCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeTierPrefEvent } from '$lib/core/generated/FeeTierPrefEvent';
import type { FeeTierPrefShellResult } from '$lib/core/generated/FeeTierPrefShellResult';
import type { FeeTierPrefView } from '$lib/core/generated/FeeTierPrefView';
import {
	executeFeeTierPrefOperation,
	feeTierPrefOperationFailure,
	type FeeTierPrefEffect
} from './fee-tier-executor';

/**
 * The machine's own initial view, mirrored until the core rules: the factory
 * default, uncommitted. `fast` because that is what every shell hard-coded
 * before this preference existed — a surface that renders before the read
 * lands must show today's behaviour, not a guess.
 */
const INITIAL: FeeTierPrefView = {
	tier: 'fast',
	committed: false,
	offered: ['fast', 'standard', 'slow']
};

class FeeTierPreference {
	view = $state<FeeTierPrefView>(INITIAL);

	#loop: EffectLoop<FeeTierPrefEvent> | null = null;
	#booted: Promise<void> | null = null;

	/** Idempotent; a later surface's boot is a coalesced refresh, never a race. */
	boot(): Promise<void> {
		if (this.#booted) return this.#booted;
		this.#booted = (async () => {
			await loadCore();
			this.#loop = createJsonWasmShell<
				FeeTierPrefView,
				FeeTierPrefEvent,
				FeeTierPrefEffect,
				FeeTierPrefShellResult
			>(new FeeTierPrefCore(), {
				onView: (view) => {
					this.view = view;
				},
				execute: executeFeeTierPrefOperation,
				toFailure: feeTierPrefOperationFailure,
				onError: (error) => console.error('[fee-tier-pref] core fault:', error)
			});
			this.#loop.start({ type: 'refresh' });
		})();
		return this.#booted;
	}

	/** The person chose a default in Settings. The core persists and commits. */
	choose(tier: FeeTier): void {
		this.#loop?.dispatch({ type: 'user_chose', tier });
	}

	refresh(): void {
		this.#loop?.dispatch({ type: 'refresh' });
	}
}

/** Browser-only: `boot()` loads wasm — callers guard on mount. */
export const feeTierPreference = new FeeTierPreference();
