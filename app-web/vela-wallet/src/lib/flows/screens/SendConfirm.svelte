<script lang="ts">
	/**
	 * SD3 / SD3b / SD3c / DSD3L — the last screen before the money moves.
	 *
	 * Two blocks and nothing else: what is being sent, and the four facts that
	 * decide whether that is right (from, to, network, fee). A split or a sweep
	 * adds a second card listing the parts. Per the SPEC sheet this is the ONE
	 * accent CTA in the whole send journey — every other button on the way here
	 * is an outline.
	 */
	import Button from '$lib/ui/Button.svelte';
	import Breakdown from '../ui/Breakdown.svelte';
	import FactRow from '../ui/FactRow.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { SendConfirmModel } from '../model';

	interface Props {
		model: SendConfirmModel;
		onconfirm?: () => void;
	}

	let { model, onconfirm }: Props = $props();
</script>

<div class="confirm">
	<div class="hero">
		{#if model.mark !== undefined}
			<TokenIcon
				ticker={model.mark.ticker}
				badgeColor={model.mark.badgeColor}
				logoUrls={model.mark.logoUrls}
				badgeLogoUrl={model.mark.badgeLogoUrl}
				badgeHidden={model.mark.badgeHidden}
			/>
		{/if}
		<p class="amount">{model.amount}</p>
		<p class="subline">{model.subline}</p>
	</div>

	<ul class="facts">
		{#each model.facts as fact (fact.label)}
			<li><FactRow {fact} /></li>
		{/each}
	</ul>

	{#if model.breakdown !== undefined}
		<Breakdown rows={model.breakdown} />
	{/if}

	{#if model.alert !== undefined}
		<p class="alert" role="alert">{model.alert}</p>
	{/if}

	<div class="cta">
		<Button variant="primary" shape="rounded" onclick={onconfirm}>{model.cta}</Button>
	</div>
</div>

<style>
	.confirm {
		display: flex;
		flex-direction: column;
		flex: 1;
		gap: var(--space-lg);
	}

	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		text-align: center;
		padding-block: var(--space-lg) var(--space-xl);
	}

	.amount {
		margin: 0;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-4xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
	}

	.subline {
		margin: 0;
		padding-top: var(--space-xs);
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: var(--space-xs) var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.facts li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	/* Pushed to the bottom of the screen: confirming is the end of a journey,
	   and the mocks leave the space between the facts and the button empty
	   rather than filling it. */
	.cta {
		margin-top: auto;
		padding-block: var(--space-3xl) var(--space-xl);
	}
	.alert {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-danger-base);
	}
</style>
