/**
 * Local persistence layer — the web's AsyncStorage (spec 024, research D2).
 *
 * The Expo executors were written against an async string KV, so this module
 * exposes exactly that shape (`getItem`/`setItem`/`removeItem`) and nothing
 * more: ports keep a zero-diff storage seam. Backed by IndexedDB — database
 * `vela`, one object store `kv` — because contacts and networks are
 * unbounded structured data and localStorage's ~5 MB sync ceiling plus
 * main-thread JSON cost rule it out. No library: the whole wrapper is the
 * promise plumbing below.
 *
 * Key/value formats are the Expo compatibility contract, byte-for-byte
 * (`vela.contacts` stays camelCase, `vela.contacts.dismissed` stays an
 * address→ms map, …) — see data-model.md. The four onboarding localStorage
 * keys are NOT here and must not migrate (spec FR-009); the one shared
 * record, `vela.serviceEndpoints`, deliberately stays in localStorage with
 * its onboarding readers (research D3a).
 *
 * Failure contract: these functions REJECT on storage faults (private-mode
 * denial, quota, a torn database). Classification is not this module's job —
 * each executor's failure twin turns the rejection into the result variant
 * that operation answers with, so the core keeps ownership of what a failed
 * read *means*.
 */

const DB_NAME = 'vela';
const STORE = 'kv';

/** The open (or opening) database. Promise-cached so screens share one open. */
let opening: Promise<IDBDatabase> | null = null;

function openDb(): Promise<IDBDatabase> {
	if (opening) return opening;
	const started = new Promise<IDBDatabase>((resolve, reject) => {
		const request = indexedDB.open(DB_NAME, 1);
		request.onupgradeneeded = () => {
			request.result.createObjectStore(STORE);
		};
		request.onsuccess = () => {
			const db = request.result;
			// A version change from another tab (a future migration) closes this
			// handle; dropping the cache lets the next call reopen cleanly.
			db.onversionchange = () => {
				db.close();
				if (opening === started) opening = null;
			};
			resolve(db);
		};
		request.onerror = () => reject(request.error ?? new Error('indexedDB open failed'));
		request.onblocked = () => reject(new Error('indexedDB open blocked'));
	});
	// A failed open must not be cached as "open": the next attempt retries.
	started.catch(() => {
		if (opening === started) opening = null;
	});
	opening = started;
	return started;
}

function inStore<T>(
	mode: IDBTransactionMode,
	run: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T> {
	return openDb().then(
		(db) =>
			new Promise<T>((resolve, reject) => {
				const tx = db.transaction(STORE, mode);
				const request = run(tx.objectStore(STORE));
				// Resolve on transaction commit, not request success: a write's
				// request can succeed and its transaction still abort on quota.
				tx.oncomplete = () => resolve(request.result);
				tx.onerror = () => reject(tx.error ?? new Error('indexedDB transaction failed'));
				tx.onabort = () => reject(tx.error ?? new Error('indexedDB transaction aborted'));
			})
	);
}

/** The stored string, or `null` when the key has never been written. */
export async function getItem(key: string): Promise<string | null> {
	const value = await inStore('readonly', (store) => store.get(key));
	return typeof value === 'string' ? value : null;
}

export async function setItem(key: string, value: string): Promise<void> {
	await inStore('readwrite', (store) => store.put(value, key));
}

export async function removeItem(key: string): Promise<void> {
	await inStore('readwrite', (store) => store.delete(key));
}

/**
 * Every key in the store (spec 028 T434).
 *
 * The erase is a NAMESPACE sweep, not a delete-list, and a sweep has to be able
 * to ask what is actually here — a list of keys one module happens to own is
 * exactly the thing that drifts out of date in silence.
 */
export async function getAllKeys(): Promise<string[]> {
	const keys = await inStore<IDBValidKey[]>('readonly', (store) => store.getAllKeys());
	return keys.filter((key): key is string => typeof key === 'string');
}

/** Test seam: drop the cached connection so a fresh open is observable. */
export function resetStorageForTests(): void {
	opening = null;
}
