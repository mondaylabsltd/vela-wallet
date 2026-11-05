/**
 * Where a per-origin grant is kept (spec 027 T330).
 *
 * Ported from src/services/dapp-permissions.ts @ 52ad8fa9 — the STORAGE half
 * only. That file also carries `resolveGranted` and `shouldDropGrant`, which
 * are TypeScript twins of rules `dapp_permissions` owns in Rust; they are not
 * ported for the same reason 026 left `clear-signing`'s twin behind. The core
 * is asked (`decidePopupRequest`), never re-implemented.
 *
 * The key shape `vela.perm.<origin>` is deliberately the one the Safari
 * extension already uses, so the two extensions describe a grant identically.
 *
 * It lives in `chrome.storage.local` rather than the app's IndexedDB because
 * BOTH sides need it: the wallet writes a grant, and the service worker reads
 * it to answer an already-connected origin without opening a window. It is the
 * one piece of wallet state the worker may see, and it is a fact the core
 * authored — never a decision the worker makes.
 */
import { siteStartChainFor } from '$lib/core/kernels';
import {
	ACCOUNT_CHAINS_KEY,
	CHAIN_PREFIX,
	CHAINS_KEY,
	EXT_CACHE_KEY,
	LAST_CHAIN_KEY,
	PERM_PREFIX
} from './keys';

/** The stored grant. `grantedAt` is kept even though no rule reads it — a wire
 *  shape that drops what is persisted is a wire shape that lies. */
export interface DAppGrant {
	origin: string;
	address: string;
	chainId: number;
	grantedAt: number;
}

export function grantKey(origin: string): string {
	return PERM_PREFIX + origin;
}

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

export async function getGrant(origin: string): Promise<DAppGrant | null> {
	const store = area();
	if (!store) return null;
	try {
		const key = grantKey(origin);
		const all = await store.get(key);
		const value = all[key];
		return isGrant(value) ? value : null;
	} catch {
		return null;
	}
}

export async function setGrant(grant: DAppGrant): Promise<void> {
	const store = area();
	if (!store) return;
	try {
		await store.set({ [grantKey(grant.origin)]: grant });
	} catch {
		/* best-effort persist */
	}
}

export async function revokeGrant(origin: string): Promise<void> {
	const store = area();
	if (!store) return;
	try {
		await store.remove(grantKey(origin));
	} catch {
		/* best-effort */
	}
}

/**
 * The chain a site is on: its own switch (`vela.chain.<origin>`, written by
 * the worker on `wallet_switchEthereumChain`), else the chain it connected
 * on, else where a new site starts (dapp-17: the network last used in this
 * space, else the space's first — the core's `siteStartChain`, over the
 * catalog the wallet published), else `fallback` (the snapshot's default).
 * The same resolution the worker uses for `eth_chainId`, so the chain a
 * connect or a signature is asked on is the chain the site believes it is on.
 */
export async function getOriginChain(origin: string, fallback: number): Promise<number> {
	const store = area();
	if (!store) return fallback;
	try {
		const all = await store.get([
			CHAIN_PREFIX + origin,
			grantKey(origin),
			CHAINS_KEY,
			LAST_CHAIN_KEY,
			ACCOUNT_CHAINS_KEY,
			EXT_CACHE_KEY
		]);
		const picked = all[CHAIN_PREFIX + origin];
		if (typeof picked === 'number' && Number.isFinite(picked) && picked > 0) return picked;
		const grant = all[grantKey(origin)];
		if (isGrant(grant) && grant.chainId > 0) return grant.chainId;
		const start = startChain(
			all[CHAINS_KEY],
			all[LAST_CHAIN_KEY],
			accountChainsOf(all[ACCOUNT_CHAINS_KEY], all[EXT_CACHE_KEY])
		);
		if (start > 0) return start;
	} catch {
		/* unreadable → the default */
	}
	return fallback;
}

/**
 * r12: does `origin` have a network of its own — its own switch, or the one
 * it connected on? Only a site with neither starts where the account lives,
 * and only for such a site is the start worth reading again.
 */
export async function originHasOwnChain(origin: string): Promise<boolean> {
	const store = area();
	if (!store) return false;
	try {
		const all = await store.get([CHAIN_PREFIX + origin, grantKey(origin)]);
		const picked = all[CHAIN_PREFIX + origin];
		if (typeof picked === 'number' && Number.isFinite(picked) && picked > 0) return true;
		const grant = all[grantKey(origin)];
		return isGrant(grant) && grant.chainId > 0;
	} catch {
		return false;
	}
}

/** What a new site's start is read from (`getOriginChain`'s start half). */
const START_INPUTS = [ACCOUNT_CHAINS_KEY, LAST_CHAIN_KEY, CHAINS_KEY, EXT_CACHE_KEY];

interface StorageChangedLike {
	addListener(fn: (changes: Record<string, unknown>, areaName: string) => void): void;
	removeListener(fn: (changes: Record<string, unknown>, areaName: string) => void): void;
}

/**
 * r12: `onChange` each time something a new site's start is read from
 * changes — where an account lives (the wallet page writes it when its
 * holdings land), the network last used, the catalog, who is signed in —
 * until the returned function is called. The connect sheet's network follows
 * the account's holdings until Connect is pressed: money that arrived a
 * moment before the connect is where the site starts, never the space's
 * first network said with confidence. A no-op off the extension.
 */
export function watchStartChain(onChange: () => void): () => void {
	const changed = (globalThis as { chrome?: { storage?: { onChanged?: unknown } } }).chrome
		?.storage?.onChanged as StorageChangedLike | undefined;
	if (!changed) return () => {};
	const listener = (changes: Record<string, unknown>, areaName: string) => {
		if (areaName !== 'local') return;
		if (START_INPUTS.some((key) => key in changes)) onChange();
	};
	changed.addListener(listener);
	return () => changed.removeListener(listener);
}

/**
 * Where the signed-in account lives (r10): its list under
 * `ACCOUNT_CHAINS_KEY`, for the address the snapshot says is signed in — the
 * worker's `accountChainsOf`, so the window asks on the chain the site is on.
 */
function accountChainsOf(byAccount: unknown, snapshot: unknown): number[] {
	const address =
		snapshot && typeof snapshot === 'object'
			? (snapshot as { address?: unknown }).address
			: undefined;
	if (typeof address !== 'string' || !byAccount || typeof byAccount !== 'object') return [];
	const ids = (byAccount as Record<string, unknown>)[address.toLowerCase()];
	return Array.isArray(ids) ? ids.filter((id): id is number => Number.isInteger(id) && id > 0) : [];
}

/** Where a new site starts, from the published catalog; `0` for a catalog that does not say. */
function startChain(catalog: unknown, lastBySpace: unknown, held: number[]): number {
	if (!catalog || typeof catalog !== 'object') return 0;
	const { order, space } = catalog as { order?: unknown; space?: unknown };
	if (!Array.isArray(order)) return 0;
	const ids = order.filter((id): id is number => Number.isInteger(id) && id > 0);
	if (ids.length === 0) return 0;
	const last =
		typeof space === 'string' && lastBySpace && typeof lastBySpace === 'object'
			? (lastBySpace as Record<string, unknown>)[space]
			: undefined;
	// r9: where the account holds something or used, before the space's first.
	return siteStartChainFor(ids, typeof last === 'number' && last > 0 ? last : null, held);
}

function isGrant(value: unknown): value is DAppGrant {
	if (!value || typeof value !== 'object') return false;
	const v = value as Partial<DAppGrant>;
	return (
		typeof v.origin === 'string' &&
		typeof v.address === 'string' &&
		typeof v.chainId === 'number' &&
		typeof v.grantedAt === 'number'
	);
}
