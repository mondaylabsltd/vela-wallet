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
</script>

<div class="segmented" role="radiogroup" aria-label={model.label}>
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
			<span>{segment.label}</span>
		</button>
	{/each}
</div>

<style>
	.segmented {
		display: flex;
		padding: var(--space-sm);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		/* Dark mode sinks sunken BELOW raised, so the unselected track needs a
		   hairline to stay legible against bg.base (SPEC 暗色注意). */
		border: var(--border-hairline) solid var(--color-border-base);
	}

	button {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-height: var(--size-control-sm);
		padding-inline: var(--space-md);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		white-space: nowrap;
	}

	.selected {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
		box-shadow: var(--shadow-sm);
	}
</style>
