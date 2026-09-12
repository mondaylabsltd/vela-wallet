// Ported from src/services/erase-device.ts @ 28d25ae9 — one store became three,
// the prefix rule and the keep-list are carried verbatim.
/**
 * "Erase this device" — the destructive counterpart to signing out (spec 028
 * T434, research D49, contracts/erase-scope.md).
 *
 * Sign-out is deliberately narrow: it drops the account list and the active
 * index and nothing else, because the address is derived from the passkey and
 * every address-keyed record lines back up on the next sign-in. THIS is the
 * action that means "this browser is no longer mine".
 *
 * ## Why a prefix scan and not a key list
 *
 * Carried from the Expo module, including the reason it was rewritten once
 * already. Its predecessor walked a hand-maintained list of the eleven keys ONE
 * module happened to own. It never covered contacts, contact groups, the
 * `vela.perm.*` grants, the receive acknowledgements, the balance / rate /
 * token-metadata caches, or a single preference — and it drifted out of date
 * silently, because nothing about a delete-list fails when the app grows a key.
 * **A delete-list erase is wrong by default and only accidentally right.**
 *
 * So the direction is inverted: enumerate what is ACTUALLY stored, delete
 * everything under the `vela.` namespace, and name the exceptions. A key added
 * next year is erased on the day it is first written, with no edit here.
 *
 * ## Three stores, because the web has three
 *
 * Expo had one AsyncStorage. A browser has `localStorage` (the onboarding
 * records, the preferences, the flags), the IndexedDB KV (contacts, history,
 * caches — `services/storage.ts`), and, in the extension build only,
 * `chrome.storage.local` (the `vela.perm.*` grants and the `ext_cache`
 * snapshot). All three are swept by the same rule; a store that is not present
 * in this build contributes nothing rather than failing.
 *
 * ## Why no core owns this
 *
 * The rule is `startsWith('vela.') && !KEEP.has(key)` applied to an ENUMERATION
 * of three key-value stores. The core has no port that can list keys, and
 * adding one so it could re-express a `startsWith` would leave the core holding
 * a string comparison while the enumeration, the retry and the verification
 * pass stayed in the shell anyway. The two judgements that ARE rules — the
 * prefix and the keep-list — are stated as exported constants below, so a
 * reader finds them without reading the loop.
 */
import { getAllKeys, removeItem } from './storage';

/** Every key this app writes is namespaced. The scan is this prefix. */
export const VELA_KEY_PREFIX = 'vela.';

/**
 * The only `vela.` keys an erase leaves behind, and the reason is not
 * convenience. A record in `vela.pendingUploads` is a passkey public key the
 * index service has never confirmed; the retry on the next launch needs no
 * account list to re-send it, but a DELETED record can never be retried — and
 * that credential then cannot be found at login on any device. Erasing it
 * would downgrade "recoverable" to "possibly ruined", which is strictly worse
 * here than at sign-out, because the account list is going too and the retry is
 * the only remaining path to that key.
 *
 * Uploading first and erasing after was the alternative. It was rejected
 * because it makes a destructive action the person asked for depend on a
 * network that may be down.
 */
export const ERASE_KEEP_KEYS: readonly string[] = ['vela.pendingUploads'];

const KEEP = new Set<string>(ERASE_KEEP_KEYS);

/** Would {@link eraseDeviceData} delete this key? */
export function isErasableKey(key: string): boolean {
	return key.startsWith(VELA_KEY_PREFIX) && !KEEP.has(key);
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
 * Delete every `vela.` key except {@link ERASE_KEEP_KEYS}, then VERIFY.
 *
 * Resolves with the keys that were removed. Rejects with
 * {@link EraseIncompleteError} if anything survived — a caller that sends the
 * person back to first run on a rejected promise would be claiming an erase
 * that did not happen.
 *
 * Keys outside the `vela.` namespace are not this module's to judge and are
 * left alone.
 */
export async function eraseDeviceData(): Promise<readonly string[]> {
	const doomed = await erasableEverywhere();

	dropLocal(doomed.local);
	await dropKv(doomed.kv);
	await dropExtension(doomed.extension);

	const after = await erasableEverywhere();
	const remaining = [...after.local, ...after.kv, ...after.extension];
	if (remaining.length > 0) throw new EraseIncompleteError(remaining);

	return [...doomed.local, ...doomed.kv, ...doomed.extension];
}
