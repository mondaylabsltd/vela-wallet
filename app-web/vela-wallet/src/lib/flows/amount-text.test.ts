/**
 * The send amount, made readable for the core before it is sent on (issue 231).
 *
 * The core parses a fiat figure the way `parseFloat` does, so "4,5" is 4 and a
 * different sum goes out. These are the cases where cleaning the text could
 * itself move a figure — which is the only kind of bug worth having here.
 *
 * The rule is the core's since spec 073 (`l10n::amount_text`, which carries
 * the same cases); this suite runs them through the wasm the web ships, with
 * the web's own arguments — the preset key and `inputType`'s paste flag.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { amountTextCaret as caretAfterClean, amountTextClean } from '$lib/core/kernels';

/** The suite's old spelling: the preset's decimal mark. */
const cleanAmountText = (raw: string, decimal: string, pasted = false, previous?: string) =>
	amountTextClean(raw, decimal === ',' ? 'dot_comma' : 'comma_dot', pasted, previous);

describe('a decimal-comma preset', () => {
	const clean = (raw: string, pasted = false) => cleanAmountText(raw, ',', pasted);

	it('reads a typed comma as the decimal mark', () => {
		expect(clean('4,5')).toBe('4.5');
		expect(clean('4,')).toBe('4.');
		expect(clean(',5')).toBe('.5');
	});

	it("keeps the field's own dot-decimal text intact on the next keystroke", () => {
		// The core echoes "4.5"; Max writes "0.00075". Dropping the dot as
		// "grouping" would make 4.56 into 456 one keystroke after it was right.
		expect(clean('4.56')).toBe('4.56');
		expect(clean('0.000751')).toBe('0.000751');
	});

	it('drops a second mark rather than re-reading the figure around it', () => {
		expect(clean('4.5,')).toBe('4.5');
		expect(clean('4,5,6')).toBe('4.56');
		// Typed into the middle of 1.23456 — NOT one thousand two hundred.
		expect(clean('1.234,56')).toBe('1.23456');
	});

	it('reads a pasted grouped figure for what it is', () => {
		expect(clean('1.234,56', true)).toBe('1234.56');
		expect(clean('1.234.567', true)).toBe('1234567');
		expect(clean('1 234,56', true)).toBe('1234.56');
		// Written the other way round, it is still unmistakable.
		expect(clean('1,234.56', true)).toBe('1234.56');
	});
});

describe('a decimal-point preset', () => {
	const clean = (raw: string, pasted = false) => cleanAmountText(raw, '.', pasted);

	it('drops commas as grouping, never mapping them onto the decimal point', () => {
		// The blind `,` → `.` would make this 1.234.
		expect(clean('1,234.56')).toBe('1234.56');
		expect(clean('1,234.5')).toBe('1234.5');
		expect(clean('12,34,567.89')).toBe('1234567.89');
		expect(clean('1,234,567', true)).toBe('1234567');
	});

	it('keeps one decimal point, the first', () => {
		expect(clean('4.5.')).toBe('4.5');
		expect(clean('1.5.2')).toBe('1.52');
	});
});

describe('under any preset', () => {
	it('leaves a clean figure exactly as typed', () => {
		for (const text of ['', '0', '4', '4.', '.5', '0.50', '007', '53.483600000000000001']) {
			expect(cleanAmountText(text, '.'), text).toBe(text);
			expect(cleanAmountText(text, ','), text).toBe(text);
		}
	});

	it('refuses a paste that is not one figure, instead of salvaging a different one', () => {
		for (const preset of ['.', ',']) {
			// 1.57 is ten million times 1.5e-7.
			expect(cleanAmountText('1.5e-7', preset, true)).toBeNull();
			expect(cleanAmountText('1e5', preset, true)).toBeNull();
			expect(cleanAmountText('0x10', preset, true)).toBeNull();
			expect(cleanAmountText('4.5.6', preset, true)).toBeNull();
		}
		expect(cleanAmountText('4,5,6', ',', true)).toBeNull();
		// A label beside a figure is not a letter inside one.
		expect(cleanAmountText('$4.00 USD', '.', true)).toBe('4.00');
		expect(cleanAmountText('0.5 ETH', '.', true)).toBe('0.5');
	});

	it('reads ONE typed comma as the decimal mark, under a decimal-point preset too', () => {
		expect(cleanAmountText('4,', '.', false, '4')).toBe('4.');
		expect(cleanAmountText(',5', '.', false, '5')).toBe('.5');
		// Not a keystroke — a paste, an autofill, a keyboard's clipboard strip
		// calling itself typing: still grouping.
		expect(cleanAmountText('1,234', '.', false, '')).toBe('1234');
		expect(cleanAmountText('1,234', '.', true, '')).toBe('1234');
		expect(cleanAmountText('1,234', '.')).toBe('1234');
		// A figure that has its decimal point already: the comma is dropped.
		expect(cleanAmountText('1,2.5', '.', false, '12.5')).toBe('12.5');
	});

	it('keeps only what an amount is written with', () => {
		expect(cleanAmountText('-4', '.')).toBe('4');
		expect(cleanAmountText('1e5', '.')).toBe('15');
		expect(cleanAmountText('$4.00 ', '.')).toBe('4.00');
		expect(cleanAmountText("1'234.5", '.')).toBe('1234.5');
	});

	it('maps Arabic-Indic and Persian digits onto the ones the core reads', () => {
		expect(cleanAmountText('٤٫٥'.replace('٫', '.'), '.')).toBe('4.5');
		expect(cleanAmountText('۱۲', '.')).toBe('12');
	});

	it('maps their decimal marks with them — digits without the mark is ten times the figure', () => {
		expect(cleanAmountText('٤٫٥', '.')).toBe('4.5');
		expect(cleanAmountText('٤٫٥', ',')).toBe('4.5');
		// Full-width, as a ja / zh / ko keyboard writes it.
		expect(cleanAmountText('４．５', '.')).toBe('4.5');
		expect(cleanAmountText('4。5', '.')).toBe('4.5');
		expect(cleanAmountText('４，５', ',')).toBe('4.5');
		expect(cleanAmountText('１，２３４．５', '.', true)).toBe('1234.5');
	});
});

describe('the caret after a clean', () => {
	it('stands after as many kept characters as stood before it', () => {
		// "1," alone cleans to "1." — two characters — but in the whole figure
		// the comma is dropped, and the caret belongs after the 1.
		expect(caretAfterClean('1,2.5', '12.5', 2)).toBe(1);
		expect(caretAfterClean('4.5x', '4.5', 4)).toBe(3);
		expect(caretAfterClean('4,5', '4.5', 2)).toBe(2);
		expect(caretAfterClean('1.234,56', '1234.56', 8)).toBe(7);
		expect(caretAfterClean('٤٫٥', '4.5', 3)).toBe(3);
		expect(caretAfterClean('12', '12', 0)).toBe(0);
	});
});
