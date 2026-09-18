<script lang="ts">
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import RemoteLogo from '$lib/wallet/ui/RemoteLogo.svelte';
	import type { SigningModel } from '../model';

	interface Props {
		dapp: SigningModel['dapp'];
		network: SigningModel['network'];
	}

	let { dapp, network }: Props = $props();
</script>

<header class="header">
	{#if dapp.own}
		<!-- The wallet asking itself: its own mark, never a letter on a disc. -->
		<span class="own" style:width="36px" style:height="36px"><BrandMark size={22} /></span>
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

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.host {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		/* The letter avatar's own box (36), set inline as that component does. */
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

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
	}
</style>
