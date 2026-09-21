/**
 * Issue 682 — an unpriced native coin must not overcharge 12×.
 *
 * The relay's in-band quote carries `usdPrice: null` for any coin its price
 * feed does not know, and a network the user ADDED is almost never in that
 * feed. The core's "$0.01 worth of the native coin" minimum then fell back to
 * a blind flat 0.001 of the coin — on XLayer's OKB (~$120) that is $0.12,
 * twelve times the cent, on every send.
 *
 * This wallet knew the price the whole time: it is what the fee line renders
 * "≈$0.12" from. What is pinned here is that the executor hands that price to
 * the core, from the balances feed and nowhere else, ONLY on the native row and
 * ONLY when the relay published none — because a floor price that leaked onto a
 * priced quote, or onto a stablecoin row, would change what a send costs.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seams = vi.hoisted(() => ({
	inBandQuotes: vi.fn(),
	nativePrice: vi.fn((): number | null => null)
}));

vi.mock('$lib/services/bundler-service', () => ({
	fetchInBandGasQuotes: seams.inBandQuotes,
	fetchBundlerAccountInfo: vi.fn()
}));

vi.mock('$lib/services/wallet-api', () => ({
	getCachedNativePriceUsd: seams.nativePrice
}));

vi.mock('$lib/services/safe-transaction', () => ({
	fetchRawBundlerQuote: vi.fn(),
	fetchRawGasSignals: vi.fn(),
	keySetOf: vi.fn(),
	simulateUserOpGas: vi.fn()
}));

vi.mock('$lib/services/accounts', () => ({ findAccountByAddress: vi.fn() }));

import { createFeeExecutor } from './fee-executor';

const ACCOUNT = '0x1111111111111111111111111111111111111111';
const XLAYER = 196;

function row(asset: 'native' | 'erc20', usdPrice: string | null) {
	return {
		recipient: '0x2222222222222222222222222222222222222222',
		asset,
		feeToken: asset === 'erc20' ? '0x3333333333333333333333333333333333333333' : null,
		balance: 10n ** 18n,
		decimals: asset === 'erc20' ? 6 : 18,
		symbol: asset === 'erc20' ? 'USDC' : 'OKB',
		usdBalance: '0',
		usdPrice
	};
}

async function quotes(rows: ReturnType<typeof row>[]) {
	seams.inBandQuotes.mockResolvedValue(rows);
	const execute = createFeeExecutor({
		onView: () => {},
		onError: () => {},
		publicKeyHex: () => undefined
	});
	const result = await execute(
		{ id: 1, operation: { type: 'fetch_in_band_quotes', chain_id: XLAYER, account: ACCOUNT } },
		new AbortController().signal
	);
	if (result.type !== 'in_band_quotes') throw new Error('wrong result variant');
	return result.quotes ?? [];
}

describe('the native floor price the executor hands the core (issue 682)', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		seams.nativePrice.mockReturnValue(null);
	});

	it('sends the wallet-derived price when the relay published none', async () => {
		seams.nativePrice.mockReturnValue(120.5);
		const [native] = await quotes([row('native', null)]);
		expect(native.usd_price).toBe(null);
		// The core's own precision, and never exponent notation — it parses a
		// plain decimal and would call "1.205e2" garbage.
		expect(native.native_usd_floor_price).toBe('120.50000000');
		// Asked of the balances feed for THIS account on THIS chain, not of a
		// price source of the executor's own.
		expect(seams.nativePrice).toHaveBeenCalledWith(ACCOUNT, XLAYER);
	});

	it('omits it when the relay priced the coin, so a priced send cannot move', async () => {
		seams.nativePrice.mockReturnValue(120.5);
		const [native] = await quotes([row('native', '11.539')]);
		expect(native.usd_price).toBe('11.539');
		expect(native.native_usd_floor_price).toBe(null);
	});

	it('omits it when the wallet has no price either — the blind floor stays the last resort', async () => {
		seams.nativePrice.mockReturnValue(null);
		const [native] = await quotes([row('native', null)]);
		expect(native.native_usd_floor_price).toBe(null);
	});

	it('never puts it on a stablecoin row, which has no floor to value', async () => {
		seams.nativePrice.mockReturnValue(120.5);
		const rows = await quotes([row('native', null), row('erc20', '1')]);
		const stable = rows.find((r) => r.asset === 'erc20');
		expect(stable?.native_usd_floor_price).toBe(null);
		// …and the stablecoin's own price is untouched, so the core's refusal to
		// convert without a native rate is still the core's call to make.
		expect(stable?.usd_price).toBe('1');
	});

	it('refuses to put a non-price on the wire', async () => {
		// 0 would cross as "0.00000000" and the other two as the literal words
		// "NaN"/"Infinity". The core refuses all of them, but this wallet should
		// not be sending any of them as its opinion of what a coin costs.
		for (const price of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
			seams.nativePrice.mockReturnValue(price);
			const [native] = await quotes([row('native', null)]);
			expect(native.native_usd_floor_price).toBe(null);
		}
	});

	it('reads "the relay could not price it" the way the CORE reads it, not just as null', async () => {
		seams.nativePrice.mockReturnValue(120.5);
		// Each of these is a price the core throws away — it drops zero ("a zero
		// price is unpriceable") and parses only a plain decimal, while
		// `bundler-service.ts`'s `parseDecimalString` lets all of them through.
		// If this shell called any of them "priced" it would withhold the floor
		// price it has and the 12× overcharge would survive the fix.
		for (const published of ['0', '0.00', '1e-7', 'abc', '-3']) {
			const [native] = await quotes([row('native', published)]);
			expect(native.usd_price).toBe(published);
			expect(native.native_usd_floor_price).toBe('120.50000000');
		}
	});

	it('leaves every other field of the row exactly as it was', async () => {
		seams.nativePrice.mockReturnValue(120.5);
		const [native] = await quotes([row('native', null)]);
		expect(native).toMatchObject({
			recipient: '0x2222222222222222222222222222222222222222',
			asset: 'native',
			fee_token: null,
			balance: '1000000000000000000',
			decimals: 18,
			symbol: 'OKB',
			usd_balance: '0'
		});
	});
});
