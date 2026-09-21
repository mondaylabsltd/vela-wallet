/**
 * The storage page's accounting (spec 028 Phase 8): every key belongs to one
 * drawn row, "clear all caches" is exactly the cache group, and the wallet's
 * own keys are never a cache. The rows are the core's catalog (spec 072), so
 * these run over the real one.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, test } from 'vitest';
import { storageItems } from '$lib/core/client';
import {
	clearAllCaches,
	clearStorageItem,
	formatBytes,
	groupOfKey,
	isCacheKey,
	itemOfKey,
	measureDeviceStorage,
	STORAGE_ITEM_IDS
} from './device-storage';

// The kv store is IndexedDB; under node the local half is the whole store.
const local = new Map<string, string>();
const localStorageShim = {
	get length() {
		return local.size;
	},
	key: (i: number) => [...local.keys()][i] ?? null,
	getItem: (k: string) => local.get(k) ?? null,
	setItem: (k: string, v: string) => void local.set(k, v),
	removeItem: (k: string) => void local.delete(k),
	clear: () => local.clear()
};
Object.defineProperty(globalThis, 'localStorage', { value: localStorageShim, configurable: true });

beforeEach(() => local.clear());

describe('classification', () => {
	test('the wallet itself is your data, never a cache', () => {
		expect(itemOfKey('vela.accounts')).toBeNull();
		expect(groupOfKey('vela.accounts')).toBe('user');
		expect(isCacheKey('vela.accounts')).toBe(false);
		expect(isCacheKey('vela.activeAccountIndex')).toBe(false);
		expect(isCacheKey('vela.pendingUploads')).toBe(false);
	});

	test('each drawn row owns its keys', () => {
		expect(itemOfKey('vela.transactionHistory')).toBe('transactions');
		expect(itemOfKey('vela.contacts')).toBe('contacts');
		expect(itemOfKey('vela.contactGroups')).toBe('contacts');
		expect(itemOfKey('vela.customNetworks')).toBe('custom');
		expect(itemOfKey('vela.customTokens')).toBe('custom');
		expect(itemOfKey('vela.balanceCache')).toBe('balances');
		expect(itemOfKey('vela.fiatRates.v1')).toBe('rates');
		expect(itemOfKey('vela.fxRates.v1')).toBe('rates');
		expect(itemOfKey('vela.tokenMeta.1.0xabc')).toBe('scan');
		expect(itemOfKey('recipient_id:0xabc')).toBe('scan');
		expect(itemOfKey('vela.perm.https://app.example')).toBe('dapps');
		expect(itemOfKey('vela.req.r1')).toBe('dapps');
		expect(itemOfKey('vela.ext.cache')).toBe('dapps');
	});

	test("the drawn rows are the catalog's, in its order", () => {
		// The fixture draws these ids and the core files keys under them; a row
		// one side renamed would draw a meta line nobody measures.
		const rows = JSON.parse(storageItems()) as { id: string }[];
		expect(rows.map((row) => row.id)).toEqual(STORAGE_ITEM_IDS);
	});

	test('"tokens and networks" is not the API keys or the endpoints (spec 072)', () => {
		// The web's own table filed both under `custom`, so clearing custom
		// tokens deleted a person's RPC provider keys and their self-hosted
		// endpoints. They have their own pages, and their own reset.
		expect(itemOfKey('vela.networkConfig')).toBe('custom');
		expect(itemOfKey('vela.rpcProviders')).toBeNull();
		expect(itemOfKey('vela.serviceEndpoints')).toBeNull();
	});

	test('the cache group is exactly what "clear all caches" sweeps', () => {
		for (const key of [
			'vela.balanceCache',
			'vela.fiatRates.v1',
			'vela.fxRates.v1',
			'vela.fiatFeedAddrs.v1',
			'vela.tokenMeta.137.0x1',
			'vela.rpc.banned'
		]) {
			expect(isCacheKey(key), key).toBe(true);
		}
		// `balanceHidden` is a preference: filed as a cache, "clear all caches"
		// un-hid every balance (the phones did, spec 072).
		for (const key of [
			'vela.contacts',
			'vela.transactionHistory',
			'vela.perm.x',
			'vela.theme',
			'vela.balanceHidden'
		]) {
			expect(isCacheKey(key), key).toBe(false);
		}
	});
});

describe('measure and clear', () => {
	test('counts records, sites and bytes per row and group', async () => {
		local.set('vela.accounts', JSON.stringify([{ id: 'a' }]));
		local.set('vela.transactionHistory', JSON.stringify([{ id: 't1' }, { id: 't2' }]));
		local.set('vela.contacts', JSON.stringify([{ address: '0x1' }]));
		local.set('vela.balanceCache', JSON.stringify({ '0x1': 12 }));
		local.set('vela.perm.https://a.example', '{}');
		local.set('vela.perm.https://b.example', '{}');
		local.set('vela.req.r1', '{}');
		local.set('unrelated.key', 'not ours');

		const report = await measureDeviceStorage();
		expect(report.keyCount).toBe(7);
		expect(report.items.transactions.count).toBe(2);
		expect(report.items.contacts.count).toBe(1);
		expect(report.items.dapps.count).toBe(2);
		expect(report.items.balances.count).toBe(1);
		expect(report.groups.user).toBeGreaterThan(0);
		expect(report.groups.cache).toBe(report.items.balances.bytes);
		expect(report.groups.sessions).toBe(report.items.dapps.bytes);
		expect(report.totalBytes).toBe(
			report.groups.user + report.groups.cache + report.groups.sessions
		);
	});

	test('clearing caches leaves your data and sessions byte-identical', async () => {
		local.set('vela.accounts', '[1]');
		local.set('vela.contacts', '[2]');
		local.set('vela.balanceCache', '{}');
		local.set('vela.fxRates.v1', '{}');
		local.set('vela.tokenMeta.1.0x1', '{}');
		local.set('vela.perm.https://a.example', '{}');

		const removed = await clearAllCaches();
		expect([...removed].sort()).toEqual([
			'vela.balanceCache',
			'vela.fxRates.v1',
			'vela.tokenMeta.1.0x1'
		]);
		expect(local.get('vela.accounts')).toBe('[1]');
		expect(local.get('vela.contacts')).toBe('[2]');
		expect(local.get('vela.perm.https://a.example')).toBe('{}');
	});

	test('clearing custom tokens and networks keeps the provider keys and endpoints', async () => {
		local.set('vela.customTokens', '[1]');
		local.set('vela.customNetworks', '[2]');
		local.set('vela.rpcProviders', '{"alchemy":"k"}');
		local.set('vela.serviceEndpoints', '{"ethereum_data":"https://data.example"}');
		const removed = await clearStorageItem('custom');
		expect([...removed].sort()).toEqual(['vela.customNetworks', 'vela.customTokens']);
		expect(local.get('vela.rpcProviders')).toBe('{"alchemy":"k"}');
		expect(local.get('vela.serviceEndpoints')).toBe('{"ethereum_data":"https://data.example"}');
	});

	test('a row clears only its own keys', async () => {
		local.set('vela.transactionHistory', '[1]');
		local.set('vela.contacts', '[2]');
		local.set('vela.contactGroups', '[3]');
		const removed = await clearStorageItem('contacts');
		expect([...removed].sort()).toEqual(['vela.contactGroups', 'vela.contacts']);
		expect(local.get('vela.transactionHistory')).toBe('[1]');
	});
});

describe('formatBytes', () => {
	test('kilobytes under a megabyte, one decimal above', () => {
		expect(formatBytes(0)).toEqual({ amount: '0', unit: 'KB' });
		expect(formatBytes(42 * 1024)).toEqual({ amount: '42', unit: 'KB' });
		expect(formatBytes(2.4 * 1024 * 1024)).toEqual({ amount: '2.4', unit: 'MB' });
	});
});
