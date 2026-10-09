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
</script>

{#snippet face()}
	<span class="label">{fee.label}</span>
	<!-- The label and the fee are two wholes (078 round 3): side by side
			     while both fit; otherwise the label keeps a line of its own, WHOLE,
			     and the fee goes under it at the row's end. German at the largest
			     size cut the label mid-word ("Netzwerkg…") and the value with it;
			     a label is never elided and never broken inside a word. -->
	<span class="amount">
		<TokenIcon
			ticker={fee.mark.ticker}
			badgeColor={fee.mark.badgeColor}
			logoUrls={fee.mark.logoUrls}
			badgeLogoUrl={fee.mark.badgeLogoUrl}
			badgeHidden={fee.mark.badgeHidden}
			size="inline"
		/>
		<!-- Two pieces, each unbreakable (issue 231): when even the fee's own
				     line is tight, the money drops under the coin WHOLE, right-aligned.
				     No "·" between them: "≈" already joins a coin to its money, and a
				     dropped line that began "· ≈ $0.55" read as a rendering leftover. -->
		<span class="values">
			<span class="value">{fee.value}</span>
			{#if fee.valueFiat}
				<span class="value">{fee.valueFiat}</span>
			{/if}
		</span>
		<!-- The chevron promises the list of coins: only where a tap opens it. -->
		{#if tap === 'open'}
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		{/if}
	</span>
{/snippet}

<div class="fee">
	<div class="row">
		{#if tap === 'none'}
			<!-- PR 2 polish: the relay answered that it would fail and no other
			     coin is left to pay with — the dash, stated: no control, no
			     chevron, no press (spec 081, a control that cannot act is not
			     drawn as one). The line under the confirm says what happened. -->
			<div class="open stated" data-testid="fee-row-stated">{@render face()}</div>
		{:else}
			<!-- A failed fee (PR 2 note 1) the core says a tap asks again: the
			     refresh's own path, no list promised; while a re-ask is out it is
			     the answer awaited — a second tap asks nothing. Otherwise the coins
			     (and, PR 2 polish, "Pay with another coin" after the relay
			     answered that it would fail). -->
			<button
				type="button"
				class="open"
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

	/* Within its line the money still drops under the coin, whole, when even
	   that line is tight. */
	.values {
		flex: 0 1 auto;
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
</style>
