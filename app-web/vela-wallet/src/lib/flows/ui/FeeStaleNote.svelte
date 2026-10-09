<script lang="ts">
	/**
	 * `FeeView.stale`, said under a fee row (spec 068) — the send form's note,
	 * and since spec 079 the signing sheet's. A quote past its TTL is OLD, not
	 * WRONG — so this is a muted line beside the control that fixes it, never a
	 * warning tone. Someone who reads it as an error learns to distrust a row
	 * that is working.
	 *
	 * The line is always in the layout and only its INK is toggled. Below a fee
	 * row sit the speed control and the button that pays;
	 * letting the note appear would push them down by a line at the moment
	 * somebody is reaching for them, and a control that moves under a thumb is
	 * how a wrong tap happens. The owner chose the standing blank over that.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		/** The note, or nothing — the line keeps its room either way. */
		note?: string;
	}

	let { note }: Props = $props();
</script>

<p class="stale" class:empty={!note} aria-hidden={note ? undefined : 'true'}>
	{#if note}
		<Icon icon={UTILITY_ICONS.clock} size="sm" />
	{/if}
	<!-- A no-break space, not a plain one: a plain space collapses and the
	     standing line would have no height to stand in. -->
	<span>{note ?? '\u00a0'}</span>
</p>

<style>
	/* `visibility`, not `display`: the box has to keep occupying its line. */
	.stale.empty {
		visibility: hidden;
	}

	/* Loud enough to be read, quiet enough not to alarm. It was the smallest
	   size in the faintest colour, which on a row of numbers is the same as
	   not being there — the owner read right past it. It now carries the body
	   size and the muted tone the row's own label uses, plus a glyph, because
	   a line of prose among figures is found by its shape first. The danger
	   tone is still wrong: this quote is OLD, not BROKEN. */
	.stale {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		margin: 0;
		padding-inline: var(--space-lg);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
