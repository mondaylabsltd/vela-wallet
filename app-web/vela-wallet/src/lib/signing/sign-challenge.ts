/**
 * The one place a challenge gets signed, by the passkey the account signs with
 * — in Vela, or on the account's trusted signing page.
 *
 * Every signing path (the dApp sheet's transactions and messages — on the web
 * and in the Chrome extension, which packages this same client — the wallet's
 * own send, the key backup, a multisig executor's operation) calls this with
 * the digest it computed, and continues exactly as before: what comes back is
 * an `Assertion` over THAT digest, by one of the account's keys.
 *
 * WHICH key, and over which route, is not asked here (founder, 2026-09-26): a
 * person says where their passkey is when they create the wallet or sign in,
 * and every later signature reuses that answer. The account record names it
 * and the core reads it (`signingPlan`, spec 102); nothing here chooses.
 *
 * WHERE it is reviewed and signed is the account's venue, and the core's one
 * rule says where each signature goes (`signingRoute`, R7 — owner,
 * 2026-10-30: the page is usable everywhere, superseding "the web opens no
 * signing page"):
 *
 * - `in_vela` — Vela's own sheet already showed it; the passkey signs here;
 * - `page` — the account chose to review and sign on a trusted page (or, for
 *   a custom-domain account, its page is the only place its keys answer). The
 *   request the CORE builds (`trustedSignerSignatureRequest`: the method, the
 *   params, the assembled operation) is handed to the checked page
 *   (`page-handoff.svelte.ts`), and its answer is accepted only as the core
 *   accepts it (`trustedSignerSignatureAccept`): over exactly the bytes the
 *   core computes from that request, which must be `challenge`, by one of the
 *   account's keys, user-verified, under the Safe verifier's client-data
 *   rules. Nothing the page says about what it signed is read;
 * - `blocked` — nothing on this device can reach the account's keys; the
 *   refusal carries the core's reason to the sign/send core
 *   (`venue_blocked`), which says it in the person's language.
 *
 * A multisig member's approval goes to its page through
 * `multisig/core/page-approval.svelte.ts` (the page needs the raw SafeTx, not
 * a digest); one that reaches here routed to a page is refused, never signed
 * in Vela behind the person's choice. A member's owner proof
 * (`vela_multisigCreate`, §16) is a signature like the others here: the page
 * is handed the multisig's genesis, never the challenge.
 */

import {
	signingPlan,
	signingRoute,
	toHex,
	trustedSignerSignatureAccept,
	trustedSignerSignatureRequest,
	type TrustedSignerKey
} from '$lib/core/kernels';
import type { KeyMethod } from '$lib/core/generated/KeyMethod';
import type { PageFailure } from '$lib/core/generated/PageFailure';
import type { KeyRoute } from '$lib/core/generated/KeyRoute';
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import { cancelSign, sign, signWithAny, type Assertion } from '$lib/onboarding/core/passkey';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { chainName, nativeSymbol } from '$lib/services/networks';
import { AskerGoneError } from '$lib/signing/core/sign-types';
import {
	handOff,
	pageHandoff,
	PageHandoffError,
	type PageHandoffDeps
} from './page-handoff.svelte';

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
	/**
	 * Tell the asker the request is still being answered while the page has it
	 * (spec 082: the extension's `claim('sign')`); `false` — the request is
	 * gone. Absent: nobody to keep.
	 */
	keepAlive?: () => Promise<boolean>;
}

/**
 * The operation a SafeOp digest covers, as the page decodes it
 * (`safe-transaction.ts` `OperationToSign`): the assembled op in the page's
 * field names, and the legs before the fee leg.
 */
export interface OperationForPage {
	userOp: Record<string, string>;
	calls: { to: string; value: string; data: string }[];
}

/**
 * This account cannot sign here (spec 102): nothing on this device can reach
 * its keys. `block` is the core's reason — the same one Settings draws under
 * the account's "Where you review and sign" — and the executors hand it back
 * to the core as `venue_blocked { block }` (the sign sheet's
 * `SignErrorNotice.venue_block`, the send screen's `SendView.tx_venue_block`),
 * which says it in the person's words. The message is for logs only: it names
 * the block's type, never a sentence of ours.
 */
export class VenueBlockedError extends Error {
	constructor(readonly block: VenueBlock) {
		super(`venue blocked: ${block.type}`);
		this.name = 'VenueBlockedError';
	}
}

/**
 * The person declined on the trusted signing page, or closed it unsigned. A
 * passkey cancel to every path that has no asker (the wallet's own send keeps
 * its confirm open for another try, `passkey_cancelled`); the dApp path
 * answers the site 4001 — the page is the venue, so its "no" is the person's
 * answer (`SignSubmitOutcome::PageDeclined`).
 */
export class PageDeclinedError extends Error {
	/** Read as a passkey prompt the person closed, wherever one is. */
	readonly kind = 'cancelled' as const;
	readonly timedOut = false;
	constructor() {
		super('The signing page was closed without signing');
		this.name = 'PageDeclinedError';
	}
}

/**
 * The page did not sign, and not because the person said no: it was not
 * opened (its check failed), never answered, refused by its own rules, or
 * answered with something that is not this account's signature of the core's
 * bytes. Nothing was signed. `failure` and `code` say which, for the sheet's
 * line and the logs.
 */
export class PageSignFailedError extends Error {
	constructor(
		readonly failure: string,
		readonly code = ''
	) {
		super(
			failure === 'unverified'
				? `The signing page's answer did not check out (${code}) — nothing was signed`
				: failure === 'declined'
					? 'The signing page refused this request — nothing was signed'
					: failure === 'not_opened'
						? 'The signing page did not pass its check, so it was not opened — nothing was signed'
						: failure === 'silent'
							? 'The signing page did not answer — nothing was signed'
							: 'The signing page could not take this request — nothing was signed'
		);
		this.name = 'PageSignFailedError';
	}

	/**
	 * The core's word for it (round 7, r7-2 / r7-7): the sheet and the asking
	 * site hear WHY the page did not sign — not answering, an answer that is
	 * not this request's, its own refusal, a check that failed — never "the
	 * transaction could not be submitted", which a message never is.
	 */
	get page(): PageFailure {
		switch (this.failure) {
			case 'silent':
				return 'unanswered';
			case 'unverified':
				return 'mismatch';
			case 'not_opened':
				return 'not_opened';
			default:
				return 'refused';
		}
	}
}

/** The kinds the page decodes from an assembled operation (`SignatureKind::{UserOperation, Batch}`). */
const OPERATION_METHODS = new Set([
	'',
	'eth_sendTransaction',
	'wallet_sendCalls',
	'vela_multisigExecute'
]);

/** Stands in for the browser in tests (`page-handoff.svelte.ts`). */
let handoffDeps: PageHandoffDeps | undefined;
export function setPageHandoffDeps(deps: PageHandoffDeps | undefined): void {
	handoffDeps = deps;
}

/**
 * Sign `challenge` where the account signs: with the key it signed in with,
 * over the route it signed in over (or, for a record written before it named
 * that key, the way it always did) — or on its trusted signing page, handed
 * the request the core builds from `signer.request` and `operation`.
 */
export async function signChallenge(
	challenge: Uint8Array,
	signer: ChallengeSigner,
	operation?: OperationForPage
): Promise<Assertion> {
	const record = loadAccounts().find(
		(account) => account.address.toLowerCase() === signer.account.toLowerCase()
	);
	// The STORED record, read by the core: it migrates one from before 102.
	const plan = record === undefined ? null : signingPlan(record);
	if (record === undefined || plan === null) return signAsBefore(challenge, signer);
	if (plan.blocked != null) throw new VenueBlockedError(plan.blocked);
	// R7: the core's one rule. A method it routes nowhere (one no venue
	// signs) can still be signed in Vela by an account whose venue is Vela —
	// as before — and never behind a page the person chose.
	const route = signingRoute(record, signer.request.method);
	if (route?.type === 'blocked') throw new VenueBlockedError(route.block);
	if (route?.type === 'page') {
		return signOnPage(challenge, signer, route.url, record.name, plan, operation);
	}
	if (route === null && plan.venue.type === 'page') {
		throw new PageSignFailedError('unsupported', signer.request.method);
	}
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
 * The page reviews and signs: the core's request, the hand-off card, the
 * checked page, and back the answer the core accepted — as an `Assertion`
 * over `challenge`, which every path continues with unchanged.
 */
async function signOnPage(
	challenge: Uint8Array,
	signer: ChallengeSigner,
	page: string,
	name: string,
	plan: NonNullable<ReturnType<typeof signingPlan>>,
	operation: OperationForPage | undefined
): Promise<Assertion> {
	const { method, params, origin, chainId } = signer.request;
	// An approval is the multisig's to hand over (it carries the SafeTx); an
	// operation the page cannot see is not signed behind it.
	if (method === 'vela_multisigApprove') throw new PageSignFailedError('unsupported', method);
	// A multisig's owner proof (§16) goes like any signature: its params are
	// the core's (`PasskeyPurpose::Record.params` — the genesis, the member,
	// the networks), the page reads the address, the members and the rule
	// from the genesis itself, and the core accepts only an answer over the
	// sponsor challenge for the address that genesis makes — which must be
	// `challenge`. The Safari package, which has no passkeys of its own (R8),
	// creates a multisig this way.
	if (OPERATION_METHODS.has(method) && operation === undefined) {
		throw new PageSignFailedError('unsupported', 'no operation');
	}
	const request = trustedSignerSignatureRequest({
		method,
		params,
		origin,
		chainId,
		chainName: chainName(chainId) || null,
		nativeSymbol: nativeSymbol(chainId) || null,
		account: signer.account,
		accountName: name,
		credentialIdsHex: signer.keys.map((k) => k.credentialId),
		keyRoute: plan.key ?? undefined,
		userOp: OPERATION_METHODS.has(method) ? operation?.userOp : null,
		calls: OPERATION_METHODS.has(method) ? operation?.calls : []
	});
	const digestHex = toHex(challenge);
	try {
		return await handOff<Assertion>(
			{
				kind: 'signature',
				page,
				signer: name,
				keyLabel: plan.key_label,
				request,
				accept: (result) =>
					trustedSignerSignatureAccept({ request, result, keys: signer.keys, digestHex }),
				keepAlive: signer.keepAlive
			},
			handoffDeps
		);
	} catch (error) {
		if (!(error instanceof PageHandoffError)) throw error;
		if (error.failure === 'gone') throw new AskerGoneError('sign');
		if (error.byPerson) throw new PageDeclinedError();
		throw new PageSignFailedError(error.failure, error.code);
	}
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

/** Abort whatever is signing — the passkey prompt, or a signature on the page. */
export function cancelChallenge(): void {
	cancelSign();
	if (pageHandoff.current?.kind === 'signature') pageHandoff.current.cancel();
}
