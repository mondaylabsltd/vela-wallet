/**
 * What this device holds for Vela, and what each "Clear" on the storage page
 * removes (spec 028 Phase 8; the drawn ST13 / DST7).
 *
 * 023 drew the page with fixture numbers and 027 wired only its connections
 * group. This is the accounting behind the other two groups, and the one
 * place the page's clears are defined — as key lists, so a row's "Clear"
 * and the page's byte count cannot disagree about what a row IS.
 *
 * ## The rule
 *
 * Every key this app writes is `vela.`-namespaced (`erase-device.ts` scans
 * that prefix), in one of two stores: `localStorage` and the IndexedDB `kv`
 * store behind `storage.ts`. Each key belongs to exactly one drawn row, by
 * name or by prefix, and every row belongs to one of the three drawn groups.
 * A key nobody named is "your data": the safe default for a byte count,
 * because it is then never swept by "clear all caches".
 *
 * Two keys are deliberately NOT here: `vela.accounts` and
 * `vela.activeAccountIndex` are the wallet itself, and no storage row clears
 * them — that is sign-out's and erase's job. They are counted as your data.
 */

import { EXT_CACHE_KEY, PERM_PREFIX, REQUEST_PREFIX } from '$lib/dapp/keys';
import { VELA_KEY_PREFIX } from './erase-device';
import { clearRecipientRiskCache } from './recipient-risk';
import { invalidateAllPools } from './rpc-pool';
import { clearSelectorCache } from './selector-registry';
import { getAllKeys, getItem, removeItem } from './storage';
import { clearTokenMetadataCache } from './token-metadata';
import { clearTokenCache } from './wallet-api';

export type StorageGroupId = 'user' | 'cache' | 'sessions';

/** The drawn rows, by the ids the fixture layer already gives them. */
export type StorageItemId =
	'transactions' | 'contacts' | 'custom' | 'browsing' | 'balances' | 'rates' | 'scan' | 'dapps';

export const STORAGE_ITEM_IDS: readonly StorageItemId[] = [
	'transactions',
	'contacts',
	'custom',
	'browsing',
	'balances',
	'rates',
	'scan',
	'dapps'
];

export const GROUP_OF_ITEM: Record<StorageItemId, StorageGroupId> = {
	transactions: 'user',
	contacts: 'user',
	custom: 'user',
	browsing: 'user',
	balances: 'cache',
	rates: 'cache',
	scan: 'cache',
	dapps: 'sessions'
};

/** Exact keys, per row. */
const ITEM_KEYS: Record<StorageItemId, readonly string[]> = {
	transactions: ['vela.transactionHistory'],
	contacts: ['vela.contacts', 'vela.contactGroups', 'vela.contacts.dismissed'],
	custom: [
		'vela.customNetworks',
		'vela.customTokens',
		'vela.networkConfig',
		'vela.rpcProviders',
		'vela.serviceEndpoints'
	],
	// The web has no in-app browser (spec 022): its history key is native-only,
	// and the row honestly reads zero here.
	browsing: [],
	balances: ['vela.balanceCache'],
	rates: ['vela.fiatRates.v1', 'vela.fxRates.v1', 'vela.fiatFeedAddrs.v1'],
	scan: ['vela.rpc.banned'],
	dapps: [EXT_CACHE_KEY]
};

/** Key prefixes, per row — for the stores that write one key per subject. */
const ITEM_PREFIXES: Record<StorageItemId, readonly string[]> = {
	transactions: [],
	contacts: [],
	custom: [],
	browsing: [],
	balances: [],
	rates: [],
	// Per-token metadata, per-address identity lookups, the transfer scan's
	// cursors: everything a scan rebuilds on its own.
	scan: ['vela.tokenMeta.', 'recipient_id:', 'vela.scan'],
	dapps: [PERM_PREFIX, REQUEST_PREFIX]
};

/** Which drawn row a key belongs to, or `null` for your data nobody named. */
export function itemOfKey(key: string): StorageItemId | null {
	for (const id of STORAGE_ITEM_IDS) {
		if (ITEM_KEYS[id].includes(key)) return id;
	}
	for (const id of STORAGE_ITEM_IDS) {
		if (ITEM_PREFIXES[id].some((prefix) => key.startsWith(prefix))) return id;
	}
	return null;
}

/** Which group a key counts toward. */
export function groupOfKey(key: string): StorageGroupId {
	const item = itemOfKey(key);
	return item === null ? 'user' : GROUP_OF_ITEM[item];
}

/** Would "clear all caches" remove this key? Exactly the cache group's keys. */
export function isCacheKey(key: string): boolean {
	return groupOfKey(key) === 'cache';
}

/** Is this one of ours at all? The `vela.` namespace, plus the one unprefixed cache. */
function isOurs(key: string): boolean {
	return key.startsWith(VELA_KEY_PREFIX) || key.startsWith('recipient_id:');
}

export interface StorageItemReport {
	bytes: number;
	/** Records for the user rows and sites for dApps; keys for the caches. */
	count: number;
}

export interface DeviceStorageReport {
	totalBytes: number;
	/** Every key of ours, both stores. */
	keyCount: number;
	groups: Record<StorageGroupId, number>;
	items: Record<StorageItemId, StorageItemReport>;
}

interface Entry {
	key: string;
	value: string;
	where: 'local' | 'kv';
}

function utf8Bytes(text: string): number {
	return new TextEncoder().encode(text).length;
}

/** How many things a JSON array holds; a non-array value counts as one. */
function recordsIn(value: string): number {
	try {
		const parsed: unknown = JSON.parse(value);
		return Array.isArray(parsed) ? parsed.length : 1;
	} catch {
		return 1;
	}
}

async function entries(): Promise<Entry[]> {
	const out: Entry[] = [];
	if (typeof localStorage !== 'undefined') {
		for (let i = 0; i < localStorage.length; i += 1) {
			const key = localStorage.key(i);
			if (key === null || !isOurs(key)) continue;
			out.push({ key, value: localStorage.getItem(key) ?? '', where: 'local' });
		}
	}
	let kvKeys: string[] = [];
	try {
		kvKeys = await getAllKeys();
	} catch {
		// No IndexedDB (a private window, a test runner): the local half stands.
	}
	for (const key of kvKeys) {
		if (!isOurs(key)) continue;
		const value = (await getItem(key).catch(() => null)) ?? '';
		out.push({ key, value, where: 'kv' });
	}
	return out;
}

function emptyReport(): DeviceStorageReport {
	const items = {} as Record<StorageItemId, StorageItemReport>;
	for (const id of STORAGE_ITEM_IDS) items[id] = { bytes: 0, count: 0 };
	return { totalBytes: 0, keyCount: 0, groups: { user: 0, cache: 0, sessions: 0 }, items };
}

/** Count records, sites and keys the way each row's meta line reads them. */
function countFor(id: StorageItemId, entry: Entry): number {
	switch (id) {
		case 'transactions':
		case 'contacts':
		case 'custom':
			return recordsIn(entry.value);
		case 'dapps':
			// One grant per site; the request and cache keys are not sites.
			return entry.key.startsWith(PERM_PREFIX) ? 1 : 0;
		default:
			return 1;
	}
}

/** Measure both stores. Pure reading; nothing is touched. */
export async function measureDeviceStorage(): Promise<DeviceStorageReport> {
	const report = emptyReport();
	for (const entry of await entries()) {
		const bytes = utf8Bytes(entry.key) + utf8Bytes(entry.value);
		report.totalBytes += bytes;
		report.keyCount += 1;
		report.groups[groupOfKey(entry.key)] += bytes;
		const item = itemOfKey(entry.key);
		if (item !== null) {
			report.items[item].bytes += bytes;
			report.items[item].count += countFor(item, entry);
		}
	}
	return report;
}

async function drop(doomed: Entry[]): Promise<string[]> {
	for (const entry of doomed) {
		if (entry.where === 'local') localStorage.removeItem(entry.key);
		else await removeItem(entry.key).catch(() => {});
	}
	return doomed.map((entry) => entry.key);
}

/** The in-memory caches beside the stored ones; a clear empties both. */
function dropMemoryCaches(): void {
	clearTokenMetadataCache();
	clearTokenCache();
	clearSelectorCache();
	clearRecipientRiskCache();
	invalidateAllPools();
}

/**
 * The page's "Clear all caches": every key of the cache group, both stores,
 * and the memory caches beside them. Resolves with what was removed. Your
 * data and sessions are untouched by construction (`isCacheKey`).
 */
export async function clearAllCaches(): Promise<readonly string[]> {
	const doomed = (await entries()).filter((entry) => isCacheKey(entry.key));
	const removed = await drop(doomed);
	dropMemoryCaches();
	return removed;
}

/**
 * One row's "Clear". The dApp row is NOT here: a grant is `dapp_permissions`'
 * to revoke (`$lib/dapp/connections`), and the settings route routes it there.
 */
export async function clearStorageItem(
	id: Exclude<StorageItemId, 'dapps'>
): Promise<readonly string[]> {
	const doomed = (await entries()).filter((entry) => itemOfKey(entry.key) === id);
	const removed = await drop(doomed);
	if (GROUP_OF_ITEM[id] === 'cache') dropMemoryCaches();
	return removed;
}

/** "1.0 MB" / "42 KB", the drawn units. Under a kilobyte still reads in KB. */
export function formatBytes(bytes: number): { amount: string; unit: 'KB' | 'MB' } {
	if (bytes >= 1024 * 1024) return { amount: (bytes / (1024 * 1024)).toFixed(1), unit: 'MB' };
	return { amount: Math.max(0, Math.round(bytes / 1024)).toString(), unit: 'KB' };
}
