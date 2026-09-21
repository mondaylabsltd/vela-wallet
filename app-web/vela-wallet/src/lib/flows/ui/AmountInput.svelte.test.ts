/**
 * The ⇄ denomination toggle (issue 197): the icon beneath the figure was
 * drawn on every send form and had nothing behind it — no handler on the
 * shells' side, and no read of the three `SendView.denom_toggle_*` fields
 * that say whether it should be there and whether it can act.
 *
 * A `.svelte.test.ts`: whether a real click reaches a real handler, and
 * whether a disabled control refuses it, is the browser's to answer.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { preferences } from '$lib/services/preferences.svelte';
import AmountInput from './AmountInput.svelte';

const BASE = { value: '0.5', fiat: '≈ $1,500.00', denomLabel: 'ETH' };

describe('AmountInput', () => {
	it('presses through to the core when the swap is on offer', async () => {
		const ondenom = vi.fn();
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: true }, ondenom }
		});
		const toggle = screen.container.querySelector('button.fiat');
		expect(toggle).not.toBeNull();
		expect((toggle as HTMLButtonElement).disabled).toBe(false);
		(toggle as HTMLButtonElement).click();
		expect(ondenom).toHaveBeenCalledTimes(1);
	});

	it('refuses the press, visibly, when the core says the swap would do nothing', async () => {
		const ondenom = vi.fn();
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: false }, ondenom }
		});
		const toggle = screen.container.querySelector('button.fiat') as HTMLButtonElement;
		expect(toggle.disabled).toBe(true);
		toggle.click();
		expect(ondenom).not.toHaveBeenCalled();
	});

	it('draws no control at all where the core offers no swap', () => {
		const screen = render(AmountInput, { props: BASE });
		expect(screen.container.querySelector('button.fiat')).toBeNull();
		// The other denomination is still worth reading — it is just not a door.
		expect(screen.container.querySelector('p.fiat')?.textContent).toBe('≈ $1,500.00');
	});

	it('names the button by the line it shows, not by the unit being left', () => {
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: true }, ondenom: () => {} }
		});
		const toggle = screen.container.querySelector('button.fiat') as HTMLButtonElement;
		expect(toggle.getAttribute('aria-label')).toBeNull();
		expect(toggle.textContent).toContain('≈ $1,500.00');
	});
});

/**
 * Issue 231: the hero read "4.00" and nothing said of what. The unit was known
 * — it reached `aria-label` and stopped there.
 *
 * In a browser because every claim here is about what is DRAWN and where: a
 * node test of the view-model stayed green through the original bug.
 */
describe('the unit on the figure', () => {
	const typedInto = (props: Record<string, unknown>) => {
		const screen = render(AmountInput, {
			props: { ...BASE, oninput: () => {}, ...props }
		});
		const input = screen.container.querySelector('input') as HTMLInputElement;
		return { screen, input, figure: screen.container.querySelector('.figure') as HTMLElement };
	};

	/** One keystroke, as the browser reports it — and NO echo from the core. */
	const type = async (input: HTMLInputElement, value: string, inputType = 'insertText') => {
		input.value = value;
		input.setSelectionRange(value.length, value.length);
		input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType }));
		await tick();
	};

	afterEach(() => {
		preferences.numberFormat = 'auto';
	});

	it('draws a currency symbol before the figure and a token after it', () => {
		const fiat = typedInto({ value: '4.00', denomLabel: 'USD', adornment: { prefix: '$' } });
		expect(fiat.figure.textContent?.replace(/\s+/g, '')).toMatch(/^\$/);
		expect(fiat.figure.querySelector('.prefix')?.textContent).toBe('$');

		const token = typedInto({ value: '0.00075', adornment: { suffix: 'BNB' } });
		expect(token.figure.querySelector('.suffix')?.textContent).toBe('BNB');
		// After the field, in reading order.
		const suffix = token.figure.querySelector('.suffix') as HTMLElement;
		expect(
			token.input.compareDocumentPosition(suffix) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
		expect(suffix.getBoundingClientRect().left).toBeGreaterThanOrEqual(
			token.input.getBoundingClientRect().right - 1
		);
	});

	it('shows the unit on the drawn figure too', () => {
		const screen = render(AmountInput, {
			props: { ...BASE, value: '120', adornment: { suffix: 'USDT' } }
		});
		expect(screen.container.querySelector('.figure')?.textContent).toContain('USDT');
	});

	it('says the unit once to a screen reader: in the name, not in the adornment', () => {
		const { input, figure } = typedInto({ denomLabel: 'USD', adornment: { prefix: '$' } });
		expect(input.getAttribute('aria-label')).toBe('USD');
		expect(input.getAttribute('inputmode')).toBe('decimal');
		expect(input.getAttribute('autocomplete')).toBe('off');
		expect(figure.querySelector('.prefix')?.getAttribute('aria-hidden')).toBe('true');
		expect(figure.querySelector('.mirror')?.getAttribute('aria-hidden')).toBe('true');
	});

	it('puts a tap on the unit into the field', () => {
		const { input, figure } = typedInto({ adornment: { suffix: 'BNB' } });
		expect(document.activeElement).not.toBe(input);
		(figure.querySelector('.suffix') as HTMLElement).click();
		expect(document.activeElement).toBe(input);
	});

	// A guard on the component's contract, not the regression test: the old
	// component passed this too. The "0" that stood in for an empty amount was
	// `live-send.ts`'s, and `live-send.test.ts` is what pins its removal.
	it('is empty when nothing is typed, and says 0 as a placeholder', async () => {
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', placeholder: '0', oninput });
		expect(input.value).toBe('');
		expect(input.placeholder).toBe('0');
		await type(input, '4');
		expect(oninput).toHaveBeenLastCalledWith('4');
	});

	it('makes room for a keystroke before the core has echoed it', async () => {
		// The `value` prop is never updated in this test: that IS the interval
		// between the keystroke and the echo. A field sized from the prop is one
		// character short for exactly that long.
		const { input } = typedInto({ value: '4.0', adornment: { prefix: '$' } });
		const before = input.clientWidth;
		await type(input, '4.00');
		expect(input.clientWidth).toBeGreaterThan(before);
		expect(input.scrollWidth).toBeLessThanOrEqual(input.clientWidth);
		await type(input, '4.0012345');
		expect(input.scrollWidth).toBeLessThanOrEqual(input.clientWidth);
	});

	it('keeps an 18-decimal figure inside a narrow column', async () => {
		const { screen, figure, input } = typedInto({
			value: '0.000750000000000001',
			adornment: { suffix: 'BNB' }
		});
		screen.container.style.width = '300px';
		await tick();
		const column = screen.container.getBoundingClientRect();
		const box = figure.getBoundingClientRect();
		expect(figure.dataset.size).toBe('tight');
		expect(box.left).toBeGreaterThanOrEqual(column.left - 0.5);
		expect(box.right).toBeLessThanOrEqual(column.right + 0.5);
		// The unit is still on screen — it is the digits that scroll.
		const suffix = (figure.querySelector('.suffix') as HTMLElement).getBoundingClientRect();
		expect(suffix.right).toBeLessThanOrEqual(column.right + 0.5);
		expect(input.getBoundingClientRect().width).toBeGreaterThan(0);
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});

	it('steps down the ladder as the figure grows', async () => {
		const { input, figure } = typedInto({ value: '4', adornment: { prefix: '$' } });
		expect(figure.dataset.size).toBe('hero');
		const hero = parseFloat(getComputedStyle(input).fontSize);
		await type(input, '1234.5678');
		expect(figure.dataset.size).toBe('compact');
		const compact = parseFloat(getComputedStyle(input).fontSize);
		expect(compact).toBeLessThan(hero);
		// The mirror that sizes the field is set in the very same type.
		const mirror = figure.querySelector('.mirror') as HTMLElement;
		for (const property of ['fontSize', 'fontFamily', 'fontWeight', 'letterSpacing'] as const) {
			expect(getComputedStyle(mirror)[property], property).toBe(getComputedStyle(input)[property]);
		}
	});

	it('reads a decimal comma as a decimal, where that is the preset', async () => {
		// The core parses fiat like parseFloat: "4,5" would go out as 4.
		preferences.numberFormat = 'dot_comma';
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', oninput });
		await type(input, '4,5');
		expect(oninput).toHaveBeenLastCalledWith('4.5');
		// Written back, so the screen shows what was sent on…
		expect(input.value).toBe('4.5');
		// …and the next digit lands in that figure, not in a regrouped one.
		await type(input, '4.56');
		expect(oninput).toHaveBeenLastCalledWith('4.56');
	});

	it('drops a grouping comma, never mapping it onto the decimal point', async () => {
		preferences.numberFormat = 'comma_dot';
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', oninput });
		await type(input, '1,234.5', 'insertFromPaste');
		expect(oninput).toHaveBeenLastCalledWith('1234.5');
		expect(input.value).toBe('1234.5');
	});

	it('reads one typed comma as the decimal mark under any preset, and a pasted one as grouping', async () => {
		// A decimal pad follows the device's region and the preset follows the
		// browser's language: where they disagree "," is the pad's only
		// separator, and dropping it made "4,5" into 45.
		preferences.numberFormat = 'comma_dot';
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', oninput });
		await type(input, '4');
		await type(input, '4,');
		expect(input.value).toBe('4.');
		await type(input, '4.5');
		expect(oninput).toHaveBeenLastCalledWith('4.5');
		// Arriving whole it is not a keystroke, whatever the keyboard calls it.
		await type(input, '');
		await type(input, '1,234');
		expect(oninput).toHaveBeenLastCalledWith('1234');
	});

	it('refuses a paste that is not one figure, and keeps what the field had', async () => {
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', oninput });
		await type(input, '2');
		oninput.mockClear();
		// How a tiny balance is printed on many screens; salvaged, it is 1.57.
		for (const paste of ['1.5e-7', '0x10', '4.5.6']) {
			await type(input, paste, 'insertFromPaste');
			expect(input.value, paste).toBe('2');
		}
		expect(oninput).not.toHaveBeenCalled();
	});

	it('leaves the caret where the person was when a character is dropped', async () => {
		preferences.numberFormat = 'dot_comma';
		const { input } = typedInto({ value: '12.5' });
		// A stray "," typed after the 1. Cleaned on its own, "1," reads as
		// "1." and the caret landed after the 2.
		input.value = '1,2.5';
		input.setSelectionRange(2, 2);
		input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
		await tick();
		expect(input.value).toBe('12.5');
		expect(input.selectionStart).toBe(1);
	});

	it('does not write into the field while an IME is composing, and reads it when it ends', async () => {
		const oninput = vi.fn();
		const { input } = typedInto({ value: '', oninput });
		input.value = '４．５';
		input.dispatchEvent(
			new InputEvent('input', {
				bubbles: true,
				inputType: 'insertCompositionText',
				isComposing: true
			})
		);
		await tick();
		expect(input.value).toBe('４．５');
		expect(oninput).not.toHaveBeenCalled();
		// Still sized for what it holds.
		expect(input.scrollWidth).toBeLessThanOrEqual(input.clientWidth);
		input.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '４．５' }));
		await tick();
		expect(input.value).toBe('4.5');
		expect(oninput).toHaveBeenLastCalledWith('4.5');
	});

	it('is one height on every rung, so nothing beneath it moves as the figure grows', async () => {
		const { screen, input } = typedInto({ value: '4', adornment: { prefix: '$' } });
		const block = screen.container.querySelector('.amount') as HTMLElement;
		const hero = block.getBoundingClientRect().height;
		await type(input, '135000.00');
		expect(block.getBoundingClientRect().height).toBeCloseTo(hero, 1);
		await type(input, '0.000750000000000001');
		expect(block.getBoundingClientRect().height).toBeCloseTo(hero, 1);
		// …and with a quieter, smaller suffix beside it instead of a symbol.
		for (const value of ['4', '0.00075123', '0.000750000000000001']) {
			const token = typedInto({ value, adornment: { suffix: 'BNB' } });
			const height = (
				token.screen.container.querySelector('.amount') as HTMLElement
			).getBoundingClientRect().height;
			expect(height, value).toBeCloseTo(hero, 1);
		}
	});

	it('follows the core when the figure is set from outside — Max, the swap', async () => {
		const { screen, input } = typedInto({ value: '4' });
		await type(input, '45');
		await screen.rerender({ value: '0.00075' });
		expect(input.value).toBe('0.00075');
		expect(input.scrollWidth).toBeLessThanOrEqual(input.clientWidth);
	});
});
