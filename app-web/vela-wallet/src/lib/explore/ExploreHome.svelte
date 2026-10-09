<script lang="ts">
	import SectionHeader from '$lib/wallet/ui/SectionHeader.svelte';
	import TabBar from '$lib/wallet/ui/TabBar.svelte';
	import SigningSheet from '$lib/signing/SigningSheet.svelte';
	import type { SigningModel } from '$lib/signing/model';
	import ScanSurface from '$lib/flows/ui/ScanSurface.svelte';
	import type { ScanModel } from '$lib/flows/model';
	import { scanner, scanNotice, type ScanNoticeMessages } from '$lib/flows/core/scanner.svelte';
	import { exploreScanUrl } from './scan';
	import AddressBar from './ui/AddressBar.svelte';
	import ConnectionPanel from './ui/ConnectionPanel.svelte';
	import DemoPage from './ui/DemoPage.svelte';
	import ExploreEmpty from './ui/ExploreEmpty.svelte';
	import GroupManageSheet from './ui/GroupManageSheet.svelte';
	import SearchField from './ui/SearchField.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import SiteMenuSheet from './ui/SiteMenuSheet.svelte';
	import SiteRow from './ui/SiteRow.svelte';
	import SiteTile from './ui/SiteTile.svelte';
	import TabsScreen from './ui/TabsScreen.svelte';
	import type {
		ExploreHomeModel,
		ExploreSheet,
		ExploreView,
		ResumeTabModel,
		SiteModel
	} from './model';
	import type { ExploreMessages } from './messages';

	/**
	 * The phone Explore screen (spec 022): one surface with three views —
	 * the home (start page), a page being browsed, and the tab switcher.
	 *
	 * Spec 099 navigation: the app's four-tab bar stays under the page, so the
	 * wallet is one tap away from any dApp, and the browser draws ONE bar of
	 * its own, at the top (`AddressBar`). Still never two bars stacked at the
	 * bottom — that rule of 022's holds; the old bottom toolbar is gone, its
	 * controls in the top bar and its rarer ones in the site menu.
	 *
	 * 探索 while browsing returns to the home with the tab kept alive, and the
	 * home's resume section brings it back in one tap.
	 */
	interface Props {
		model: ExploreHomeModel;
		copy: ExploreMessages;
		/** A signing request from the page. Fixture-driven for now. */
		signing?: SigningModel;
		/** The wallet's own tab bar. Absent in the gallery, where it is a picture. */
		onselect?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
		/**
		 * The scanner behind the search field's scan button (issue 273) — the
		 * wallet's own, reading only web addresses here. Absent = the button
		 * stays a picture, as everything else in a mock is.
		 */
		scan?: { model: ScanModel; messages: ScanNoticeMessages };
	}

	let { model, copy, signing, onselect, scan }: Props = $props();

	// Pure UI state. Everything a person can DO on this screen lives here; the
	// fixture decides only where the screen STARTS — so each piece is an
	// override over the model rather than a copy of it, and a model swap (the
	// gallery's state picker, a locale change) still lands.
	let viewOverride = $state<ExploreView | undefined>();
	let sheetOverride = $state<ExploreSheet | null | undefined>();
	let signingUp = $state(false);
	/** Where the switcher was opened from — Done goes back there. */
	let tabsFrom = $state<ExploreView | undefined>();

	const view = $derived(viewOverride ?? model.view);
	const sheet = $derived(sheetOverride === undefined ? model.sheet : (sheetOverride ?? undefined));

	function openTabs(): void {
		tabsFrom = view;
		viewOverride = 'tabs';
	}

	/**
	 * The tab bar while Explore is up. 探索 again is the way home from a page
	 * (the tab stays alive); every other tab is the wallet's to answer. In the
	 * gallery the bar stays a picture, as it always was: a live bar warms the
	 * other destinations' code, which a board has no use for.
	 */
	function select(id: 'wallet' | 'contacts' | 'explore' | 'settings'): void {
		if (id === 'explore') viewOverride = 'start';
		else onselect?.(id);
	}

	/**
	 * A resume row as a site row: the tab's own title over its host, the host
	 * cut from its start like the browsing bar's pill (`hostLine`).
	 */
	function resumeRow(tab: ResumeTabModel): SiteModel {
		return { ...tab.site, id: tab.id, name: tab.title, host: tab.host, subtitle: tab.host };
	}

	/** What a pick in the site menu does on a board: the sheet goes either way. */
	function pick(id: string): void {
		sheetOverride = null;
		if (id === 'close') viewOverride = 'start';
	}

	/** An address from the search field, or from a scanned code. */
	function submit(): void {
		viewOverride = 'browsing';
	}

	// --- The scanner (issue 273) ---------------------------------------------
	//
	// The wallet home's scanner (spec 028 T422): the same surface, the same
	// camera, the same refusals. Only what a read MEANS differs — here a code
	// is a web address to open, or it is refused in one line.

	let scanning = $state(false);
	let scanVideo = $state<HTMLVideoElement | null>(null);
	let scanPicker = $state<HTMLInputElement | null>(null);
	/** A code that WAS read and is not a web address. */
	let scanUnusable = $state(false);

	/**
	 * As on the wallet home: a code nobody can use must not end the scan, and
	 * re-arming at once would decode that same code forever.
	 */
	const SCAN_REARM_MS = 2000;
	let scanRearm = 0;

	$effect(() => {
		const video = scanVideo;
		if (!scanning || !video) return;
		scanUnusable = false;
		void scanner.start(video);
		return () => {
			clearTimeout(scanRearm);
			scanner.stop();
		};
	});

	// One code is acted on once: the read is taken off the surface first.
	$effect(() => {
		const found = scanner.result;
		if (found === null) return;
		scanner.clear();
		scanned(found);
	});

	function scanned(value: string): void {
		if (exploreScanUrl(value) !== null) {
			scanning = false;
			submit();
			return;
		}
		scanUnusable = true;
		clearTimeout(scanRearm);
		scanRearm = setTimeout(() => {
			if (scanning && scanVideo) void scanner.start(scanVideo);
		}, SCAN_REARM_MS) as unknown as number;
	}

	function scanTool(id: 'gallery' | 'torch' | 'flip'): void {
		if (id === 'gallery') scanPicker?.click();
		else if (id === 'torch') void scanner.toggleTorch();
		else void scanner.flip();
	}

	async function pickScanImage(event: Event): Promise<void> {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		// Cleared before reading: the same file picked twice fires no change.
		input.value = '';
		if (!file) return;
		scanUnusable = false;
		await scanner.pick(file);
	}

	const scanCopy = $derived(
		scan
			? scanNotice(
					{ status: scanner.status, nothingFound: scanner.nothingFound, unusable: scanUnusable },
					scan.messages
				)
			: undefined
	);
</script>

<!-- What fills the scanner's frame; the file input is mounted so `click()` opens it. -->
{#snippet scanFeed()}
	<video class="scan-video" bind:this={scanVideo} muted playsinline></video>
	<input
		class="scan-picker"
		type="file"
		accept="image/*"
		tabindex="-1"
		aria-hidden="true"
		bind:this={scanPicker}
		onchange={pickScanImage}
	/>
{/snippet}

<div class="explore">
	{#if scanning && scan}
		<ScanSurface
			model={scan.model}
			feed={scanFeed}
			notice={scanCopy}
			ontool={scanTool}
			onclose={() => (scanning = false)}
		/>
	{:else if view === 'tabs'}
		<TabsScreen
			tabs={model.tabs}
			copy={model.tabsScreen}
			ondone={() => (viewOverride = tabsFrom ?? 'browsing')}
			onopen={() => (viewOverride = 'browsing')}
			onnew={() => (viewOverride = 'start')}
			oncloseall={() => (viewOverride = 'start')}
		/>
	{:else if view === 'browsing'}
		<!-- The board has no page history to walk, so ‹ shows where the last
		     step back lands: the Explore home, tab kept. -->
		<AddressBar
			browser={model.browser}
			{copy}
			onback={() => (viewOverride = 'start')}
			onaccount={() => (sheetOverride = model.menus.connection)}
			ontabs={openTabs}
			onmenu={() => (sheetOverride = model.menus.siteMenu)}
		/>
		<div class="page">
			<DemoPage page={model.browser.page} onaction={() => (signingUp = signing !== undefined)} />
		</div>
		<TabBar tabs={model.navLabels} selected="explore" onselect={onselect ? select : undefined} />
	{:else}
		<div class="scroll">
			<!-- No tab count up here any more: the resume section's header says
			     how many tabs are open, in words, and opens the switcher. -->
			<header class="top">
				<h1>{model.title}</h1>
			</header>

			<div class="search">
				<SearchField
					placeholder={model.searchPlaceholder}
					scanLabel={model.scanLabel}
					onscan={scan ? () => (scanning = true) : undefined}
					onsubmit={submit}
				/>
			</div>

			{#if model.resume}
				<section class="resume">
					<SectionHeader
						title={model.resume.title}
						action={model.resume.action}
						onaction={openTabs}
					/>
					<ul>
						{#each model.resume.tabs as tab (tab.id)}
							<li>
								<SiteRow
									site={resumeRow(tab)}
									hostLine
									onopen={() => (viewOverride = 'browsing')}
								/>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if model.empty}
				<ExploreEmpty
					title={model.empty.title}
					caption={model.empty.caption}
					cta={model.empty.cta}
					onbrowse={() => (viewOverride = 'browsing')}
				/>
			{/if}

			{#if model.favorites}
				<SectionHeader
					title={model.favorites.title}
					action={model.favorites.action}
					onaction={() => (sheetOverride = model.menus.groupManage)}
				/>
				<div class="grid">
					{#each model.favorites.tiles as tile, i (i)}
						<SiteTile {tile} onopen={() => (viewOverride = 'browsing')} />
					{/each}
				</div>
			{/if}

			{#each model.groups as group (group.id)}
				<SectionHeader title={group.title} action={group.action === 'clear' ? copy.clear : ''} />
				<ul>
					{#each group.sites as site (site.id + group.id)}
						<li><SiteRow {site} onopen={() => (viewOverride = 'browsing')} /></li>
					{/each}
				</ul>
			{/each}
		</div>

		<TabBar tabs={model.navLabels} selected="explore" onselect={onselect ? select : undefined} />
	{/if}

	{#if sheet}
		<BottomSheet
			title={sheet.kind === 'connection' ? sheet.connection.title : copy.siteMenu}
			hideTitle
			variant="menu"
			onclose={() => (sheetOverride = null)}
		>
			{#if sheet.kind === 'group-manage'}
				<GroupManageSheet
					title={sheet.title}
					rows={sheet.rows}
					closeLabel={copy.close}
					hideLabel={copy.hide}
					showLabel={copy.show}
					onclose={() => (sheetOverride = null)}
				/>
			{:else if sheet.kind === 'site-menu'}
				<SiteMenuSheet
					site={sheet.site}
					statusLine={sheet.statusLine}
					items={sheet.items}
					closeLabel={copy.close}
					onclose={() => (sheetOverride = null)}
					onpick={pick}
				/>
			{:else}
				<ConnectionPanel
					connection={sheet.connection}
					closeLabel={copy.close}
					onclose={() => (sheetOverride = null)}
					ondisconnect={() => (sheetOverride = null)}
				/>
			{/if}
		</BottomSheet>
	{/if}

	{#if signingUp && signing}
		<SigningSheet
			model={signing}
			onclose={() => (signingUp = false)}
			onconfirm={() => (signingUp = false)}
		/>
	{/if}
</div>

<style>
	.explore {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	.scroll {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding-inline: var(--layout-screenPaddingX);
	}

	.page {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
	}

	.top {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-2xl) var(--space-xl);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.search {
		padding-bottom: var(--space-2xl);
	}

	/* Open on the page like every other section (DESIGN-LANGUAGE §1); the
	   space under it is the same section gap the favourites keep. */
	.resume {
		padding-bottom: var(--space-lg);
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		justify-items: center;
		gap: var(--space-2xl) var(--space-md);
		padding-block: var(--space-lg);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.scan-video {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		pointer-events: none;
	}

	/* Mounted, not drawn: `click()` on a detached input opens nothing. */
	.scan-picker {
		position: absolute;
		width: var(--border-hairline);
		height: var(--border-hairline);
		opacity: 0;
		pointer-events: none;
	}
</style>
