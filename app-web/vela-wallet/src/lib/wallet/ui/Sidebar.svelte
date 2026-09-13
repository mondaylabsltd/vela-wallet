<script lang="ts">
	import type { SidebarModel } from '../model';
	import { navIcon } from '../icons';
	import ChainFilterList from './ChainFilterList.svelte';
	import Icon from './Icon.svelte';
	import WalletHeader from './WalletHeader.svelte';

	interface Props {
		sidebar: SidebarModel;
		onnav?: (id: SidebarModel['nav'][number]['id']) => void;
		/** The header's name button: the account switcher. Absent in the gallery. */
		onaccounts?: () => void;
		/** A network row was chosen. Absent in the gallery, where the list is a picture. */
		onchainselect?: (row: SidebarModel['networks'][number]) => void;
	}

	let { sidebar, onnav, onaccounts, onchainselect }: Props = $props();
</script>

<!-- No command bar. The drawn "search or run ⌘K" field ran nothing, and a
     field that promises a command palette it does not have is a lie in the
     corner of every screen (founder call, 2026-09-05). -->
<aside class="sidebar">
	<div class="top">
		<WalletHeader header={sidebar.header} onclick={onaccounts} />
	</div>

	<nav>
		{#each sidebar.nav as item (item.id)}
			<button
				type="button"
				class="nav-item"
				class:selected={item.selected}
				aria-current={item.selected ? 'page' : undefined}
				onclick={() => onnav?.(item.id)}
			>
				<Icon icon={navIcon(item.id, item.selected)} size="lg" />
				<span>{item.label}</span>
			</button>
		{/each}
	</nav>

	<!-- The network list is the wallet's filter (spec 028 Phase 9, RULING 2):
	     the routes that have nothing to filter pass none, and the rail ends
	     at the nav. -->
	{#if sidebar.networks.length > 0}
		<hr />

		<p class="networks-title">{sidebar.networksTitle}</p>
		<div class="networks">
			<ChainFilterList rows={sidebar.networks} onselect={onchainselect} />
		</div>
	{/if}
</aside>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		width: calc(var(--space-4xl) * 7 + var(--space-xl));
		flex-shrink: 0;
		height: 100%;
		padding: var(--space-xl);
		background: var(--color-bg-sunken);
		border-inline-end: var(--border-hairline) solid var(--color-border-base);
	}

	.top {
		padding-block: var(--space-lg) var(--space-2xl);
	}

	nav {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		text-align: start;
	}

	.nav-item:hover {
		color: var(--color-fg-base);
	}

	.nav-item.selected {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}

	hr {
		width: 100%;
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin-block: var(--space-xl);
	}

	.networks-title {
		margin: 0 0 var(--space-md);
		padding-inline: var(--space-lg);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
	}

	.networks {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding-inline: var(--space-lg);
	}

	.networks :global(button) {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}
</style>
