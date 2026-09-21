<script lang="ts">
	/**
	 * SD2's amount (spec 021 component 8): the number, big and centred, with
	 * its fiat equivalent and the toggle that swaps which of the two you type.
	 *
	 * The figure is the largest type on the screen because it is the one thing
	 * the person came to decide. The fiat line stays subordinate even when the
	 * denominations swap — the amount being ENTERED leads, whichever it is.
	 */
	import { numberSeparators } from '$lib/services/locale-format';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { caretAfterClean, cleanAmountText } from '../amount-text';

	interface Props {
		value: string;
		/** The OTHER denomination — money while tokens are typed, and back. */
		fiat: string;
		denomLabel: string;
		/**
		 * Present ⇒ the core offers the ⇄ swap (`denom_toggle_shown`), and
		 * `enabled` is whether pressing it would change anything
		 * (`denom_toggle_enabled` — entering fiat needs a rate). Absent, the
		 * line below the figure is text: issue 197 was this control drawn
		 * unconditionally, with nothing behind it in either direction.
		 */
		denomToggle?: { enabled: boolean };
		ondenom?: () => void;
		/**
		 * Present ⇒ the figure is TYPED here (spec 026). The gallery passes
		 * nothing and keeps the drawn picture, exactly as the balance hero's
		 * tap-to-hide does: an affordance appears only where something is
		 * listening for it.
		 */
		oninput?: (value: string) => void;
		placeholder?: string;
		/**
		 * The figure's unit, drawn ON the figure (issue 231): a currency symbol
		 * leads ("$4.00"), a currency code or a token symbol follows, quieter
		 * ("4.00 PLN", "0.00075 BNB"). The hero read "4.00" and left a person to
		 * work out from the line beneath whether that was dollars or coins.
		 */
		adornment?: { prefix?: string; suffix?: string };
	}

	let {
		value,
		fiat,
		denomLabel,
		denomToggle,
		ondenom,
		oninput,
		placeholder = '0',
		adornment = {}
	}: Props = $props();

	/**
	 * What the field holds RIGHT NOW. It follows the `value` prop — Max, the ⇄
	 * swap and the core's echo all arrive that way — and is overwritten in the
	 * input handler, in the same tick as the keystroke.
	 *
	 * It is not the prop itself because the prop comes back only after the
	 * round trip through the core, and the field is sized from this text: sized
	 * from the prop, the input is one character too narrow for exactly as long
	 * as the newest digit is new, and that digit is drawn clipped.
	 *
	 * Safe because dispatch is synchronous and the core stores the text
	 * verbatim, so the echo IS what was just typed. If either changes, a late
	 * echo must not be allowed to overwrite newer local text.
	 */
	let text = $derived(value);

	const shown = $derived(oninput ? text : value);

	/**
	 * The hero ladder, by how much there is to draw: 46 / 38 / 31, the same
	 * three rungs the Welcome headline steps down. Counted in characters
	 * because nothing here measures the DOM — the width itself comes from CSS
	 * (the mirror below). The quieter suffix is set smaller, so it counts half.
	 * Past the last rung a figure scrolls inside its own field.
	 */
	const size = $derived.by(() => {
		const drawn =
			(shown || placeholder).length +
			(adornment.prefix?.length ?? 0) +
			(adornment.suffix?.length ?? 0) / 2;
		return drawn <= 8 ? 'hero' : drawn <= 11 ? 'compact' : 'tight';
	});

	/**
	 * Made readable for the core before it is sent on — `amount-text.ts` says
	 * why a decimal comma cannot be left to it.
	 */
	function take(el: HTMLInputElement, pasted: boolean) {
		const raw = el.value;
		const clean = cleanAmountText(raw, numberSeparators().decimal, pasted, text);
		if (clean === null) {
			// A paste with no reading as ONE figure ("1.5e-7", "4.5.6"). The
			// field keeps what it had and nothing is sent on: an amount of
			// money is refused whole, never salvaged into a different one.
			el.value = text;
			el.setSelectionRange(text.length, text.length);
			return;
		}
		if (clean !== raw) {
			// Written back only when it differs, and with the caret kept where
			// the person left it: a dropped character must not throw them to the
			// end of the figure.
			const caret = el.selectionStart;
			el.value = clean;
			if (caret !== null) {
				const at = caretAfterClean(raw, clean, caret);
				el.setSelectionRange(at, at);
			}
		}
		text = clean;
		oninput?.(clean);
	}

	function typed(event: Event & { currentTarget: HTMLInputElement }) {
		const el = event.currentTarget;
		if ('isComposing' in event && event.isComposing) {
			// An IME is mid-word (a ja / zh / ko keyboard in full-width mode):
			// writing to the field now would break the composition. The field
			// is still SIZED for what it holds; the text is read, cleaned and
			// sent on when the composition ends.
			text = el.value;
			return;
		}
		const kind = 'inputType' in event ? event.inputType : undefined;
		take(el, kind === 'insertFromPaste' || kind === 'insertFromDrop');
	}
</script>

<div class="amount">
	{#if oninput}
		<!-- A <label>, so a tap on the "$" or the "BNB" lands in the field like a
		     tap on the digits does. The adornments are hidden from assistive
		     tech because the input's own name already says the unit — read
		     twice, "USD, dollar sign, 4.00" is noise. -->
		<label class="figure" data-size={size}>
			{#if adornment.prefix}
				<span class="unit prefix" aria-hidden="true">{adornment.prefix}</span>
			{/if}
			<!-- The field is as wide as what is in it, with no measuring: the
			     hidden mirror holds the same text in the same type and sets the
			     cell's width, and the input fills the cell. CSS re-lays it out by
			     itself when the numeric font swaps in; a JS measurement would
			     have to be told. -->
			<span class="sizer">
				<span class="value mirror" aria-hidden="true">{text || placeholder}</span>
				<input
					class="value entry"
					inputmode="decimal"
					autocomplete="off"
					aria-label={denomLabel}
					{placeholder}
					value={text}
					oninput={typed}
					oncompositionend={(event) => take(event.currentTarget, false)}
				/>
			</span>
			{#if adornment.suffix}
				<span class="unit suffix" aria-hidden="true">{adornment.suffix}</span>
			{/if}
		</label>
	{:else}
		<div class="figure" data-size={size}>
			{#if adornment.prefix}
				<span class="unit prefix">{adornment.prefix}</span>
			{/if}
			<p class="value">{value || placeholder}</p>
			{#if adornment.suffix}
				<span class="unit suffix">{adornment.suffix}</span>
			{/if}
		</div>
	{/if}
	{#if denomToggle}
		<!-- Named by what it switches TO, which is the line it shows. The old
		     `aria-label` said the unit already being typed — a two-state
		     control announced by the state you are leaving. -->
		<button type="button" class="fiat" onclick={ondenom} disabled={!denomToggle.enabled}>
			<span>{fiat}</span>
			<Icon icon={UTILITY_ICONS['chevrons-up-down']} size="sm" />
		</button>
	{:else if fiat}
		<p class="fiat">{fiat}</p>
	{/if}
</div>

<style>
	.amount {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-sm);
		padding-block: var(--space-3xl);
	}

	/* The figure and its unit, on one baseline. The size lives HERE and is
	   inherited, so the adornments step down the ladder with the digits. It
	   may shrink below its content (`min-width: 0`): an 18-decimal Max then
	   scrolls inside the field instead of clipping against the column.

   The LINE is the hero rung's on every rung — a length, not a ratio — so
   the block is one height whatever is typed into it. As a ratio it shrank
   with the type at the 9th and the 12th character, and on Max and the swap,
   and the recipient, the fee row and Continue all jumped up under a finger
   that was reaching for them. */
	.figure {
		display: flex;
		align-items: baseline;
		justify-content: center;
		max-width: 100%;
		min-width: 0;
		margin: 0;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-hero) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: calc(var(--text-hero) * var(--text-scale, 1) * var(--leading-tight));
		cursor: text;
	}

	div.figure {
		cursor: default;
	}

	.figure[data-size='compact'] {
		font-size: calc(var(--text-heroCompact) * var(--text-scale, 1));
	}

	.figure[data-size='tight'] {
		font-size: calc(var(--text-heroTight) * var(--text-scale, 1));
	}

	/* The mirror and the input MUST set type identically — they share this
	   class and nothing else touches their font — or the text and the width
	   made for it drift apart and the last digit clips. */
	.value {
		margin: 0;
		font: inherit;
		font-variant-numeric: tabular-nums;
		letter-spacing: inherit;
		color: var(--color-fg-base);
	}

	/* The drawn figure behaves as the typed one does at rest: one line, and
	   past the last rung it is cut at the END, inside its own box — with an
	   ellipsis, so the cut SAYS it is one. A glyph sliced down the middle is
	   what the report's "4.0C" looked like, and nobody should have to wonder
	   whether that is a digit. */
	p.value {
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}

	.unit {
		flex: none;
		color: var(--color-fg-muted);
	}

	/* A code or a ticker is a word beside a number: quieter, smaller, and
	   lighter, so "0.00075 BNB" still reads as the number first — and set off
	   by a space, where a symbol sits tight against its digits ("$4.00"). */
	.suffix {
		margin-inline-start: var(--space-sm);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		/* Its own tight line: in the figure's tall one, the smaller type's
		   baseline sits higher, and lining it up with the digits would push
		   the row taller by a different amount on every rung. */
		line-height: var(--leading-none);
	}

	/* …and down the ladder with the figure, a rung of the type scale each. */
	.figure[data-size='compact'] .suffix {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
	}

	.figure[data-size='tight'] .suffix {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
	}

	.sizer {
		display: inline-grid;
		grid-template-columns: minmax(0, 1fr);
		flex: 0 1 auto;
		min-width: 0;
		overflow: hidden;
	}

	.mirror,
	.entry {
		grid-area: 1 / 1;
	}

	.mirror {
		visibility: hidden;
		white-space: pre;
		/* Room for the caret after the last digit, so it is never the thing
		   that overflows. */
		padding-inline-end: var(--space-sm);
	}

	/* `width: 0` and not `100%`: a percentage counts as `auto` while the cell
	   is being measured, and an input's `auto` is its twenty-character default
	   — the field would set the cell's width instead of the mirror. Zero takes
	   it out of the measurement; `min-width` then fills the cell it got. */
	.entry {
		width: 0;
		min-width: 100%;
		border: none;
		background: none;
		/* From the start, so the caret's slack is all at the END and a leading
		   "$" sits against its digits. The figure as a whole is centred by
		   `.figure`; the cell is exactly as wide as its text, so there is
		   nothing left in here to centre. */
		text-align: start;
		padding: 0;
		/* At rest only; a focused field scrolls to its caret instead. */
		text-overflow: ellipsis;
	}

	.entry::placeholder {
		color: var(--color-fg-muted);
		opacity: 1;
	}

	.entry:focus {
		outline: none;
	}

	.fiat {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		margin: 0;
		padding: var(--space-xs) var(--space-sm);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.fiat:hover:not(:disabled) {
		color: var(--color-fg-base);
	}

	/* The core's refusal, made visible: no rate to enter this currency
	   against. The sentence that says WHY rides the form's one notice line
	   (`denom_toggle_reason`), the same slot the desktop puts it in. */
	.fiat:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	p.fiat {
		cursor: default;
	}
</style>
