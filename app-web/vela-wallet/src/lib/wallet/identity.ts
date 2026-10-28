/**
 * Whose wallet this is — swapped into the fixture model at runtime (spec 019).
 *
 * The wallet body is still the spec-015 fixture layer. Two things in it are
 * NOT allowed to be fixtures once somebody is signed in: the address, because
 * it is the one value on the page a person acts on, and the name, because it
 * is the line they read as "this is mine". The other three clients do exactly
 * this swap (`withAddress`/`withName` on the phones, `Identity` on the
 * desktop); this is the web's copy of it.
 *
 * Blank identity is a real state, not a bug: the page is prerendered before
 * anyone is signed in, and a prerendered fixture address would flash a
 * stranger's wallet at the person for one frame after hydration. So the
 * server ships an EMPTY header and the browser fills it.
 */
import type { WalletDesktopModel, WalletHeaderModel, WalletHomeModel } from './model';
import { IDENTICON_PLACEHOLDER_SVG } from './identicon-placeholder';

export interface WalletIdentity {
	name: string;
	address: string;
	identiconSvg: string;
	/** A multisig's approval rule, linking to its settings. */
	rule?: { label: string; title: string; href: string };
}

/** The header a page carries before the session has answered. */
export const EMPTY_HEADER: WalletHeaderModel = {
	name: '',
	addressDisplay: '',
	addressFull: '',
	identiconSvg: IDENTICON_PLACEHOLDER_SVG
};

/**
 * `0x14fB1f…D1eA5c` — this client's short form, matching its own fixtures and
 * the desktop app's `Identity::display` (8 leading, 6 trailing). The phones
 * cut 6/4 for a 390pt frame; the web's narrowest layout is the same width but
 * its widest is a 1280px desktop, and one form across both is worth more here
 * than matching the phones digit for digit.
 */
export function shortenAddress(address: string): string {
	if (address.length <= 14) return address;
	return `${address.slice(0, 8)}…${address.slice(-6)}`;
}

/** The wallet header for an identity — also the explore sidebar's (spec 022). */
export function identityHeader(identity: WalletIdentity): WalletHeaderModel {
	return {
		name: identity.name,
		addressDisplay: shortenAddress(identity.address),
		addressFull: identity.address,
		identiconSvg: identity.identiconSvg,
		...(identity.rule ? { rule: identity.rule } : {})
	};
}

/** The mobile home model, wearing the signed-in wallet's identity. */
export function homeWithIdentity(
	model: WalletHomeModel,
	identity: WalletIdentity
): WalletHomeModel {
	return { ...model, header: identityHeader(identity) };
}

/** The desktop model, whose header lives in the sidebar. */
export function desktopWithIdentity(
	model: WalletDesktopModel,
	identity: WalletIdentity
): WalletDesktopModel {
	return { ...model, sidebar: { ...model.sidebar, header: identityHeader(identity) } };
}
