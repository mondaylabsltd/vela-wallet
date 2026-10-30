/**
 * A payment link kept for the wallet (spec 039 Part B): taken once, within
 * the hour, and only ever as the link's own parameters.
 */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { KEPT_PAY_KEY, KEPT_PAY_MS, keepPay, takeKeptPay } from './kept';

const QUERY = { to: '0x' + 'ab'.repeat(20), chain: '1', amount: '0.5' };
const NOW = 1_800_000_000_000;

class MemoryStorage {
	map = new Map<string, string>();
	getItem(key: string) {
		return this.map.get(key) ?? null;
	}
	setItem(key: string, value: string) {
		this.map.set(key, value);
	}
	removeItem(key: string) {
		this.map.delete(key);
	}
}

let store: MemoryStorage;
beforeEach(() => {
	store = new MemoryStorage();
	(globalThis as { localStorage?: unknown }).localStorage = store;
});
afterEach(() => {
	delete (globalThis as { localStorage?: unknown }).localStorage;
});

describe('kept pay link', () => {
	it('is taken once', () => {
		expect(keepPay(QUERY, NOW)).toBe(true);
		expect(takeKeptPay(NOW + 1000)).toEqual(QUERY);
		expect(takeKeptPay(NOW + 2000)).toBeNull();
		expect(store.getItem(KEPT_PAY_KEY)).toBeNull();
	});

	it('waits an hour and no longer', () => {
		keepPay(QUERY, NOW);
		expect(takeKeptPay(NOW + KEPT_PAY_MS)).toEqual(QUERY);
		keepPay(QUERY, NOW);
		expect(takeKeptPay(NOW + KEPT_PAY_MS + 1)).toBeNull();
		// Gone either way: a stale link is dropped, not left for later.
		expect(store.getItem(KEPT_PAY_KEY)).toBeNull();
	});

	it('is not one when the clock went back past a minute', () => {
		keepPay(QUERY, NOW);
		expect(takeKeptPay(NOW - 61_000)).toBeNull();
	});

	it('keeps only the link’s parameters, as strings', () => {
		store.setItem(
			KEPT_PAY_KEY,
			JSON.stringify({ query: { to: QUERY.to, chain: 1, evil: 'x', amount: '2' }, at: NOW })
		);
		expect(takeKeptPay(NOW)).toEqual({ to: QUERY.to, amount: '2' });
	});

	it('reads damage as nothing kept', () => {
		store.setItem(KEPT_PAY_KEY, '{not json');
		expect(takeKeptPay(NOW)).toBeNull();
		store.setItem(KEPT_PAY_KEY, JSON.stringify({ query: 'x', at: NOW }));
		expect(takeKeptPay(NOW)).toBeNull();
	});

	it('says so when storage refuses', () => {
		(globalThis as { localStorage?: unknown }).localStorage = {
			setItem: () => {
				throw new Error('QuotaExceededError');
			}
		};
		expect(keepPay(QUERY, NOW)).toBe(false);
	});
});
