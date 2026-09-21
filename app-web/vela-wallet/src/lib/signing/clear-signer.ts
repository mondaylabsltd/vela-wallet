/**
 * The Clear Signer, reached from the web wallet (spec 071, contract §4).
 *
 * The fourth "Sign with": instead of asking a passkey to sign a digest this
 * app computed and displayed, the request goes to a separate page — the
 * official one or the person's own copy — which decodes it from the
 * operation's own bytes, derives the digest itself, runs the passkey ceremony
 * and answers. The web's channel is `postMessage` to a page it `window.open`s:
 * the only channel where the BROWSER vouches for both origins.
 *
 * ```text
 *   window.open(<page>sign.html?ch=post)
 *   page   → {vela:'ready'}                       from the page's origin, from that window
 *   wallet → {vela:'intent', id, intent, context} to that origin only
 *   page   → {vela:'result', id, result}          → the core's verdict (clearSignerVerify)
 *          | {vela:'error', id, code}             → declined, or the page refused
 *          | the window closes                    → declined
 *   5 minutes, nothing                            → timeout
 * ```
 *
 * What this module decides is only what the transport needs: who a message
 * is from (the exact origin AND the window this wallet opened) and whether it
 * answers THIS request (`id`). Whether the answer is a valid signature over
 * the digest this wallet computed, by one of this account's keys, is the
 * core's (`clearSignerVerify`) — the same verdict the phones and the desktop
 * reach, so nothing here parses a WebAuthn answer.
 */

import { clearSignerVerify, type ClearSignerKey, type ClearSignerRequest } from '$lib/core/kernels';
import { PasskeyError, type Assertion } from '$lib/onboarding/core/passkey';

/** The whole ceremony, as on every shell (contract §2–4). */
export const CLEAR_SIGNER_TIMEOUT_MS = 5 * 60_000;

/** How often a popup is checked for having been closed. */
const CLOSED_POLL_MS = 500;

/** One window, reused: "open the page again" brings back the same one. */
const WINDOW_NAME = 'vela-clear-signer';
const WINDOW_FEATURES = 'popup,width=460,height=760';

/**
 * `<base>sign.html?ch=post` — built the way the core's `sign_page` builds
 * every channel's address: a base that already names a page keeps it, one
 * ending in `/` gets `sign.html`, anything else `/sign.html`.
 */
export function signerPageUrl(base: string): string {
	const trimmed = base.trim();
	const page = trimmed.endsWith('.html')
		? trimmed
		: trimmed.endsWith('/')
			? `${trimmed}sign.html`
			: `${trimmed}/sign.html`;
	return `${page}?ch=post`;
}

/**
 * How a ceremony ended. `refused.code` is the core's (`wrong_challenge`,
 * `foreign_key`, `bad_signature`, `not_verified`, `malformed`, …) or this
 * channel's own: `declined` (the person closed the page, or it said
 * `user_rejected`), `refused` (the page's rules said no — `detail` is its
 * code) and `timeout`.
 */
export type ClearSignerOutcome =
	{ kind: 'accepted'; assertion: Assertion } | { kind: 'refused'; code: string; detail: string };

/** Which sentence a refusal is shown with (contract §5). */
export type ClearSignerNotice = 'closed' | 'refused' | 'mismatch' | 'timeout';

export function noticeOf(code: string): ClearSignerNotice {
	switch (code) {
		case 'declined':
			return 'closed';
		case 'refused':
			return 'refused';
		case 'timeout':
			return 'timeout';
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

/** What the channel needs of the browser — `window`, or a test's stand-in. */
export interface ClearSignerHost {
	open(url: string, target: string, features: string): Window | null;
	addEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
	removeEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
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

/** Open the Clear Signer for one request and wait for its answer. */
export function openClearSigner(options: ClearSignerOptions): ClearSignerCeremony {
	const host: ClearSignerHost = options.host ?? window;
	const url = signerPageUrl(options.signerUrl);
	const origin = new URL(url).origin;
	const id = crypto.randomUUID();

	let popup: Window | null = null;
	let settled = false;
	let closedSeen = false;
	let settle: (outcome: ClearSignerOutcome) => void = () => {};
	const outcome = new Promise<ClearSignerOutcome>((resolve) => {
		settle = (value) => {
			if (settled) return;
			settled = true;
			host.removeEventListener('message', onMessage);
			clearInterval(poll);
			clearTimeout(timer);
			// The page has nothing left to do; the person goes back to where they
			// signed from, as the phones bring the app back over the tab.
			try {
				if (popup && !popup.closed) popup.close();
			} catch {
				/* a window we cannot close is the browser's to keep */
			}
			resolve(value);
		};
	});
	const refused = (code: string, detail = '') => settle({ kind: 'refused', code, detail });

	function onMessage(event: MessageEvent): void {
		if (settled) return;
		// Both, never one: the origin says WHICH SITE is speaking, the source
		// says it is the window this wallet opened — not another tab of the
		// same site, and not a frame it embeds.
		if (event.origin !== origin || popup === null || event.source !== popup) return;
		const data = event.data as { vela?: unknown; id?: unknown; result?: unknown; code?: unknown };
		if (!data || typeof data !== 'object') return;
		if (data.vela === 'ready') {
			// Every `ready` gets the intent: a page that reloaded has forgotten it.
			popup.postMessage(
				{ vela: 'intent', id, intent: options.request.intent, context: options.request.context },
				origin
			);
			return;
		}
		if (data.id !== id) return;
		if (data.vela === 'result') {
			let verdict;
			try {
				verdict = clearSignerVerify(data.result, options.digest, options.keys);
			} catch (error) {
				// The core could not even read the question (never the page's
				// answer — that is refused, not thrown). Still an outcome, never
				// a ceremony left waiting for its timeout.
				refused('malformed', error instanceof Error ? error.message : String(error));
				return;
			}
			if ('accepted' in verdict) {
				settle({ kind: 'accepted', assertion: assertionOf(verdict.accepted) });
			} else {
				refused(verdict.refused.code, verdict.refused.detail);
			}
		} else if (data.vela === 'error') {
			// The page's own vocabulary (PROTOCOL.md): `user_rejected` is the
			// person, anything else is the page's rules refusing.
			const code = typeof data.code === 'string' ? data.code : '';
			if (code === '' || code === 'user_rejected') refused('declined');
			else refused('refused', code);
		}
	}

	function open(): void {
		popup = host.open(url, WINDOW_NAME, WINDOW_FEATURES);
		closedSeen = false;
	}

	host.addEventListener('message', onMessage);
	const poll = setInterval(() => {
		if (popup === null || !popup.closed) {
			closedSeen = false;
			return;
		}
		// One more tick first: an answer posted as the page closed may still be
		// queued behind this check, and it must win over "closed".
		if (closedSeen) refused('declined');
		closedSeen = true;
	}, CLOSED_POLL_MS);
	const timer = setTimeout(() => refused('timeout'), options.timeoutMs ?? CLEAR_SIGNER_TIMEOUT_MS);
	open();

	return {
		outcome,
		reopen() {
			if (settled) return;
			if (popup !== null && !popup.closed) popup.focus();
			else open();
		},
		cancel() {
			refused('declined');
		}
	};
}
