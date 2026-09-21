/**
 * The speed control of one fee surface — the send form, or the dApp signing
 * sheet (spec 069). One class, so the two surfaces cannot drift: the design
 * sheet says of the fee card that Send and signing "must not drift", and the
 * speed control is part of that card.
 *
 * Every rule is the `fee_speed` core's ({@link FeeSpeedSession}). What this
 * class owns is the fee SESSIONS — the {@link FeeQuote} in force, handed in by
 * the surface, and the {@link TierPreview} rows beside it — and the reconcile
 * step that keeps them in step with the core (`fee_speed.rs`).
 */
import { untrack } from 'svelte';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';
import { feeTierPreference } from '$lib/settings/core/fee-tier.svelte';
import { resolvedFormatKeys } from '$lib/services/locale-format';
import { FeeSpeedSession, reconcileSpeed } from './fee-speed.svelte';
import { IDLE_FEE_VIEW, type FeeQuote } from './fee-quote.svelte';
import { TierPreview } from './tier-preview.svelte';

export class SpeedControl {
	readonly speed = new FeeSpeedSession();
	readonly tierPreview = new TierPreview();

	readonly #feeQuote: () => FeeQuote;

	/**
	 * @param feeQuote the session in force — the one the fee row renders and
	 *   the submit signs. A getter is accepted so a component can hand over a
	 *   prop without capturing its first value.
	 * @param live whether the surface has an operation behind it at all; with
	 *   none there is nothing to reconcile and nothing to preview.
	 */
	constructor(
		feeQuote: FeeQuote | (() => FeeQuote),
		private readonly live: () => boolean
	) {
		this.#feeQuote = typeof feeQuote === 'function' ? feeQuote : () => feeQuote;
	}

	get feeQuote(): FeeQuote {
		return this.#feeQuote();
	}

	/** The tier this operation runs at, as the core decided it. */
	get tier(): FeeTier {
		return this.speed.view.tier;
	}

	get view(): FeeSpeedView {
		return this.speed.view;
	}

	/**
	 * The fee in force, as the fee row reads it: the session's view with
	 * `pending` folded into `busy` (issue 681). "A measurement is out" has to
	 * be true from the moment one is decided on, not from the dispatch an
	 * account-context read later.
	 */
	get feeInForce(): FeeView {
		const view = this.feeQuote.view ?? IDLE_FEE_VIEW;
		return { ...view, busy: view.busy || this.feeQuote.pending };
	}

	/** The fee-coin options of the session pricing `tier`, for formatting its option. */
	feeOptions(tier: FeeTier): FeeView['options'] {
		if (tier === this.tier) return this.feeInForce.options;
		return this.tierPreview.rows.find((row) => row.tier === tier)?.quote.view.options ?? [];
	}

	/**
	 * Run a step that may move the tier in force, then bring the sessions in
	 * line AT ONCE: promote the preview that priced this operation at the new
	 * tier — a tap, or a free upgrade the core just took — else re-price. At
	 * once, not in an effect of its own: the preview effect re-prices the OTHER
	 * tiers the moment the core's `previews` change, and run first it would
	 * dispose the very session the person tapped, turning "the price you tap
	 * is the price you get" (issue 681) back into a second, possibly
	 * different, answer.
	 */
	step(run: () => void): void {
		run();
		if (this.live()) reconcileSpeed(this.speed.view.tier, this.feeQuote, this.tierPreview);
	}

	/**
	 * Wire the control to the surface. Call ONCE, while the owning component
	 * initialises — the effects belong to it and end with it.
	 *
	 * @param onForm whether the person is still choosing: a free upgrade is
	 *   only decided then, never under somebody reading the last screen.
	 */
	attach(onForm: () => boolean): void {
		// The stored default and the number preset.
		$effect(() => {
			const preferred = feeTierPreference.view.tier;
			const number = resolvedFormatKeys().number;
			untrack(() => this.step(() => this.speed.configure(preferred, number)));
		});
		$effect(() => {
			this.speed.stage(onForm());
		});
		// The OTHER tiers' figures: all of them while the control is open, and
		// — folded — only the free-upgrade partner, when there is one.
		$effect(() => {
			const base = this.feeQuote.lastRequest;
			const tiers = this.speed.view.previews;
			if (tiers.length === 0 || !this.live() || base === null) {
				this.tierPreview.hide();
				return;
			}
			// `generation` read reactively, so a refresh — or anything else
			// that re-prices the fee in force — re-prices these rows with it.
			this.tierPreview.show(base, tiers, this.feeQuote.generation);
		});
		// Tell the core what every session holds, whenever any of it moves.
		// The dispatch writes `speed.view`, which the effects around it read,
		// so it runs untracked.
		$effect(() => {
			void this.feeQuote.view;
			void this.feeQuote.pending;
			void this.feeQuote.lastRequest;
			for (const row of this.tierPreview.rows) {
				void row.quote.view;
				void row.quote.pending;
			}
			untrack(() => this.step(() => this.speed.quotes(this.feeQuote, this.tierPreview)));
		});
		// …and once more whenever the tier or a measurement moves: a tier that
		// changed while a measurement was out (which the core may be waiting
		// on, so nothing supersedes it) is re-priced the moment it lands.
		$effect(() => {
			const tier = this.tier;
			void this.feeQuote.pending;
			untrack(() => {
				if (this.live()) reconcileSpeed(tier, this.feeQuote, this.tierPreview);
			});
		});
	}

	boot(): Promise<void> {
		return this.speed.boot();
	}

	toggle(): void {
		this.speed.toggle();
	}

	/** One-shot: never reaches the stored preference. */
	pick(tier: FeeTier): void {
		this.step(() => this.speed.pick(tier));
	}

	/**
	 * The refresh control: `requote` drops the 15 s fee-signal cache first
	 * (issue 212), so this is a fresh measurement, and the previews follow.
	 */
	refresh(): void {
		this.feeQuote.requote();
	}

	/** The operation ended or a new one began: the one-shot pick and the fold die with it. */
	reset(): void {
		this.speed.reset();
		this.tierPreview.hide();
	}

	dispose(): void {
		this.tierPreview.hide();
		this.speed.dispose();
	}
}
