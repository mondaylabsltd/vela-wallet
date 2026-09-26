/**
 * The one place a challenge gets signed, by the passkey the account signs with.
 *
 * Every signing path (the dApp sheet's transactions and messages, the
 * wallet's own send, the key backup) used to call `signWithAny` with the
 * digest it computed. They call this instead and continue exactly as before:
 * what comes back is an `Assertion` over THAT digest, by one of the account's
 * keys.
 *
 * WHICH key, and over which route, is not asked here (founder, 2026-09-26): a
 * person says where their passkey is when they create the wallet or sign in,
 * and every later signature reuses that answer. The account record names it
 * and the core reads it (`signInRoute`); nothing here chooses.
 */

import { signInRoute, toHex, type TrustedSignerKey } from '$lib/core/kernels';
import { cancelSign, sign, signWithAny, type Assertion } from '$lib/onboarding/core/passkey';
import { loadAccounts } from '$lib/onboarding/core/storage';
import type { Account } from '$lib/onboarding/generated/Account';

export interface ChallengeSigner {
	/** The Safe the signature is for. */
	account: string;
	/** Its founding keys: the Trusted Signer's answer must be by one of them. */
	keys: TrustedSignerKey[];
	/** The allow-list for a record that names no sign-in key, as each path built it before. */
	credentials: { id: string; transports?: string }[];
	/**
	 * What was asked: a dApp's own method, params and origin; the wallet's own
	 * send says `''` for both and its calls become the page's intent.
	 */
	request: { method: string; params: unknown; origin: string; chainId: number };
}

/**
 * Sign `challenge` with the key the account signed in with, over the route it
 * signed in over — or, for a record written before it named that key, the way
 * it always did.
 */
export async function signChallenge(
	challenge: Uint8Array,
	signer: ChallengeSigner
): Promise<Assertion> {
	const account = loadAccounts().find(
		(record) => record.address.toLowerCase() === signer.account.toLowerCase()
	);
	const route = account ? signInRoute(account) : null;
	if (route === null) return signAsBefore(challenge, signer, account);
	// Spec 075: a key behind a Trusted Signer page can only be signed THERE —
	// and the web wallet has no Trusted Signer (owner, 2026-09-23), so it says
	// so instead of asking a platform sheet for a key no authenticator on this
	// device holds.
	if (route.method === 'trusted_signer') {
		throw new Error(
			`this key lives behind ${route.signer_origin ?? 'a Trusted Signer page'}, which only the Vela app can open`
		);
	}
	// The one credential, over the transports the core names — where it was
	// chosen to be and where the sign-in found it — and no WebAuthn hint: the
	// web's sign-in applies none, and a signature must never be stricter than
	// the ceremony that proved the key answers.
	return sign(toHex(challenge), route.credential_id, route.transports);
}

/**
 * A record with no sign-in key: every founding credential allowed, each with
 * the transports it registered with, and the browser left to pick — unless the
 * first key lives behind a Trusted Signer page, which only that page can reach
 * (the core's `sign_route` for `auto`).
 */
async function signAsBefore(
	challenge: Uint8Array,
	signer: ChallengeSigner,
	account: Account | undefined
): Promise<Assertion> {
	const pinned = account?.keys.find((key) => key.credential_id !== '');
	if (pinned?.signer_origin) {
		throw new Error(
			`this key lives behind ${pinned.signer_origin}, which only the Vela app can open`
		);
	}
	return signWithAny(toHex(challenge), signer.credentials);
}

/** Abort whatever is signing. */
export function cancelChallenge(): void {
	cancelSign();
}
