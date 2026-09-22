/**
 * The passkey ceremonies, run on a Clear Signer page (spec 075).
 *
 * With `method = clear_signer` the four passkey operations — register, sign
 * in, prove, confirm membership — do not open the OS sheet. They go to the
 * signer page as its own requests (`vela_createPasskey`, `vela_signIn`,
 * `vela_proof`, `vela_memberProof`), and what comes back is reported to the
 * machine exactly as the platform ceremony's would be.
 *
 * Nothing here judges an answer. The core builds the request from the
 * operation's own wire JSON (`clearSignerCeremonyRequest`) and judges the
 * answer against it (`clearSignerVerifyCeremony`): the signed origin, user
 * verification, and a challenge the PAGE derived — never one a requester
 * supplied. The single exception is the member proof, whose challenge is 32
 * bytes: the wallet fetches it from the registry itself and hands those bytes
 * to the core, which accepts the page's answer only if it signed exactly them.
 *
 * One page visit per flow: create → member proof, sign in → recover ×2. The
 * session is the resident one (`$lib/signing/core/clear-signer.svelte`), so
 * the person sees one sheet through the whole flow, and `endCeremonyFlow()`
 * says goodbye when the flow ends — including when they leave the screen.
 */

import {
	clearSignerCeremonyRequest,
	clearSignerVerifyCeremony,
	type ClearSignerCeremonyVerdict
} from '$lib/core/kernels';
import { ClearSignerRefusedError } from '$lib/signing/clear-signer';
import { clearSignerSession } from '$lib/signing/core/clear-signer.svelte';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import * as Registry from './registry';
import { buildMemberProof } from './wasm-client';
import { loadAccounts } from './storage';
import type { ShellOperation } from '../generated/ShellOperation';
import type { ShellResult } from '../generated/ShellResult';

/** The four operations a Clear Signer page can answer. */
export type CeremonyOperation = Extract<
	ShellOperation,
	{ type: 'register_passkey' | 'authenticate_passkey' | 'sign_proof' | 'sign_member_proof' }
>;

/**
 * The ceremony this operation asks the Clear Signer for, or `null` for every
 * other operation — including the same four on another route.
 *
 * Deliberately not a type predicate: the executor's switch still has to
 * handle all four for the platform routes, and a predicate would narrow them
 * out of its own `else`.
 */
export function clearSignerCeremonyOf(operation: ShellOperation): CeremonyOperation | null {
	switch (operation.type) {
		case 'register_passkey':
		case 'authenticate_passkey':
		case 'sign_proof':
		case 'sign_member_proof':
			return operation.method === 'clear_signer' ? operation : null;
		default:
			return null;
	}
}

/**
 * The wallet's name for the card the page draws — how a person recognises
 * their own wallet on a page that is deliberately not this app.
 *
 * A create names it in the operation itself; the ceremonies that follow do
 * not, so the name is remembered for the flow. A sign-in has no name at all
 * until the account is resolved, and the page simply shows none.
 */
let flowWalletName = '';

/** The page a key lives behind, when the operation names one. */
function signerOriginOf(operation: CeremonyOperation): string {
	switch (operation.type) {
		case 'sign_proof':
		case 'sign_member_proof':
			return operation.signer_origin ?? '';
		default:
			return '';
	}
}

/**
 * The relying party a page at `origin` uses — its hostname, with every
 * `*.getvela.app` folded to `getvela.app` (PROTOCOL.md §"创始人已经拍板的规矩" 2).
 * The member challenge must be fetched under the page's rpId, because the
 * page fetches its own under that rpId and the two must be the same bytes.
 */
export function rpIdOfSigner(origin: string): string {
	let host: string;
	try {
		host = new URL(origin).hostname.toLowerCase();
	} catch {
		return '';
	}
	if (host === 'getvela.app' || host.endsWith('.getvela.app')) return 'getvela.app';
	return host;
}

/** Run one ceremony on the Clear Signer and report it as the machine expects. */
export async function runClearSignerCeremony(operation: CeremonyOperation): Promise<ShellResult> {
	if (operation.type === 'register_passkey') flowWalletName = operation.name;
	else if (flowWalletName === '') flowWalletName = loadAccounts()[0]?.name ?? '';

	await signPreference.ready();
	const named = signerOriginOf(operation);
	const signerUrl = named !== '' ? named : signPreference.view.signer_url;
	const operationJson = JSON.stringify(operation);
	const request = clearSignerCeremonyRequest(
		operationJson,
		crypto.randomUUID(),
		flowWalletName,
		Registry.registryUrl()
	);
	if (request === undefined) {
		// The core does not read this operation as a ceremony at all — a shell
		// that asked for one anyway must not pretend it happened.
		throw new ClearSignerRefusedError('malformed', `not a ceremony: ${operation.type}`);
	}

	// The member proof is the one challenge the page does not invent: it
	// fetches the registry's, and so does this wallet, and the core accepts
	// the answer only if they are the same 32 bytes.
	let expected: Uint8Array | undefined;
	if (operation.type === 'sign_member_proof') {
		const challenge = await Registry.memberChallenge({
			rpId: rpIdOfSigner(signerUrl),
			groupPublicKey: operation.group_public_key_hex,
			publicKey: operation.public_key_hex,
			attestation: operation.attestation_hex
		});
		expected = bytesOfHex(challenge.challenge);
	}

	const { reply, signerOrigin } = await clearSignerSession.ceremony(
		{ intent: request.intent, context: request.context },
		signerUrl
	);
	if (reply.kind === 'refused') {
		throw new ClearSignerRefusedError(reply.code, reply.detail);
	}
	const verdict: ClearSignerCeremonyVerdict = clearSignerVerifyCeremony(
		operationJson,
		reply.payload,
		signerOrigin,
		expected
	);
	if ('refused' in verdict) {
		// The page answered, and the core would not take it. The person is told
		// which way it failed, on the same sheet the wait was on.
		clearSignerSession.noteRefusal(verdict.refused.code);
		throw new ClearSignerRefusedError(verdict.refused.code, verdict.refused.detail);
	}

	const nowIso = new Date().toISOString();
	if ('registration' in verdict) {
		return { type: 'passkey_registered', registration: verdict.registration, now_iso: nowIso };
	}
	const assertion = verdict.assertion;
	switch (operation.type) {
		case 'authenticate_passkey':
			return { type: 'passkey_authenticated', assertion, now_iso: nowIso };
		case 'sign_proof':
			return { type: 'proof_signed', assertion, now_iso: nowIso };
		case 'sign_member_proof':
			// Exactly as the platform path builds it: the proof is the core's
			// from the same three fields, never assembled here.
			return {
				type: 'member_proof_signed',
				proof: buildMemberProof(
					assertion.authenticator_data_hex,
					assertion.client_data_json_hex,
					assertion.signature_der_hex
				)
			};
		default:
			// A registration answered a ceremony that wanted an assertion.
			throw new ClearSignerRefusedError('malformed', 'the page answered the wrong ceremony');
	}
}

/** The flow ended (or was abandoned): let the page go. */
export function endCeremonyFlow(): void {
	flowWalletName = '';
	clearSignerSession.endFlow();
}

function bytesOfHex(hex: string): Uint8Array {
	const bare = hex.startsWith('0x') ? hex.slice(2) : hex;
	return Uint8Array.from((bare.match(/../g) ?? []).map((pair) => Number.parseInt(pair, 16)));
}
