/**
 * The web balance_dashboard executor (spec 025 T125): one op ↔ one service
 * call, the fetch stream reaches the sink, cache/privacy bytes round-trip,
 * and the failure twin answers every operation.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { APIToken } from '$lib/services/tokens-model';
import type { BalanceEffect, BalanceStreamSink } from './balance-types';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

const ETH: APIToken = {
	network: 'eth-mainnet',
	chainName: 'Ethereum',
	symbol: 'ETH',
	balance: '1.5',
	decimals: 18,
	logo: null,
	name: 'Ether',
	tokenAddress: null,
	priceUsd: 3000,
	spam: false
};

vi.mock('$lib/services/wallet-api', () => ({
	fetchTokens: vi.fn(
		async (
			_address: string,
			options: {
				onProgress?: (t: APIToken[]) => void;
				onFailedChains?: (ids: number[]) => void;
			} = {}
		) => {
			options.onProgress?.([ETH]);
			options.onFailedChains?.([137]);
			return [ETH];
		}
	)
}));
vi.mock('$lib/services/rpc-pool', () => ({
	getRateLimitedChains: vi.fn(() => new Set([56]))
}));

import { createBalanceExecutor } from './balance-executor';

const effect = (operation: BalanceEffect['operation']): BalanceEffect => ({ id: 1, operation });
const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';

function sink(): BalanceStreamSink & { arrivals: unknown[] } {
	const arrivals: unknown[] = [];
	return {
		arrivals,
		chainAssetsArrived: (address, tokens) => arrivals.push({ address, tokens })
	};
}

beforeEach(() => kv.clear());

describe('fetch_tokens', () => {
	it('streams every progress snapshot to the sink, then settles with both chain sets', async () => {
		const stream = sink();
		const executor = createBalanceExecutor(stream);
		const result = await executor.execute(
			effect({ type: 'fetch_tokens', address: ADDR, force: true, pull: false }),
			new AbortController().signal
		);
		expect(stream.arrivals).toHaveLength(1);
		expect(stream.arrivals[0]).toMatchObject({
			address: ADDR,
			tokens: [{ chain_id: 1, symbol: 'ETH', balance: '1.5', price_usd: 3000 }]
		});
		expect(result).toMatchObject({
			type: 'fetch_settled',
			address: ADDR,
			pull: false,
			failed_chain_ids: [137],
			rate_limited_chain_ids: [56]
		});
	});

	// Send's retry-once rule waits for the NEXT round to end — and a round that
	// errored may change nothing on the view, so the end is said by the
	// executor, settled or errored alike.
	it('says the round ended, settled or errored', async () => {
		const ended: string[] = [];
		const stream = { ...sink(), roundEnded: (address: string) => ended.push(address) };
		const executor = createBalanceExecutor(stream);
		await executor.execute(
			effect({ type: 'fetch_tokens', address: ADDR, force: true, pull: false }),
			new AbortController().signal
		);
		executor.toFailure(
			effect({ type: 'fetch_tokens', address: ADDR, force: false, pull: false }),
			new Error('net')
		);
		expect(ended).toEqual([ADDR, ADDR]);
	});
});

describe('the cache and the privacy byte', () => {
	it('read_balance_cache is null until written, then the written total', async () => {
		const executor = createBalanceExecutor(sink());
		const signal = new AbortController().signal;
		expect(
			await executor.execute(effect({ type: 'read_balance_cache', address: ADDR }), signal)
		).toEqual({ type: 'cached_total_loaded', address: ADDR, usd: null });
		await executor.execute(
			effect({ type: 'write_balance_cache', address: ADDR, usd: 4500 }),
			signal
		);
		expect(
			await executor.execute(effect({ type: 'read_balance_cache', address: ADDR }), signal)
		).toEqual({ type: 'cached_total_loaded', address: ADDR, usd: 4500 });
	});

	it('write_privacy writes the Expo byte', async () => {
		const executor = createBalanceExecutor(sink());
		await executor.execute(
			effect({ type: 'write_privacy', hidden: true }),
			new AbortController().signal
		);
		expect(kv.get('vela.balanceHidden')).toBe('1');
	});
});

describe('start_retry_timer', () => {
	it('resolves early when aborted, still answering its timer id', async () => {
		const executor = createBalanceExecutor(sink());
		const controller = new AbortController();
		const pending = executor.execute(
			effect({ type: 'start_retry_timer', timer_id: 7, ms: 60_000 }),
			controller.signal
		);
		controller.abort();
		expect(await pending).toEqual({ type: 'retry_elapsed', timer_id: 7 });
	});
});

describe('the failure twin', () => {
	it('a failed fetch keeps last-known state (fetch_errored), never a fake zero', () => {
		const executor = createBalanceExecutor(sink());
		expect(
			executor.toFailure(
				effect({ type: 'fetch_tokens', address: ADDR, force: false, pull: true }),
				new Error('net')
			)
		).toEqual({ type: 'fetch_errored', address: ADDR, pull: true });
		expect(
			executor.toFailure(effect({ type: 'read_balance_cache', address: ADDR }), new Error('io'))
		).toEqual({ type: 'cached_total_loaded', address: ADDR, usd: null });
	});
});
