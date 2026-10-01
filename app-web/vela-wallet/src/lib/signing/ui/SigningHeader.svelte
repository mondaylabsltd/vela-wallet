<script lang="ts">
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import RemoteLogo from '$lib/wallet/ui/RemoteLogo.svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { SigningModel } from '../model';

	interface Props {
		dapp: SigningModel['dapp'];
		network: SigningModel['network'];
		/**
		 * Spec 079: the sheet's one close — a quiet ✕ at the end of the row.
		 * Absent where there is nothing to close (the gallery, the desktop
		 * third column).
		 */
		onclose?: () => void;
		/** The ✕'s accessible name. */
		closeLabel?: string;
		/** The signature is in flight: the ✕ is drawn, and shut. */
		closeDisabled?: boolean;
	}

	let { dapp, network, onclose, closeLabel, closeDisabled = false }: Props = $props();
</script>

<header class="header">
	{#if dapp.own}
		<!-- The wallet asking itself: its own mark, never a letter on a disc. -->
		<span class="own"><BrandMark size={22} /></span>
	{:else}
		<!-- The site's own icon over its initial: the letter shows until the icon
		     lands, and stays when the site has none. -->
		<span class="site">
			<LetterAvatar letter={dapp.letter} tint={dapp.tint} size={36} />
			<RemoteLogo urls={dapp.iconUrls} />
		</span>
	{/if}
	<span class="who">
		<span class="name">{dapp.name}</span>
		{#if dapp.host !== ''}<span class="host">{dapp.host}</span>{/if}
	</span>
	<span class="network">
		<!-- The chain's logo over a drawn dot, which is what shows until it lands. -->
		<span class="chain">
			<span class="dot" style:background={network.dot}></span>
			<RemoteLogo urls={network.logoUrl === undefined ? undefined : [network.logoUrl]} />
		</span>
		{network.name}
	</span>
	{#if onclose}
		<button
			type="button"
			class="close"
			aria-label={closeLabel}
			disabled={closeDisabled}
			onclick={() => onclose?.()}
		>
			<Icon icon={UTILITY_ICONS.x} size="lg" />
		</button>
	{/if}
</header>

<style>
	.header {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	/*
	  Who is asking is never cut short (spec 089). An ellipsis keeps the START
	  of a host and drops its end — and the end is the registrable domain:
	  "app.uniswap.org.secure-login.example" read as "app.uniswap.org.se…" in
	  the 360 px side panel. The name and the host wrap instead, anywhere, so
	  every character of the origin the browser stated is on screen.
	*/
	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.host {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.network {
		display: inline-flex;
		align-items: center;
		gap: var(--space-md);
		flex-shrink: 0;
		height: var(--size-networkChip);
		padding-inline: var(--space-lg);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.own {
		display: grid;
		flex: none;
		place-items: center;
		/* The letter avatar's own box (CONTROL.sm = 36). */
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
	}

	.site {
		position: relative;
		display: grid;
		flex: none;
	}

	.chain {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		width: var(--space-xl);
		height: var(--space-xl);
	}

	/* Quiet on purpose (the sheet's rule since 022: no big Reject button) —
	   the same icon button as the sheet title row's ✕. */
	.close {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		margin-inline-end: calc(var(--space-md) * -1);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close:hover:not(:disabled) {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.close:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
	}
</style>
