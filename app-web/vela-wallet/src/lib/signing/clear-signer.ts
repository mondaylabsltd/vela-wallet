/**
 * Signing on the Clear Signer (spec 071 contract §4; 075 for the channels).
 *
 * The fourth "Sign with": instead of asking a passkey to sign a digest this
 * app computed and displayed, the request goes to a separate page — the
 * official one, the person's own copy, or the page a key lives behind — which
 * decodes it from the operation's own bytes, derives the digest itself, runs
 * the passkey ceremony and answers.
 *
 * HOW it is reached is `clear-signer-channel.ts`: `postMessage` to a window
 * this wallet opened (same device), or the relay (another device). What this
 * module adds is the signature's own half — which answer is a signature this
 * wallet may use, and the sentence a refusal ends with — and it decides
 * neither: the verdict is the core's (`clearSignerVerify`), the same verdict
 * the phones and the desktop reach, so nothing here parses a WebAuthn answer.
 */

import { clearSignerVerify, type ClearSignerKey, type ClearSignerRequest } from '$lib/core/kernels';
import { PasskeyError, type Assertion } from '$lib/onboarding/core/passkey';
import {
	CLEAR_SIGNER_TIMEOUT_MS,
	openPostMessageChannel,
	signerPageUrl,
	type ClearSignerChannel,
	type ClearSignerHost
} from './clear-signer-channel';

export { CLEAR_SIGNER_TIMEOUT_MS, signerPageUrl };
export type { ClearSignerHost };

/**
 * How a ceremony ended. `refused.code` is the core's (`wrong_challenge`,
 * `foreign_key`, `bad_signature`, `not_verified`, `malformed`, …) or the
 * channel's own: `declined` (the person closed the page, or it said
 * `user_rejected`), `refused` (the page's rules said no — `detail` is its
 * code), `timeout` and `relay_down`.
 */
export type ClearSignerOutcome =
	{ kind: 'accepted'; assertion: Assertion } | { kind: 'refused'; code: string; detail: string };

/** Which sentence a refusal is shown with (contract §5, plus 075's relay). */
export type ClearSignerNotice = 'closed' | 'refused' | 'mismatch' | 'timeout' | 'relay';

export function noticeOf(code: string): ClearSignerNotice {
	switch (code) {
		case 'declined':
			return 'closed';
		case 'refused':
			return 'refused';
		case 'timeout':
			return 'timeout';
		case 'relay_down':
			return 'relay';
		default:
			// wrong_challenge, foreign_key, bad_signature, not_verified,
			// wrong_token, malformed: whatever came back is not a signature this
			// wallet will send.
			return 'mismatch';
	}
}

/**
 * A ceremony that did not produce a signature. A `PasskeyError` of kind
 * `cancelled` on purpose: every pipeline already treats that as "nothing was
 * signed, the request stays open, sign another way" (contract §5) — which is
 * exactly what a closed page, a refusal, a mismatch and a timeout all mean.
 * The words are the Clear Signer's own, shown by its waiting sheet.
 */
export class ClearSignerRefusedError extends PasskeyError {
	readonly code: string;
	readonly detail: string;
	constructor(code: string, detail: string) {
		super('cancelled', `Clear Signer: ${code}${detail ? ` (${detail})` : ''}`);
		this.name = 'ClearSignerRefusedError';
		this.code = code;
		this.detail = detail;
	}
}

/** The page's answer, accepted by the core, in the shape a passkey assertion has. */
function assertionOf(accepted: {
	credentialIdHex: string;
	signatureDer: string;
	authenticatorData: string;
	clientDataJSON: string;
}): Assertion {
	const bare = (hex: string) => hex.replace(/^0x/i, '');
	return {
		credentialId: accepted.credentialIdHex,
		signatureHex: bare(accepted.signatureDer),
		authenticatorDataHex: bare(accepted.authenticatorData),
		clientDataJSONHex: bare(accepted.clientDataJSON),
		authenticatorAttachment: ''
	};
}

/**
 * Put one signing request on an OPEN channel and judge what comes back.
 *
 * The channel is not ended here: a flow may carry more than one request, and
 * only its owner knows whether this was the last.
 */
export async function signOnChannel(
	channel: ClearSignerChannel,
	request: ClearSignerRequest,
	digest: Uint8Array,
	keys: ClearSignerKey[]
): Promise<ClearSignerOutcome> {
	const reply = await channel.ask({ intent: request.intent, context: request.context });
	if (reply.kind === 'refused') return reply;
	let verdict;
	try {
		verdict = clearSignerVerify(reply.payload.result, digest, keys);
	} catch (error) {
		// The core could not even read the question (never the page's answer —
		// that is refused, not thrown). Still an outcome, never a ceremony left
		// waiting for its timeout.
		return {
			kind: 'refused',
			code: 'malformed',
			detail: error instanceof Error ? error.message : String(error)
		};
	}
	if ('accepted' in verdict) return { kind: 'accepted', assertion: assertionOf(verdict.accepted) };
	return { kind: 'refused', code: verdict.refused.code, detail: verdict.refused.detail };
}

export interface ClearSignerCeremony {
	/** Settles exactly once. Never rejects. */
	outcome: Promise<ClearSignerOutcome>;
	/**
	 * "Open the page again": bring the window back, or open it if the browser
	 * blocked it — a tap on this is the user activation a popup needs, which
	 * the swipe that started the signature may have spent by the time the
	 * operation was assembled.
	 */
	reopen(): void;
	/** The waiting sheet's Cancel: declined, like a cancelled passkey sheet. */
	cancel(): void;
}

export interface ClearSignerOptions {
	/** The page the person chose (`SignPrefView.signer_url`). */
	signerUrl: string;
	/** The core's `{intent, context}` for this request. */
	request: ClearSignerRequest;
	/** The digest THIS wallet computed — the one a passkey would have signed. */
	digest: Uint8Array;
	/** The account's keys: the answer must be by one of them. */
	keys: ClearSignerKey[];
	host?: ClearSignerHost;
	timeoutMs?: number;
}

/**
 * Open the Clear Signer on THIS device for one signature and wait for its
 * answer. The page is let go as soon as the request is settled — one
 * signature is a flow of one.
 */
export function openClearSigner(options: ClearSignerOptions): ClearSignerCeremony {
	const channel = openPostMessageChannel({
		signerUrl: options.signerUrl,
		host: options.host,
		timeoutMs: options.timeoutMs
	});
	const outcome = signOnChannel(channel, options.request, options.digest, options.keys).then(
		(settled) => {
			channel.end();
			return settled;
		}
	);
	return {
		outcome,
		reopen: () => channel.reopen(),
		cancel: () => channel.cancel()
	};
}
