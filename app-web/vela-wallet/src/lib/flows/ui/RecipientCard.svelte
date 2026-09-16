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
	 * A row the core has flagged as a repeat of an earlier recipient carries that
	 * sentence under its address (issue 203): the same address could take two
	 * lines of one batch with nothing on screen saying so, and the batch is sent
	 * exactly as asked — so this warns, beside the row it is about, and refuses
	 * nothing.
	 */
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
</script>

<div class="card" class:editable={oninput !== undefined}>
	<Identicon svg={recipient.identiconSvg} size="row" address={recipient.address || undefined} />
	<span class="text">
		<span class="ordinal">{recipient.ordinal}</span>
		{#if oninput}
			<span class="entry-row" data-field>
				<input
					class="entry address"
					spellcheck="false"
					autocomplete="off"
					aria-label={`${recipient.ordinal} · ${recipient.addressLabel ?? ''}`}
					aria-describedby={recipient.duplicateNote ? duplicateId : undefined}
					value={recipient.address}
					oninput={(event) => oninput({ address: event.currentTarget.value })}
				/>
				{#if onpick}
					<button type="button" class="pick" aria-label={recipient.pickLabel} onclick={onpick}>
						<Icon icon={UTILITY_ICONS['user-round']} size="sm" />
					</button>
				{/if}
			</span>
		{:else}
			<span class="name">{recipient.name}</span>
		{/if}
		{#if recipient.duplicateNote}
			<span class="duplicate" id={duplicateId}>{recipient.duplicateNote}</span>
		{/if}
	</span>
	{#if oninput}
		<span class="amount-entry" data-field>
			<input
				class="entry amount"
				inputmode="decimal"
				autocomplete="off"
				aria-label={`${recipient.ordinal} · ${symbol ?? ''}`}
				placeholder="0"
				value={recipient.amountValue ?? ''}
				oninput={(event) => oninput({ amount: event.currentTarget.value })}
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
	/*
	 * Spec 038 #E7. Two lines per card when editable — ordinal, then the
	 * address across the card's full width — with the amount sized to what it
	 * holds and right-aligned beside its symbol. Before this the amount was a
	 * fixed six-space box and the address shared its line with the pick
	 * button, so at four recipients the address was cut to "0x3187ł" and the
	 * amount read as a stray box.
	 */
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
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
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
	.amount {
		grid-area: amount;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
		text-align: end;
	}
	/* The fields sit on the card's own surface: the card is the field, and a
	   sunken box inside a raised one would be a well inside a well. A
	   hairline underneath says "type here" without a browser border. */
	.entry-row,
	.amount-entry {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		min-width: 0;
		border-block-end: var(--border-hairline) solid var(--color-border-base);
	}
	.entry-row:focus-within,
	.amount-entry:focus-within {
		border-block-end-color: var(--color-fg-base);
	}
	.entry {
		min-width: 0;
		border: none;
		background: none;
		padding: var(--space-xs) 0;
		color: var(--color-fg-base);
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
	.amount-entry {
		grid-area: amount;
		justify-content: flex-end;
	}
	.entry.amount {
		/* Sized to its content, never narrower than a short amount and never
		   wider than a third of the card. */
		field-sizing: content;
		min-width: 6ch;
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
	}

	.pick:hover,
	.remove:hover {
		color: var(--color-fg-base);
	}
</style>
