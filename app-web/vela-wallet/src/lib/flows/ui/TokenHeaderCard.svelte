<script lang="ts">
	/**
	 * The send form's token card (spec 021 component 16): which token is being
	 * sent, off which chain, out of how much — and the Max that fills the
	 * amount with all of it.
	 *
	 * It is a card and not a header because on SD2 it is also the thing you tap
	 * to change your mind about the token.
	 */
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { SendTokenCardModel } from '../model';

	interface Props {
		token: SendTokenCardModel;
		onmax?: () => void;
	}

	let { token, onmax }: Props = $props();
</script>

<div class="card">
	<TokenIcon
		ticker={token.mark.ticker}
		badgeColor={token.mark.badgeColor}
		logoUrls={token.mark.logoUrls}
		badgeLogoUrl={token.mark.badgeLogoUrl}
		badgeHidden={token.mark.badgeHidden}
	/>
	<span class="text">
		<span class="symbol">{token.symbol}</span>
		<span class="detail">{token.detail}</span>
	</span>
	{#if token.max !== undefined}
		<button type="button" onclick={onmax}>{token.max}</button>
	{/if}
</div>

<style>
	.card {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.symbol {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.detail {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	button {
		flex-shrink: 0;
		padding: var(--space-sm) var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	button:hover {
		opacity: var(--opacity-hover);
	}
</style>
