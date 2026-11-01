/**
 * What a page must know of a multisig link BEFORE any wasm: whether a hash
 * is meant as one (`#ms=` / `#msa=`, readable or not), the hash to carry to
 * the wallet, and — r4b-9 — keeping one opened on a device with no Vela
 * account yet, while the person creates one or signs in. Welcome
 * (`/en#msa=…`) loads no core, so this is the one part of the links in
 * TypeScript; the core's codec reads them (`core/link.ts`), and the accounts
 * machine opens the kept one once the wallet is there (`open_link`
 * `take_kept`, the same record: `{"hash":…,"at":…}` under
 * {@link KEPT_LINK_KEY}, a day, taken once).
 */

export const KEPT_LINK_KEY = 'vela.multisigKeptLink';

/**
 * A hash as the link meant it: `#msa%3D…` / `#ms%3D…` (an app that encoded
 * the `=`, r4b-17) reads as `#msa=…` / `#ms=…`, and spaces a chat app or a
 * mail client put around it are dropped. Anything else as given.
 */
function linkBody(hash: string): string {
	const body = (hash.startsWith('#') ? hash.slice(1) : hash).trim();
	if (!/^msa?%3d/i.test(body)) return body;
	try {
		return decodeURIComponent(body);
	} catch {
		return body;
	}
}

/** Whether `hash` is meant as one of these links at all, readable or not. */
export function isMultisigLink(hash: string): boolean {
	const body = linkBody(hash);
	return body.startsWith('ms=') || body.startsWith('msa=');
}

/**
 * r4b-17: the hash to carry to the wallet when a multisig link opened another
 * page (`/en#msa=…`, the site's root): the link itself, or `''`.
 */
export function carriedLinkHash(hash: string): string {
	const body = linkBody(hash);
	return isMultisigLink(body) ? `#${body}` : '';
}

/** A kept link is opened within a day of being kept (the accounts machine's rule). */
const KEPT_FOR_MS = 24 * 60 * 60 * 1000;

/**
 * r5reg: whether a multisig link opened here waits for a wallet (kept, not
 * yet a day old) — Welcome says so.
 */
export function linkWaiting(now = Date.now()): boolean {
	try {
		const raw = localStorage.getItem(KEPT_LINK_KEY);
		if (!raw) return false;
		const kept = JSON.parse(raw) as { hash?: unknown; at?: unknown };
		return (
			typeof kept.hash === 'string' &&
			isMultisigLink(kept.hash) &&
			typeof kept.at === 'number' &&
			now - kept.at < KEPT_FOR_MS
		);
	} catch {
		return false;
	}
}

/** r10: drop the kept record if it is `hash` (a link the core's codec cannot read). */
export function forgetKeptLink(hash: string): void {
	try {
		const raw = localStorage.getItem(KEPT_LINK_KEY);
		if (raw && (JSON.parse(raw) as { hash?: unknown }).hash === hash) {
			localStorage.removeItem(KEPT_LINK_KEY);
		}
	} catch {
		// Storage refused or the record does not read: nothing kept to drop.
	}
}

/** Keep `hash` if it is a multisig link. Whether it was kept. */
export function keepLink(hash: string, now = Date.now()): boolean {
	if (!isMultisigLink(hash)) return false;
	try {
		localStorage.setItem(KEPT_LINK_KEY, JSON.stringify({ hash, at: now }));
		return true;
	} catch {
		return false;
	}
}
