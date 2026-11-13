<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';

	/**
	 * Mobile page chrome (vocabulary #18): large title + trailing icon button on
	 * the list screen; back chevron + trailing pencil / ⋯ on the detail screens.
	 */
	interface Props {
		title?: string;
		back?: { label: string; onclick?: () => void };
		/** `pressed`: a toggle (the contact's star), said as one (`aria-pressed`). */
		trailing?: { icon: UtilityIconId; label: string; pressed?: boolean; onclick?: () => void }[];
	}

	let { title, back, trailing = [] }: Props = $props();
</script>

<header class="page-header">
	{#if back !== undefined}
		<button type="button" class="icon-button" aria-label={back.label} onclick={back.onclick}>
			<Icon icon={UTILITY_ICONS['chevron-left']} size="lg" />
		</button>
	{/if}
	{#if title !== undefined}
		<h1>{title}</h1>
	{/if}
	<span class="spacer"></span>
	<!-- By position: a toggle's name changes with its state, and keyed by it
	     the button would be redrawn — and lose the focus — on every press. -->
	{#each trailing as item, i (i)}
		<button
			type="button"
			class="icon-button"
			aria-label={item.label}
			aria-pressed={item.pressed}
			onclick={item.onclick}
		>
			<Icon icon={UTILITY_ICONS[item.icon]} size="lg" />
		</button>
	{/each}
</header>

<style>
	.page-header {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-xl) var(--space-lg);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.spacer {
		flex: 1;
	}

	.icon-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.icon-button:hover {
		background: var(--color-bg-raised);
	}
</style>
