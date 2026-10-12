<script lang="ts">
	import BalanceChanges from './BalanceChanges.svelte';
	import WarningBanner from './WarningBanner.svelte';
	import type { VerdictBlock } from '../model';

	/**
	 * The simulation verdict's place on a site's transaction.
	 *
	 * It is on the sheet from the request's first frame and is at least as
	 * tall as the verdicts a sheet usually ends on (`rooms`): each of them is
	 * laid out here, in the one cell the verdict is said in — unseen and
	 * unsaid — so the place has its height at this width, in this language
	 * and at this text size before anything has landed in it. What IS said
	 * (`inner`) stands over them, from the top.
	 *
	 * So a verdict that lands a moment after the sheet opened fills a place
	 * that was there, and nothing moves for it: not the header and its ✕ above
	 * (the phone sheet is anchored at the bottom, and a card that ARRIVED
	 * pushed its top up by the card's height), not the fee row, the signing
	 * account and the confirm below.
	 *
	 * A verdict taller than every room — a third balance row, the warning
	 * under an unverified token — is drawn whole and the place is as tall as
	 * it is. The place has no height of its own to cut it at and no scroll:
	 * no part of a verdict is ever under a fold of this place. The sheet's
	 * body scrolls instead, under its header and over its pinned confirm.
	 */
	interface Props {
		inner: VerdictBlock[];
		rooms: VerdictBlock[];
		/** Something to read has landed: named, so the sheet can bring it into sight. */
		landed: boolean;
	}

	let { inner, rooms, landed }: Props = $props();
</script>

{#snippet drawn(block: VerdictBlock, room: boolean)}
	{#if block.kind === 'warning'}
		<WarningBanner tone={block.tone} text={block.text} {room} />
	{:else}
		<BalanceChanges
			title={block.title}
			rows={block.rows}
			note={block.note}
			noteTone={block.noteTone}
			{room}
		/>
	{/if}
{/snippet}

<div class="place" data-testid="signing-verdict" data-stage={landed ? 'landed' : 'out'}>
	{#each rooms as room, i (i)}
		<div class="room" aria-hidden="true">{@render drawn(room, true)}</div>
	{/each}
	<div class="said" data-verdict={landed ? '' : undefined} aria-live="polite">
		{#each inner as block, i (i)}
			{@render drawn(block, false)}
		{/each}
	</div>
</div>

<style>
	/* Every room and what is said share one cell: the place is as tall as the
	   tallest of them, and as wide as the column. */
	.place {
		display: grid;
	}

	.place > * {
		grid-area: 1 / 1;
		min-width: 0;
	}

	.room {
		visibility: hidden;
		pointer-events: none;
	}

	/* From the top of the place, at its own height. */
	.said {
		align-self: start;
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}
</style>
