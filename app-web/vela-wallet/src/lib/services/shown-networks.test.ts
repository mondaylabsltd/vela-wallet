/**
 * Owner decision 2026-10-29: Stable, MegaETH, Plume and XRPL EVM are out of
 * the default lists. A device with no record is settled once, by the core,
 * from what this browser holds — and a network holding the person's money
 * as far as it knows is never the one taken away.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

import { setItem } from '$lib/services/storage';
import { CHAINS } from './chains';
import {
	ensureShownNetworks,
	isHiddenNetwork,
	networkRank,
	resetNetworksForTests,
	spaceNetworksSync
} from './networks';
import { SHOWN_NETWORKS_KEY } from './shown-networks';

const PLUME = 98_866;
const FOUR = [988, 4326, PLUME, 1_440_000];
/** The owner's order for the twenty shown by default. */
const TWENTY = [
	1, 56, 8453, 137, 42161, 43114, 10, 143, 4663, 5042, 196, 5000, 100, 4217, 480, 42220, 130, 57073,
	1868, 8217
];

const mainnets = () => spaceNetworksSync().map((n) => n.chainId);
const stored = () => JSON.parse(kv.get(SHOWN_NETWORKS_KEY) ?? 'null') as { shown: number[] } | null;

function holding(chainId: number, balance: string, spam = false) {
	return {
		chain_id: chainId,
		symbol: 'X',
		name: 'X',
		balance,
		decimals: 18,
		token_address: null,
		price_usd: null,
		spam
	};
}

beforeEach(() => {
	kv.clear();
	vi.mocked(setItem).mockClear();
	resetNetworksForTests();
});

describe('the table', () => {
	it('is the core’s order: the twenty, then the four shown on the person’s word', () => {
		const mainnet = CHAINS.filter((c) => !c.testnet);
		expect(mainnet.map((c) => c.chainId)).toEqual([...TWENTY, ...FOUR]);
		expect(mainnet.filter((c) => c.shownByDefault === false).map((c) => c.chainId)).toEqual(FOUR);
		expect(networkRank(1)).toBe(0);
		expect(networkRank(8453)).toBeLessThan(networkRank(137));
		expect(networkRank(424242)).toBeGreaterThan(networkRank(42431));
	});
});

describe('a device settled once', () => {
	it('a fresh profile shows the twenty, in order, and stores the record', async () => {
		await ensureShownNetworks();
		expect(mainnets()).toEqual(TWENTY);
		expect(stored()).toEqual({ version: 1, shown: [] });
		for (const id of FOUR) expect(isHiddenNetwork(id)).toBe(true);
	});

	it('nothing is hidden before it is settled', () => {
		expect(mainnets()).toEqual([...TWENTY, ...FOUR]);
	});

	it('a last-known Plume balance keeps Plume shown, after Kaia', async () => {
		kv.set(
			'vela.holdingsCache',
			JSON.stringify({
				'0xabc#mainnet': {
					at: 1,
					tokens: [holding(1, '0'), holding(PLUME, '1000000000000000000')]
				}
			})
		);
		await ensureShownNetworks();
		expect(mainnets()).toEqual([...TWENTY, PLUME]);
		expect(stored()?.shown).toEqual([PLUME]);
	});

	it('a spam airdrop or an empty row is not money', async () => {
		kv.set(
			'vela.holdingsCache',
			JSON.stringify({
				'0xabc#mainnet': { at: 1, tokens: [holding(PLUME, '5', true), holding(988, '0')] }
			})
		);
		await ensureShownNetworks();
		expect(mainnets()).toEqual(TWENTY);
	});

	it('activity, a token added there, or the person’s own node keeps it shown', async () => {
		kv.set('vela.transactionHistory', JSON.stringify([{ id: 't1', chainId: 988 }]));
		kv.set('vela.customTokens', JSON.stringify([{ id: '4326_0x1', chainId: 4326 }]));
		kv.set(
			'vela.networkConfig',
			JSON.stringify([{ chainId: 1_440_000, rpcURL: 'https://my-node.example' }])
		);
		await ensureShownNetworks();
		expect(mainnets()).toEqual([...TWENTY, 988, 4326, 1_440_000]);
	});

	it('Vela’s own values written back by an opened card are not the person’s settings', async () => {
		kv.set(
			'vela.networkConfig',
			JSON.stringify([
				{
					chainId: PLUME,
					rpcURL: 'https://rpc.plume.org',
					explorerURL: 'https://explorer.plume.org'
				}
			])
		);
		await ensureShownNetworks();
		expect(mainnets()).toEqual(TWENTY);
		// …and the record leaves the setting where it was.
		expect(JSON.parse(kv.get('vela.networkConfig') ?? '[]')).toHaveLength(1);
	});

	it('a stored record is the person’s word: never settled again', async () => {
		kv.set(SHOWN_NETWORKS_KEY, JSON.stringify({ version: 1, shown: [] }));
		kv.set(
			'vela.holdingsCache',
			JSON.stringify({ '0xabc#mainnet': { at: 1, tokens: [holding(PLUME, '7')] } })
		);
		await ensureShownNetworks();
		expect(mainnets()).toEqual(TWENTY);
		expect(setItem).not.toHaveBeenCalledWith(SHOWN_NETWORKS_KEY, expect.any(String));
	});
});
