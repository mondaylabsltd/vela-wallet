/**
 * The person's sites, set aside while the wallet is in the parallel space
 * (device walk of 2026-10-11, item 5).
 *
 * The space swaps the account list (`dev/parallel-space.ts`), and a grant is
 * pinned to an account. Left where they were, the person's grants met the
 * fixture accounts and the core's ordinary rule — "this site's account left
 * the wallet" — removed every one of them on the way in. The core now knows a
 * space when it is told of one (`popup_account_switch.space`) and leaves a
 * grant of the person's alone; this module is the other half, the one the
 * accounts already have: the person's sites are MOVED out of the prefixes the
 * worker reads, into one record, and put back exactly on the way out.
 *
 * So the space starts with no connected site, and a site connected in both —
 * the test dApp, on every device walk — cannot overwrite the person's grant:
 * what it writes inside goes under the live keys, and those are dropped whole
 * when the person's come back.
 *
 * Both prefixes move: `vela.perm.<origin>` (the grant) and `vela.chain.<origin>`
 * (the network the site put itself on). Values are carried as stored, whatever
 * their shape — this lists keys and moves them; what a grant MEANS is
 * `dapp_permissions`'.
 *
 * Off the extension there is no `chrome.storage.local`, so there are no grants
 * and every function here does nothing.
 */
import { CHAIN_PREFIX, PERM_PREFIX, SPACE_SITES_KEY } from './keys';

interface StorageAreaLike {
	get(keys: null | string): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
	remove(keys: string | string[]): Promise<void>;
}

function area(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

function isSiteKey(key: string): boolean {
	return key.startsWith(PERM_PREFIX) || key.startsWith(CHAIN_PREFIX);
}

/** A record still holding the person's sites. Anything else — absent, or the
 *  `false` a finished restore leaves for a moment — is "nothing set aside". */
function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function siteEntries(all: Record<string, unknown>): Record<string, unknown> {
	return Object.fromEntries(Object.entries(all).filter(([key]) => isSiteKey(key)));
}

/**
 * On the way IN, and only the FIRST time (the caller's rule, the one the
 * account backup follows): move every site entry into the set-aside record.
 * A second entry must not call this — what is under the live keys by then is
 * the space's own.
 *
 * The record is written first and the live keys are removed only once it is:
 * a write that fails leaves the person's sites where they were, and the
 * `space` fact the follow passes to the core is what keeps them whole then.
 * Returns whether the person's sites are set aside when this returns.
 */
export async function setSitesAside(): Promise<boolean> {
	const store = area();
	if (!store) return false;
	try {
		const mine = siteEntries(await store.get(null));
		await store.set({ [SPACE_SITES_KEY]: mine });
		const keys = Object.keys(mine);
		if (keys.length > 0) await store.remove(keys);
		return true;
	} catch {
		return false;
	}
}

/**
 * Whether the person's sites are set aside right now: the record is there.
 * Then every site entry under the two prefixes is the SPACE's own.
 */
export async function sitesAreSetAside(): Promise<boolean> {
	const store = area();
	if (!store) return false;
	try {
		return isRecord((await store.get(SPACE_SITES_KEY))[SPACE_SITES_KEY]);
	} catch {
		return false;
	}
}

/**
 * How a restore ended. `held` is the one that matters to the caller: the
 * record is still whole and could not be given back, so the person's sites are
 * in it and nowhere else — leaving the space now would strand them.
 */
export type SitesRestore = 'restored' | 'nothing' | 'held';

/**
 * On the way OUT: drop what the space wrote under the two prefixes and put
 * the person's entries back as they were set aside.
 *
 * Three steps, each safe to be the last one a dying page performs — the
 * space's flag is still set until the exit finishes, so the exit is asked
 * again and this runs again:
 *
 *   1. the space's entries go (interrupted here: the record is whole);
 *   2. the person's come back AND the record is spent, in ONE write — so a
 *      record is never found whole beside sites it has already given back,
 *      which a second pass would take for the space's and drop;
 *   3. the spent record is removed.
 *
 * With no record there is nothing to put back, and nothing is dropped either:
 * the set-aside did not happen, so what is under the live keys may be the
 * person's. A grant the space made is then pinned to an account that has left,
 * and the core removes it on the next follow, as it removes any other.
 */
export async function restoreSites(): Promise<SitesRestore> {
	const store = area();
	if (!store) return 'nothing';
	let all: Record<string, unknown>;
	try {
		all = await store.get(null);
	} catch {
		// Unreadable: no grant can be read either, and there is nothing this
		// could put back. The exit goes on.
		return 'nothing';
	}
	const mine = all[SPACE_SITES_KEY];
	if (!isRecord(mine)) {
		// A spent record whose removal did not happen — finish it.
		if (SPACE_SITES_KEY in all) await store.remove(SPACE_SITES_KEY).catch(() => {});
		return 'nothing';
	}
	try {
		const theirs = Object.keys(siteEntries(all));
		if (theirs.length > 0) await store.remove(theirs);
		await store.set({ ...siteEntries(mine), [SPACE_SITES_KEY]: false });
	} catch {
		return 'held';
	}
	await store.remove(SPACE_SITES_KEY).catch(() => {});
	return 'restored';
}

/**
 * "Disconnect all", asked from inside the space, takes the grants set aside
 * too: those are the person's own words about this browser (the rule
 * `dapp_browser` keeps for the in-app browsers). The networks sites chose stay
 * in the record, as "Disconnect all" leaves them outside a space.
 */
export async function revokeSitesSetAside(): Promise<void> {
	const store = area();
	if (!store) return;
	try {
		const mine = (await store.get(SPACE_SITES_KEY))[SPACE_SITES_KEY];
		if (!isRecord(mine)) return;
		const kept = Object.fromEntries(
			Object.entries(mine).filter(([key]) => !key.startsWith(PERM_PREFIX))
		);
		if (Object.keys(kept).length === Object.keys(mine).length) return;
		await store.set({ [SPACE_SITES_KEY]: kept });
	} catch {
		/* best-effort, like the revoke beside it */
	}
}
