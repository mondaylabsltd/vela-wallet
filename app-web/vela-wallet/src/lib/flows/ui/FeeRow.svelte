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
	import FeeRefreshButton from './FeeRefreshButton.svelte';
	import FeeStaleNote from './FeeStaleNote.svelte';
	import type { FeeRowModel } from '../model';

	interface Props {
		fee: FeeRowModel;
		onopen?: () => void;
		/** Ask the chain again. Absent ⇒ the drawn row, with no live session. */
		onrefresh?: () => void;
	}

	let { fee, onopen, onrefresh }: Props = $props();

	/** What a tap does (PR 2 polish): the core's word for a failed fee, else the coins. */
	const tap = $derived(fee.tap ?? 'open');

	/**
	 * The money under the coin, held (0.8 — withholding never moves the
	 * layout).
	 *
	 * The coin and its money share a line while both fit, and with the money
	 * withheld — "≈ …" until the display currency commits — they always do.
	 * Then the figure lands: "≈ ₫112,500.00" is too long for that line, the fee
	 * drops under the label, and everything beneath the row moves down a line
	 * (measured at 390 px: 18 px). How long the figure will be is exactly what
	 * is not known yet. So while it is withheld the money takes a line of its
	 * own under the coin — the one layout whose line count does not depend on
	 * the figure's length — and KEEPS it for as long as this row stands: a row
	 * that folded back up for a short figure would be the same jump the other
	 * way.
	 */
	let stackedOnce = $state(false);
	$effect.pre(() => {
		if (fee.valueFiatWithheld === true) stackedOnce = true;
	});
	const stacked = $derived(stackedOnce || fee.valueFiatWithheld === true);
</script>

{#snippet coin()}
	<!-- The coin's mark is ONE piece with the coin's figure (PR 3 final note
	     F13). It stood before the two figures as a column of their own, so it
	     sat at that column's start — and the column is as wide as its longer
	     line: when "≈ …" became "≈ ₫112,500.00" the mark slid 19 px left, away
	     from the coin it marks. Beside the coin it is anchored to the row's
	     end with it, whatever lands underneath. -->
	<span class="coin">
		<TokenIcon
			ticker={fee.mark.ticker}
			badgeColor={fee.mark.badgeColor}
			logoUrls={fee.mark.logoUrls}
			badgeLogoUrl={fee.mark.badgeLogoUrl}
			badgeHidden={fee.mark.badgeHidden}
			size="inline"
		/>
		<span class="value">{fee.value}</span>
	</span>
{/snippet}

{#snippet chevron()}
	<!-- The chevron promises the list of coins: only where a tap opens it. -->
	{#if tap === 'open'}
		<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
	{/if}
{/snippet}

{#snippet face()}
	<span class="label">{fee.label}</span>
	{#if stacked}
		<!-- Held (see `stacked`): the coin — its mark, its figure, the chevron
		     that belongs with it — beside the label or under it, and the money
		     on a line that is its own whatever its length. -->
		<span class="lead">
			{@render coin()}
			{@render chevron()}
		</span>
		{#if fee.valueFiat}
			<span class="value money" class:indent={tap === 'open'}>{fee.valueFiat}</span>
		{/if}
	{:else}
		<!-- The label and the fee are two wholes (078 round 3): side by side
		     while both fit; otherwise the label keeps a line of its own, WHOLE,
		     and the fee goes under it at the row's end. German at the largest
		     size cut the label mid-word ("Netzwerkg…") and the value with it;
		     a label is never elided and never broken inside a word. -->
		<span class="amount">
			<!-- Two pieces, each unbreakable (issue 231): when even the fee's
			     own line is tight, the money drops under the coin WHOLE,
			     right-aligned. No "·" between them: "≈" already joins a coin to
			     its money, and a dropped line that began "· ≈ $0.55" read as a
			     rendering leftover. -->
			<span class="values">
				{@render coin()}
				{#if fee.valueFiat}
					<span class="value">{fee.valueFiat}</span>
				{/if}
			</span>
			{@render chevron()}
		</span>
	{/if}
{/snippet}

<div class="fee">
	<div class="row">
		{#if tap === 'none'}
			<!-- PR 2 polish: the relay answered that it would fail and no other
			     coin is left to pay with — the dash, stated: no control, no
			     chevron, no press (spec 081, a control that cannot act is not
			     drawn as one). The line under the confirm says what happened. -->
			<div class="open stated" class:stacked data-testid="fee-row-stated">{@render face()}</div>
		{:else}
			<!-- A failed fee (PR 2 note 1) the core says a tap asks again: the
			     refresh's own path, no list promised; while a re-ask is out it is
			     the answer awaited — a second tap asks nothing. Otherwise the coins
			     (and, PR 2 polish, "Pay with another coin" after the relay
			     answered that it would fail). -->
			<button
				type="button"
				class="open"
				class:stacked
				aria-label={tap === 'retry' ? fee.refreshLabel : fee.openLabel}
				onclick={tap === 'retry' ? () => fee.refreshing !== true && onrefresh?.() : onopen}
			>
				{@render face()}
			</button>
		{/if}
		<FeeRefreshButton label={fee.refreshLabel} refreshing={fee.refreshing === true} {onrefresh} />
	</div>
	<!-- `FeeView.stale`, which had no consumer in this shell at all (spec
	     068): a standing line whose INK is toggled — see `FeeStaleNote`. A
	     failed fee's reason (PR 2 note 1) stands in the same line. -->
	<FeeStaleNote note={fee.staleNote} reason={fee.reason} />
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
		flex-wrap: wrap;
		align-items: center;
		column-gap: var(--space-md);
		row-gap: var(--space-xs);
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

	/* The same row, stated: nothing to press. */
	.stated {
		cursor: default;
	}

	/* The label is WHOLE: it may wrap between words when a line cannot hold
	   it, never inside one (`min-content` is its longest word) and never into
	   "…". When it and the fee do not share a line, the fee — the next flex
	   item — drops under it. */
	.label {
		flex: 0 1 auto;
		min-width: min-content;
		overflow-wrap: normal;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The mark, the figures and the chevron, at the row's end — beside the
	   label, or on the line under it. It never shrinks below its widest
	   unbroken piece: a column narrower than its figure is painted leftward,
	   under the coin's mark ("0 [ETH] 01329 AVAX"), on the screen where a
	   person decides to pay. */
	.amount {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-md);
		min-width: min-content;
	}

	.chevron {
		display: flex;
		flex: none;
	}

	/* Held (see `stacked`): the money has a line to itself, the full width of
	   the row, at the row's end. The label and the coin above it share a line
	   or not as THEY fit — both are known before the money is — so a figure
	   landing in the money's line, long or short, at any width or text size,
	   changes no line count, and the coin and its mark above it do not move.
	   (As a column beside the label the pair was as wide as its longer line:
	   a long figure slid the mark left, and in a tight row pushed the whole
	   pair under the label.) */
	.lead {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-md);
		min-width: min-content;
	}

	.money {
		flex: 0 0 100%;
		/* The whole line and no more, its end padding included. */
		box-sizing: border-box;
		text-align: end;
	}

	/* Under the coin, not under the chevron beside it: the two figures end at
	   one edge. */
	.money.indent {
		padding-inline-end: calc(var(--icon-sm) + var(--space-md));
	}

	/* Within its line the money still drops under the coin, whole, when even
	   that line is tight. */
	.values {
		flex: 0 1 auto;
		min-width: min-content;
		display: flex;
		flex-wrap: wrap;
		/* The coin's piece is as tall as its mark; the money beside it sits on
		   the same middle. */
		align-items: center;
		justify-content: flex-end;
		column-gap: var(--space-sm);
	}

	/* The mark and its coin: one unbreakable piece, the gap the mark always
	   kept from the figures. */
	.coin {
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: var(--space-md);
		white-space: nowrap;
	}

	.value {
		white-space: nowrap;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}
</style>
