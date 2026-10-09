/**
 * A mark drawn before the core has loaded. The filter lists the person's own
 * networks from storage a frame before the wasm arrives; asking the core then
 * threw, and a throw in a derived value takes the page down with it. Until the
 * core is aboard a mark is its glyph alone — no URL invented here, no badge
 * claimed — and the next frame after it lands draws the real one.
 *
 * Its own file: the module graph is per file, so here the wasm is never
 * initialized.
 */
import { describe, expect, it } from 'vitest';
import { chainLogoURL, chainMark, tokenLogoURLs, tokenMarkFor } from './marks';

describe('before the core is aboard', () => {
	it('every mark is its glyph alone, and nothing throws', () => {
		expect(chainLogoURL(1)).toBeUndefined();
		expect(tokenLogoURLs(1, 'USDC', '0x' + 'a0'.repeat(20))).toEqual([]);
		const coin = tokenMarkFor(8453, 'ETH', null, ['https://named.example/eth.png']);
		expect(coin.ticker).toBe('ETH');
		expect(coin.logoUrls).toBeUndefined();
		expect(coin.badgeHidden).toBe(true);
		expect(chainMark(8453)).toMatchObject({ ticker: 'ETH', badgeHidden: true });
		expect(chainMark(8453).logoUrls).toBeUndefined();
	});
});
