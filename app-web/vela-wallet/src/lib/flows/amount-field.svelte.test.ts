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
