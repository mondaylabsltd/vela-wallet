/**
 * The one place a challenge gets signed: by a passkey, or — when this
 * request's "Sign with" is the Clear Signer — on the Clear Signer's page
 * (spec 071).
 *
 * Every signing path (the dApp sheet's transactions and messages, the
 * wallet's own send, the key backup) used to call `signWithAny` with the
 * digest it computed. They call this instead and continue exactly as before:
 * whichever way it was signed, what comes back is an `Assertion` over THAT
 * digest, by one of the account's keys. For the Clear Signer the core built
 * the page's request and judged its answer; nothing here decides either.
 */

import { toHex, type ClearSignerKey } from '$lib/core/kernels';
import {
	cancelSign,
	getSignMethod,
	signWithAny,
	type Assertion,
	type SignMethod
} from '$lib/onboarding/core/passkey';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import type { AccountKey } from '$lib/onboarding/generated/AccountKey';
import { signRoute, type DeviceKey } from './sign-route';

export interface ChallengeSigner {
	/** The Safe the signature is for. */
	account: string;
	/** Its founding keys: the Clear Signer's answer must be by one of them. */
	keys: ClearSignerKey[];
	/** The passkey ceremony's allow-list, exactly as each path built it before. */
	credentials: { id: string; transports?: string }[];
	/**
	 * What was asked: a dApp's own method, params and origin; the wallet's own
	 * send says `''` for both and its calls become the page's intent.
	 */
	request: { method: string; params: unknown; origin: string; chainId: number };
}

/**
 * Sign `challenge` the way this request's "Sign with" says.
 *
 * It used to take the ASSEMBLED operation the digest covers, for the clear
 * signer to show on its own page. Spec 075 cut the clear signer, and with it
 * the only reader: what is signed here is the challenge, and nothing else.
 */
export async function signChallenge(
	challenge: Uint8Array,
	signer: ChallengeSigner
): Promise<Assertion> {
	// The open sheet's pick — which started at Settings' default — or, with no
	// sheet (the wallet's own send), Settings' default itself: the person's,
	// read from the store, not the factory's.
	await signPreference.settled();
	const method = getSignMethod() ?? (signPreference.view.method as SignMethod);
	const account = loadAccounts().find(
		(record) => record.address.toLowerCase() === signer.account.toLowerCase()
	);
	// Spec 075: WHERE the key lives has the last word. A key minted or found
	// through a Clear Signer page can only be signed THERE — and the web wallet
	// has no Clear Signer (owner, 2026-09-23), so it says so instead of asking
	// a platform sheet for a key no authenticator on this device holds.
	const route = signRoute(deviceKeysOf(account, signer), method);
	if (route?.method === 'clear_signer') {
		throw new Error(
			`this key lives behind ${route.signerOrigin}, which only the Vela app can open`
		);
	}
	return signWithAny(toHex(challenge), signer.credentials, method);
}

/**
 * The account's keys as the route reads them — credential, transports and the
 * page a key lives behind. A record with no `keys` array is the legacy
 * single-key shape, and its one credential is the request's own allow-list.
 */
function deviceKeysOf(
	account: { keys?: AccountKey[] } | undefined,
	signer: ChallengeSigner
): DeviceKey[] {
	const keys = account?.keys ?? [];
	if (keys.length > 0) {
		return keys.map((key) => ({
			credential_id: key.credential_id,
			transports: key.transports,
			signer_origin: key.signer_origin
		}));
	}
	return signer.credentials.map((credential) => ({
		credential_id: credential.id,
		transports: credential.transports ?? ''
	}));
}

/** Abort whatever is signing. */
export function cancelChallenge(): void {
	cancelSign();
}
