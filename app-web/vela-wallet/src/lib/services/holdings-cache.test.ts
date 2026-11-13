/**
 * The rows the home last showed (home-1, home-3): kept per account AND space,
 * a day at most, and only in the shape the core reads.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';

const kv = new Map<string, string>();
vi.mock('./storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value))
}));

import { dropHoldingsMemory, loadHoldings, saveHoldings } from './holdings-cache';

const USDC: BalanceToken = {
	chain_id: 5_042_002,
	symbol: 'USDC',
	name: 'USDC',
	balance: '0.25',
	decimals: 18,
	token_address: null,
	price_usd: 1,
	spam: false
};
const ADDR = '0x4afB65cbcCcb9b6f64Fd0962c5a55f86cAE5Bdc5';

beforeEach(() => {
	kv.clear();
	dropHoldingsMemory();
	vi.useRealTimers();
});

describe('holdings cache', () => {
	it('keeps rows per account and space, case-blind on the address', async () => {
		await saveHoldings(ADDR, 'testnet', [USDC]);
		expect(await loadHoldings(ADDR.toLowerCase(), 'testnet')).toEqual([USDC]);
		expect(await loadHoldings(ADDR, 'mainnet')).toEqual([]);
	});

	it('survives a reload (the store), and a day-old entry is not handed out', async () => {
		vi.useFakeTimers({ now: 1_000_000 });
		await saveHoldings(ADDR, 'mainnet', [USDC]);
		await Promise.resolve();
		dropHoldingsMemory(); // a reload: memory gone, the store kept
		expect(await loadHoldings(ADDR, 'mainnet')).toEqual([USDC]);
		vi.setSystemTime(1_000_000 + 24 * 60 * 60 * 1000 + 1);
		expect(await loadHoldings(ADDR, 'mainnet')).toEqual([]);
	});

	it('a corrupt store, or a row not in the core’s shape, is read as nothing', async () => {
		kv.set(
			'vela.holdingsCache',
			JSON.stringify({
				[`${ADDR.toLowerCase()}#mainnet`]: { at: Date.now(), tokens: [USDC, { chain_id: 'x' }] },
				junk: { at: 'soon', tokens: [] }
			})
		);
		expect(await loadHoldings(ADDR, 'mainnet')).toEqual([USDC]);
		dropHoldingsMemory();
		kv.set('vela.holdingsCache', '{not json');
		expect(await loadHoldings(ADDR, 'mainnet')).toEqual([]);
	});
});
