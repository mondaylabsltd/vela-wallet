/**
 * The speed control's session — WEB (spec 069).
 *
 * Every rule the control keeps lives in the `fee_speed` core: which tier is in
 * force, when a slower default goes Fast because Fast costs no more (issue
 * 686 A), when a network has one speed (686 B), each tier's gas bid as text
 * (684/685), and which other tiers must be kept priced. The same machine runs
 * on desktop, Android and iOS, which is why none of it is TypeScript any more.
 *
 * What this shell keeps is the fee SESSIONS — the `FeeQuote` in force and the
 * `TierPreview` rows beside it — and the one reconcile rule every shell
 * follows ({@link reconcileSpeed}). The core asks for no operations: it is
 * told what the sessions hold ({@link FeeSpeedSession.quotes}) and answers
 * with a view the sessions are reconciled against.
 */
import { FeeSpeedCore, loadCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { FeeSpeedEvent } from '$lib/core/generated/FeeSpeedEvent';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { NumberPreset } from '$lib/core/generated/NumberPreset';
import type { TierPreviewQuote } from '$lib/core/generated/TierPreviewQuote';
import type { FeeQuote } from './fee-quote.svelte';
import type { TierPreview } from './tier-preview.svelte';

/**
 * The machine's own initial view, mirrored until the core has loaded: the
 * factory `fast`, folded, nothing priced beside it — exactly what every send
 * did before there was a choice, so a surface drawn before the wasm lands
 * shows today's behaviour rather than a guess.
 */
export const IDLE_SPEED_VIEW: FeeSpeedView = {
	tier: 'fast',
	preferred: 'fast',
	previews: [],
	open: false,
	picked: false,
	free: false,
	free_note: false,
	single: false,
	gas_price_line: true,
	options: (['fast', 'standard', 'slow'] as const).map((tier) => ({
		tier,
		selected: tier === 'fast',
		fee: null,
		measuring: true,
		gas_price: null
	}))
};

type FeeSpeedEffect = { id: number; operation: never };

export class FeeSpeedSession {
	view = $state.raw<FeeSpeedView>(IDLE_SPEED_VIEW);

	#loop: EffectLoop<FeeSpeedEvent> | null = null;
	#booting: Promise<void> | null = null;
	/**
	 * Events dispatched before the wasm has loaded, replayed in order the
	 * moment it has. Dropping them would lose a `configure` — and with it the
	 * person's stored default — to a race with the module download.
	 */
	#early: FeeSpeedEvent[] = [];

	/** Idempotent. Surfaces call it on mount; dispatches before it are kept. */
	boot(): Promise<void> {
		if (this.#booting) return this.#booting;
		this.#booting = (async () => {
			await loadCore();
			const loop = createJsonWasmShell<FeeSpeedView, FeeSpeedEvent, FeeSpeedEffect, never>(
				new FeeSpeedCore(),
				{
					onView: (view) => {
						this.view = view;
					},
					// The machine asks for nothing (`FeeSpeedOperation` is empty).
					execute: async (effect) => {
						throw new Error(`fee_speed asked for an operation: ${JSON.stringify(effect)}`);
					},
					toFailure: (effect) => {
						throw new Error(`fee_speed asked for an operation: ${JSON.stringify(effect)}`);
					},
					onError: (error) => console.error('[fee-speed] core fault:', error)
				}
			);
			const [first, ...rest] = this.#early.length > 0 ? this.#early : [{ type: 'reset' } as const];
			this.#early = [];
			loop.start(first);
			for (const event of rest) loop.dispatch(event);
			this.#loop = loop;
		})();
		return this.#booting;
	}

	#dispatch(event: FeeSpeedEvent): void {
		if (this.#loop) this.#loop.dispatch(event);
		else this.#early.push(event);
	}

	/** The stored default and the number preset — repeat freely. */
	configure(preferred: FeeTier, number: NumberPreset): void {
		this.#dispatch({ type: 'configure', preferred, number });
	}

	/** A send starts or ends: a new send starts at the stored default. */
	reset(): void {
		this.#dispatch({ type: 'reset' });
	}

	stage(onForm: boolean): void {
		this.#dispatch({ type: 'stage_changed', on_form: onForm });
	}

	toggle(): void {
		this.#dispatch({ type: 'toggle' });
	}

	/** One-shot: never reaches the stored preference. */
	pick(tier: FeeTier): void {
		this.#dispatch({ type: 'pick', tier });
	}

	/**
	 * What every session holds, whole. `pending` is folded into `busy` on
	 * both sides: the shell's account-context read before a dispatch is as
	 * much "a measurement is out" as the core's own run (issue 681).
	 */
	quotes(feeQuote: FeeQuote, tierPreview: TierPreview): void {
		const previews: TierPreviewQuote[] = tierPreview.rows.map((row) => ({
			tier: row.tier,
			busy: row.quote.pending || row.quote.view.busy,
			fee: row.quote.view.fee
		}));
		this.#dispatch({
			type: 'quotes_changed',
			chain_id: feeQuote.lastRequest?.chainId ?? null,
			in_force: {
				busy: feeQuote.pending || feeQuote.view.busy,
				fee: feeQuote.view.fee
			},
			previews
		});
	}

	dispose(): void {
		this.#loop?.dispose();
		this.#loop = null;
		this.#booting = null;
		this.#early = [];
		this.view = IDLE_SPEED_VIEW;
	}
}

/**
 * The shell's half of the speed control, rule 1 of the reconcile contract
 * (`fee_speed.rs`): make the session in force price the tier in force.
 *
 * Nothing to do while nothing is priced — the first quote carries the tier
 * itself — or while the session in force already asks at that tier.
 * Otherwise PROMOTE the preview that priced this very operation at that tier
 * (issue 681: the price tapped is the price paid, and a free upgrade signs
 * the quote it was judged on), and only when there is none to promote ask
 * again — never over a measurement that is out, because the `send` core may
 * be waiting on it and would hear its own question refused.
 *
 * Returns what it did, for tests: `'promoted'`, `'requoted'`, or `null`.
 */
export function reconcileSpeed(
	tier: FeeTier,
	feeQuote: FeeQuote,
	tierPreview: TierPreview
): 'promoted' | 'requoted' | null {
	const base = feeQuote.lastRequest;
	if (base === null || (base.tier ?? 'fast') === tier) return null;
	if (tierPreview.promote(feeQuote, tier)) return 'promoted';
	if (feeQuote.pending) return null;
	void feeQuote.requestQuote({ ...base, tier });
	return 'requoted';
}
