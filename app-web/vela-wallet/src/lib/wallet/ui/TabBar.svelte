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

<!--
	Icons only (078 round 2, founder): 钱包 / 通讯录 / 设置 fit, but
	"Configuración", "Einstellungen" and "Impostazioni" were cut to "Configur…"
	under a 97-pixel tab. The glyph carries the tab now, larger, and the word
	is still the tab's NAME — the accessible one, with the selected state — so
	a screen reader still says "Settings, current page".
-->
<nav class="tabbar">
	{#each items as item (item.id)}
		<button
			type="button"
			class:selected={item.selected}
			aria-label={item.label}
			aria-current={item.selected ? 'page' : undefined}
			title={item.label}
			onpointerdown={() => {
				if (onselect && !item.selected) preloadDestination(item.id);
			}}
			onclick={() => onselect?.(item.id)}
		>
			<Icon icon={navIcon(item.id, item.selected)} size="tab" />
		</button>
	{/each}
</nav>

<style>
	/* 56 without the safe area; an installed app on a phone with a home
	   indicator gets that inset added under the glyphs, never taken from them. */
	.tabbar {
		display: flex;
		align-items: stretch;
		box-sizing: content-box;
		height: var(--layout-tabBarHeight);
		padding-bottom: env(safe-area-inset-bottom, 0);
		background: var(--color-bg-base);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	/* Equal, full-height targets: the whole quarter (or third) of the bar is
	   the tab, as it was, and the glyph sits at its centre. */
	button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.selected {
		color: var(--color-accent-base);
	}
</style>
