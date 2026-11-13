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

/**
 * The database's delete was issued and has not gone through (`deleteDatabase`)
 * — not finished yet, or held back by another connection that keeps the
 * database open: what the delete's wait ends with, and what an open meets
 * meanwhile.
 */
export class DatabaseHeldError extends Error {
	constructor() {
		super('indexedDB "vela" is being deleted, and another connection may be holding it open');
		this.name = 'DatabaseHeldError';
	}
}

/**
 * A delete the browser has queued and not yet carried out — kept, because it
 * cannot be withdrawn: it lands whenever the connection holding the database
 * closes, this document's next open waits behind it, and a second delete
 * would only queue behind the first.
 */
let deleting: Promise<void> | null = null;

function openDb(): Promise<IDBDatabase> {
	// An open issued now would wait behind the delete in the browser's own
	// queue — for as long as whoever holds the database holds it. Refused
	// instead, like any storage fault: nothing here waits without end.
	if (deleting) return Promise.reject(new DatabaseHeldError());
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

/**
 * An erase is under way: the store takes no more writes (spec 028 T434).
 *
 * The erase lists what is here, removes it, and checks that nothing is left
 * (`erase-device.ts`). The wallet goes on running while it does: a rates
 * refresh answers, and its cache is written behind the sweep's back. The
 * check then finds a key — `vela.fxRates.v1`, the time the e2e suite named
 * it — and the person is told "the erase did not finish" about an erase that
 * removed everything it found.
 *
 * Sealed, that cannot happen. A write issued BEFORE the seal is ahead of the
 * erase's listing in the store's own queue (IndexedDB runs a transaction
 * only after every earlier read-write transaction on its store), so the
 * listing sees it and the sweep takes it. A write issued after is refused
 * here. Nothing can land between the listing and the check — or between the
 * check and the page being replaced.
 *
 * Refused means REJECTED, like any other storage fault (the failure contract
 * above): the caller is not told that a write happened which did not.
 */
let sealed = false;

/** What a write is refused with while the store is sealed. */
export class StorageSealedError extends Error {
	constructor(key: string) {
		super(`storage is sealed for an erase: ${key} was not written`);
		this.name = 'StorageSealedError';
	}
}

/** From here on every {@link setItem} is refused, until {@link unsealStorage}. */
export function sealStorage(): void {
	sealed = true;
}

/**
 * Whether an erase holds the seal — for the few small records kept in
 * `localStorage` beside this store, which honour it too (perf-8's
 * `vela.backupCheck`).
 */
export function storageSealed(): boolean {
	return sealed;
}

/** The erase did not go through, and the wallet is still in use: writes again. */
export function unsealStorage(): void {
	sealed = false;
}

export async function setItem(key: string, value: string): Promise<void> {
	if (sealed) throw new StorageSealedError(key);
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

/**
 * Delete the whole database — the erase's last step (owner, 2026-10-28:
 * erasing means everything, the database itself included, not an empty one
 * left behind).
 *
 * This document's connection is closed first; another tab's closes itself on
 * the `versionchange` the delete raises (`openDb`), after which the delete
 * goes through. A connection that does not — a page that opened the database
 * itself and never closed it, a tab of a build older than that handler — keeps
 * it `blocked`: then this waits `timeoutMs` and rejects with
 * {@link DatabaseHeldError}, and the erase says it did not finish. The delete
 * stays queued (it cannot be withdrawn): until it lands, every open here is
 * refused at once rather than left waiting behind it, and a retry waits on
 * the same delete again, for as long again.
 */
export async function deleteDatabase(timeoutMs = 5_000): Promise<void> {
	const open = opening;
	opening = null;
	if (open) {
		try {
			(await open).close();
		} catch {
			// A failed open holds no connection to close.
		}
	}
	if (typeof indexedDB === 'undefined') return;
	if (!deleting) {
		const started = new Promise<void>((resolve, reject) => {
			const request = indexedDB.deleteDatabase(DB_NAME);
			request.onsuccess = () => resolve();
			request.onerror = () => reject(request.error ?? new Error('indexedDB delete failed'));
			// `blocked`: another connection is still open after the
			// `versionchange`. The request stays queued and lands when that
			// connection closes — this only waits for it, and not for ever.
			request.onblocked = () => {};
		});
		deleting = started;
		const settled = () => {
			if (deleting === started) deleting = null;
		};
		started.then(settled, settled);
	}
	const pending = deleting;
	let timer: ReturnType<typeof setTimeout> | undefined;
	try {
		await Promise.race([
			pending,
			new Promise<never>((_, reject) => {
				timer = setTimeout(() => reject(new DatabaseHeldError()), timeoutMs);
			})
		]);
	} finally {
		clearTimeout(timer);
	}
}

/** Test seam: drop the cached connection so a fresh open is observable. */
export function resetStorageForTests(): void {
	opening = null;
	sealed = false;
	deleting = null;
}
