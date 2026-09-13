/**
 * Spec 038 — the founder's Celo report: CELO is an ERC-20 at the address the
 * chain data calls its wrapped native, so listing "WCELO" beside CELO counted
 * one holding twice.
 */
import { describe, expect, it } from 'vitest';
import { wrappedNativeIsTheNative } from './wallet-api';

describe('the wrapped native that is the native', () => {
	it("is Celo's GoldToken, whichever case the address arrives in", () => {
		expect(wrappedNativeIsTheNative(42220, '0x471EcE3750Da237f93B8E339c536989b8978a438')).toBe(
			true
		);
		expect(wrappedNativeIsTheNative(42220, '0x471ece3750da237f93b8e339c536989b8978a438')).toBe(
			true
		);
	});

	it('is not WETH, WBNB or a token that merely shares an address elsewhere', () => {
		expect(wrappedNativeIsTheNative(1, '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2')).toBe(false);
		expect(wrappedNativeIsTheNative(56, '0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c')).toBe(false);
		expect(wrappedNativeIsTheNative(1, '0x471ece3750da237f93b8e339c536989b8978a438')).toBe(false);
	});
});
