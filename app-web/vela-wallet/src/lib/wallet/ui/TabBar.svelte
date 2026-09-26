<script lang="ts">
	import type { TabsModel } from '../model';
	import { navIcon, type NavIconId } from '../icons';
	import Icon from './Icon.svelte';
	import { preloadDestination, warmDestinations } from '../tab-preload';

	interface Props {
		tabs: TabsModel;
		selected?: NavIconId;
		/**
		 * Which destinations this client actually has (spec 022 founder call).
		 * The mocks — and every gallery board — draw all four; the WEB app
		 * drops 探索, because a page inside a browser cannot host a browser,
		 * and a tab that opens nothing is worse than a tab that is not there.
		 */
		destinations?: readonly NavIconId[];
		onselect?: (id: NavIconId) => void;
	}

	let {
		tabs,
		selected = 'wallet',
		destinations = ['wallet', 'contacts', 'explore', 'settings'],
		onselect
	}: Props = $props();

	// A live bar only — the gallery draws this with nowhere to go.
	$effect(() => (onselect ? warmDestinations(selected) : undefined));

	const items = $derived(
		destinations.map((id) => ({
			id,
			label: tabs[id],
			selected: id === selected
		}))
	);
</script>

<nav class="tabbar">
	{#each items as item (item.id)}
		<button
			type="button"
			class:selected={item.selected}
			aria-current={item.selected ? 'page' : undefined}
			onpointerdown={() => {
				if (onselect && !item.selected) preloadDestination(item.id);
			}}
			onclick={() => onselect?.(item.id)}
		>
			<Icon icon={navIcon(item.id, item.selected)} size="xl" />
			<span>{item.label}</span>
		</button>
	{/each}
</nav>

<style>
	.tabbar {
		display: flex;
		align-items: stretch;
		height: var(--layout-dockBarHeight);
		background: var(--color-bg-base);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	button {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-sm);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.selected {
		color: var(--color-accent-base);
	}
</style>
