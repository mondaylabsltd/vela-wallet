<script lang="ts">
	import type { BalanceRow, Tone } from '../model';

	interface Props {
		title: string;
		rows: BalanceRow[];
		note?: string;
		noteTone?: Tone;
		/** The card that lands after the sheet has opened: named, so the sheet can keep it in sight. */
		verdict?: boolean;
		/**
		 * Drawn as ROOM, not as a card (`VerdictPlace`): the same card, laid out
		 * at the same size and never said. Its words are generated content, so
		 * nothing of a verdict the sheet has not given is in the page's text.
		 */
		room?: boolean;
	}

	let { title, rows, note, noteTone = 'neutral', verdict = false, room = false }: Props = $props();
</script>

<!-- A room's words: in the layout, not in the text. -->
{#snippet words(text: string)}
	{#if room}<span class="unsaid" data-room={text}></span>{:else}{text}{/if}
{/snippet}

<!-- The simulation's own account of what moves. It is the ONE part of a
     signing sheet a malicious site cannot author, which is why the deeper
     degradation rungs promote it from footnote to protagonist. Every row and
     the note are drawn whole: no height of its own, nothing clipped, nothing
     scrolled inside it (PR 3 device round) — a long figure wraps. -->
<section class="balances" data-verdict={verdict ? '' : undefined}>
	<h3>{@render words(title)}</h3>
	{#each rows as row, i (i)}
		<div class="row">
			<span class="symbol">{@render words(row.symbol)}</span>
			<span class="delta" data-tone={row.tone}>{@render words(row.delta)}</span>
		</div>
	{/each}
	{#if note}
		<p class="note" data-tone={noteTone}>{@render words(note)}</p>
	{/if}
</section>

<style>
	.balances {
		padding: var(--space-lg) var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
	}

	h3 {
		margin: 0;
		padding-block: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.row {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-xl);
		padding-block: var(--space-md);
	}

	.symbol {
		min-width: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.delta {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
		min-width: 0;
		text-align: end;
		overflow-wrap: anywhere;
	}

	.delta[data-tone='success'] {
		color: var(--color-success-base);
	}

	.delta[data-tone='danger'] {
		color: var(--color-error-base);
	}

	/* A token nobody vouches for: its direction, in the caution's ink — the
	   colour of the sentence under the card that says why it has no figure. */
	.delta[data-tone='caution'] {
		color: var(--color-warning-base);
	}

	.unsaid::before {
		content: attr(data-room);
	}

	.note {
		margin: 0;
		padding-top: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.note[data-tone='danger'] {
		color: var(--color-error-base);
	}

	/* PR 3: nothing could be checked before the deadline — a caution, in the
	   warning banner's own ink. */
	.note[data-tone='caution'] {
		color: var(--color-warning-base);
	}
</style>
