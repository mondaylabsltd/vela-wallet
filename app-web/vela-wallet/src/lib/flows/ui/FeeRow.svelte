<script lang="ts">
	/**
	 * The network-fee row (spec 021 component 26), on every send form.
	 *
	 * A row and not a card: the fee is a fact about the transfer, and what
	 * there is to DO with it is change which token pays it — which is what
	 * the chevron opens — or ask for the figure again, which is what the ⟳
	 * does (spec 068). The SPEC sheet is explicit that the tier picker does
	 * not live IN this row: the speed control is its own folded control
	 * beneath it, and the fee here is shown, not chosen.
	 *
	 * Two buttons, not one nested inside another: the row used to BE the
	 * button, and a refresh inside it would be invalid markup and an
	 * ambiguous tap. The coin opener keeps the whole comfortable target; the
	 * refresh is a small target of its own with its own name.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { FeeRowModel } from '../model';

	interface Props {
		fee: FeeRowModel;
		onopen?: () => void;
		/** Ask the chain again. Absent ⇒ the drawn row, with no live session. */
		onrefresh?: () => void;
	}

	let { fee, onopen, onrefresh }: Props = $props();
</script>

<div class="fee">
	<div class="row">
		<button type="button" class="open" aria-label={fee.openLabel} onclick={onopen}>
			<span class="label">{fee.label}</span>
			<TokenIcon
				ticker={fee.mark.ticker}
				badgeColor={fee.mark.badgeColor}
				logoUrls={fee.mark.logoUrls}
				badgeLogoUrl={fee.mark.badgeLogoUrl}
				badgeHidden={fee.mark.badgeHidden}
				size="inline"
			/>
			<!-- Two pieces, each unbreakable (issue 231): when the row is tight the
			     money drops to a second, right-aligned line WHOLE. As one string the
			     row wrapped wherever it ran out — "Network fee" onto two lines, a
			     figure split from its ticker. No "·" between them: "≈" already joins
			     a coin to its money, and a dropped line that began "· ≈ $0.55" — the
			     everyday look on the narrowest phones — read as a rendering leftover. -->
			<span class="values">
				<span class="value">{fee.value}</span>
				{#if fee.valueFiat}
					<span class="value">{fee.valueFiat}</span>
				{/if}
			</span>
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		</button>
		<!-- A tap must never be ambiguous: while the measurement is out the icon
		     turns (and the control refuses a second tap), so "did that do
		     anything?" has an answer on screen instead of in the network tab. -->
		<button
			type="button"
			class="refresh"
			aria-label={fee.refreshLabel}
			aria-busy={fee.refreshing === true}
			disabled={onrefresh === undefined || fee.refreshing === true}
			onclick={onrefresh}
		>
			<!-- The turn belongs to the GLYPH, not to the button. The button's
			     padding is deliberately lopsided (none at the start, so the icon
			     sits against the figure it refreshes), which puts its geometric
			     centre away from the icon — rotating the button swung the icon
			     around that off-centre point in an orbit instead of turning it
			     in place. This span shrink-wraps the square icon, so the two
			     centres are the same one. -->
			<span class="turn" class:spinning={fee.refreshing === true}>
				<Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" />
			</span>
		</button>
	</div>
	<!-- `FeeView.stale`, which had no consumer in this shell at all. A quote
	     past its TTL is OLD, not WRONG — so this is a muted line beside the
	     control that fixes it, never a warning tone. Someone who reads it as an
	     error learns to distrust a row that is working.

	     The line is always in the layout and only its INK is toggled. Below this
	     row sit the speed control and Continue; letting the note appear would
	     push them down by a line at the moment somebody is reaching for
	     Continue, and a button that moves under a thumb is how a wrong tap
	     happens. The owner chose the standing blank over that. -->
	<p class="stale" class:empty={!fee.staleNote} aria-hidden={fee.staleNote ? undefined : 'true'}>
		{#if fee.staleNote}
			<Icon icon={UTILITY_ICONS.clock} size="sm" />
		{/if}
		<span>{fee.staleNote ?? ' '}</span>
	</p>
</div>

<style>
	.fee {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	/* The card is the ROW now, not the button inside it, so the two controls
	   sit on one surface and the seam between them is invisible. */
	.row {
		display: flex;
		align-items: center;
		width: 100%;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.open {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		flex: 1 1 auto;
		min-width: 0;
		padding: var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		text-align: start;
		cursor: pointer;
	}

	/* What gives way, in order. First the VALUE: its money drops to a second
	   line (the enormous shrink factor is that ordering — flex has no other
	   way to say "this one first"), but never below its widest unbroken piece
	   (`min-content`), because a column narrower than its figure is painted
	   leftward, under the coin's mark: "0 [ETH] 01329 AVAX", on the screen
	   where a person decides to pay. Only then the LABEL, which keeps its one
	   line and is elided — "Commissione di rete" on a narrow phone. An elided word is
	   better than a hidden number. */
	.label {
		flex: 1 1 auto;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.values {
		flex: 0 100000 auto;
		min-width: min-content;
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		column-gap: var(--space-sm);
	}

	.value {
		white-space: nowrap;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	/* Quiet on purpose: a fee that is fine is the normal case, and a loud
	   refresh button next to a good number invites a tap nobody needs.

	   Exactly the card's height (078 round 2): the same top and bottom edges
	   at every text size, so a two-line fee at the largest size does not leave
	   a small target floating in the middle of a tall card. The glyph stays
	   centred inside it. */
	.refresh {
		display: flex;
		align-items: center;
		align-self: stretch;
		flex: 0 0 auto;
		padding: var(--space-lg);
		padding-inline-start: 0;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.refresh:disabled {
		cursor: default;
	}

	/* Shrink-wrapped around the square glyph, so `spin` turns it on its own
	   centre rather than orbiting it around the lopsided button's. */
	.turn {
		display: flex;
	}

	.turn.spinning {
		/* A calm turn, not a frantic one: the token is the longest duration the
		   system defines, taken twice, because a full rotation in 400ms reads
		   as alarm on a screen where somebody is about to pay. */
		animation: spin calc(var(--motion-duration-slow) * 2) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

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

	/* The turn is feedback, not decoration; somebody who asked for no motion
	   still gets the disabled control and the busy state. */
	@media (prefers-reduced-motion: reduce) {
		.turn.spinning {
			animation: none;
		}
	}
</style>
