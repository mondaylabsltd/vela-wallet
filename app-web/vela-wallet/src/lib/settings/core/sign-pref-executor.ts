/**
 * The only place the `sign_pref` core touches the outside world — WEB
 * (spec 071).
 *
 * Shaped on `fee-tier-executor.ts`, deliberately: the same sentences (read a
 * preference, persist a preference) against the same store, so there is one
 * way a committed preference reaches disk in this shell.
 *
 * Both keys live under the `vela.` prefix and **survive sign-out**: how a
 * person signs, and which Clear Signer page they trust, belong to them and
 * the device rather than to one account. Neither is listed in
 * `device-storage.ts` — a preference is not a cache.
 *
 * The stored values are handed back RAW. Whether a string is a method this
 * build offers, or a page it would open, is the core's to judge on the way
 * in — a value this build does not understand reads as "never chose" rather
 * than being coerced here.
 *
 * Failure contract (shared effect loop): nothing rejects.
 */

import { getItem, removeItem, setItem } from '$lib/services/storage';
import type { SignPrefOperation } from '$lib/core/generated/SignPrefOperation';
import type { SignPrefShellResult } from '$lib/core/generated/SignPrefShellResult';

export type SignPrefEffect = { id: number; operation: SignPrefOperation };

/** The default "Sign with". */
export const SIGN_METHOD_KEY = 'vela.signMethod';
/** The Clear Signer page a person chose; absent = the official one. */
export const CLEAR_SIGNER_URL_KEY = 'vela.clearSignerUrl';
/** Spec 075: the relay a cross-device pairing goes through; absent = the official one. */
export const CLEAR_SIGNER_RELAY_KEY = 'vela.clearSignerRelay';

export async function executeSignPrefOperation(
	effect: SignPrefEffect
): Promise<SignPrefShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored': {
			const [method, signerUrl, relayUrl] = await Promise.all([
				getItem(SIGN_METHOD_KEY),
				getItem(CLEAR_SIGNER_URL_KEY),
				getItem(CLEAR_SIGNER_RELAY_KEY)
			]);
			return {
				type: 'stored',
				method: method ?? null,
				signer_url: signerUrl ?? null,
				relay_url: relayUrl ?? null
			};
		}
		case 'write_method':
			await setItem(SIGN_METHOD_KEY, operation.method);
			return { type: 'written' };
		case 'write_signer_url':
			if (operation.url === null) await removeItem(CLEAR_SIGNER_URL_KEY);
			else await setItem(CLEAR_SIGNER_URL_KEY, operation.url);
			return { type: 'written' };
		case 'write_relay_url':
			if (operation.url === null) await removeItem(CLEAR_SIGNER_RELAY_KEY);
			else await setItem(CLEAR_SIGNER_RELAY_KEY, operation.url);
			return { type: 'written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled sign_pref operation: ${JSON.stringify(never)}`);
		}
	}
}

export function signPrefOperationFailure(effect: SignPrefEffect): SignPrefShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored':
			// An unreadable preference means "the person never chose": `auto` and
			// the official page — exactly how this wallet signed before the
			// preference existed. It must never read as a page nobody picked.
			return { type: 'stored', method: null, signer_url: null, relay_url: null };
		case 'write_method':
		case 'write_signer_url':
		case 'write_relay_url':
			// Best effort, as every preference write is. What is on screen stays;
			// the next launch reads the old value.
			return { type: 'written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled sign_pref operation: ${JSON.stringify(never)}`);
		}
	}
}
