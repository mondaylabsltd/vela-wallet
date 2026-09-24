<script lang="ts">
	/**
	 * SD2b's split row (spec 021 component 13): one of N people, what they get,
	 * and the way to drop them.
	 *
	 * The ordinal ("Recipient 2") is a label above the name rather than a
	 * number beside it, because in a split the ROW is the person and the number
	 * is only there to keep three otherwise-similar cards apart.
	 *
	 * Live (spec 028 Phase 10), the row is ALSO where the person and the amount
	 * are typed: the drawn card assumed every recipient arrived from the book or
	 * a spreadsheet, and a "+ add recipient" that produced a card nothing could
	 * fill was a dead promise. `oninput` present ⇒ the address and the amount
	 * are fields, with the book's door beside the address, exactly as the
	 * single form's field has one.
	 *
	 * The card has two faces, and they are the same fields. AT REST a filled row
	 * reads as the drawn card does — a name over a short address, or the short
	 * address alone — because forty imported people shown as forty clipped hex
	 * strings in underlined boxes is a form, not a payroll. IN HAND (focused, or
	 * still empty) the field is a well that says what it wants. The input never
	 * leaves; at rest its characters step aside for the reading of them.
	 *
	 * A row the core has flagged as a repeat of an earlier recipient carries that
	 * sentence under its address (issue 203): the same address could take two
	 * lines of one batch with nothing on screen saying so, and the batch is sent
	 * exactly as asked — so this warns, beside the row it is about, and refuses
	 * nothing.
	 */
	import { composing, pasted, takeAmount } from '../amount-field';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import type { RecipientCardModel } from '../model';

	interface Props {
		recipient: RecipientCardModel;
		/** The token the amount is counted in — the amount field's own name. */
		symbol?: string;
		onremove?: () => void;
		/** Present ⇒ the row can be typed into. Absent, the drawn card stands. */
		oninput?: (patch: { address?: string; amount?: string }) => void;
		/** The book, for THIS row. */
		onpick?: () => void;
	}

	let { recipient, symbol, onremove, oninput, onpick }: Props = $props();

	/** Ties the warning to the field it is about, for a screen reader. */
	const duplicateId = $props.id();
	const amountNoteId = `${duplicateId}-amount`;

	const hasAddress = $derived(recipient.address !== '');
</script>

<div class="card" class:editable={oninput !== undefined}>
	{#if recipient.identiconSvg !== ''}
		<Identicon svg={recipient.identiconSvg} size="row" address={recipient.address} />
	{:else}
		<!-- Nobody yet: a seat, not a blank disc that reads as artwork that failed to load. -->
		<span class="seat" aria-hidden="true">
			<Icon icon={UTILITY_ICONS['user-round']} size="base" />
		</span>
	{/if}
	<span class="text">
		<span class="ordinal">{recipient.ordinal}</span>
		{#if oninput}
			<span
				class="well address-well"
				class:filled={hasAddress}
				class:named={hasAddress && recipient.addressShort !== undefined}
				data-field
				data-tone={recipient.addressNote ? 'error' : undefined}
			>
				<input
					class="entry address"
					spellcheck="false"
					autocomplete="off"
					autocapitalize="none"
					aria-label={`${recipient.ordinal} · ${recipient.addressLabel ?? ''}`}
					aria-invalid={recipient.addressNote ? 'true' : undefined}
					aria-describedby={recipient.duplicateNote || recipient.addressNote
						? duplicateId
						: undefined}
					placeholder={recipient.addressPlaceholder}
					value={recipient.address}
					oninput={(event) => oninput({ address: event.currentTarget.value })}
				/>
				{#if hasAddress}
					<span class="reading" aria-hidden="true">
						<span class="name" class:mono={recipient.addressShort === undefined}>
							{recipient.name}
						</span>
						{#if recipient.addressShort !== undefined}
							<span class="short">{recipient.addressShort}</span>
						{/if}
					</span>
				{/if}
				{#if onpick}
					<button type="button" class="pick" aria-label={recipient.pickLabel} onclick={onpick}>
						<Icon icon={UTILITY_ICONS['user-round']} size="sm" />
					</button>
				{/if}
			</span>
		{:else}
			<span class="name" class:mono={recipient.addressShort === undefined}>{recipient.name}</span>
			{#if recipient.addressShort !== undefined}
				<span class="short">{recipient.addressShort}</span>
			{/if}
		{/if}
		{#if recipient.addressNote}
			<span class="wrong" id={duplicateId}>{recipient.addressNote}</span>
		{:else if recipient.duplicateNote}
			<span class="duplicate" id={duplicateId}>{recipient.duplicateNote}</span>
		{/if}
		{#if recipient.amountNote}
			<span class="wrong" id={amountNoteId}>{recipient.amountNote}</span>
		{/if}
	</span>
	{#if oninput}
		<span
			class="well amount-well"
			class:filled={(recipient.amountValue ?? '') !== ''}
			data-field
			data-tone={recipient.amountNote ? 'error' : undefined}
		>
			<input
				class="entry amount"
				inputmode="decimal"
				autocomplete="off"
				aria-label={`${recipient.ordinal} · ${symbol ?? ''}`}
				aria-invalid={recipient.amountNote ? 'true' : undefined}
				aria-describedby={recipient.amountNote ? amountNoteId : undefined}
				placeholder="0"
				value={recipient.amountValue ?? ''}
				oninput={(event) => {
					// Cleaned by the core's rule before it goes on (spec 073): a
					// share typed "4,5" on a decimal-comma keypad is 4.5.
					if (composing(event)) return;
					const clean = takeAmount(event.currentTarget, recipient.amountValue ?? '', pasted(event));
					if (clean !== null) oninput({ amount: clean });
				}}
				oncompositionend={(event) => {
					const clean = takeAmount(event.currentTarget, recipient.amountValue ?? '', false);
					if (clean !== null) oninput({ amount: clean });
				}}
			/>
			{#if symbol !== undefined}<span class="symbol">{symbol}</span>{/if}
		</span>
	{:else}
		<span class="amount">{recipient.amount}</span>
	{/if}
	<button
		type="button"
		class="remove"
		style="grid-area: remove"
		aria-label={recipient.removeLabel}
		onclick={onremove}
	>
		<Icon icon={UTILITY_ICONS.x} size="md" />
	</button>
</div>

<style>
	.card {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto auto;
		grid-template-areas: 'avatar text amount remove';
		gap: var(--space-md) var(--space-lg);
		align-items: center;
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}
	.card :global(.identicon),
	.card > :first-child {
		grid-area: avatar;
	}
	.seat {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border: var(--border-hairline) dashed var(--color-border-strong);
		border-radius: var(--radius-full);
		color: var(--color-fg-subtle);
	}
	.text {
		grid-area: text;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}
	.ordinal {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	/* No name: the short address is who, and an address is set in mono. */
	.name.mono {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
	}
	.short {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	/* The repeat warning: beside the row it is about, in the colour the app
	   uses for "look at this", never the refusal red — nothing is refused. */
	.duplicate {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-warning-base);
	}
	/* Something IS in the field and the core cannot use it. The refusal colour,
	   on the sentence and on the field's own edge (`data-tone`) — never on an
	   empty field, which is only unfinished. */
	.wrong {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
	.well[data-tone='error'] {
		outline: var(--border-hairline) solid var(--color-error-base);
		outline-offset: calc(var(--border-hairline) * -1);
	}
	.amount {
		grid-area: amount;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
		text-align: end;
	}

	/*
	 * A field is a well: a box with its own corners, so the app's one focus
	 * edge (app.css, `data-field`) has a shape to follow. These were hairline
	 * underlines wearing that attribute, and focus drew a hard rectangle round
	 * a line — the "stray box" in the founder's screenshot.
	 *
	 * The well is only SEEN while it is wanted: empty, hovered, or in hand. A
	 * filled row at rest sits on the card like the drawn one.
	 */
	.well {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		min-width: 0;
		border-radius: var(--radius-md);
		background: var(--color-bg-base);
		transition: background-color var(--motion-duration-fast) ease-out;
	}
	.well.filled:not(:hover, :focus-within) {
		background: none;
	}
	.address-well {
		margin-inline-start: calc(var(--space-md) * -1);
		padding-inline: var(--space-md) var(--space-xs);
	}
	.amount-well {
		grid-area: amount;
		justify-content: flex-end;
		padding-inline: var(--space-md);
	}
	.entry {
		/* The split form pins its total and Continue to the bottom; a field
		   brought into view by focus stops above them, not underneath. */
		scroll-margin-block-end: calc(var(--size-control-lg) * 3);
		min-width: 0;
		border: none;
		background: none;
		padding: var(--space-md) 0;
		color: var(--color-fg-base);
	}
	.entry::placeholder {
		color: var(--color-fg-subtle);
	}
	.entry:focus {
		outline: none;
	}
	.address {
		flex: 1;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-overflow: ellipsis;
	}
	/*
	 * The reading of a filled address — name over short address — laid over
	 * the input while it is at rest. The input keeps its characters (a screen
	 * reader, a test and a paste all still find them); they only turn clear,
	 * and come back the moment the field is in hand.
	 */
	.reading {
		position: absolute;
		inset-block: 0;
		/* At rest the book's door is away, so the reading has the whole field;
		   under the pointer the door comes back and the reading makes room. */
		inset-inline: var(--space-md) var(--space-xs);
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: var(--space-xs);
		min-width: 0;
		pointer-events: none;
	}
	.address-well:not(:focus-within) .address:not(:placeholder-shown) {
		color: transparent;
	}
	.address-well:focus-within .reading {
		display: none;
	}
	.address-well:hover .reading {
		inset-inline-end: var(--icon-2xl);
	}
	/* A named row is two lines tall, in hand or not, so taking it up does not
	   make the list jump. */
	.address-well.named {
		min-height: calc(
			(var(--text-base) + var(--text-xs)) * var(--text-scale, 1) * var(--leading-normal) +
				var(--space-md)
		);
	}
	.entry.amount {
		/* Sized to its content, never narrower than a short amount and never
		   wider than a third of the card. */
		field-sizing: content;
		min-width: 3ch;
		max-width: 12ch;
		text-align: end;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}
	.symbol {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
	.pick,
	.remove {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
		transition: transform var(--motion-duration-fast) ease-out;
	}
	/* The book's door belongs to a field that is asking: a filled row at rest
	   has its person, and the door comes back with the hand. */
	.address-well.filled:not(:hover, :focus-within) .pick {
		opacity: 0;
		pointer-events: none;
	}

	.pick:hover,
	.remove:hover {
		color: var(--color-fg-base);
	}

	.pick:active,
	.remove:active {
		transform: scale(var(--motion-press-fab));
	}

	@media (prefers-reduced-motion: reduce) {
		.well,
		.pick,
		.remove {
			transition: none;
		}
	}
</style>
