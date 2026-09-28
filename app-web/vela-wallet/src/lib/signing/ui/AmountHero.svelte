<script lang="ts">
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import type { AmountLine } from '../model';

	interface Props {
		line: AmountLine;
		/** Boxed in its own tone — the burn intercept (cs28). */
		card?: boolean;
		/** Second line inside the card. */
		note?: string;
		/** Swap lines render one step smaller than a lone hero. */
		compact?: boolean;
	}

	let { line, card = false, note, compact = false }: Props = $props();

	/**
	 * Spec 082 G60 (SC-008): the amount is the one number a person checks
	 * before sliding, and at the side panel's 360 px a transfer of 10^30 ran
	 * off the edge. A long figure steps down in size — one step past 10 digits,
	 * the floor past 14 — and wraps at its digit groups (a break opportunity
	 * after every separator, never inside a group). Nothing is ever cut.
	 */
	const figure = $derived(`${line.sign}${line.value}`);
	const size = $derived.by(() => {
		const digits = figure.replace(/\D/g, '').length;
		return digits > 14 ? 'floor' : digits > 10 ? 'step' : 'full';
	});
	/** The figure in pieces that each end at a group separator (or the end). */
	const groups = $derived(
		figure.match(/[^,.\s\u00a0\u202f'’]+[,.\s\u00a0\u202f'’]?|[,.\s\u00a0\u202f'’]/g) ?? [figure]
	);
</script>

<div class="hero" class:card class:compact data-tone={line.tone}>
	{#if line.caption && !card}
		<p class="caption">{line.caption}</p>
	{/if}
	<p class="value">
		<span class="number" data-size={size}
			>{#each groups as group, i (i)}{group}{#if i < groups.length - 1}<wbr />{/if}{/each}</span
		>
		{#if line.token}
			<LetterAvatar letter={line.token.letter} tint={line.token.tint} size={compact ? 20 : 22} />
		{/if}
		<span class="symbol">{line.symbol}</span>
	</p>
	{#if note}
		<p class="note">{note}</p>
	{:else if line.fiat}
		<p class="fiat">{line.caption && card ? `${line.caption} ` : ''}{line.fiat}</p>
	{/if}
</div>

<style>
	.hero {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.card {
		gap: var(--space-xs);
		padding: var(--space-xl);
		border-radius: var(--radius-xl);
	}

	.card[data-tone='danger'] {
		background: var(--color-error-soft);
		border: var(--border-hairline) solid var(--color-error-base);
	}

	.caption {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.value {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-xs) var(--space-md);
		margin: 0;
		min-width: 0;
	}

	.number {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-4xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: var(--leading-amountHero);
		color: var(--color-fg-base);
		/* Breaks at the groups first (the <wbr>s); anywhere only as the last
		   resort, so the figure can never run past the sheet. */
		min-width: 0;
		max-width: 100%;
		overflow-wrap: anywhere;
	}

	.compact .number {
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
	}

	.card .number {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
	}

	/* G60: a long figure steps down, one step and then the floor. */
	.number[data-size='step'] {
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
	}

	.number[data-size='floor'],
	.compact .number[data-size='step'] {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
	}

	.compact .number[data-size='floor'],
	.card .number[data-size='step'],
	.card .number[data-size='floor'] {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
	}

	[data-tone='success'] .number,
	[data-tone='success'] .symbol {
		color: var(--color-success-base);
	}

	[data-tone='danger'] .number,
	[data-tone='danger'] .symbol {
		color: var(--color-error-base);
	}

	.symbol {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.fiat,
	.note {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.note {
		color: var(--color-fg-muted);
	}
</style>
