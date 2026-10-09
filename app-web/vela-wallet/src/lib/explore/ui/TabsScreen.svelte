<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import TabCard from './TabCard.svelte';
	import type { ExploreHomeModel } from '../model';

	interface Props {
		tabs: ExploreHomeModel['tabs'];
		copy: ExploreHomeModel['tabsScreen'];
		ondone?: () => void;
		onopen?: (id: string) => void;
		onclose?: (id: string) => void;
		onnew?: () => void;
		oncloseall?: () => void;
	}

	let { tabs, copy, ondone, onopen, onclose, onnew, oncloseall }: Props = $props();
</script>

<div class="tabs">
	<header>
		<h1>{copy.title}</h1>
		<button type="button" class="done" onclick={ondone}>{copy.done}</button>
	</header>

	<div class="grid">
		{#each tabs as tab (tab.id)}
			<TabCard {tab} closeLabel={copy.close} {onopen} {onclose} />
		{/each}
		<button type="button" class="new" onclick={onnew}>
			<Icon icon={UTILITY_ICONS.plus} size="lg" />
			<span>{copy.newTab}</span>
		</button>
	</div>

	<button type="button" class="close-all" onclick={oncloseall}>{copy.closeAll}</button>
</div>

<style>
	.tabs {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding-inline: var(--layout-screenPaddingX);
		overflow-y: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-2xl);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.done {
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-xl);
	}

	.new {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		aspect-ratio: 3 / 4;
		border: none;
		border-radius: var(--radius-xl);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close-all {
		align-self: center;
		padding: var(--space-2xl);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
	}
</style>
