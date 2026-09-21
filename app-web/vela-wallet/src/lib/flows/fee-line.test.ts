/**
 * The fee, as one string and as two pieces (issue 231).
 *
 * `feeLine` is the signing sheet's sentence and must not move. `feeLineParts`
 * is the same decision cut in two for the send form's row, so the money can
 * drop to a second line whole — and the two must never disagree about WHETHER
 * there is a money figure.
 */
import { describe, expect, it } from 'vitest';
import { feeAmountText, feeLine, feeLineParts } from './fee-line';

const USD = { code: 'USD', rate: 1, committed: true };
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
		const unpriced = { code: 'EUR', rate: null, committed: false };
		expect(feeLineParts(PARTS, 9, unpriced).fiat).toBe('≈ $0.01');
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
	it('keeps the first significant digit however small the amount is', () => {
		// The measured XLayer fee: 83,333,333,333,334 wei of an 18-decimal coin.
		expect(feeAmountText(83_333_333_333_334 / 1e18, 4)).toBe('0.000083');
		// The relay's own admission floor, 0.00001 of the coin.
		expect(feeAmountText(1e-5, 4)).toBe('0.00001');
		// Far below any real fee, but still not "0".
		expect(feeAmountText(2.5e-9, 4)).toBe('0.0000000025');
	});

	it('is the plain trim for anything the budget already fits', () => {
		expect(feeAmountText(0.0021, 6)).toBe('0.0021');
		expect(feeAmountText(0.001329, 6)).toBe('0.001329');
		expect(feeAmountText(1.5, 4)).toBe('1.5');
		expect(feeAmountText(2, 4)).toBe('2');
		// Past the budget it still truncates — this widens the window, it does
		// not start showing more of a figure that already had digits to spare.
		expect(feeAmountText(0.123456789, 4)).toBe('0.1234');
	});

	it('says zero only when the amount really is zero', () => {
		expect(feeAmountText(0, 4)).toBe('0');
	});
});

describe('feeLine', () => {
	it("is still the signing sheet's exact sentence", () => {
		expect(feeLine(PARTS, 9, USD)).toBe('0.001329 AVAX · ≈$0.01');
		expect(feeLine(PARTS, null, USD)).toBe('0.001329 AVAX');
		expect(feeLine(PARTS, 0.001, USD)).toBe('0.001329 AVAX');
	});
});
