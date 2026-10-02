/**
 * Whether Chrome lets the extension onto websites (spec 094 S2).
 *
 * The manifest asks for two host permissions, and Chrome lets a person
 * withhold both after install (the extension's "Site access": "On click" or
 * "On specific sites"). Withheld, nothing fails where anyone would see it:
 *
 *   - getvela.app — an extension page may claim the `getvela.app`
 *     relying party only while it holds this (spec 027 D31), so every passkey
 *     ceremony fails at once with Chrome's own `SecurityError`;
 *   - every site — the provider is a content script, and on a site Vela may not
 *     run on, `window.ethereum` and the EIP-6963 announcement never appear: the
 *     dApp just says there is no wallet.
 *
 * This reads both with `chrome.permissions.contains` and asks for them back
 * with `chrome.permissions.request`, which Chrome allows — from a click — for
 * host permissions the manifest declares and the person withheld. Outside the
 * packaged extension every answer is "nothing to say".
 */
import { isPackagedApp } from './page-url';

/** The host permissions the manifest declares, all of which the wallet needs. */
export const SITE_ORIGINS: readonly string[] = ['https://getvela.app/*', '*://*/*'];

interface PermissionEvent {
	addListener(listener: () => void): void;
	removeListener(listener: () => void): void;
}

interface PermissionsApi {
	contains(query: { origins: string[] }): Promise<boolean>;
	request(query: { origins: string[] }): Promise<boolean>;
	onAdded?: PermissionEvent;
	onRemoved?: PermissionEvent;
}

function permissions(): PermissionsApi | null {
	if (!isPackagedApp()) return null;
	const api = (globalThis as { chrome?: { permissions?: PermissionsApi } }).chrome?.permissions;
	return api && typeof api.contains === 'function' ? api : null;
}

/**
 * `true` when Vela may run on every site (and so use its passkey), `false`
 * when the person limited it, `null` when there is nothing to ask — not the
 * packaged extension, or Chrome would not say.
 */
export async function siteAccessGranted(): Promise<boolean | null> {
	const api = permissions();
	if (!api) return null;
	try {
		return await api.contains({ origins: [...SITE_ORIGINS] });
	} catch {
		return null;
	}
}

/**
 * Ask Chrome to give the access back. Call it from the click handler itself:
 * `permissions.request` needs the user gesture, which the first `await` before
 * it would spend. Resolves `true` when granted.
 */
export function requestSiteAccess(): Promise<boolean> {
	const api = permissions();
	if (!api) return Promise.resolve(false);
	try {
		return api.request({ origins: [...SITE_ORIGINS] }).catch(() => false);
	} catch {
		return Promise.resolve(false);
	}
}

/** Call `listener` whenever the extension's permissions change; returns the unsubscribe. */
export function onSiteAccessChange(listener: () => void): () => void {
	const api = permissions();
	if (!api) return () => {};
	api.onAdded?.addListener(listener);
	api.onRemoved?.addListener(listener);
	return () => {
		api.onAdded?.removeListener(listener);
		api.onRemoved?.removeListener(listener);
	};
}
