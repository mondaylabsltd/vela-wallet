/**
 * perf-5: the Ethereum Chainlink read used to gate every chain's read — one
 * slow Ethereum node held the home at "Checking…" for 42 s, and with Ethereum
 * unreachable nothing was read at all. Now each chain is read at once, and a
 * chain that needs Ethereum's price waits for it only after its own answer,
 * at most `CHAINLINK_PRICE_CAP_MS`, then prices with the last prices known.
 *
 * The wire, the registry document and the price seams are mocked; the
 * orchestration is the real code.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const price = vi.hoisted(() => ({
	/** What Ethereum's Chainlink read does: answer `prices`, or never. */
	live: null as Record<string, number> | null,
	known: {} as Record<string, number>,
	asked: 0
}));

vi.mock('./rpc-pool', () => ({
	PoolFailedError: class PoolFailedError extends Error {},
	poolRpcCall: async () => ({
		result: '0x' + (10n ** 18n).toString(16).padStart(64, '0')
	}),
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

vi.mock('./abi', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./abi')>();
	return {
		...actual,
		encAggregate3: () => '0x00',
		decAggregate3: (hex: string) => [{ success: true, data: hex }]
	};
});

vi.mock('./chain-tokens', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./chain-tokens')>();
	return {
		...actual,
		readChainTokens: async (chainId: number) => ({
			kind: 'doc' as const,
			data: {
				chainId,
				nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
				stables: [],
				wrappedNativeToken: null,
				dex: null
			}
		})
	};
});

vi.mock('./records', () => ({ loadCustomTokens: async () => [] }));

vi.mock('./price-service', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./price-service')>();
	return {
		...actual,
		fetchChainlinkPrices: () => {
			price.asked += 1;
			return price.live === null ? new Promise(() => {}) : Promise.resolve(price.live);
		},
		lastKnownChainlinkPrices: async () => price.known
	};
});

// The ladder passes Ethereum's price through; no DEX, no on-chain feed here.
vi.mock('./native-price', () => ({
	bestNativeDexPrice: () => null,
	chooseNativePrice: (_dex: unknown, _local: unknown, eth: number | null) => ({
		price: eth,
		source: 'chainlink_eth'
	}),
	peggedNativeUsd: () => null
}));

import { CHAINLINK_PRICE_CAP_MS, clearTokenCache, fetchTokens } from './wallet-api';

const ADDRESS = '0x2C1c9400000000000000000000000000000347c23';

beforeEach(() => {
	clearTokenCache();
	price.live = null;
	price.known = {};
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

describe('Ethereum’s price read never holds the chains (perf-5)', () => {
	it('a hung Ethereum read: the round ends at the cap, priced with the last known prices', async () => {
		vi.useFakeTimers();
		try {
			price.known = { ETH: 2_000 };
			let settled = false;
			const round = fetchTokens(ADDRESS, { forceRefresh: true }).then((tokens) => {
				settled = true;
				return tokens;
			});
			await vi.advanceTimersByTimeAsync(CHAINLINK_PRICE_CAP_MS - 1);
			expect(settled).toBe(false);
			await vi.advanceTimersByTimeAsync(1);
			const tokens = await round;
			expect(tokens.length).toBeGreaterThan(0);
			expect(tokens.every((t) => t.priceUsd === 2_000)).toBe(true);
		} finally {
			vi.useRealTimers();
		}
	});

	it('nothing known: the rows land unpriced at the cap, never waiting for Ethereum', async () => {
		vi.useFakeTimers();
		try {
			const round = fetchTokens(ADDRESS, { forceRefresh: true });
			await vi.advanceTimersByTimeAsync(CHAINLINK_PRICE_CAP_MS);
			const tokens = await round;
			expect(tokens.length).toBeGreaterThan(0);
			expect(tokens.every((t) => t.priceUsd === null)).toBe(true);
		} finally {
			vi.useRealTimers();
		}
	});

	it('r4D-18, spec 105: the testnet space prices nothing, so reads no price feed', async () => {
		const { networkSpace } = await import('$lib/network-space/network-space.svelte');
		networkSpace.set('testnet');
		try {
			price.asked = 0;
			// No feed asked, and nothing waits for one: the round ends at once.
			await fetchTokens(ADDRESS, { forceRefresh: true });
			expect(price.asked).toBe(0);
		} finally {
			networkSpace.set('mainnet');
		}
	});

	it('an Ethereum read that answers in time is the price', async () => {
		price.live = { ETH: 3_100 };
		price.known = { ETH: 2_000 };
		const tokens = await fetchTokens(ADDRESS, { forceRefresh: true });
		expect(tokens.every((t) => t.priceUsd === 3_100)).toBe(true);
	});
});
