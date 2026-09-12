<script lang="ts">
	/**
	 * T2 — one token: what you hold, what it is, and what it has done.
	 *
	 * Receive and Send sit directly under the balance because they are the two
	 * reasons anyone opens this sheet. Everything below them is reference.
	 */
	import Button from '$lib/ui/Button.svelte';
	import ActivityRow from '$lib/wallet/ui/ActivityRow.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import FactRow from '../ui/FactRow.svelte';
	import { copyText } from '$lib/services/clipboard';
	import type { TokenDetailModel } from '../model';

	interface Props {
		model: TokenDetailModel;
		onreceive?: () => void;
		onsend?: () => void;
		onexplorer?: () => void;
		/** A transaction row was tapped (spec 038 #E2). */
		onselect?: (index: number) => void;
	}

	let { model, onreceive, onsend, onexplorer, onselect }: Props = $props();

	let copiedIndex = $state(-1);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copy(index: number) {
		const fact = model.facts[index];
		void copyText(fact?.copyValue ?? fact?.value ?? '');
		copiedIndex = index;
		clearTimeout(timer);
		timer = setTimeout(() => (copiedIndex = -1), 150);
	}
</script>

<div class="token">
	<div class="head">
		<TokenIcon
			ticker={model.mark.ticker}
			badgeColor={model.mark.badgeColor}
			logoUrls={model.mark.logoUrls}
			badgeLogoUrl={model.mark.badgeLogoUrl}
			badgeHidden={model.mark.badgeHidden}
		/>
		<span class="names">
			<span class="symbol">{model.symbol}</span>
			<span class="chain">{model.chain}</span>
		</span>
	</div>

	<p class="balance">{model.balance}</p>
	<p class="fiat">{model.fiat}</p>

	<div class="actions">
		<Button variant="secondary" onclick={onreceive}>{model.receive}</Button>
		<Button variant="secondary" onclick={onsend}>{model.send}</Button>
	</div>

	<ul class="facts">
		{#each model.facts as fact, i (fact.label)}
			<li><FactRow {fact} copied={copiedIndex === i} oncopy={() => copy(i)} /></li>
		{/each}
	</ul>

	<h3>{model.transactionsTitle}</h3>
	<ul class="rows">
		{#each model.rows as row, i (i)}
			<li><ActivityRow {row} onclick={onselect ? () => onselect(i) : undefined} /></li>
		{/each}
	</ul>

	{#if model.explorerUrl !== undefined}
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an explorer page outside the app -->
		<a class="explorer" href={model.explorerUrl} target="_blank" rel="noreferrer noopener">
			{model.viewOnExplorer}
		</a>
	{:else}
		<button type="button" class="explorer" onclick={onexplorer}>{model.viewOnExplorer}</button>
	{/if}
</div>

<style>
	.token {
		display: flex;
		flex-direction: column;
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-bottom: var(--space-lg);
	}

	.names {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.symbol {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.chain {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.balance {
		margin: 0;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-4xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
	}

	.fiat {
		margin: 0;
		padding-top: var(--space-xs);
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.actions {
		display: flex;
		gap: var(--space-md);
		padding-block: var(--space-xl);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.facts {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.facts li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	h3 {
		margin: 0;
		padding-block: var(--space-xl) var(--space-sm);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.explorer {
		align-self: center;
		padding: var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		text-decoration: none;
	}
</style>
