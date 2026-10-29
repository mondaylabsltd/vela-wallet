/**
 * A multisig link opened on a device with no Vela account yet (r4b-9): kept
 * while the person creates one or signs in, and opened once the wallet is
 * there — never lost to the trip through Welcome. Kept on this device only
 * (the links carry nothing secret: an address, a transaction's hash, labels),
 * for a day; taken once.
 *
 * No wasm here: Welcome keeps a link too (`/en#msa=…`), and it loads none.
 */
import { isMultisigLink } from './link';

export const KEPT_LINK_KEY = 'vela.multisigKeptLink';
/** How long a kept link waits for the wallet to be there. */
export const KEPT_LINK_MS = 24 * 60 * 60_000;

/** Keep `hash` if it is a multisig link (`#ms=` / `#msa=`, readable or not). Whether it was kept. */
export function keepLink(hash: string, now = Date.now()): boolean {
	if (!isMultisigLink(hash)) return false;
	try {
		localStorage.setItem(KEPT_LINK_KEY, JSON.stringify({ hash, at: now }));
		return true;
	} catch {
		return false;
	}
}

/** The link kept, once: `null` when none is, or it waited too long. */
export function takeKeptLink(now = Date.now()): string | null {
	let kept: { hash?: unknown; at?: unknown } | null;
	try {
		kept = JSON.parse(localStorage.getItem(KEPT_LINK_KEY) ?? 'null') as typeof kept;
		localStorage.removeItem(KEPT_LINK_KEY);
	} catch {
		return null;
	}
	if (!kept || typeof kept.hash !== 'string' || typeof kept.at !== 'number') return null;
	if (now - kept.at > KEPT_LINK_MS || now < kept.at - 60_000) return null;
	return isMultisigLink(kept.hash) ? kept.hash : null;
}
