/**
 * The rate waterfall (spec 025 Phase 5, D13): Chainlink first for the
 * currencies it feeds, the configured endpoint next, and null — never 1 —
 * when neither can price it. Plus the quote that refuses to be mislabelled.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const sources = {
	chainlink: null as number | null,
	fx: null as number | null,
	chainlinkThrows: false,
	fxThrows: false
};
vi.mock('#lib/services/fiat-rates.ts', () => ({
	isChainlinkFiat: (code: string) => ['EUR', 'JPY'].includes(code),
	getChainlinkRate: vi.fn(async () => {
		if (sources.chainlinkThrows) throw new Error('rpc down');
		return sources.chainlink;
	})
}));
vi.mock('#lib/services/fiat-fx.ts', () => ({
	getFxRate: vi.fn(async () => {
		if (sources.fxThrows) throw new Error('endpoint down');
		return sources.fx;
	})
}));

import { resolveQuote, resolveRate } from './currency-rate';
import { convertibleRate } from './fiat-rate-quote';
import { getChainlinkRate } from './fiat-rates';

beforeEach(() => {
	sources.chainlink = null;
	sources.fx = null;
	sources.chainlinkThrows = false;
	sources.fxThrows = false;
	vi.mocked(getChainlinkRate).mockClear();
});

describe('resolveRate', () => {
	it('USD is 1 without asking anyone', async () => {
		expect(await resolveRate('USD')).toBe(1);
		expect(getChainlinkRate).not.toHaveBeenCalled();
	});
	it('a Chainlink-fed currency takes the feed first', async () => {
		sources.chainlink = 0.92;
		sources.fx = 0.93;
		expect(await resolveRate('EUR')).toBe(0.92);
	});
	it('falls through to the endpoint when the feed cannot price it or throws', async () => {
		sources.fx = 155;
		expect(await resolveRate('JPY')).toBe(155);
		sources.chainlinkThrows = true;
		expect(await resolveRate('JPY')).toBe(155);
	});
	it('a currency without a feed skips Chainlink', async () => {
		sources.fx = 25000;
		expect(await resolveRate('VND')).toBe(25000);
		expect(getChainlinkRate).not.toHaveBeenCalled();
	});
	it('nothing can price it → null, never 1 (even when a source throws)', async () => {
		expect(await resolveRate('VND')).toBeNull();
		sources.fxThrows = true;
		expect(await resolveRate('VND')).toBeNull();
		expect(await resolveRate('EUR')).toBeNull();
	});
});

describe('resolveQuote + convertibleRate', () => {
	it('a quote carries its currency; conversion refuses unknown, invalid, mislabelled', async () => {
		sources.fx = 25000;
		const quote = await resolveQuote('VND');
		expect(quote).toEqual({ code: 'VND', rate: 25000 });
		expect(convertibleRate(quote, 'VND')).toBe(25000);
		expect(convertibleRate(quote, 'CNY')).toBeNull();
		expect(convertibleRate(null, 'VND')).toBeNull();
		expect(convertibleRate({ code: 'VND', rate: 0 }, 'VND')).toBeNull();
		expect(convertibleRate({ code: 'VND', rate: NaN }, 'VND')).toBeNull();
		sources.fx = null;
		expect(await resolveQuote('VND')).toBeNull();
	});
});
