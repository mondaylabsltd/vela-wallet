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
// The balance read plan is the core's (spec 082 RE9): the wasm module is up.
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const NATIVE_DECIMALS = 18;

/** The pool's own error, as `rpc-pool.ts` defines it (hoisted for the mock). */
const pool = vi.hoisted(() => ({
	PoolFailedError: class PoolFailedError extends Error {
		readonly maybeDelivered: boolean;
		readonly rateLimited: boolean;
		readonly internal: boolean;
		constructor(
			message: string,
			facts: { maybeDelivered: boolean; rateLimited: boolean; internal?: boolean }
		) {
			super(message);
			this.maybeDelivered = facts.maybeDelivered;
			this.rateLimited = facts.rateLimited;
			this.internal = facts.internal ?? false;
		}
	}
}));

/** Chains whose multicall rejects, standing in for an RPC outage. */
const down = new Set<number>();
/** Chains whose request pool fails inside the app (issue 483): nothing leaves. */
const faulted = new Set<number>();
/** Chains whose wire never answers at all — a connection held open (spec 092). */
const stalled = new Set<number>();
/** Raw native balance each healthy chain reports. */
const balances = new Map<number, bigint>();

vi.mock('./rpc-pool', () => ({
	PoolFailedError: pool.PoolFailedError,
	poolRpcCall: async (_method: string, _params: unknown[], chainId: number) => {
		if (faulted.has(chainId)) {
			throw new pool.PoolFailedError(`pool fault for chain ${chainId}`, {
				maybeDelivered: false,
				rateLimited: false,
				internal: true
			});
		}
		if (stalled.has(chainId)) return new Promise(() => {});
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

import {
	carryOverUnansweredChains,
	clearTokenCache,
	fetchTokens,
	readFailedInsideApp
} from './wallet-api';
import { balanceChainReadDeadlineMs } from '$lib/core/kernels';
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
	faulted.clear();
	stalled.clear();
	balances.clear();
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

describe('a chain whose connection is held open (spec 092)', () => {
	it('is failed at the core’s deadline, and the chains that answered still land', async () => {
		vi.useFakeTimers();
		try {
			balances.set(ARBITRUM, ONE);
			stalled.add(GNOSIS);
			let failed: number[] = [];
			let settled = false;
			const round = fetchTokens(ADDRESS, {
				forceRefresh: true,
				onFailedChains: (ids) => (failed = ids)
			}).then((tokens) => {
				settled = true;
				return tokens;
			});
			await vi.advanceTimersByTimeAsync(balanceChainReadDeadlineMs() - 1);
			expect(settled).toBe(false);
			await vi.advanceTimersByTimeAsync(1);
			const tokens = await round;
			expect(failed).toContain(GNOSIS);
			expect(chainIdsOf(tokens)).toEqual([ARBITRUM]);
		} finally {
			vi.useRealTimers();
		}
	});
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

/**
 * PR 2 note 11 (issue 483): home with an internal fault read "Can't reach
 * Ethereum". A read that never left the app — the pool faulted or never
 * booted, or this code threw before it sent anything — is said apart from a
 * chain that did not answer, so the balance machine can word it as Vela's own.
 */
describe('a read that never left the app', () => {
	it('is reported apart from a chain that did not answer — a subset of the failed', async () => {
		faulted.add(GNOSIS);
		down.add(BASE);
		balances.set(ARBITRUM, ONE);
		let failed: number[] = [];
		let internal: number[] = [];
		await fetchTokens(ADDRESS, {
			forceRefresh: true,
			onFailedChains: (ids, inside) => {
				failed = ids;
				internal = inside;
			}
		});
		expect(failed).toEqual(expect.arrayContaining([GNOSIS, BASE]));
		expect(internal).toEqual([GNOSIS]);
	});

	it('a caller that joins the round hears which failed inside the app too', async () => {
		faulted.add(GNOSIS);
		let joined: number[] = [];
		await Promise.all([
			fetchTokens(ADDRESS),
			fetchTokens(ADDRESS, { onFailedChains: (_ids, inside) => (joined = inside) })
		]);
		expect(joined).toEqual([GNOSIS]);
	});

	it('classifies by where it broke, never by the words', () => {
		const fault = (internal: boolean) =>
			new pool.PoolFailedError('x', { maybeDelivered: false, rateLimited: false, internal });
		expect(readFailedInsideApp(fault(true))).toBe(true);
		expect(readFailedInsideApp(fault(false))).toBe(false);
		// Thrown by this code, not by any read: inside the app.
		expect(readFailedInsideApp(new TypeError('cannot read properties of undefined'))).toBe(true);
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
