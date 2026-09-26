<script lang="ts">
	/**
	 * The only segmented control in the product (design review 2026-07: one
	 * segmented control, no lookalikes). The three-up theme picker uses it, and
	 * the desktop reuses it verbatim in its form rows. (The two-up avatar
	 * picker it also drew was retired in spec 074.)
	 */
	import type { SegmentedModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		model: SegmentedModel;
		onselect?: (id: string) => void;
	}

	let { model, onselect }: Props = $props();

	/**
	 * A label never ends in "…" and never runs out of its segment (founder,
	 * 2026-09-26: 浅色 / 深色 / 跟随系统 cut short in es and it). Each label
	 * keeps ONE line wherever it can:
	 *
	 * 1. side by side with its icon, as drawn, set a little smaller if it is
	 *    only a little too long (to 85% — a shortfall nobody can see);
	 * 2. otherwise every segment puts its icon ABOVE its label — all three
	 *    together, so the control stays one shape — and the label gets the
	 *    segment's whole width, again set a little smaller if need be;
	 * 3. only then may a label of several words take a second line inside
	 *    its segment, and a single word that still does not fit is set
	 *    smaller until it does — never below 60% of the size it asked for.
	 *
	 * Measured, because only the browser knows how wide a word is in the font
	 * it has: again whenever the control's width, the labels, the document's
	 * text size (`--text-scale` on <html>) or the loaded fonts change. The
	 * layouts are classes set on the DOM here rather than state, because each
	 * step reads the layout the previous one produced, synchronously.
	 */
	function fitSegments(group: HTMLElement, labels: string) {
		void labels;
		/** How much of this label's width its box can hold — 1 when all of it. */
		const room = (label: HTMLElement) =>
			label.scrollWidth > label.clientWidth + 0.5 && label.clientWidth > 0
				? label.clientWidth / label.scrollWidth
				: 1;
		const measure = () => {
			const spans = [...group.querySelectorAll<HTMLElement>('.label')];
			group.classList.remove('stacked', 'wrapped');
			for (const span of spans) span.style.removeProperty('--fit');
			const worst = () => Math.min(...spans.map(room));
			if (worst() < 0.85) {
				group.classList.add('stacked');
				if (worst() < 0.85) group.classList.add('wrapped');
			}
			for (const span of spans) {
				const fits = room(span);
				if (fits < 1) span.style.setProperty('--fit', String(Math.max(0.6, fits)));
			}
		};
		const resized = new ResizeObserver(measure);
		resized.observe(group);
		const rescaled = new MutationObserver(measure);
		rescaled.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
		void document.fonts?.ready.then(measure);
		measure();
		return {
			update: measure,
			destroy: () => {
				resized.disconnect();
				rescaled.disconnect();
			}
		};
	}
</script>

<div
	class="segmented"
	role="radiogroup"
	aria-label={model.label}
	use:fitSegments={model.segments.map((segment) => segment.label).join('\u0000')}
>
	{#each model.segments as segment (segment.id)}
		<button
			type="button"
			role="radio"
			aria-checked={segment.id === model.selected}
			class:selected={segment.id === model.selected}
			onclick={() => onselect?.(segment.id)}
		>
			{#if segment.icon !== undefined}
				<Icon icon={UTILITY_ICONS[segment.icon]} size="sm" />
			{/if}
			<span class="label">{segment.label}</span>
		</button>
	{/each}
</div>

<style>
	/* Equal thirds, as wide as the widest label wants them where the column
	   leaves room (the desktop's 280-pixel control column) and the whole row
	   where it does not (the phone) — never a longer word in one language
	   squeezing its neighbours. */
	.segmented {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: minmax(0, 1fr);
		max-width: 100%;
		padding: var(--space-sm);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		/* Dark mode sinks sunken BELOW raised, so the unselected track needs a
		   hairline to stay legible against bg.base (SPEC 暗色注意). */
		border: var(--border-hairline) solid var(--color-border-base);
	}

	button {
		display: flex;
		min-width: 0;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-height: var(--size-control-sm);
		padding: var(--space-xs) var(--space-md);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	/* One line, never elided; `fitSegments` decides how it fits. */
	.label {
		min-width: 0;
		font-size: calc(1em * var(--fit, 1));
		line-height: var(--leading-tight);
		text-align: center;
		white-space: nowrap;
	}

	/* Step 2 of `fitSegments`: the icon above the label, in every segment.
	   `:global` because the class is set by the measurement, not the markup. */
	.segmented:global(.stacked) button {
		flex-direction: column;
		gap: var(--space-xs);
		padding-inline: var(--space-sm);
	}

	.segmented:global(.stacked) .label {
		max-width: 100%;
	}

	/* Step 3: several words may share two balanced lines. A word is never
	   broken — CJK included (`keep-all`: "ライ / ト" is not a wrap, it is a
	   broken word). */
	.segmented:global(.wrapped) .label {
		white-space: normal;
		text-wrap: balance;
		word-break: keep-all;
		overflow-wrap: normal;
	}

	.selected {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
		box-shadow: var(--shadow-sm);
	}
</style>
