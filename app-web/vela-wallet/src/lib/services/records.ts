/**
 * Readers/writers for the stored records the services share — WEB (spec 025).
 *
 * The Expo client keeps these in `services/storage.ts`; here they live in one
 * small module over the IndexedDB KV so every consumer reads the same bytes
 * the settings executors write (same keys, same camelCase shapes — the
 * cross-client compatibility contract). Readers trust like the Expo loaders
 * did: absent/corrupt reads as empty, never throws.
 */

import { getItem, setItem } from './storage';
import type { CustomToken } from './tokens-model';
import type { LocalTransaction } from './transactions-model';

/** The Expo `CustomNetwork` stored shape (camelCase), verbatim. */
export interface CustomNetworkRecord {
	id: string;
	displayName: string;
	chainId: number;
	iconLabel: string;
	iconColor: string;
	iconBg: string;
	logoURL: string;
	isL2: boolean;
	rpcURL: string;
	explorerURL: string;
	bundlerURL: string;
	nativeSymbol: string;
	addedAt: string;
}

export interface NetworkConfigRecord {
	chainId: number;
	rpcURL?: string;
	explorerURL?: string;
	bundlerURL?: string;
}

export type RpcProviderKeys = Partial<Record<'alchemy' | 'drpc' | 'ankr', string>>;

async function loadArray<T>(key: string): Promise<T[]> {
	try {
		const raw = await getItem(key);
		if (!raw) return [];
		const parsed: unknown = JSON.parse(raw);
		return Array.isArray(parsed) ? (parsed as T[]) : [];
	} catch {
		return [];
	}
}

export async function loadCustomNetworks(): Promise<CustomNetworkRecord[]> {
	return loadArray<CustomNetworkRecord>('vela.customNetworks');
}

export async function getNetworkConfig(chainId: number): Promise<NetworkConfigRecord | null> {
	const configs = await loadArray<NetworkConfigRecord>('vela.networkConfig');
	return configs.find((c) => c.chainId === chainId) ?? null;
}

export async function getRpcProviderKeys(): Promise<RpcProviderKeys> {
	try {
		const raw = await getItem('vela.rpcProviders');
		if (!raw) return {};
		const parsed: unknown = JSON.parse(raw);
		return parsed !== null && typeof parsed === 'object' ? (parsed as RpcProviderKeys) : {};
	} catch {
		return {};
	}
}

// ---------------------------------------------------------------------------
// The local transaction store (spec 025 D14) — Expo `storage.ts` semantics.
// ---------------------------------------------------------------------------

const TX_KEY = 'vela.transactionHistory';
/** Same cap the Expo store applies: the newest 200 records. */
const TX_CAP = 200;

/** Writers queue behind one another — the `withTxLock` port. */
let txTail: Promise<unknown> = Promise.resolve();
function withTxLock<T>(task: () => Promise<T>): Promise<T> {
	const next = txTail.then(task, task);
	txTail = next.catch(() => undefined);
	return next;
}

export async function loadTransactions(): Promise<LocalTransaction[]> {
	return loadArray<LocalTransaction>(TX_KEY);
}

/**
 * Merge new records in: dedupe by id, newest first, capped. Answers how many
 * were actually new — the count the feed's celebration is built on.
 */
export async function mergeTransactions(incoming: LocalTransaction[]): Promise<number> {
	if (incoming.length === 0) return 0;
	return withTxLock(async () => {
		const existing = await loadTransactions();
		const ids = new Set(existing.map((t) => t.id));
		const fresh = incoming.filter((t) => !ids.has(t.id));
		if (fresh.length === 0) return 0;
		const merged = [...fresh, ...existing].sort((a, b) => b.timestamp - a.timestamp);
		if (merged.length > TX_CAP) merged.length = TX_CAP;
		await setItem(TX_KEY, JSON.stringify(merged));
		return fresh.length;
	});
}

export async function deleteTransaction(id: string): Promise<void> {
	return withTxLock(async () => {
		const txs = await loadTransactions();
		const next = txs.filter((t) => t.id !== id);
		if (next.length === txs.length) return;
		await setItem(TX_KEY, JSON.stringify(next));
	});
}

/**
 * Remove several records in ONE atomic read-modify-write — a write-ahead
 * send's siblings, proven never sent (spec 082 RJ1). A no-op for ids not there.
 */
export async function deleteTransactions(ids: string[]): Promise<void> {
	if (ids.length === 0) return;
	const idSet = new Set(ids);
	return withTxLock(async () => {
		const txs = await loadTransactions();
		const next = txs.filter((t) => !idSet.has(t.id));
		if (next.length === txs.length) return;
		await setItem(TX_KEY, JSON.stringify(next));
	});
}

/**
 * Persist ONE record: de-duped by id (a resubmitted UserOp shares its hash =
 * the record id), newest first, capped at 200. Ported from storage.ts:453.
 */
export async function saveTransaction(tx: LocalTransaction): Promise<void> {
	return withTxLock(async () => {
		const txs = (await loadTransactions()).filter((t) => t.id !== tx.id);
		txs.unshift(tx);
		if (txs.length > 200) txs.length = 200;
		await setItem(TX_KEY, JSON.stringify(txs));
	});
}

/**
 * Persist several records in ONE atomic read-modify-write — the batch send's
 * siblings (split / multi-select). A per-record `Promise.all` raced the RMW
 * and collapsed a batch to one line in Activity. Ported from storage.ts:472.
 */
export async function saveTransactions(incoming: LocalTransaction[]): Promise<void> {
	if (incoming.length === 0) return;
	return withTxLock(async () => {
		const ids = new Set(incoming.map((t) => t.id));
		const txs = (await loadTransactions()).filter((t) => !ids.has(t.id));
		txs.unshift(...incoming);
		if (txs.length > 200) txs.length = 200;
		await setItem(TX_KEY, JSON.stringify(txs));
	});
}

/** Patch one record by id (pending → confirmed); no-op when absent. */
export async function updateTransaction(
	id: string,
	patch: Partial<LocalTransaction>
): Promise<void> {
	return withTxLock(async () => {
		const txs = await loadTransactions();
		const idx = txs.findIndex((t) => t.id === id);
		if (idx === -1) return;
		txs[idx] = { ...txs[idx], ...patch };
		await setItem(TX_KEY, JSON.stringify(txs));
	});
}

/** The same patch over several ids in ONE atomic RMW (a batch's shared receipt). */
export async function updateTransactions(
	ids: string[],
	patch: Partial<LocalTransaction>
): Promise<void> {
	if (ids.length === 0) return;
	const idSet = new Set(ids);
	return withTxLock(async () => {
		const txs = await loadTransactions();
		let changed = false;
		for (let i = 0; i < txs.length; i++) {
			if (idSet.has(txs[i].id)) {
				txs[i] = { ...txs[i], ...patch };
				changed = true;
			}
		}
		if (changed) await setItem(TX_KEY, JSON.stringify(txs));
	});
}

export async function loadCustomTokens(): Promise<CustomToken[]> {
	return loadArray<CustomToken>('vela.customTokens');
}

/** Upsert by id — the Expo `saveCustomToken` contract. */
export async function saveCustomToken(token: CustomToken): Promise<void> {
	const tokens = await loadCustomTokens();
	const rest = tokens.filter((t) => t.id !== token.id);
	rest.push(token);
	await setItem('vela.customTokens', JSON.stringify(rest));
}

export async function removeCustomToken(id: string): Promise<void> {
	const tokens = await loadCustomTokens();
	await setItem('vela.customTokens', JSON.stringify(tokens.filter((t) => t.id !== id)));
}
