<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';

	interface Props {
		label: string;
		onclick?: () => void;
	}

	let { label, onclick }: Props = $props();
</script>

<button type="button" class="ghost" {onclick}>
	<span class="circle" aria-hidden="true"><Icon icon={UTILITY_ICONS.plus} size="md" /></span>
	<span class="label">{label}</span>
</button>

<style>
	.ghost {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.circle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border: var(--border-hairline) dashed var(--color-border-strong);
		border-radius: var(--radius-full);
		color: var(--color-fg-subtle);
		flex-shrink: 0;
	}

	.label {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.ghost:hover .label,
	.ghost:hover .circle {
		color: var(--color-fg-muted);
	}
</style>
