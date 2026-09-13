<script lang="ts">
	import SectionHeader from '$lib/wallet/ui/SectionHeader.svelte';
	import Sidebar from '$lib/wallet/ui/Sidebar.svelte';
	import ThirdPanel from '$lib/wallet/ui/ThirdPanel.svelte';
	import SigningPanel from '$lib/signing/SigningPanel.svelte';
	import type { SigningModel } from '$lib/signing/model';
	import type { SidebarModel } from '$lib/wallet/model';
	import ConnectionPanel from './ui/ConnectionPanel.svelte';
	import ContextMenu from './ui/ContextMenu.svelte';
	import DemoPage from './ui/DemoPage.svelte';
	import DesktopToolbar from './ui/DesktopToolbar.svelte';
	import ExploreEmpty from './ui/ExploreEmpty.svelte';
	import SiteRow from './ui/SiteRow.svelte';
	import SiteTile from './ui/SiteTile.svelte';
	import TabStrip from './ui/TabStrip.svelte';
	import type { ExploreDesktopModel } from './model';
	import type { ExploreMessages } from './messages';

	/**
	 * The desktop Explore surface (spec 022, DE1–DE4): the wallet's own sidebar,
	 * a tab strip and toolbar, the page, and the third column that hosts
	 * either the connection panel or a signing request.
	 *
	 * The third column is the desktop's real advantage over the phone: a
	 * signing request sits BESIDE the page that raised it, so the request and
	 * the site making it can be read against each other without dismissing
	 * either.
	 */
	interface Props {
		model: ExploreDesktopModel;
		copy: ExploreMessages;
		sidebar: SidebarModel;
		/** Fixture-driven signing request for the third column (DE4/DCS1–8). */
		signing?: SigningModel;
		onnav?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
	}

	let { model, copy, sidebar, signing, onnav }: Props = $props();

	type Panel = 'none' | 'connection' | 'signing';

	// Overrides over the model, not copies of it: swapping the model (the
	// gallery's state picker) has to land, and a copied $state would freeze.
	let browsingOverride = $state<boolean | undefined>();
	let panelOverride = $state<Panel | undefined>();
	let menuOverride = $state<ExploreDesktopModel['contextMenu'] | null | undefined>();

	const browsing = $derived(browsingOverride ?? (model.state === 'de3' || model.state === 'de4'));
	const panel = $derived(panelOverride ?? model.initialPanel);
	const menu = $derived(
		menuOverride === undefined ? model.contextMenu : (menuOverride ?? undefined)
	);
</script>

<svelte:window
	onkeydown={(event: KeyboardEvent) => {
		if (event.key === 'Escape') panelOverride = 'none';
	}}
/>

<div class="desktop">
	<Sidebar {sidebar} {onnav} />

	<main>
		<TabStrip
			tabs={model.tabStrip.tabs}
			newTabLabel={model.tabStrip.newTabLabel}
			closeLabel={copy.closeTab}
			onselect={() => (browsingOverride = true)}
			onnew={() => (browsingOverride = false)}
			onclose={() => (browsingOverride = false)}
		/>
		<DesktopToolbar
			toolbar={model.toolbar}
			browser={model.browser}
			{browsing}
			secureLabel={copy.secureSite}
			accountLabel={copy.account}
			connectedLabel={copy.connectedTag}
			onaccount={() => (panelOverride = panel === 'connection' ? 'none' : 'connection')}
		/>

		{#if browsing}
			<div class="page">
				<DemoPage
					page={model.browser.page}
					onaction={() => (panelOverride = signing ? 'signing' : panel)}
				/>
			</div>
		{:else}
			<div class="start">
				{#if model.start.empty}
					<ExploreEmpty
						title={model.start.empty.title}
						caption={model.start.empty.caption}
						cta={model.start.empty.cta}
						onbrowse={() => (browsingOverride = true)}
					/>
				{/if}

				{#if model.start.favorites}
					<SectionHeader
						title={model.start.favorites.title}
						action={model.start.favorites.action}
					/>
					<div class="grid">
						{#each model.start.favorites.tiles as tile, i (i)}
							<SiteTile
								{tile}
								onopen={() => (browsingOverride = true)}
								oncontext={(_id, x, y) =>
									(menuOverride = { items: model.contextMenu?.items ?? [], x, y })}
							/>
						{/each}
					</div>
				{/if}

				{#each model.start.groups as group (group.id)}
					<SectionHeader
						title={group.title}
						action={group.action === 'clear' ? copy.clear : group.action === 'menu' ? '⋯' : ''}
					/>
					<div class="rows">
						{#each group.sites as site (site.id + group.id)}
							<SiteRow {site} onopen={() => (browsingOverride = true)} />
						{/each}
					</div>
				{/each}
			</div>
		{/if}
	</main>

	{#if panel === 'signing' && signing}
		<ThirdPanel
			title={signing.panelTitle}
			closeLabel={model.closeLabel}
			onclose={() => (panelOverride = 'none')}
		>
			<SigningPanel model={signing} onconfirm={() => (panelOverride = 'none')} />
		</ThirdPanel>
	{:else if panel === 'connection'}
		<ThirdPanel
			title={copy.connectionTitle}
			closeLabel={model.closeLabel}
			onclose={() => (panelOverride = 'none')}
		>
			<ConnectionPanel
				connection={model.connection}
				ondisconnect={() => (panelOverride = 'none')}
			/>
		</ThirdPanel>
	{/if}

	{#if menu && menu.items.length > 0}
		<ContextMenu items={menu.items} x={menu.x} y={menu.y} onclose={() => (menuOverride = null)} />
	{/if}
</div>

<style>
	.desktop {
		display: flex;
		height: 100%;
		/* Capped and centred past the widest the mocks were drawn for (spec
		   038 T078): on a 4 K display the frame no longer hugs the left edge. */
		width: 100%;
		max-width: var(--layout-frameMax);
		margin-inline: auto;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	main {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	.page {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
	}

	.start {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-4xl);
		max-width: var(--layout-maxContentWidth);
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(8, 1fr);
		gap: var(--space-2xl) var(--space-md);
		padding-block: var(--space-lg);
	}

	.rows {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 var(--space-4xl);
	}
</style>
