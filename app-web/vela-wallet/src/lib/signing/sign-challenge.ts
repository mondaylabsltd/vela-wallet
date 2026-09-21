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
import { chainName, nativeSymbol } from '$lib/services/networks';
import type { OperationToSign } from '$lib/services/safe-transaction';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import { clearSignerSession } from './core/clear-signer.svelte';

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
 * Sign `challenge` the way this request's "Sign with" says. A transaction
 * hands over the ASSEMBLED `operation` its digest covers (the code that
 * assembled it does — see `safe-transaction.ts`); a message has none.
 */
export async function signChallenge(
	challenge: Uint8Array,
	signer: ChallengeSigner,
	operation?: OperationToSign
): Promise<Assertion> {
	// The open sheet's pick — which started at Settings' default — or, with no
	// sheet (the wallet's own send), Settings' default itself: the person's,
	// read from the store, not the factory's.
	await signPreference.settled();
	const method = getSignMethod() ?? (signPreference.view.method as SignMethod);
	if (method !== 'clear_signer') {
		return signWithAny(toHex(challenge), signer.credentials, method);
	}
	const { chainId } = signer.request;
	const name = loadAccounts().find(
		(account) => account.address.toLowerCase() === signer.account.toLowerCase()
	)?.name;
	return clearSignerSession.sign(
		{
			method: signer.request.method,
			params: signer.request.params,
			origin: signer.request.origin,
			chainId,
			chainName: chainName(chainId),
			nativeSymbol: nativeSymbol(chainId),
			account: signer.account,
			accountName: name || undefined,
			credentialIdsHex: signer.keys.map((key) => key.credentialId),
			userOp: operation?.userOp,
			calls: operation?.calls
		},
		challenge,
		signer.keys
	);
}

/** Abort whatever is signing: the passkey ceremony, or the wait on the Clear Signer. */
export function cancelChallenge(): void {
	cancelSign();
	clearSignerSession.cancel();
}
