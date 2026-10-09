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
 * and the core reads it (`signingPlan`, spec 102); nothing here chooses.
 *
 * WHERE it is reviewed and signed is the account's venue (spec 102) — and the
 * web has one venue only: its own sheet. It opens no signing page (owner,
 * 2026-09-23; plan P2-11), so
 *
 * - a `getvela.app` account whose venue is a trusted page signs HERE, natively:
 *   its keys are this site's passkeys, and the web has no hand-off to make;
 * - an account on a custom signing domain cannot sign here at all — its keys
 *   answer only on its own page — and the refusal carries the core's reason
 *   (`signingVenueBlock(domain, in_vela)`), never a guess of ours.
 */

import { signingPlan, signingVenueBlock, toHex, type TrustedSignerKey } from '$lib/core/kernels';
import type { KeyMethod } from '$lib/core/generated/KeyMethod';
import type { KeyRoute } from '$lib/core/generated/KeyRoute';
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import { cancelSign, sign, signWithAny, type Assertion } from '$lib/onboarding/core/passkey';
import { loadAccounts } from '$lib/onboarding/core/storage';

export interface ChallengeSigner {
	/** The Safe the signature is for. */
	account: string;
	/** Its founding keys: a signature must be by one of them. */
	keys: TrustedSignerKey[];
	/** The allow-list for a record that names no sign-in key, as each path built it before. */
	credentials: { id: string; transports?: string }[];
	/**
	 * What was asked: a dApp's own method, params and origin; the wallet's own
	 * send says `''` for both.
	 */
	request: { method: string; params: unknown; origin: string; chainId: number };
}

/**
 * Nothing on the web can reach this account's keys (spec 102 R1): they live on
 * another signing domain, behind that domain's own page. `block` is the core's
 * reason — the same one Settings draws under the account's "Where you review
 * and sign" — so a surface that catches this says it in the person's words.
 * The message is the English line, for logs and for a surface that does not.
 */
export class VenueBlockedError extends Error {
	constructor(readonly block: VenueBlock) {
		super(`Vela can't reach keys on ${block.domain}.`);
		this.name = 'VenueBlockedError';
	}
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
	const record = loadAccounts().find(
		(account) => account.address.toLowerCase() === signer.account.toLowerCase()
	);
	// The STORED record, read by the core: it migrates one from before 102.
	const plan = record === undefined ? null : signingPlan(record);
	if (record === undefined || plan === null) return signAsBefore(challenge, signer);
	// P2-11: the web's only venue is its own sheet. Asked of THAT venue, so a
	// custom-domain account is refused whatever its own venue says.
	const block = signingVenueBlock(plan.domain, { type: 'in_vela' });
	if (block !== null) throw new VenueBlockedError(block);
	const key = plan.key ?? null;
	if (key === null) return signAsBefore(challenge, signer);
	// The one credential, over the transports the core names — where it was
	// chosen to be and where the sign-in found it. The chosen method's hint
	// only when that is the same place: a key the sign-in found somewhere else
	// must not be steered away from it, and a signature is never stricter than
	// the ceremony that proved the key answers.
	return sign(
		toHex(challenge),
		key.credential_id,
		key.transports,
		foundWhereChosen(record, key) ? (key.method as KeyMethod) : undefined
	);
}

/**
 * Whether the sign-in found the key where the person chose it to be: the route
 * names nothing beyond what the choice alone would. "What the choice alone
 * would" is the core's answer for the same record without where the key was
 * found — the rule stays the core's, not a copy of it here.
 */
function foundWhereChosen(record: object, key: KeyRoute): boolean {
	const probe: Record<string, unknown> = {
		...record,
		sign_in_key: { credential_id: key.credential_id, method: key.method, transports: '' }
	};
	// An older build's copy would win over the probe when it names another key.
	delete probe.signed_in_with;
	const chosen = signingPlan(probe)?.key ?? null;
	return chosen === null || chosen.transports === key.transports;
}

/**
 * A record with no sign-in key (or none stored for the address): every
 * founding credential allowed, each with the transports it registered with,
 * and the browser left to pick.
 */
function signAsBefore(challenge: Uint8Array, signer: ChallengeSigner): Promise<Assertion> {
	return signWithAny(toHex(challenge), signer.credentials);
}

/** Abort whatever is signing. */
export function cancelChallenge(): void {
	cancelSign();
}
