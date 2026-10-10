<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { loadCore } from '$lib/core/client';
	import { toLocale } from '$lib/i18n/locales';
	import WalletDesktop from '$lib/wallet/WalletDesktop.svelte';
	import WalletHome from '$lib/wallet/WalletHome.svelte';
	import ContactsDesktop from '$lib/contacts/ContactsDesktop.svelte';
	import ContactsHome from '$lib/contacts/ContactsHome.svelte';
	import FlowsMobile from '$lib/flows/FlowsMobile.svelte';
	import FlowsPanel from '$lib/flows/FlowsPanel.svelte';
	import ScanSurface from '$lib/flows/ui/ScanSurface.svelte';
	import ExploreDesktop from '$lib/explore/ExploreDesktop.svelte';
	import ExploreHome from '$lib/explore/ExploreHome.svelte';
	import SigningSheet from '$lib/signing/SigningSheet.svelte';
	import SettingsDesktop from '$lib/settings/SettingsDesktop.svelte';
	import SettingsHome from '$lib/settings/SettingsHome.svelte';
	import { withheldBoard } from '$lib/wallet/board-withheld';
	import Controls from '../Controls.svelte';

	let { data } = $props();

	const locale = $derived(toLocale(page.params.locale ?? '') ?? 'en');
	const stateId = $derived(page.params.state ?? '');
	/** H1s reviews the full scroll content, so its frame grows with content. */
	const expanded = $derived(stateId === 'h1s');
	/** dc2n is pinned to a 1024 stage so the <1120 overlay mode is visible. */
	const narrowStage = $derived(stateId === 'dc2n');
	/** r4 is a render product, not a screen — it gets no phone frame. */
	const bare = $derived(stateId === 'r4');

	/**
	 * The board's twin with the display currency on its way (`?fiat=withheld`,
	 * or the control beside the theme's): the same drawn state with every fiat
	 * figure withheld as the live surfaces withhold it. See `board-withheld.ts`.
	 */
	let fiatWithheld = $state(false);
	const shown = $derived(fiatWithheld ? withheldBoard(data) : data);

	// The boards are fixtures, but their fields are the product's: the send
	// amount cleans every keystroke through the core (spec 073), so a
	// reviewer typing into it needs the core loaded, as every live route has.
	onMount(() => {
		void loadCore();
		// Read here, not from the page's URL state: these pages are prerendered,
		// and a prerendered page has no query.
		fiatWithheld = new URLSearchParams(window.location.search).get('fiat') === 'withheld';
	});
</script>

<svelte:head>
	<title>Vela Wallet · {stateId.toUpperCase()}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<Controls {locale} {stateId} {fiatWithheld} onfiat={() => (fiatWithheld = !fiatWithheld)} />

{#if shown.kind === 'mobile'}
	<div class="stage">
		<div class="frame" class:expanded>
			<WalletHome model={shown.model} />
		</div>
	</div>
{:else if shown.kind === 'contacts-mobile'}
	<div class="stage">
		<div class="frame">
			<ContactsHome model={shown.model} />
		</div>
	</div>
{:else if shown.kind === 'settings-mobile'}
	<div class="stage">
		<div class="frame">
			<SettingsHome model={shown.model} />
		</div>
	</div>
{:else if shown.kind === 'settings-desktop'}
	<div class="desktop-stage">
		<SettingsDesktop model={shown.model} sidebar={shown.sidebar} />
	</div>
{:else if shown.kind === 'flow-mobile'}
	<div class="stage">
		{#if bare}
			<!-- Sized to the card itself: the flows host takes its width from
			     its parent, and a centring stage gives it none. -->
			<div class="bare"><FlowsMobile model={shown.model} /></div>
		{:else}
			<div class="frame"><FlowsMobile model={shown.model} /></div>
		{/if}
	</div>
{:else if shown.kind === 'flow-desktop'}
	<!-- The panel is only ever seen beside the wallet it opened from, so the
	     stage draws it where the real window does: as the frame's third
	     column, through the same `column` slot the wallet route fills. -->
	{#snippet flowColumn()}
		<FlowsPanel model={shown.model} />
	{/snippet}
	<div class="desktop-stage flow">
		<WalletDesktop model={shown.wallet} column={stateId !== 'ds1' ? flowColumn : undefined} />
	</div>
	{#if stateId === 'ds1'}
		<div class="scan-scrim" role="presentation">
			<div class="scan-modal"><ScanSurface model={shown.scan} variant="modal" /></div>
		</div>
	{/if}
{:else if shown.kind === 'explore-mobile'}
	<div class="stage">
		<div class="frame">
			<ExploreHome
				model={shown.model}
				copy={shown.copy}
				signing={shown.signing}
				scan={shown.scan}
			/>
		</div>
	</div>
{:else if shown.kind === 'signing'}
	<!-- A CS state IS the sheet over the page that asked for it, so the mock is
	     reproduced whole rather than as a floating panel. -->
	<div class="stage">
		<div class="frame">
			{#if shown.settings}
				<SettingsHome model={shown.settings} />
			{:else}
				<ExploreHome model={shown.model} copy={shown.copy} />
			{/if}
			<SigningSheet model={shown.signing} />
		</div>
	</div>
{:else if shown.kind === 'explore-desktop'}
	<div class="desktop-stage">
		<ExploreDesktop
			model={shown.model}
			copy={shown.copy}
			sidebar={shown.sidebar}
			signing={shown.signing}
		/>
	</div>
{:else if shown.kind === 'contacts-desktop'}
	<div class="desktop-stage" class:narrow={narrowStage}>
		<ContactsDesktop model={shown.model} />
	</div>
{:else}
	<div class="desktop-stage">
		<WalletDesktop model={shown.model} />
	</div>
{/if}

<style>
	.stage {
		min-height: 100dvh;
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding-block: var(--space-3xl);
		background: var(--color-bg-sunken);
	}

	.bare {
		width: var(--layout-shareCardW);
	}

	.frame {
		position: relative;
		width: var(--layout-frameW);
		height: var(--layout-frameH);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-2xl);
		overflow: hidden;
	}

	.frame.expanded {
		height: auto;
	}

	.frame.expanded :global(.scroll) {
		overflow-y: visible;
	}

	.desktop-stage {
		height: 100dvh;
		min-width: var(--breakpoint-desktop);
	}

	.desktop-stage.flow {
		overflow: hidden;
	}

	.scan-scrim {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-fixed-backdrop);
	}

	.scan-modal {
		width: min(90vw, calc(var(--size-qrCard) + var(--space-5xl) * 2));
		border-radius: var(--radius-2xl);
		background: var(--color-bg-base);
		box-shadow: var(--shadow-lg);
		overflow: hidden;
	}

	/* 800 + 216 + 8 = 1024: the narrow stage that shows the overlay column. */
	.desktop-stage.narrow {
		min-width: 0;
		width: calc(var(--layout-maxContentWidth) + var(--layout-contactsRailW) + var(--space-md));
		border-inline-end: var(--border-hairline) solid var(--color-border-strong);
	}
</style>
