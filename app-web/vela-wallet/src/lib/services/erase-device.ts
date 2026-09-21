// Ported from src/services/erase-device.ts @ 28d25ae9 — one store became three;
// the rule itself moved into the core in spec 072.
/**
 * "Erase this device" — the destructive counterpart to signing out (spec 028
 * T434, research D49, contracts/erase-scope.md).
 *
 * Sign-out is deliberately narrow: it drops the account list and the active
 * index and nothing else, because the address is derived from the passkey and
 * every address-keyed record lines back up on the next sign-in. THIS is the
 * action that means "this browser is no longer mine".
 *
 * ## Why a scan and not a key list
 *
 * Carried from the Expo module, including the reason it was rewritten once
 * already. Its predecessor walked a hand-maintained list of the eleven keys ONE
 * module happened to own. It never covered contacts, contact groups, the
 * `vela.perm.*` grants, the receive acknowledgements, the balance / rate /
 * token-metadata caches, or a single preference — and it drifted out of date
 * silently, because nothing about a delete-list fails when the app grows a key.
 * **A delete-list erase is wrong by default and only accidentally right.**
 *
 * So the direction is inverted: enumerate what is ACTUALLY stored and ask of
 * each key whether an erase takes it. A key added next year is erased on the
 * day it is first written, with no edit here.
 *
 * ## Whose rule
 *
 * The CORE's (`vela_core::storage_catalog::is_erasable_key`, spec 072): the
 * `vela.` namespace and its one unprefixed cache, minus `vela.pendingUploads` —
 * a passkey public key the index has never confirmed, whose retry needs no
 * account list but which, deleted, could never be found at sign-in on any
 * device. Every shell asks the same question; iOS used to walk a list.
 *
 * What stays here is what the core has no port for: listing three key-value
 * stores, deleting from them, and the verification pass.
 *
 * ## Three stores, because the web has three
 *
 * Expo had one AsyncStorage. A browser has `localStorage` (the onboarding
 * records, the preferences, the flags), the IndexedDB KV (contacts, history,
 * caches — `services/storage.ts`), and, in the extension build only,
 * `chrome.storage.local` (the `vela.perm.*` grants and the `ext_cache`
 * snapshot). All three are swept by the same rule; a store that is not present
 * in this build contributes nothing rather than failing.
 */
import { loadCore, storageIsErasableKey } from '$lib/core/client';
import { getAllKeys, removeItem } from './storage';

/** Would {@link eraseDeviceData} delete this key? The core's rule; runs after `loadCore()`. */
export function isErasableKey(key: string): boolean {
	return storageIsErasableKey(key);
}

/**
 * The erase ran but a store still holds keys it was supposed to remove.
 *
 * A distinct error type because the caller must NOT treat this as "erased":
 * telling a person their browser is clean while their transaction history is
 * still in it is the one outcome this feature cannot have.
 */
export class EraseIncompleteError extends Error {
	/** The keys that survived. Never logged with their values. */
	readonly remaining: readonly string[];

	constructor(remaining: readonly string[]) {
		super(`Erase incomplete: ${remaining.length} key(s) still present`);
		this.name = 'EraseIncompleteError';
		this.remaining = remaining;
	}
}

interface StorageAreaLike {
	get(keys: null): Promise<Record<string, unknown>>;
	remove(keys: string[]): Promise<void>;
}

/** `chrome.storage.local`, or `null` outside the extension build. */
function extensionArea(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

// ---------------------------------------------------------------------------
// The three stores, each behind the same two questions: what is here, drop this
// ---------------------------------------------------------------------------

function localKeys(): string[] {
	if (typeof localStorage === 'undefined') return [];
	const keys: string[] = [];
	for (let i = 0; i < localStorage.length; i++) {
		const key = localStorage.key(i);
		if (key !== null) keys.push(key);
	}
	return keys;
}

function dropLocal(keys: readonly string[]): void {
	for (const key of keys) {
		// Individually swallowed on purpose: the verification pass below is the
		// authority on whether the erase succeeded, not this loop.
		try {
			localStorage.removeItem(key);
		} catch {
			/* checked below */
		}
	}
}

async function kvKeys(): Promise<string[]> {
	try {
		return await getAllKeys();
	} catch {
		// An unopenable database holds nothing this pass can delete. If it is
		// merely unreachable right now, the verification pass fails too and the
		// person is told the erase did not finish — which is the truth.
		return [];
	}
}

async function dropKv(keys: readonly string[]): Promise<void> {
	for (const key of keys) {
		try {
			await removeItem(key);
		} catch {
			/* checked below */
		}
	}
}

async function extensionKeys(): Promise<string[]> {
	const area = extensionArea();
	if (area === null) return [];
	try {
		return Object.keys(await area.get(null));
	} catch {
		return [];
	}
}

async function dropExtension(keys: readonly string[]): Promise<void> {
	const area = extensionArea();
	if (area === null || keys.length === 0) return;
	try {
		await area.remove([...keys]);
	} catch {
		/* checked below */
	}
}

/** Everything erasable, across every store this build has. */
async function erasableEverywhere(): Promise<{
	local: string[];
	kv: string[];
	extension: string[];
}> {
	const [kv, extension] = await Promise.all([kvKeys(), extensionKeys()]);
	return {
		local: localKeys().filter(isErasableKey),
		kv: kv.filter(isErasableKey),
		extension: extension.filter(isErasableKey)
	};
}

/**
 * Delete every key {@link isErasableKey} names, in every store, then VERIFY.
 *
 * Resolves with the keys that were removed. Rejects with
 * {@link EraseIncompleteError} if anything survived — a caller that sends the
 * person back to first run on a rejected promise would be claiming an erase
 * that did not happen.
 *
 * Keys that are not ours are not this module's to judge and are left alone.
 */
export async function eraseDeviceData(): Promise<readonly string[]> {
	await loadCore();
	const doomed = await erasableEverywhere();

	dropLocal(doomed.local);
	await dropKv(doomed.kv);
	await dropExtension(doomed.extension);

	const after = await erasableEverywhere();
	const remaining = [...after.local, ...after.kv, ...after.extension];
	if (remaining.length > 0) throw new EraseIncompleteError(remaining);

	return [...doomed.local, ...doomed.kv, ...doomed.extension];
}
