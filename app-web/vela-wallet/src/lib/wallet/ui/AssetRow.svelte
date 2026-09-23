<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { AssetRowModel } from '../model';
	import TokenIcon from './TokenIcon.svelte';

	interface Props {
		row: AssetRowModel;
		/**
		 * Spec 021 SD1b: the row is off the network the multi-send is locked
		 * to. Still readable, still there — it is a token the person owns —
		 * but not selectable, and saying so by weight rather than by hiding it.
		 */
		dimmed?: boolean;
		/** Spec 021 SD1b: chosen for a multi-token send. */
		selected?: boolean;
		/**
		 * Spec 021 SD2d: a trailing control (Max) after the numbers.
		 *
		 * A control inside a control is why this row stopped being a `<button>`
		 * unconditionally: the sweep form's Max is a real `<button>`, and
		 * `<button>` inside `<button>` is markup no parser keeps — the HTML
		 * parser closes the outer one at the inner's start tag, so the server's
		 * page and the client's DOM disagree about the row's shape. See
		 * `onclick` below.
		 */
		trailing?: Snippet;
		/**
		 * What picking this row does. Absent ⇒ the row is a READING, not a
		 * control: no button, no pointer, no press. The sweep form lists the
		 * tokens already chosen and only the Max beside each one acts, so the
		 * row there answers to nothing (spec 081, dead-controls #19).
		 */
		onclick?: () => void;
	}

	let { row, dimmed = false, selected = false, trailing, onclick }: Props = $props();
</script>

{#snippet body()}
	<TokenIcon
		ticker={row.ticker}
		badgeColor={row.badgeColor}
		logoUrls={row.logoUrls}
		badgeLogoUrl={row.badgeLogoUrl}
		badgeHidden={row.badgeHidden}
	/>
	<span class="text">
		<span class="ticker">{row.ticker}</span>
		<span class="chain">{row.chain}</span>
	</span>
	<span class="numbers" class:masked={row.masked}>
		<span class="balance">{row.balance}</span>
		{#if row.fiat.kind === 'value'}
			<span class="fiat">{row.fiat.text}</span>
		{:else if row.fiat.kind === 'no-price'}
			<span class="fiat no-price">{row.fiat.text}</span>
		{:else if row.fiat.kind === 'masked'}
			<span class="fiat">••••</span>
		{/if}
	</span>
	{#if trailing}<span class="trailing">{@render trailing()}</span>{/if}
{/snippet}

{#if onclick !== undefined}
	<button
		type="button"
		class="row"
		class:dimmed
		class:selected
		disabled={dimmed}
		aria-pressed={selected ? true : undefined}
		{onclick}
	>
		{@render body()}
	</button>
{:else}
	<div class="row stated" class:dimmed class:selected>{@render body()}</div>
{/if}

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.row:active:not(:disabled) {
		transform: scale(var(--motion-press-row));
	}

	/* Nothing to pick: no invitation, and no press under the finger. */
	.row.stated,
	.row.stated:active {
		cursor: default;
		transform: none;
	}

	.dimmed {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.selected {
		/* The whole row lifts rather than growing a checkbox: the list is the
		   selection, and a column of empty boxes down the leading edge would
		   push the token marks off the margin every other screen aligns to. */
		background: var(--color-bg-raised);
		border-radius: var(--radius-lg);
		padding-inline: var(--space-md);
		margin-inline: calc(var(--space-md) * -1);
	}

	.trailing {
		display: flex;
		flex-shrink: 0;
		margin-inline-start: var(--space-lg);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.ticker {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.chain {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.numbers {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-xs);
		flex-shrink: 0;
	}

	.balance {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.fiat {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-subtle);
	}

	.no-price {
		color: var(--color-warning-base);
	}

	.masked .balance,
	.masked .fiat {
		letter-spacing: var(--space-xs);
	}
</style>
