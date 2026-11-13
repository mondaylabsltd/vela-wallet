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
				onFailedChains?: (ids: number[], internal: number[], registry: number[]) => void;
			} = {}
		) => {
			options.onProgress?.([ETH]);
			// Polygon did not answer; Optimism's read never left the app; Tempo's
			// token list could not be loaded.
			options.onFailedChains?.([137, 10, 4217], [10], [4217]);
			return [ETH];
		}
	),
	// The real classifier is `wallet-api-chain-outage.test.ts`'s; here, a word.
	readFailedInsideApp: (error: unknown) => error instanceof Error && error.message === 'inside'
}));
vi.mock('$lib/services/rpc-pool', () => ({
	getRateLimitedChains: vi.fn(() => new Set([56])),
	// Owner decision 2026-10-28: the person's own nodes, as the pool says.
	getOwnNodeUnreachedChains: vi.fn(() => [137]),
	getFallbackChains: vi.fn(() => [100])
}));

import { createBalanceExecutor } from './balance-executor';
import { onPrivacyElsewhere, usePrivacyChannel } from './privacy-sync';
import { fetchTokens } from '$lib/services/wallet-api';
import { spaceNetworksSync } from '$lib/services/networks';

const effect = (operation: BalanceEffect['operation']): BalanceEffect => ({ id: 1, operation });
const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';

function sink(): BalanceStreamSink & { arrivals: unknown[] } {
	const arrivals: unknown[] = [];
	return {
		arrivals,
		chainAssetsArrived: (address, generation, tokens) =>
			arrivals.push({ address, generation, tokens })
	};
}

beforeEach(() => kv.clear());

describe('fetch_tokens', () => {
	it('streams every progress snapshot to the sink, then settles with both chain sets', async () => {
		const stream = sink();
		const executor = createBalanceExecutor(stream);
		const result = await executor.execute(
			effect({ type: 'fetch_tokens', address: ADDR, force: true, pull: false, generation: 1 }),
			new AbortController().signal
		);
		expect(stream.arrivals).toHaveLength(1);
		expect(stream.arrivals[0]).toMatchObject({
			// D3: labelled with the fetch's own address and generation.
			address: ADDR,
			generation: 1,
			tokens: [{ chain_id: 1, symbol: 'ETH', balance: '1.5', price_usd: 3000 }]
		});
		expect(result).toMatchObject({
			type: 'fetch_settled',
			address: ADDR,
			pull: false,
			failed_chain_ids: [137, 10, 4217],
			rate_limited_chain_ids: [56],
			// PR 2 note 11: the subset that failed inside the app.
			internal_chain_ids: [10],
			// PR 3 note 4: the subset not read for want of its token list.
			registry_chain_ids: [4217],
			// Owner decision 2026-10-28: of the failed chains, the one read
			// through the person's own node alone; and the chain public nodes
			// served because theirs is not answering — the pool's own lists.
			own_node_chain_ids: [137],
			fallback_chain_ids: [100]
		});
		// Spec 092: the chains the round asked, so the core can tell a network
		// that answered holding nothing from one never read.
		// Spec 105: the space in front's (the mainnet space, here).
		const asked = spaceNetworksSync().map((net) => net.chainId);
		expect(asked).toEqual(expect.arrayContaining([1, 56, 137]));
		expect(result).toMatchObject({ read_chain_ids: asked });
	});

	// Send's retry-once rule waits for the NEXT round to end — and a round that
	// errored may change nothing on the view, so the end is said by the
	// executor, settled or errored alike.
	it('says the round ended, settled or errored', async () => {
		const ended: string[] = [];
		const stream = { ...sink(), roundEnded: (address: string) => ended.push(address) };
		const executor = createBalanceExecutor(stream);
		await executor.execute(
			effect({ type: 'fetch_tokens', address: ADDR, force: true, pull: false, generation: 1 }),
			new AbortController().signal
		);
		executor.toFailure(
			effect({ type: 'fetch_tokens', address: ADDR, force: false, pull: false, generation: 1 }),
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
				effect({ type: 'fetch_tokens', address: ADDR, force: false, pull: true, generation: 1 }),
				new Error('net')
			)
		).toEqual({ type: 'fetch_errored', address: ADDR, pull: true, internal: false });
		// PR 2 note 11: a fetch that threw inside the app says so.
		expect(
			executor.toFailure(
				effect({ type: 'fetch_tokens', address: ADDR, force: false, pull: true, generation: 1 }),
				new Error('inside')
			)
		).toEqual({ type: 'fetch_errored', address: ADDR, pull: true, internal: true });
		expect(
			executor.toFailure(effect({ type: 'read_balance_cache', address: ADDR }), new Error('io'))
		).toEqual({ type: 'cached_total_loaded', address: ADDR, usd: null });
	});
});

/**
 * ms-24: the switcher showed "$0.00" for a multisig whose networks had not
 * answered. An account read that some network did not answer has no total:
 * the row keeps what was known, or shows nothing — never a zero nobody read.
 */
describe('fetch_account_assets (ms-24)', () => {
	it('an account some network did not answer for has no figure yet', async () => {
		const executor = createBalanceExecutor(sink());
		await expect(
			executor.execute(
				effect({ type: 'fetch_account_assets', address: ADDR }),
				new AbortController().signal
			)
		).resolves.toEqual({ type: 'account_assets_fetched', address: ADDR, tokens: null });
	});

	it('an account every network answered for is its figure', async () => {
		vi.mocked(fetchTokens).mockImplementationOnce(async () => [ETH]);
		const executor = createBalanceExecutor(sink());
		await expect(
			executor.execute(
				effect({ type: 'fetch_account_assets', address: ADDR }),
				new AbortController().signal
			)
		).resolves.toMatchObject({ tokens: [{ chain_id: 1, symbol: 'ETH' }] });
	});
});

/** home-12: another tab hears the person's privacy choice as it is written. */
describe('the privacy byte reaches the other tabs (home-12)', () => {
	it('announces the written choice', async () => {
		const posted: unknown[] = [];
		usePrivacyChannel({ postMessage: (m) => posted.push(m), addEventListener: () => {} });
		try {
			const executor = createBalanceExecutor(sink());
			await executor.execute(
				effect({ type: 'write_privacy', hidden: true }),
				new AbortController().signal
			);
			expect(posted).toEqual([{ hidden: true }]);
		} finally {
			usePrivacyChannel(null);
		}
	});

	it('hears another tab’s choice, and nothing that is not one', () => {
		let listener: (event: { data: unknown }) => void = () => {};
		usePrivacyChannel({ postMessage: () => {}, addEventListener: (_t, l) => (listener = l) });
		try {
			const heard: boolean[] = [];
			onPrivacyElsewhere((hidden) => heard.push(hidden));
			listener({ data: { hidden: false } });
			listener({ data: { hidden: 'yes' } });
			listener({ data: null });
			expect(heard).toEqual([false]);
		} finally {
			usePrivacyChannel(null);
		}
	});
});
