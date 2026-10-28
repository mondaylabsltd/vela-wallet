/**
 * The snapshot the service worker reads (spec 027 T332).
 *
 * Ported from src/services/wallet-state-core/ext-cache-{executor,session}.ts
 * @ 52ad8fa9, with one substitution: on Safari the snapshot is a file in an App
 * Group, because the wallet and the extension are two processes. Here they are
 * the same extension, so it is a key in `chrome.storage.local` — a shorter path
 * to the same place.
 *
 * What it is FOR: a page that is already connected asks `eth_accounts` and
 * `eth_chainId` on every load. Opening a window for those would be absurd, and
 * inventing an answer in the service worker would put a business rule in the
 * worst possible place. So the wallet publishes what the core decided, and the
 * worker only reads it.
 *
 * The core owns everything in the snapshot — the account projection, the chain
 * id (invariant ⑤: always its own constant, never a shell default), the
 * loading gate. This module contributes storage and a clock.
 *
 * Two operations have no counterpart in Chrome and are answered rather than
 * skipped: the Universal-Link attestation exists only because Safari hands
 * signing to a native app, so it is answered "never attested" — which is the
 * truth here — and `request_extension_sign` is acked. Answering beats skipping:
 * an unanswered effect stalls the loop.
 */
import { ExtCacheCore, loadCore } from '$lib/core/client';
import type { Account } from '$lib/core/generated/Account';
import type { ExtCacheOperation } from '$lib/core/generated/ExtCacheOperation';
import type { ExtCacheShellResult } from '$lib/core/generated/ExtCacheShellResult';
import type { ExtSnapshot } from '$lib/core/generated/ExtSnapshot';
import { EXT_CACHE_KEY } from '../keys';

interface DispatchResult {
	effects: { id: number; operation: ExtCacheOperation }[];
}

/** What the wallet observed about itself — facts, not judgements. */
export interface SnapshotFacts {
	isLoading: boolean;
	hasWallet: boolean;
	accounts: Account[];
	active: Account | null;
	theme: string;
	locale: string;
}

/**
 * Recompute the snapshot from the session as it is now, and store what the core
 * authors. Best-effort by contract: a failed write leaves the previous snapshot,
 * and the worker's answers stay as they were.
 */
export async function publishExtSnapshot(facts: SnapshotFacts): Promise<ExtSnapshot | null> {
	await loadCore();
	const core = new ExtCacheCore();
	let written: ExtSnapshot | null = null;
	try {
		const dispatch = (event: unknown): DispatchResult =>
			JSON.parse(core.dispatch(JSON.stringify(event))) as DispatchResult;
		const resolve = (id: number, result: ExtCacheShellResult): DispatchResult =>
			JSON.parse(core.resolve_effect(BigInt(id), JSON.stringify(result))) as DispatchResult;

		const queue = dispatch({
			type: 'accounts_changed',
			is_loading: facts.isLoading,
			has_wallet: facts.hasWallet,
			accounts: facts.accounts,
			active: facts.active,
			theme: facts.theme,
			locale: facts.locale
		}).effects;

		while (queue.length > 0) {
			const { id, operation } = queue.shift()!;
			const [result, snapshot] = await perform(operation);
			if (snapshot) written = snapshot;
			queue.push(...resolve(id, result).effects);
		}
		return written;
	} finally {
		core.free();
	}
}

async function perform(
	operation: ExtCacheOperation
): Promise<[ExtCacheShellResult, ExtSnapshot | null]> {
	switch (operation.type) {
		case 'write_snapshot':
			await write(operation.snapshot);
			return [{ type: 'snapshot_written' }, operation.snapshot];
		case 'remove_snapshot':
			await remove();
			return [{ type: 'snapshot_removed' }, null];
		case 'read_attestation':
			// There is no Universal-Link hand-off in a Chrome extension: the wallet
			// IS the extension. "Never attested" is the fact, not a fallback.
			return [{ type: 'attestation_read', ts: 0, now_ms: Date.now() }, null];
		case 'persist_attestation':
			return [{ type: 'attestation_persisted' }, null];
		case 'request_extension_sign':
			// Safari's bus asks the native app to sign. Here the request already
			// arrived through this extension's own channel.
			return [{ type: 'sign_requested' }, null];
	}
}

// ---------------------------------------------------------------------------
// Storage — the only outside world this module touches
// ---------------------------------------------------------------------------

interface StorageAreaLike {
	get(keys: string | string[]): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
	remove(keys: string | string[]): Promise<void>;
}

function area(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

async function write(snapshot: ExtSnapshot): Promise<void> {
	try {
		await area()?.set({ [EXT_CACHE_KEY]: snapshot });
	} catch {
		/* best-effort, as the App Group write is */
	}
}

async function remove(): Promise<void> {
	try {
		await area()?.remove(EXT_CACHE_KEY);
	} catch {
		/* best-effort */
	}
}

/** What the worker (or the request window) can see right now. */
export async function readExtSnapshot(): Promise<ExtSnapshot | null> {
	try {
		const all = await area()?.get(EXT_CACHE_KEY);
		const value = all?.[EXT_CACHE_KEY];
		return value && typeof value === 'object' ? (value as ExtSnapshot) : null;
	} catch {
		return null;
	}
}
