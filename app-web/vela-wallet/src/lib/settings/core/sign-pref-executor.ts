/**
 * The only place the `sign_pref` core touches the outside world — WEB
 * (spec 071).
 *
 * Shaped on `fee-tier-executor.ts`, deliberately: the same sentences (read a
 * preference, persist a preference) against the same store, so there is one
 * way a committed preference reaches disk in this shell.
 *
 * The key lives under the `vela.` prefix and **survives sign-out**: which
 * Trusted Signer page a person trusts belongs to them and the device rather
 * than to one account. It is not listed in `device-storage.ts` — a preference
 * is not a cache.
 *
 * The stored value is handed back RAW. Whether a string is a page this build
 * would open is the core's to judge on the way in — a value this build does
 * not understand reads as "never chose" rather than being coerced here.
 *
 * Failure contract (shared effect loop): nothing rejects.
 */

import { getItem, removeItem, setItem } from '$lib/services/storage';
import type { SignPrefOperation } from '$lib/core/generated/SignPrefOperation';
import type { SignPrefShellResult } from '$lib/core/generated/SignPrefShellResult';

export type SignPrefEffect = { id: number; operation: SignPrefOperation };

/** The Trusted Signer page a person chose; absent = the official one. */
export const TRUSTED_SIGNER_URL_KEY = 'vela.trustedSignerUrl';

export async function executeSignPrefOperation(
	effect: SignPrefEffect
): Promise<SignPrefShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored':
			return { type: 'stored', signer_url: (await getItem(TRUSTED_SIGNER_URL_KEY)) ?? null };
		case 'write_signer_url':
			if (operation.url === null) await removeItem(TRUSTED_SIGNER_URL_KEY);
			else await setItem(TRUSTED_SIGNER_URL_KEY, operation.url);
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
			// An unreadable preference means "the person never chose": the
			// official page. It must never read as a page nobody picked.
			return { type: 'stored', signer_url: null };
		case 'write_signer_url':
			// Best effort, as every preference write is. What is on screen stays;
			// the next launch reads the old value.
			return { type: 'written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled sign_pref operation: ${JSON.stringify(never)}`);
		}
	}
}
