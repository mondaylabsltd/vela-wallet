/**
 * Issue 196 — a chain that cannot be read must not be reported as a chain
 * that holds nothing.
 *
 * `queryChainAssets` used to swallow an RPC failure and `return []`, which is
 * byte-identical to "this account holds nothing here". The core takes a
 * SETTLED list at its word (`balance_dashboard.rs` `accept`: "the final result
 * replaces everything"), so one bad round deleted that chain's tokens from the
 * Assets list and their value from the Total balance — while the send picker,
 * which snapshots the same fetch at a different moment, still listed them.
 *
 * Only `poolRpcCall` (the wire), the chain registry document and the price
 * seams are mocked: the orchestration under test — per-chain outcomes, the
 * carry-over and the failed-chain report — is the real code.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const NATIVE_DECIMALS = 18;

/** Chains whose multicall rejects, standing in for an RPC outage. */
const down = new Set<number>();
/** Raw native balance each healthy chain reports. */
const balances = new Map<number, bigint>();

vi.mock('./rpc-pool', () => ({
	poolRpcCall: async (_method: string, _params: unknown[], chainId: number) => {
		if (down.has(chainId)) return { error: { message: 'every endpoint failed' } };
		const raw = balances.get(chainId) ?? 0n;
		return { result: '0x' + raw.toString(16).padStart(64, '0') };
	},
	getFailedRpcChains: () => down,
	getRateLimitedChains: () => new Set<number>()
}));

// The multicall carries one call per healthy chain here (native balance only —
// no stables, no wrapped native, no DEX), so the wire's answer IS the result.
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
		fetchChainTokens: async (chainId: number) => ({
			chainId,
			nativeCurrency: { name: 'Coin', symbol: 'COIN', decimals: NATIVE_DECIMALS },
			stables: [],
			wrappedNativeToken: null,
			dex: null
		})
	};
});

vi.mock('./records', () => ({ loadCustomTokens: async () => [] }));

vi.mock('./price-service', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./price-service')>();
	return { ...actual, fetchChainlinkPrices: async () => ({}) };
});

vi.mock('./native-price', () => ({
	bestNativeDexPrice: () => null,
	chooseNativePrice: () => ({ price: null, source: 'none' }),
	// The core's $1 peg (spec 060). `COIN` here is not a dollar coin, so the
	// seam answers null and the ladder runs exactly as it did before.
	peggedNativeUsd: () => null
}));

import { carryOverUnansweredChains, clearTokenCache, fetchTokens } from './wallet-api';
import { tokenChainId, type APIToken } from './tokens-model';
import { networkId } from './networks';

const ADDRESS = '0x2C1c9400000000000000000000000000000347c23';
const ONE = 10n ** BigInt(NATIVE_DECIMALS);

/** Gnosis, Base and Polygon — the three the report named. */
const GNOSIS = 100;
const BASE = 8453;
const POLYGON = 137;
const ARBITRUM = 42161;

function chainIdsOf(tokens: APIToken[]): number[] {
	return tokens.map(tokenChainId).sort((a, b) => a - b);
}

beforeEach(() => {
	clearTokenCache();
	down.clear();
	balances.clear();
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

describe('holdings on a chain that did not answer', () => {
	it('survive a refresh taken while the chain is unreachable', async () => {
		balances.set(GNOSIS, ONE);
		balances.set(BASE, ONE);
		balances.set(ARBITRUM, ONE);
		const first = await fetchTokens(ADDRESS);
		expect(chainIdsOf(first)).toEqual([BASE, ARBITRUM, GNOSIS].sort((a, b) => a - b));

		// Gnosis and Base go dark; Arbitrum answers as before.
		down.add(GNOSIS);
		down.add(BASE);
		const second = await fetchTokens(ADDRESS, { forceRefresh: true });

		expect(chainIdsOf(second)).toEqual([BASE, ARBITRUM, GNOSIS].sort((a, b) => a - b));
	});

	it('are dropped once the chain answers and no longer reports them', async () => {
		balances.set(POLYGON, ONE);
		balances.set(ARBITRUM, ONE);
		await fetchTokens(ADDRESS);

		// The POL was spent: Polygon answers, with nothing.
		balances.set(POLYGON, 0n);
		const after = await fetchTokens(ADDRESS, { forceRefresh: true });

		expect(chainIdsOf(after)).toEqual([ARBITRUM]);
	});

	it('are reported as a partial round, so no stale total becomes last-known-good', async () => {
		balances.set(GNOSIS, ONE);
		await fetchTokens(ADDRESS);

		down.add(GNOSIS);
		let failed: number[] = [];
		await fetchTokens(ADDRESS, { forceRefresh: true, onFailedChains: (ids) => (failed = ids) });

		expect(failed).toContain(GNOSIS);
	});

	// A round a chain did not answer is not a fresh answer to hold: served from
	// the cache it would report no failures, and the balance machine's retry
	// would take a launch-time blip for a complete, empty wallet.
	it('a partial round is never served from the cache as if complete', async () => {
		down.add(GNOSIS);
		down.add(BASE);
		let failed: number[] = [];
		const blip = await fetchTokens(ADDRESS, { onFailedChains: (ids) => (failed = ids) });
		expect(blip).toEqual([]);
		expect(failed).toEqual(expect.arrayContaining([GNOSIS, BASE]));

		// The blip is over; a plain (non-forced) ask reads the chains again.
		down.clear();
		balances.set(GNOSIS, ONE);
		failed = [];
		const next = await fetchTokens(ADDRESS, { onFailedChains: (ids) => (failed = ids) });
		expect(chainIdsOf(next)).toEqual([GNOSIS]);
		expect(failed).toEqual([]);

		// A complete round IS held: the next plain ask is the cache's.
		balances.set(GNOSIS, 2n * ONE);
		expect((await fetchTokens(ADDRESS))[0].balance).toBe('1');
	});

	it('a caller that JOINS a round in flight hears its failures too', async () => {
		down.add(GNOSIS);
		let ownerFailed: number[] = [];
		let joinerFailed: number[] = [];
		await Promise.all([
			fetchTokens(ADDRESS, { onFailedChains: (ids) => (ownerFailed = ids) }),
			fetchTokens(ADDRESS, { onFailedChains: (ids) => (joinerFailed = ids) })
		]);
		expect(ownerFailed).toContain(GNOSIS);
		expect(joinerFailed).toEqual(ownerFailed);
	});

	it('are already in the first streamed snapshot, so the list never shrinks mid-refresh', async () => {
		balances.set(GNOSIS, ONE);
		balances.set(ARBITRUM, ONE);
		await fetchTokens(ADDRESS);

		down.add(GNOSIS);
		const snapshots: number[][] = [];
		await fetchTokens(ADDRESS, {
			forceRefresh: true,
			onProgress: (partial) => snapshots.push(chainIdsOf(partial))
		});

		expect(snapshots.every((ids) => ids.includes(GNOSIS))).toBe(true);
	});
});

describe('carryOverUnansweredChains', () => {
	const token = (chainId: number, symbol: string): APIToken => ({
		network: networkId(chainId),
		chainName: 'Chain',
		symbol,
		balance: '1',
		decimals: 18,
		logo: null,
		name: symbol,
		tokenAddress: null,
		priceUsd: 1,
		spam: false
	});

	it('keeps the previous holdings of a chain that is not in the answered set', () => {
		const merged = carryOverUnansweredChains(
			[token(GNOSIS, 'XDAI')],
			[token(ARBITRUM, 'ETH')],
			new Set([ARBITRUM])
		);
		expect(merged.map((t) => t.symbol).sort()).toEqual(['ETH', 'XDAI']);
	});

	it('lets an answered chain shorten the list', () => {
		const merged = carryOverUnansweredChains(
			[token(GNOSIS, 'XDAI')],
			[token(ARBITRUM, 'ETH')],
			new Set([ARBITRUM, GNOSIS])
		);
		expect(merged.map((t) => t.symbol)).toEqual(['ETH']);
	});

	it('never carries a chain twice when it both answered and was known before', () => {
		const merged = carryOverUnansweredChains(
			[token(ARBITRUM, 'ETH')],
			[token(ARBITRUM, 'ETH')],
			new Set([ARBITRUM])
		);
		expect(merged).toHaveLength(1);
	});
});
