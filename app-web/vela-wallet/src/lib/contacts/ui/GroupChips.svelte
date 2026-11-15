<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';

	interface Props {
		chips: string[];
		/** Trailing "+ 分组" add chip. */
		addLabel: string;
		onadd?: () => void;
	}

	let { chips, addLabel, onadd }: Props = $props();
</script>

<div class="chips">
	{#each chips as chip (chip)}
		<span class="chip">{chip}</span>
	{/each}
	<button type="button" class="chip add" onclick={onadd}>
		<Icon icon={UTILITY_ICONS.plus} size="xs" />
		<span>{addLabel}</span>
	</button>
</div>

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-md);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		height: var(--icon-xl);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.chip.add {
		background: none;
		cursor: pointer;
		color: var(--color-fg-subtle);
	}

	.chip.add:hover {
		color: var(--color-fg-base);
	}
</style>
