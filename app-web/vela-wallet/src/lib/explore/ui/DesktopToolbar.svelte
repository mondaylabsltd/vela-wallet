<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { BrowserModel, ExploreDesktopModel } from '../model';
	import SiteMark from './SiteMark.svelte';

	/**
	 * The desktop browser toolbar (DE1–DE4). On the start page the address
	 * field is the search box; while browsing it collapses to the domain with
	 * its mark — one control, two states, never two controls. The mark is the
	 * page's own (`browser.mark`, the core's `OriginLock`): a padlock for
	 * https and for nothing else. It was a padlock for every page.
	 */
	interface Props {
		toolbar: ExploreDesktopModel['toolbar'];
		browser: BrowserModel;
		browsing: boolean;
		accountLabel: string;
		connectedLabel: string;
		onback?: () => void;
		onforward?: () => void;
		onreload?: () => void;
		onbookmark?: () => void;
		onmenu?: () => void;
		onaccount?: () => void;
	}

	let {
		toolbar,
		browser,
		browsing,
		accountLabel,
		connectedLabel,
		onback,
		onforward,
		onreload,
		onbookmark,
		onmenu,
		onaccount
	}: Props = $props();
</script>

<div class="toolbar">
	<button
		type="button"
		class="icon"
		aria-label={toolbar.back}
		disabled={!browser.canBack}
		onclick={onback}
	>
		<Icon icon={UTILITY_ICONS['arrow-left']} size="lg" />
	</button>
	<button
		type="button"
		class="icon"
		aria-label={toolbar.forward}
		disabled={!browser.canForward}
		onclick={onforward}
	>
		<Icon icon={UTILITY_ICONS['arrow-right']} size="lg" />
	</button>
	<button type="button" class="icon" aria-label={toolbar.reload} onclick={onreload}>
		<Icon icon={UTILITY_ICONS['refresh-cw']} size="lg" />
	</button>

	<div class="address">
		{#if browsing}
			<SiteMark mark={browser.mark} named />
			<span class="host">{browser.host}</span>
		{:else}
			<Icon icon={UTILITY_ICONS.search} size="sm" />
			<input type="text" placeholder={toolbar.searchPlaceholder} autocomplete="off" />
		{/if}
	</div>

	<button type="button" class="icon" aria-label={toolbar.bookmark} onclick={onbookmark}>
		<Icon icon={UTILITY_ICONS[browser.bookmarked ? 'star-filled' : 'star']} size="lg" />
	</button>
	<button type="button" class="icon" aria-label={toolbar.menu} onclick={onmenu}>
		<Icon icon={UTILITY_ICONS.ellipsis} size="lg" />
	</button>

	<button type="button" class="account" aria-label={accountLabel} onclick={onaccount}>
		<span class="identicon"
			><!-- eslint-disable-next-line svelte/no-at-html-tags -- trusted vela-core output, no user content -->
			{@html browser.account.identiconSvg}</span
		>
		<span class="name">{browser.account.name}</span>
		{#if browser.connected}
			<span class="dot" aria-label={connectedLabel}></span>
		{/if}
	</button>
</div>

<style>
	.toolbar {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		height: var(--size-desktopToolbar);
		padding-inline: var(--space-2xl);
		background: var(--color-bg-base);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-desktopControl);
		height: var(--size-desktopControl);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.icon:hover:not(:disabled) {
		background: var(--color-bg-sunken);
	}

	.icon:disabled {
		color: var(--color-fg-subtle);
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.address {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		flex: 1;
		min-width: 0;
		height: var(--size-desktopControl);
		margin-inline: var(--space-lg);
		padding-inline: var(--space-xl);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
	}

	.host {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
		text-align: center;
	}

	input::placeholder {
		color: var(--color-fg-subtle);
	}

	.account {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		height: var(--size-desktopControl);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.identicon {
		display: flex;
		width: var(--icon-sm);
		height: var(--icon-sm);
	}

	.identicon :global(svg) {
		width: 100%;
		height: 100%;
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-success-base);
	}
</style>
