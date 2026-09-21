/**
 * What this device holds for Vela, and what each "Clear" on the storage page
 * removes (spec 028 Phase 8; the drawn ST13 / DST7).
 *
 * 023 drew the page with fixture numbers and 027 wired only its connections
 * group. This is the accounting behind the other two groups, and the one
 * place the page's clears are carried out — as key lists, so a row's "Clear"
 * and the page's byte count cannot disagree about what a row IS.
 *
 * ## Whose rule
 *
 * Which row a key belongs to, which group a row is in and what "clear all
 * caches" takes are the CORE's (`vela_core::storage_catalog`, spec 072): four
 * shells kept four copies of this table and they drifted — the phones filed a
 * preference as a balance cache, and this file's "custom" row deleted the RPC
 * provider keys and the service endpoints along with the tokens and networks
 * it names. What stays here is what only a browser can do: enumerate its two
 * stores (`localStorage` and the IndexedDB `kv` behind `storage.ts`), count
 * bytes, delete, and empty the memory caches beside them.
 *
 * A key the catalog names no row for is "your data": the safe default for a
 * byte count, because it is then never swept by "clear all caches". The
 * wallet itself (`vela.accounts`, `vela.activeAccountIndex`) and the
 * preferences are such keys — no storage row clears them; that is sign-out's
 * and erase's job.
 *
 * Every function here that reads the catalog runs after `loadCore()`; the
 * exported async ones await it themselves.
 */

import {
	loadCore,
	storageIsCacheKey,
	storageIsOurs,
	storageItemOfKey,
	storageItems,
	storageRecordsIn
} from '$lib/core/client';
import { PERM_PREFIX } from '$lib/dapp/keys';
import { clearRecipientRiskCache } from './recipient-risk';
import { invalidateAllPools } from './rpc-pool';
import { clearSelectorCache } from './selector-registry';
import { getAllKeys, getItem, removeItem } from './storage';
import { clearTokenMetadataCache } from './token-metadata';
import { clearTokenCache } from './wallet-api';

export type StorageGroupId = 'user' | 'cache' | 'sessions';

/** The drawn rows, by the ids the fixture layer gives them — the catalog's own. */
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

let catalogRows: ReadonlyMap<StorageItemId, StorageGroupId> | null = null;

/** The catalog's rows and their groups, read once from the core. */
function catalog(): ReadonlyMap<StorageItemId, StorageGroupId> {
	catalogRows ??= new Map(
		(JSON.parse(storageItems()) as { id: StorageItemId; group: StorageGroupId }[]).map((row) => [
			row.id,
			row.group
		])
	);
	return catalogRows;
}

/** Which drawn row a key belongs to, or `null` for your data nobody named. */
export function itemOfKey(key: string): StorageItemId | null {
	return (storageItemOfKey(key) as StorageItemId | undefined) ?? null;
}

/** Which group a row is in. */
export function groupOfItem(id: StorageItemId): StorageGroupId {
	return catalog().get(id) ?? 'user';
}

/** Which group a key counts toward. */
export function groupOfKey(key: string): StorageGroupId {
	const item = itemOfKey(key);
	return item === null ? 'user' : groupOfItem(item);
}

/** Would "clear all caches" remove this key? Exactly the cache group's keys. */
export function isCacheKey(key: string): boolean {
	return storageIsCacheKey(key);
}

/**
 * Is this one of ours at all? The namespace and the one unprefixed cache the
 * catalog knows — including the key the erase keeps on purpose, which is
 * still Vela's on this device (`storage_catalog::is_ours`).
 */
function isOurs(key: string): boolean {
	return storageIsOurs(key);
}

export interface StorageItemReport {
	/** The row's group, as the catalog files it. */
	group: StorageGroupId;
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

/**
 * How many records a stored list holds — the core's reading, which also finds
 * the list inside `{items|contacts|entries|records|grants: [...]}`; anything
 * that is not a list counts as one.
 */
function recordsIn(value: string): number {
	return storageRecordsIn(value) ?? 1;
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
	for (const id of STORAGE_ITEM_IDS) items[id] = { group: groupOfItem(id), bytes: 0, count: 0 };
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
	await loadCore();
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
	await loadCore();
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
	await loadCore();
	const doomed = (await entries()).filter((entry) => itemOfKey(entry.key) === id);
	const removed = await drop(doomed);
	if (groupOfItem(id) === 'cache') dropMemoryCaches();
	return removed;
}

/** "1.0 MB" / "42 KB", the drawn units. Under a kilobyte still reads in KB. */
export function formatBytes(bytes: number): { amount: string; unit: 'KB' | 'MB' } {
	if (bytes >= 1024 * 1024) return { amount: (bytes / (1024 * 1024)).toFixed(1), unit: 'MB' };
	return { amount: Math.max(0, Math.round(bytes / 1024)).toString(), unit: 'KB' };
}
