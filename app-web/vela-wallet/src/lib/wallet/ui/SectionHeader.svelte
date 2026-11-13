<script lang="ts">
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';

	interface Props {
		title: string;
		action?: string;
		/** Plain non-interactive trailing text (spec 018 C1: 联系人 · 8 位). */
		note?: string;
		onaction?: () => void;
	}

	let { title, action, note, onaction }: Props = $props();
</script>

<!-- A div, not a <header>: a list's title row is no banner landmark. -->
<div class="section">
	<h2>{title}</h2>
	{#if note !== undefined}
		<span class="note">{note}</span>
	{/if}
	{#if action !== undefined}
		<button type="button" data-tap onclick={onaction}>
			{action}
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		</button>
	{/if}
</div>

<style>
	.section {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-lg);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	button {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-sm);
		margin-inline-end: calc(var(--space-sm) * -1);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		border-radius: var(--radius-sm);
	}

	button:hover {
		color: var(--color-fg-base);
	}

	.note {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
