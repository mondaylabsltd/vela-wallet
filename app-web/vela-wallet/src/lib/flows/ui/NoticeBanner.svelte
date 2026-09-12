<script lang="ts">
	/**
	 * The inline explanation banner (spec 021 component 22): SD1b's "these are
	 * greyed out because a multi-token send stays on one network", SD2d's
	 * "every token goes to the same address".
	 *
	 * It exists because both screens do something surprising — grey out rows a
	 * person can see, or accept one address for several tokens — and the
	 * cheapest fix for a surprise is to say why, next to it.
	 */
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { TokenMarkModel } from '../model';

	interface Props {
		text: string;
		mark?: TokenMarkModel;
	}

	let { text, mark }: Props = $props();
</script>

<p class="banner">
	{#if mark !== undefined}
		<TokenIcon
			ticker={mark.ticker}
			badgeColor={mark.badgeColor}
			logoUrls={mark.logoUrls}
			badgeLogoUrl={mark.badgeLogoUrl}
			badgeHidden={mark.badgeHidden}
			size="inline"
		/>
	{/if}
	<span>{text}</span>
</p>

<style>
	.banner {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
		padding: var(--space-md) var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}
</style>
