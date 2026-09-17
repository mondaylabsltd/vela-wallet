<script lang="ts">
	import { page } from '$app/state';
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
	import Controls from '../Controls.svelte';

	let { data } = $props();

	const locale = $derived(toLocale(page.params.locale ?? '') ?? 'en');
	const state = $derived(page.params.state ?? '');
	/** H1s reviews the full scroll content, so its frame grows with content. */
	const expanded = $derived(state === 'h1s');
	/** dc2n is pinned to a 1024 stage so the <1120 overlay mode is visible. */
	const narrowStage = $derived(state === 'dc2n');
	/** r4 is a render product, not a screen — it gets no phone frame. */
	const bare = $derived(state === 'r4');
</script>

<svelte:head>
	<title>Vela Wallet · {state.toUpperCase()}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<Controls {locale} stateId={state} />

{#if data.kind === 'mobile'}
	<div class="stage">
		<div class="frame" class:expanded>
			<WalletHome model={data.model} />
		</div>
	</div>
{:else if data.kind === 'contacts-mobile'}
	<div class="stage">
		<div class="frame">
			<ContactsHome model={data.model} />
		</div>
	</div>
{:else if data.kind === 'settings-mobile'}
	<div class="stage">
		<div class="frame">
			<SettingsHome model={data.model} />
		</div>
	</div>
{:else if data.kind === 'settings-desktop'}
	<div class="desktop-stage">
		<SettingsDesktop model={data.model} sidebar={data.sidebar} />
	</div>
{:else if data.kind === 'flow-mobile'}
	<div class="stage">
		{#if bare}
			<FlowsMobile model={data.model} />
		{:else}
			<div class="frame"><FlowsMobile model={data.model} /></div>
		{/if}
	</div>
{:else if data.kind === 'flow-desktop'}
	<!-- The panel is only ever seen beside the wallet it opened from, so the
	     stage draws it where the real window does: as the frame's third
	     column, through the same `column` slot the wallet route fills. -->
	{#snippet flowColumn()}
		<FlowsPanel model={data.model} />
	{/snippet}
	<div class="desktop-stage flow">
		<WalletDesktop model={data.wallet} column={state !== 'ds1' ? flowColumn : undefined} />
	</div>
	{#if state === 'ds1'}
		<div class="scan-scrim" role="presentation">
			<div class="scan-modal"><ScanSurface model={data.scan} variant="modal" /></div>
		</div>
	{/if}
{:else if data.kind === 'explore-mobile'}
	<div class="stage">
		<div class="frame">
			<ExploreHome model={data.model} copy={data.copy} signing={data.signing} />
		</div>
	</div>
{:else if data.kind === 'signing'}
	<!-- A CS state IS the sheet over the page that asked for it, so the mock is
	     reproduced whole rather than as a floating panel. -->
	<div class="stage">
		<div class="frame">
			<ExploreHome model={data.model} copy={data.copy} />
			<SigningSheet model={data.signing} />
		</div>
	</div>
{:else if data.kind === 'explore-desktop'}
	<div class="desktop-stage">
		<ExploreDesktop
			model={data.model}
			copy={data.copy}
			sidebar={data.sidebar}
			signing={data.signing}
		/>
	</div>
{:else if data.kind === 'contacts-desktop'}
	<div class="desktop-stage" class:narrow={narrowStage}>
		<ContactsDesktop model={data.model} />
	</div>
{:else}
	<div class="desktop-stage">
		<WalletDesktop model={data.model} />
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
