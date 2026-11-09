/**
 * The fee, as one string and as two pieces (issue 231).
 *
 * `feeLine` is the signing sheet's sentence and must not move. `feeLineParts`
 * is the same decision cut in two for the send form's row, so the money can
 * drop to a second line whole — and the two must never disagree about WHETHER
 * there is a money figure.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { feeAmountText, feeLine, feeLineParts } from './fee-line';

const USD = { code: 'USD', rate: 1, committed: true, pending: null };
const PARTS = { coin: '0.001329 AVAX', symbol: 'AVAX', units: 0.001329, contract: null };

describe('feeLineParts', () => {
	it('keeps the coin and the money apart, the money spaced like the amount line', () => {
		expect(feeLineParts(PARTS, 9, USD)).toEqual({ coin: '0.001329 AVAX', fiat: '≈ $0.01' });
	});

	it('has no money half when nothing can price the coin, or it rounds to nothing', () => {
		expect(feeLineParts(PARTS, null, USD)).toEqual({ coin: '0.001329 AVAX', fiat: null });
		expect(feeLineParts({ ...PARTS, units: null }, 9, USD).fiat).toBeNull();
		expect(feeLineParts(PARTS, 0.001, USD).fiat).toBeNull();
	});

	it('states dollars AS dollars when the display currency has no rate — never a rate of 1', () => {
		// The core's unpriceable pair: committed, with no rate.
		const unpriced = { code: 'EUR', rate: null, committed: true, pending: null };
		expect(feeLineParts(PARTS, 9, unpriced).fiat).toBe('≈ $0.01');
	});

	// The core's rule (`CurrencyView.committed`): while the display currency
	// is still the USD/1 placeholder, no money figure is drawn in it — a fee
	// that read "≈ $0.01" and then "≈ ¥0.07" is the jump the rule ends.
	it('draws no money figure before the display currency is the person’s', () => {
		const placeholder = { code: 'USD', rate: 1, committed: false, pending: 'CNY' };
		expect(feeLineParts(PARTS, 9, placeholder)).toEqual({ coin: '0.001329 AVAX', fiat: '≈ …' });
	});
});

/**
 * Issue 682. The native fee floor on a coin the relay cannot price fell from
 * 0.001 of it to $0.01 worth — 0.000083 at ~$120/coin — and a truncating trim
 * at four decimals turned that into a bare "0". Every surface that prints a fee
 * amount goes through this one function now, so none of them can say "free"
 * about a fee that is not.
 */
describe('feeAmountText', () => {
	it('keeps the first significant digits however small the amount is', () => {
		// The measured XLayer fee: 83,333,333,333,334 wei of an 18-decimal coin
		// — r21-2: two significant digits, rounded UP (it read 0.000083, less
		// than the charge).
		expect(feeAmountText('83333333333334', 18, 4)).toBe('0.000084');
		// The relay's own admission floor, 0.00001 of the coin.
		expect(feeAmountText(10_000_000_000_000n, 18, 4)).toBe('0.00001');
		// Far below any real fee, but still not "0".
		expect(feeAmountText('2500000000', 18, 4)).toBe('0.0000000025');
	});

	it('is the figure itself when the budget already fits it', () => {
		expect(feeAmountText('2100000000000000', 18, 6)).toBe('0.0021');
		expect(feeAmountText('1329000000000000', 18, 6)).toBe('0.001329');
		expect(feeAmountText('1500000000000000000', 18, 4)).toBe('1.5');
		expect(feeAmountText('2000000000000000000', 18, 4)).toBe('2');
		expect(feeAmountText('10000', 6, 4)).toBe('0.01');
	});

	// r21-2 (§36): a fee is never shown below what is charged — past the
	// budget it rounds UP (the core's `fee_figure`), never down through a float.
	it('rounds up past the budget: never below the charge', () => {
		expect(feeAmountText('123456789000000000', 18, 4)).toBe('0.1235');
		expect(feeAmountText('1000000000000001', 18, 6)).toBe('0.001001');
		expect(feeAmountText('9999999', 6, 4)).toBe('10');
	});

	it('says zero only when the amount really is zero, and a dash for no count', () => {
		expect(feeAmountText('0', 18, 4)).toBe('0');
		expect(feeAmountText('not a count', 18, 4)).toBe('—');
	});
});

describe('feeLine', () => {
	it("is still the signing sheet's exact sentence", () => {
		expect(feeLine(PARTS, 9, USD)).toBe('0.001329 AVAX · ≈$0.01');
		expect(feeLine(PARTS, null, USD)).toBe('0.001329 AVAX');
		expect(feeLine(PARTS, 0.001, USD)).toBe('0.001329 AVAX');
	});
});
