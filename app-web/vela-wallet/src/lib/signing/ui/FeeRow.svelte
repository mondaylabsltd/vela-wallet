<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import PositiveNote from './PositiveNote.svelte';
	import FeeSpeedRow from '$lib/flows/ui/FeeSpeedRow.svelte';
	import FeeRefreshButton from '$lib/flows/ui/FeeRefreshButton.svelte';
	import FeeStaleNote from '$lib/flows/ui/FeeStaleNote.svelte';
	import type { FeeModel } from '../model';

	interface Props {
		fee: FeeModel;
		/** Opens / closes the fee-token selector; absent in the gallery. */
		ontoggle?: () => void;
		/** A coin was chosen — its option id. Absent in the gallery, where the list is a picture. */
		onpick?: (id: string) => void;
		/**
		 * The speed control under the fee (spec 069) — the send form's own,
		 * so a dApp transaction's speed is chosen exactly as a send's is.
		 */
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
		/**
		 * Spec 079: the send form's refresh control — ask for the fee again.
		 * Absent in the gallery, where the control is drawn shut.
		 */
		onrefresh?: () => void;
	}

	let { fee, ontoggle, onpick, onspeed, onspeedpick, onrefresh }: Props = $props();

	/**
	 * The line under the fee card says why the confirm is shut: a coin that
	 * cannot pay, a quote that failed. While the fee is measured again (the
	 * 30 s re-quote, a refresh, a new speed) that verdict is about the last
	 * quote, so it is not said — but its line keeps its height, holding the
	 * last words: the phone sheet is bottom-anchored, and a line that came and
	 * went with every measurement moved everything above it. A fee that lands
	 * with nothing to say lets the line go; that is a change, not a jump.
	 * The builder's own reservation (spec 082 G47, a failure being asked
	 * again) comes first.
	 */
	let lastWarning: string | undefined;
	/** The line under the card: said, held (drawn invisibly), or none. */
	const warningLine = $derived.by((): { text: string; said: boolean } | undefined => {
		if (fee.kind !== 'onchain') return undefined;
		if (fee.warning !== undefined) {
			lastWarning = fee.warning;
			return { text: fee.warning, said: true };
		}
		const held = fee.refreshing === true ? (fee.warningReserved ?? lastWarning) : undefined;
		if (fee.refreshing !== true) lastWarning = undefined;
		const text = held ?? fee.warningReserved;
		return text === undefined ? undefined : { text, said: false };
	});

	/**
	 * The money under the coin, held — the send form's rule for its own fee
	 * row (0.8: withholding never moves the layout), here for PR 3 final note
	 * F12.
	 *
	 * The fee was one string, "0.0015 ETH · ≈…". When the display currency
	 * committed it became "0.0015 ETH · ≈₫112,500.00"; at 320 px that no longer
	 * fitted beside "Network fee", the label wrapped, the row grew a line and
	 * the whole bottom-anchored sheet moved 18 px. How long the figure will be
	 * is exactly what is not known while it is withheld. So while it is, the
	 * money takes a line of its own under the coin — the one layout whose line
	 * count does not depend on the figure's length — from the first frame
	 * (under "Estimating…" the line is kept empty), and KEEPS it for as long
	 * as this row stands: a row that folded back up for a short figure would
	 * be the same jump the other way.
	 */
	let stackedOnce = $state(false);
	$effect.pre(() => {
		if (fee.kind === 'onchain' && fee.valueFiatWithheld === true) stackedOnce = true;
	});
	const stacked = $derived(
		stackedOnce || (fee.kind === 'onchain' && fee.valueFiatWithheld === true)
	);
</script>

<!--
	The label and the fee are two wholes, as on the send form: side by side
	while both fit; otherwise the label keeps a line of its own, WHOLE, and the
	fee goes under it at the row's end — a label is never broken to make room.
	The coin and its money are two unbreakable pieces; no "·" between them
	("≈" already joins a coin to its money, and a dropped line that began "·"
	read as a leftover).
-->
{#snippet chevronMark()}
	<!-- Only where a tap opens the list of coins; a failed quote with one
	     coin is tapped to ask again. -->
	<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
{/snippet}

{#snippet face(label: string, value: string, money: string | undefined, chevron: boolean)}
	<span class="label">{label}</span>
	{#if stacked}
		<!-- Held: the coin (and the chevron it belongs with) beside the label
		     or under it, and the money on a line that is its own whatever its
		     length. Kept empty while there is no figure to stand on it. -->
		<span class="lead">
			<span class="value">{value}</span>
			{#if chevron}{@render chevronMark()}{/if}
		</span>
		{#if money !== undefined}
			<span class="value money" class:indent={chevron}>{money}</span>
		{:else}
			<span class="value money kept" aria-hidden="true">&nbsp;</span>
		{/if}
	{:else}
		<span class="amount">
			<span class="values">
				<span class="value">{value}</span>
				{#if money !== undefined}
					<span class="value">{money}</span>
				{/if}
			</span>
			{#if chevron}{@render chevronMark()}{/if}
		</span>
	{/if}
{/snippet}

{#if fee.kind === 'offchain'}
	<PositiveNote text={fee.note} quiet />
{:else if fee.kind === 'onchain'}
	{#if fee.selector}
		<section class="selector">
			<button type="button" class="head" onclick={ontoggle}>
				<span>{fee.selector.title}</span>
				<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
			</button>
			{#each fee.selector.options as option (option.id)}
				<!-- A coin that cannot pay is DRAWN and not pickable: hiding it would be a
				     second filter beside the core's own, and a live-looking row that does
				     nothing is how somebody pays gas in a coin they do not hold (issue 211). -->
				<button
					type="button"
					class="option"
					class:selected={option.selected}
					disabled={option.insufficient === true}
					onclick={() => onpick?.(option.id)}
				>
					<!-- The send form's fee-coin mark: the coin's logo over its ticker. -->
					<TokenIcon
						ticker={option.mark.ticker}
						badgeColor={option.mark.badgeColor}
						logoUrls={option.mark.logoUrls}
						badgeLogoUrl={option.mark.badgeLogoUrl}
						badgeHidden={option.mark.badgeHidden}
					/>
					<span class="who">
						<span class="name">{option.name}</span>
						<span class="balance">{option.balance}</span>
					</span>
					<span class="numbers">
						<span class="fee">{option.fee}</span>
					</span>
					{#if option.selected}
						<span class="check"><Icon icon={UTILITY_ICONS.check} size="base" /></span>
					{/if}
				</button>
				{#if option.reason}
					<!-- Issue 408: why a greyed coin cannot pay, under its row and at full
					     strength — the dimming is not a reason. Set in past the mark. -->
					<p class="option-reason">{option.reason}</p>
				{/if}
			{/each}
		</section>
	{:else}
		<!-- The card is the LINE, not the button inside it (the send form's
		     row, spec 068): the fee and its refresh are two controls of their
		     own on one surface — a refresh nested in the row's button would be
		     invalid markup and an ambiguous tap. -->
		<div class="line">
			{#if fee.tappable}
				<button type="button" class="row" onclick={ontoggle}>
					{@render face(fee.label, fee.value, fee.valueFiat, fee.chevron !== false)}
				</button>
			{:else}
				<!-- One coin, and a quote in hand: there is nothing to choose, so
				     this is the fee STATED. No chevron, no pointer, no press — the
				     house rule that a control which cannot act is not drawn as one
				     (spec 081, dead-controls #6). -->
				<div class="row stated">
					{@render face(fee.label, fee.value, fee.valueFiat, false)}
				</div>
			{/if}
			{#if fee.refreshLabel !== undefined}
				<FeeRefreshButton
					label={fee.refreshLabel}
					refreshing={fee.refreshing === true}
					{onrefresh}
				/>
			{/if}
		</div>
		{#if warningLine?.said}
			<!-- Issue 262: the reason the confirm is shut, right under the fee it is
			     about (spec 082 G47 — it sat under the speed row) — the coin cannot
			     pay, or (spec 079) the fee could not be asked and the sheet is
			     asking again. -->
			<p class="warning" role="alert">{warningLine.text}</p>
		{:else if warningLine}
			<!-- Measuring again: the last words are not this ask's, but their
			     line keeps its height, so nothing above or below jumps (G47). -->
			<p class="warning reserved" aria-hidden="true">{warningLine.text}</p>
		{/if}
		{#if fee.refreshLabel !== undefined}
			<!-- The send form's calm note, in its standing line: the confirm
			     below does not move when a quote grows old. -->
			<FeeStaleNote note={fee.staleNote} />
		{/if}
		{#if fee.speed}
			<FeeSpeedRow speed={fee.speed} ontoggle={onspeed} onselect={onspeedpick} />
		{/if}
	{/if}
	{#if fee.selector && fee.warning}
		<!-- With the list of coins open, the reason sits under the list. -->
		<p class="warning" role="alert">{fee.warning}</p>
	{/if}
{/if}

<style>
	.warning {
		margin: var(--space-sm) var(--space-xl) 0;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: 500;
		color: var(--color-error-base);
	}

	.warning.reserved {
		visibility: hidden;
	}

	.line {
		display: flex;
		align-items: center;
		width: 100%;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.row {
		display: flex;
		flex: 1 1 auto;
		flex-wrap: wrap;
		align-items: center;
		column-gap: var(--space-md);
		row-gap: var(--space-xs);
		min-width: 0;
		padding: var(--space-lg) var(--space-xl);
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		cursor: pointer;
		text-align: start;
	}

	.row.stated {
		cursor: default;
	}

	/* The label is WHOLE: it may wrap between words when a line cannot hold
	   it, never inside one (`min-content` is its longest word). When it and
	   the fee do not share a line, the fee — the next flex item — drops under
	   it. */
	.label {
		flex: 0 1 auto;
		min-width: min-content;
		overflow-wrap: normal;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	/* The figures and the chevron, at the row's end — beside the label, or on
	   the line under it. Never narrower than its widest unbroken piece. */
	.amount {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-md);
		min-width: min-content;
	}

	/* Within its line the money drops under the coin, whole, when even that
	   line is tight. */
	.values {
		flex: 0 1 auto;
		min-width: min-content;
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		column-gap: var(--space-sm);
	}

	.chevron {
		display: flex;
		flex: none;
	}

	/* Held (see `stacked`): the money has a line to itself, the full width of
	   the row, at the row's end. The label and the coin above it share a line
	   or not as THEY fit — both are known before the money is — so a figure
	   landing in the money's line, long or short, at any width or text size,
	   changes no line count and moves nothing. (As one piece with the coin,
	   the pair was as wide as its longer line: a long figure pushed the whole
	   pair under the label, or the label onto a second line.) */
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

	.selector {
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		padding: var(--space-lg) var(--space-xl);
	}

	.head {
		width: 100%;
		border: none;
		background: none;
		font-family: var(--font-ui);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.option {
		width: 100%;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
	}

	.option:disabled {
		opacity: var(--opacity-disabled, 0.4);
		cursor: default;
	}

	.option.selected {
		background: var(--color-bg-raised);
	}

	.option-reason {
		/* The option's padding, its mark (TokenIcon's row size, twice
		   `--space-2xl`) and the gap after it. */
		margin: 0 var(--space-md) var(--space-sm)
			calc(var(--space-md) + var(--space-2xl) * 2 + var(--space-lg));
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: 500;
		color: var(--color-error-base);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.balance {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.numbers {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
	}

	.fee {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.check {
		color: var(--color-accent-base);
	}
</style>
