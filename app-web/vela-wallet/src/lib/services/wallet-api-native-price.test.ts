/**
 * Issue 682 — the price the fee core floors an unpriced coin with.
 *
 * `getCachedNativePriceUsd` is the load-bearing half of the web fix: when the
 * relay publishes no price for a coin, this is what the core values its "$0.01
 * worth" minimum at instead of a blind 0.001 of the coin. The executor's own
 * test mocks this module away, so without this file nothing would execute the
 * real function — and if its native-row match, its chain filter or its cache
 * key ever drifted it would simply answer `null`, the core would fall back to
 * the blind floor, every test would stay green and the 12× overcharge would be
 * back with nobody told.
 *
 * So the cache here is warmed the way the send screen warms it — through the
 * real `fetchTokens` — with only the wire, the chain registry and the price
 * seams mocked, exactly as `wallet-api-chain-outage.test.ts` does.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const NATIVE_DECIMALS = 18;

/** Gnosis stands in for the priced chain, Arbitrum for the unpriced one. */
const PRICED = 100;
const UNPRICED = 42161;
const PRICE = 120.5;

/** The raw native balance each chain reports; absent chains answer nothing. */
const balances = new Map<number, bigint>();

vi.mock('./rpc-pool', () => ({
	poolRpcCall: async (_method: string, _params: unknown[], chainId: number) => {
		const raw = balances.get(chainId) ?? 0n;
		return { result: '0x' + raw.toString(16).padStart(64, '0') };
	},
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

// Every call in the multicall answers the same balance — there are only two
// slots per chain here (the coin and one custom token) and neither needs a
// different figure.
vi.mock('./abi', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./abi')>();
	return {
		...actual,
		encAggregate3: () => '0x00',
		decAggregate3: (hex: string) => Array.from({ length: 8 }, () => ({ success: true, data: hex }))
	};
});

// The priced chain's coin is one the peg table knows, so the REAL
// `chooseNativePrice` runs and lands on its Chainlink rung; the other chain's
// coin is priceable by nobody, which is the case issue 682 is about.
vi.mock('./chain-tokens', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./chain-tokens')>();
	return {
		...actual,
		fetchChainTokens: async (chainId: number) => ({
			chainId,
			nativeCurrency: {
				name: 'Coin',
				symbol: chainId === PRICED ? 'PRICED' : 'UNPRICED',
				decimals: NATIVE_DECIMALS
			},
			stables: [],
			wrappedNativeToken: null,
			dex: null
		})
	};
});

// One ERC-20 on the priced chain, so a row that is NOT the coin sits in the
// same cache entry and cannot be mistaken for it.
const CUSTOM = '0x' + 'cc'.repeat(20);
vi.mock('./records', () => ({
	loadCustomTokens: async () => [
		{
			chainId: PRICED,
			contractAddress: CUSTOM,
			symbol: 'CUSTOM',
			name: 'Custom',
			decimals: 18
		}
	]
}));

vi.mock('./price-service', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./price-service')>();
	return { ...actual, fetchChainlinkPrices: async () => ({}) };
});

// The price ladder itself lives in the wasm core, which a node test cannot
// boot — `wallet-api-chain-outage.test.ts` stands the same seam up the same
// way. What is under test here is not the ladder but what `wallet-api` does
// with its answer: which ROW the price lands on, and what
// `getCachedNativePriceUsd` then finds.
vi.mock('./native-price', () => ({
	bestNativeDexPrice: () => null,
	chooseNativePrice: (dex: number | null, local: number | null, eth: number | null) => ({
		price: dex ?? local ?? eth ?? null,
		source: 'chainlink-eth'
	}),
	// What this wallet knows the coin is worth — for one of the two chains.
	peggedNativeUsd: (symbol: string) => (symbol === 'PRICED' ? PRICE : null)
}));

import { clearTokenCache, fetchTokens, getCachedNativePriceUsd } from './wallet-api';

const ADDRESS = '0x2C1c9400000000000000000000000000000347c23';
const ONE = 10n ** BigInt(NATIVE_DECIMALS);

beforeEach(() => {
	clearTokenCache();
	balances.clear();
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

describe('the native price the fee floor is valued at', () => {
	it("is the coin's own row on that chain, once the balances are in", async () => {
		balances.set(PRICED, ONE);
		balances.set(UNPRICED, ONE);
		const tokens = await fetchTokens(ADDRESS);

		// The cache really does hold an ERC-20 beside the coin on that chain, so
		// "it found the native row" is a claim with something to be wrong about.
		expect(tokens.some((t) => t.tokenAddress?.toLowerCase() === CUSTOM)).toBe(true);

		expect(getCachedNativePriceUsd(ADDRESS, PRICED)).toBe(PRICE);
		// Not the ERC-20's price, and not another chain's coin: the row has to
		// match on BOTH the chain and "this is the coin", or the core would be
		// handed a number about a different asset.
		expect(getCachedNativePriceUsd(ADDRESS, UNPRICED)).toBeNull();
	});

	it('answers for the account however it was spelled, because the cache is keyed lowercase', async () => {
		balances.set(PRICED, ONE);
		await fetchTokens(ADDRESS);

		expect(getCachedNativePriceUsd(ADDRESS.toUpperCase(), PRICED)).toBe(PRICE);
		expect(getCachedNativePriceUsd(` ${ADDRESS.toLowerCase()} `, PRICED)).toBe(PRICE);
	});

	it('says null on a cold cache rather than guessing — the blind floor is the honest fallback', () => {
		expect(getCachedNativePriceUsd(ADDRESS, PRICED)).toBeNull();
		expect(getCachedNativePriceUsd(undefined, PRICED)).toBeNull();
	});

	it('says null for an account nobody fetched, and for a chain that reported nothing', async () => {
		balances.set(PRICED, ONE);
		await fetchTokens(ADDRESS);

		expect(getCachedNativePriceUsd('0x' + '11'.repeat(20), PRICED)).toBeNull();
		expect(getCachedNativePriceUsd(ADDRESS, 8453)).toBeNull();
	});
});
