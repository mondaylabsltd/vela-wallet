/**
 * An amount `<input>`, made readable for the core on every edit (issue 231;
 * spec 073 for every field that takes an amount).
 *
 * The rule is the core's (`l10n::amount_text`): a decimal-comma keypad's
 * "4,5" is 4.5, a pasted "1.234,56" is 1234.56, a pasted "1.5e-7" is refused
 * whole. Every field that takes an amount — the send figure, a split row's
 * share, a custom allowance — runs it here, so a cap typed as "4,5" cannot
 * reach a parser that drops the comma and allows 45.
 *
 * The cleaned text is written back into the field, with the caret kept where
 * the person left it, so what is on screen is what was sent on.
 */
import { amountTextCaret, amountTextClean } from '$lib/core/kernels';
import { resolvedFormatKeys } from '$lib/services/locale-format';

/**
 * Clean `el`'s text in place. Returns the figure to send on, or `null` when a
 * paste had no reading as one figure — the field then shows `previous` again
 * and nothing is sent: an amount of money is refused whole, never salvaged
 * into a different one.
 */
export function takeAmount(el: HTMLInputElement, previous: string, pasted: boolean): string | null {
	const raw = el.value;
	const clean = amountTextClean(raw, resolvedFormatKeys().number, pasted, previous);
	if (clean === null) {
		el.value = previous;
		el.setSelectionRange(previous.length, previous.length);
		return null;
	}
	if (clean !== raw) {
		// Written back only when it differs, and with the caret kept: a dropped
		// character must not throw the person to the end of the figure.
		const caret = el.selectionStart;
		el.value = clean;
		if (caret !== null) {
			const at = amountTextCaret(raw, clean, caret);
			el.setSelectionRange(at, at);
		}
	}
	return clean;
}

/**
 * An IME is mid-word (a ja / zh / ko keyboard in full-width mode): writing to
 * the field now would break the composition, so the text is read when it ends.
 */
export function composing(event: Event): boolean {
	return 'isComposing' in event && (event as InputEvent).isComposing;
}

/** A paste or a drop, as the browser reports it — the core reads those whole. */
export function pasted(event: Event): boolean {
	const kind = 'inputType' in event ? (event as InputEvent).inputType : undefined;
	return kind === 'insertFromPaste' || kind === 'insertFromDrop';
}
