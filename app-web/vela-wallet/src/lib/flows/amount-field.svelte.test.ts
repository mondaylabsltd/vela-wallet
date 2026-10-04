/**
 * Spec 073: every field that takes an amount runs the core's rule, not just
 * the send figure. A custom allowance's parser drops every comma, so a cap
 * typed "4,5" on a decimal-comma keypad went out as 45 — ten times what was
 * typed — and a split row's "4,5" was refused only when the call was built.
 *
 * A `.svelte.test.ts`: what the real input shows after a real input event.
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { preferences } from '$lib/services/preferences.svelte';
import AllowanceEditor from '$lib/signing/ui/AllowanceEditor.svelte';
import AmountInput from './ui/AmountInput.svelte';
import RecipientCard from './ui/RecipientCard.svelte';

beforeAll(() => loadCore());

afterEach(() => {
	preferences.numberFormat = 'auto';
});

/** One edit, as the browser reports it — and NO echo from the core. */
async function type(input: HTMLInputElement, value: string, inputType = 'insertText') {
	input.value = value;
	input.setSelectionRange(value.length, value.length);
	input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType }));
	await tick();
}

describe('the custom allowance', () => {
	const editor = (value: string, oncustom: (text: string) => void) => {
		const screen = render(AllowanceEditor, {
			props: {
				label: 'Allowance',
				value: '4.5 USDC',
				valueTone: 'neutral',
				chips: [],
				custom: { value, symbol: 'USDC', placeholder: '0' },
				oncustom
			}
		});
		return screen.container.querySelector('input') as HTMLInputElement;
	};

	it('reads a decimal comma as the decimal mark — never as ten times the cap', async () => {
		preferences.numberFormat = 'dot_comma';
		const oncustom = vi.fn();
		const input = editor('', oncustom);
		await type(input, '4,5');
		expect(oncustom).toHaveBeenLastCalledWith('4.5');
		expect(input.value).toBe('4.5');
	});

	it('reads one typed comma as the decimal mark under a decimal-point preset too', async () => {
		preferences.numberFormat = 'comma_dot';
		const oncustom = vi.fn();
		const input = editor('4', oncustom);
		await type(input, '4,');
		expect(oncustom).toHaveBeenLastCalledWith('4.');
	});

	it("refuses a comma it cannot read — never 150 for somebody's 1,50", async () => {
		// A decimal-point preset, and a figure pasted from a decimal-comma
		// writer: dropped as grouping it was a cap a hundred times too high.
		preferences.numberFormat = 'comma_dot';
		const oncustom = vi.fn();
		const input = editor('2', oncustom);
		await type(input, '1,50', 'insertFromPaste');
		expect(oncustom).not.toHaveBeenCalled();
		expect(input.value).toBe('2');
	});

	it('refuses a paste that is not one figure, and keeps the cap it had', async () => {
		const oncustom = vi.fn();
		const input = editor('2', oncustom);
		await type(input, '1.5e-7', 'insertFromPaste');
		expect(oncustom).not.toHaveBeenCalled();
		expect(input.value).toBe('2');
	});
});

describe("a split row's share", () => {
	const row = (amountValue: string, oninput: (patch: { amount?: string }) => void) => {
		const screen = render(RecipientCard, {
			props: {
				recipient: {
					ordinal: '#1',
					name: '',
					address: '',
					identiconSvg: '',
					amount: '',
					amountValue,
					removeLabel: 'Remove'
				},
				symbol: 'USDC',
				oninput
			}
		});
		return screen.container.querySelector('input.amount') as HTMLInputElement;
	};

	it('reads a decimal comma as the decimal mark, and says so in the field', async () => {
		preferences.numberFormat = 'space_comma';
		const oninput = vi.fn();
		const input = row('', oninput);
		await type(input, '4,5');
		expect(oninput).toHaveBeenLastCalledWith({ amount: '4.5' });
		expect(input.value).toBe('4.5');
	});

	it('reads a pasted grouped figure for what it is', async () => {
		preferences.numberFormat = 'comma_dot';
		const oninput = vi.fn();
		const input = row('', oninput);
		await type(input, '1.234,56', 'insertFromPaste');
		expect(oninput).toHaveBeenLastCalledWith({ amount: '1234.56' });
	});
});

/**
 * Issue #421: a "0" then an "8" left "08" in the field — read as 8, ten times
 * what somebody who missed the point meant — with Continue lit. The key
 * replaces the zero, in every field that takes an amount; only a decimal mark
 * may follow a leading zero, and a bare mark is given the zero it reads with.
 */
describe('a zero leading a digit (issue #421)', () => {
	/** A key at the caret, as the browser applies it, then the field's handler. */
	async function key(input: HTMLInputElement, ch: string) {
		const at = input.selectionStart ?? input.value.length;
		const next = input.value.slice(0, at) + ch + input.value.slice(at);
		input.value = next;
		input.setSelectionRange(at + 1, at + 1);
		input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
		await tick();
	}

	const figure = (oninput: (value: string) => void) => {
		const screen = render(AmountInput, {
			props: { value: '', fiat: '≈ $0.00', denomLabel: 'POL', oninput }
		});
		return screen.container.querySelector('input.entry') as HTMLInputElement;
	};

	it('the send figure: "0" then "8" is 8 on screen and 8 sent on', async () => {
		const oninput = vi.fn();
		const input = figure(oninput);
		await key(input, '0');
		await key(input, '8');
		expect(input.value).toBe('8');
		expect(oninput).toHaveBeenLastCalledWith('8');
	});

	it('the send figure: a mark after the zero is how 0.8 is written', async () => {
		const oninput = vi.fn();
		const input = figure(oninput);
		for (const ch of '0.08') await key(input, ch);
		expect(input.value).toBe('0.08');
		expect(oninput).toHaveBeenLastCalledWith('0.08');
	});

	it('the send figure: a bare mark reads "0." and the next key lands after it', async () => {
		const oninput = vi.fn();
		const input = figure(oninput);
		await key(input, '.');
		expect(input.value).toBe('0.');
		expect(input.selectionStart).toBe(2);
		await key(input, '5');
		expect(input.value).toBe('0.5');
		expect(oninput).toHaveBeenLastCalledWith('0.5');
	});

	it("a decimal-comma person's 0,8 is 0.8, never 8", async () => {
		preferences.numberFormat = 'dot_comma';
		const oninput = vi.fn();
		const input = figure(oninput);
		for (const ch of '0,8') await key(input, ch);
		expect(oninput).toHaveBeenLastCalledWith('0.8');
	});

	it('a pasted 008.5 is 8.5', async () => {
		const oninput = vi.fn();
		const input = figure(oninput);
		await type(input, '008.5', 'insertFromPaste');
		expect(input.value).toBe('8.5');
		expect(oninput).toHaveBeenLastCalledWith('8.5');
	});

	it("a split row's share and the custom allowance take the same rule", async () => {
		const onrow = vi.fn();
		const row = render(RecipientCard, {
			props: {
				recipient: {
					ordinal: '#1',
					name: '',
					address: '',
					identiconSvg: '',
					amount: '',
					amountValue: '0',
					removeLabel: 'Remove'
				},
				symbol: 'POL',
				oninput: onrow
			}
		}).container.querySelector('input.amount') as HTMLInputElement;
		await type(row, '08');
		expect(row.value).toBe('8');
		expect(onrow).toHaveBeenLastCalledWith({ amount: '8' });

		const oncustom = vi.fn();
		const cap = render(AllowanceEditor, {
			props: {
				label: 'Allowance',
				value: '0 USDC',
				valueTone: 'neutral',
				chips: [],
				custom: { value: '0', symbol: 'USDC', placeholder: '0' },
				oncustom
			}
		}).container.querySelector('input') as HTMLInputElement;
		await type(cap, '00');
		expect(cap.value).toBe('0');
		await type(cap, '08');
		expect(cap.value).toBe('8');
		expect(oncustom).toHaveBeenLastCalledWith('8');
	});
});
