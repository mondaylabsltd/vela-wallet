/**
 * Where a site's grant, its network and the wallet's published facts live
 * (spec 027 T330; spec 109 for the web).
 *
 * In the extension this is `chrome.storage.local`: BOTH halves read it — the
 * wallet writes a grant, the service worker answers an already-connected site
 * from it. On the hosted web wallet there is no worker, and since spec 109
 * (Vela Connect) there is a dApp channel all the same: a site opens
 * `wallet.getvela.app/dapp` and asks there. Its grant has to outlive that
 * window, show in Settings and be revocable there — so off the extension the
 * same keys are kept in this origin's `localStorage`, behind the same
 * `chrome.storage.local`-shaped interface every reader already speaks.
 *
 * Stored under `vela.area.<key>` so the area's keys (`vela.perm.<origin>`,
 * `vela.chain.<origin>`, …) can never meet a key the wallet keeps for itself.
 * Values are JSON, as `chrome.storage` would keep them.
 *
 * `null` only where neither exists (a server render, a storage the browser
 * refuses): every caller already reads `null` as "no grants, nothing stored".
 */

export interface StorageAreaLike {
	get(keys: null | string | string[]): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
	remove(keys: string | string[]): Promise<void>;
}

/** The web's own prefix for the area's keys. */
export const WEB_AREA_PREFIX = 'vela.area.';

function chromeLocal(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

function webStorage(): Storage | null {
	try {
		const storage = (globalThis as { localStorage?: Storage }).localStorage;
		return storage ?? null;
	} catch {
		// A browser that refuses storage (some private modes) throws on access.
		return null;
	}
}

function parse(raw: string | null): unknown {
	if (raw === null) return undefined;
	try {
		return JSON.parse(raw);
	} catch {
		return undefined;
	}
}

/** `localStorage` behind the `chrome.storage.local` interface. */
export function localStorageArea(storage: Storage): StorageAreaLike {
	const list = (keys: string | string[]) => (Array.isArray(keys) ? keys : [keys]);
	return {
		async get(keys) {
			const out: Record<string, unknown> = {};
			if (keys === null) {
				for (let i = 0; i < storage.length; i++) {
					const name = storage.key(i);
					if (!name || !name.startsWith(WEB_AREA_PREFIX)) continue;
					const value = parse(storage.getItem(name));
					if (value !== undefined) out[name.slice(WEB_AREA_PREFIX.length)] = value;
				}
				return out;
			}
			for (const key of list(keys)) {
				const value = parse(storage.getItem(WEB_AREA_PREFIX + key));
				if (value !== undefined) out[key] = value;
			}
			return out;
		},
		async set(items) {
			for (const [key, value] of Object.entries(items)) {
				if (value === undefined) storage.removeItem(WEB_AREA_PREFIX + key);
				else storage.setItem(WEB_AREA_PREFIX + key, JSON.stringify(value));
			}
		},
		async remove(keys) {
			for (const key of list(keys)) storage.removeItem(WEB_AREA_PREFIX + key);
		}
	};
}

/** The extension's `chrome.storage.local`, else this origin's `localStorage`, else `null`. */
export function dappArea(): StorageAreaLike | null {
	const chrome = chromeLocal();
	if (chrome) return chrome;
	const storage = webStorage();
	return storage ? localStorageArea(storage) : null;
}
