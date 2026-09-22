/**
 * WHERE one signature goes — the web's mirror of `wallet_keys::sign_route`
 * (spec 075 contract §1.2).
 *
 * A key minted or found through the Clear Signer lives BEHIND that page, the
 * way a security key's key lives in one device. `auto` must follow it there;
 * "Clear Signer" by name prefers such a key; and a platform/hybrid/security-key
 * route must not be aimed at a key only a self-hosted page can reach — that
 * ceremony would find nothing, and the person would be told their own key does
 * not exist.
 *
 * This is a MIRROR, and mirrors are a liability: the rule belongs to the core,
 * and the phones read it over UniFFI (`signRoute`). It is not in wasm —
 * `vela-core-wasm` exports no `sign_route`, and this app may not add one
 * (the core is another agent's this phase) — so the rule is spelled out here,
 * against the same cases the Rust suite pins (`wallet_keys.rs` tests), and
 * `sign-route.test.ts` holds them. When the wasm door exists, this file is
 * deleted, not maintained.
 */

import { clearSignerUsesWalletPasskeys } from '$lib/core/kernels';
import type { SignMethod } from '$lib/onboarding/core/passkey';

/** A founding key as the account record holds it. */
export interface DeviceKey {
	credential_id: string;
	transports: string;
	/** Spec 075: the Clear Signer page this key lives behind, if any. */
	signer_origin?: string | null;
}

/** Where one ceremony goes: the key it is pinned to, and how it is reached. */
export interface SignRoute {
	credentialId: string;
	transports: string;
	method: Exclude<SignMethod, 'auto'>;
	/** For `clear_signer`: the page. Empty means the person's page from Settings. */
	signerOrigin: string;
}

/** `passkey::reported_method_name` — what a key's own transports say it is. */
function reportedMethod(transports: string): Exclude<SignMethod, 'auto' | 'clear_signer'> {
	const has = (hint: string) => transports.split(',').some((part) => part.trim() === hint);
	if (has('usb') || has('nfc') || has('ble')) return 'security_key';
	if (has('hybrid') && !has('internal')) return 'hybrid';
	// A report of nothing reads as `platform`: it is what every client drew
	// before any of these fields existed.
	return 'platform';
}

const ROUTED_TRANSPORTS: Record<Exclude<SignMethod, 'auto' | 'clear_signer'>, string> = {
	platform: 'internal',
	hybrid: 'hybrid,internal',
	security_key: 'usb,nfc,ble'
};

/**
 * The route for `method` over this account's keys, or `null` for "do what you
 * always did" (`auto` over keys that live nowhere special, an unknown method,
 * or a wallet with no usable credential).
 */
export function signRoute(keys: readonly DeviceKey[], method: string): SignRoute | null {
	const usable = keys.filter((key) => key.credential_id !== '');
	const behindPage = (key: DeviceKey) =>
		typeof key.signer_origin === 'string' && key.signer_origin !== '';
	const clearRoute = (key: DeviceKey): SignRoute => ({
		credentialId: key.credential_id,
		transports: '',
		method: 'clear_signer',
		signerOrigin: key.signer_origin ?? ''
	});

	if (usable.length === 0) return null;
	if (method === 'auto') {
		// The wallet's pinned key decides: only a key that lives behind a page
		// pulls `auto` there. Everything else is the platform's, as before.
		const pinned = usable[0];
		return behindPage(pinned) ? clearRoute(pinned) : null;
	}
	if (method === 'clear_signer') {
		return clearRoute(usable.find(behindPage) ?? usable[0]);
	}
	if (method !== 'platform' && method !== 'hybrid' && method !== 'security_key') return null;

	// A platform sheet reaches a Clear Signer key only when the page was the
	// wallet's own (`*.getvela.app` passkeys are this app's passkeys). A key
	// behind anybody else's page is reachable nowhere else — route it there.
	const reachable = (key: DeviceKey) =>
		!behindPage(key) || clearSignerUsesWalletPasskeys(key.signer_origin as string);
	const candidates = usable.filter(reachable);
	const pinned =
		candidates.find((key) => reportedMethod(key.transports) === method) ?? candidates[0];
	if (pinned === undefined) return clearRoute(usable[0]);
	return {
		credentialId: pinned.credential_id,
		transports: ROUTED_TRANSPORTS[method],
		method,
		signerOrigin: ''
	};
}
