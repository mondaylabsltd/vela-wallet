<script lang="ts">
	/**
	 * The send form's token card (spec 021 component 16): which token is being
	 * sent, off which chain, out of how much — and the Max that fills the
	 * amount with all of it.
	 *
	 * It is a card and not a header because on SD2 it is also the thing you tap
	 * to change your mind about the token.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { SendTokenCardModel } from '../model';

	interface Props {
		token: SendTokenCardModel;
		onmax?: () => void;
		/** The card's own tap: back to the asset picker (issue 326). */
		onchange?: () => void;
	}

	let { token, onmax, onchange }: Props = $props();

	const changeable = $derived(token.change !== undefined && onchange !== undefined);
</script>

{#snippet face()}
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
{/snippet}

<div class="card">
	<!-- The token, and — where the core allows it — the way to another one.
	     Max stays its own button beside it: one tap, one meaning. -->
	{#if changeable}
		<button type="button" class="pick" aria-label={token.change} onclick={onchange}>
			{@render face()}
			<span class="chevron" aria-hidden="true"
				><Icon icon={UTILITY_ICONS['chevron-down']} size="sm" /></span
			>
		</button>
	{:else}
		<span class="pick">{@render face()}</span>
	{/if}
	{#if token.max !== undefined}
		<button type="button" class="max" onclick={onmax}>{token.max}</button>
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

	.pick {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		flex: 1;
		min-width: 0;
		padding: 0;
		border: none;
		background: none;
		font: inherit;
		color: inherit;
		text-align: start;
	}

	button.pick {
		cursor: pointer;
	}

	button.pick:hover {
		opacity: var(--opacity-hover);
	}

	.chevron {
		display: inline-flex;
		flex-shrink: 0;
		color: var(--color-fg-muted);
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

	/* Wraps, never "…" (078 round 3): at the largest size the balance itself
	   was cut — "Ethereum · Guthaben 0.0…". The balance phrase is kept in one
	   piece (`live-send.ts`), so a wrap falls after the network's "·". */
	.detail {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: break-word;
	}

	.max {
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

	.max:hover {
		opacity: var(--opacity-hover);
	}
</style>
