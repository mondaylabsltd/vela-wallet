/**
 * Every WebAuthn ceremony this app performs, and nothing else.
 *
 * Ported from `src/modules/passkey/index.ts` in the Expo client, minus its
 * native branch — this app only ever runs in a browser. The comments that
 * explain *why* a constraint exists travel with it, because each one is a bug
 * that was paid for once.
 *
 * Nothing here decides what a failure means. It classifies the platform's error
 * into the vocabulary the core branches on (`FailureKind`) and stops; the core
 * owns what happens next.
 */

import { isPackagedApp } from '$lib/extension/page-url';
import type { FailureKind } from '../generated/FailureKind';

/** The native relying party, shared by the extension. See `relyingPartyId`. */
const RELYING_PARTY_NATIVE = 'getvela.app';

/**
 * WebAuthn caps `user.id` at 64 bytes ("The user handle... MUST NOT exceed 64
 * bytes"). `encodeUserHandle` appends '\0' plus a 36-char uuid (37 bytes), so
 * the UTF-8 name must fit the remaining 27. The core validates against the same
 * budget (`name_fits_user_handle`) before any ceremony starts.
 */
export const MAX_USER_NAME_BYTES = 64 - 37;

export class PasskeyError extends Error {
	readonly kind: FailureKind;
	constructor(kind: FailureKind, message: string) {
		super(message);
		this.name = 'PasskeyError';
		this.kind = kind;
	}
}

export type Registration = {
	credentialId: string;
	attestationObjectHex: string;
	clientDataJSONHex: string;
	authenticatorAttachment: string;
	transports: string;
};

export type Assertion = {
	credentialId: string;
	signatureHex: string;
	authenticatorDataHex: string;
	clientDataJSONHex: string;
	authenticatorAttachment: string;
	userIdHex?: string;
};

/**
 * The relying party this browser should use.
 *
 * A passkey is bound to its rpId, so this must be stable for a given
 * deployment — change it and every existing wallet becomes unreachable from
 * this origin.
 */
export function relyingPartyId(): string {
	if (typeof window === 'undefined') return RELYING_PARTY_NATIVE;

	// A dev tool may set this global to pin the rpId (the WebAuthn proxy
	// extension that used to live in this repo did; it was deleted in spec 039,
	// the seam stays because it costs nothing and lets a future tool do the same)
	// so rpId stays consistent across
	// both the ceremony and the registry queries that look keys up by it.
	const proxied = (window as unknown as { __VELA_WEBAUTHN_PROXY_RPID__?: string })
		.__VELA_WEBAUTHN_PROXY_RPID__;
	if (proxied) return proxied;

	// An extension page's origin is `chrome-extension://<id>`, which has NO
	// registrable domain of its own — so the hostname below is the extension id,
	// and taking it is the one mistake here that never announces itself: the
	// ceremony succeeds, a perfectly valid passkey is minted under a relying
	// party nothing else in the world shares, and since the address is derived
	// from the keys (spec 019 invariant ②) the person lands in a DIFFERENT,
	// empty wallet — no error, no warning, just somebody else's money missing.
	// Chrome permits an extension to claim a relying party it holds host
	// permission for and no other; `extension/manifest.json`'s
	// `https://getvela.app/*` entry is what unlocks this line, and
	// `$lib/extension/package.test.ts` guards that the entry stays. Measured in
	// spec 027 D31, down to the assertion's rpIdHash being sha256("getvela.app")
	// whatever origin called for it — one passkey, one address, two doorways.
	if (isPackagedApp()) return RELYING_PARTY_NATIVE;

	const host = window.location.hostname;
	if (host === 'localhost' || host === '127.0.0.1' || /^\d+\.\d+\.\d+\.\d+$/.test(host)) {
		return host;
	}
	if (host === RELYING_PARTY_NATIVE || host.endsWith('.' + RELYING_PARTY_NATIVE)) {
		return RELYING_PARTY_NATIVE;
	}
	// Preview deployments and self-hosted origins use their own hostname, which
	// means their passkeys are their own. The proxy extension is how you reach
	// getvela.app passkeys from elsewhere.
	return host;
}

export function passkeySupported(): boolean {
	return typeof window !== 'undefined' && !!window.PublicKeyCredential;
}

export function encodeUserHandle(name: string): string {
	return `${name}\0${crypto.randomUUID()}`;
}

/** `navigator.credentials.create()`. */
export async function register(
	name: string,
	excludeCredentialIds: string[]
): Promise<Registration> {
	assertSupported();
	try {
		const credential = (await navigator.credentials.create({
			publicKey: {
				rp: { id: relyingPartyId(), name: 'Vela Wallet' },
				user: {
					id: new TextEncoder().encode(encodeUserHandle(name)),
					name,
					displayName: name
				},
				challenge: crypto.getRandomValues(new Uint8Array(32)),
				// A multi-key wallet registers each founding key separately, and
				// the provider must refuse to silently REPLACE an earlier one —
				// the Safe address depends on every key in the set. Refusal
				// arrives as InvalidStateError and is named below.
				...(excludeCredentialIds.length > 0
					? {
							excludeCredentials: excludeCredentialIds.map((id) => ({
								type: 'public-key' as const,
								id: hexToBytes(id) as BufferSource
							}))
						}
					: {}),
				// ES256 (P-256) ONLY, deliberately without an RS256 fallback. The
				// on-chain verifier is the RIP-7212 P-256 precompile and
				// two-signature recovery is ECDSA math, so an RSA credential can
				// never become a working wallet: it would pass create() and then
				// die during key extraction — after minting an orphan passkey in
				// the person's provider. Restricting the list makes an RSA-only
				// authenticator fail up front with a standard NotSupportedError.
				pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
				authenticatorSelection: {
					residentKey: 'required',
					// WebAuthn L2 §5.4.4: set iff residentKey is 'required'. A
					// client that honours only the L1 boolean would otherwise
					// silently mint a NON-discoverable credential (issue #1).
					requireResidentKey: true,
					userVerification: 'required'
				},
				attestation: 'direct',
				// credProps.rk is the only client-side signal for whether the
				// credential actually came out discoverable.
				extensions: { credProps: true }
			}
		})) as PublicKeyCredential | null;

		if (!credential) throw new PasskeyError('other', 'No credential returned');

		// Sign-in and cross-device recovery both need a discoverable credential:
		// a non-discoverable one signs fine when pinned by id but never appears
		// in the picker and never syncs, so the wallet would die with this
		// device. Fail HERE, before anything is saved or funded. `rk` undefined
		// means the client cannot say — give it the benefit of the doubt.
		const credProps = credential.getClientExtensionResults?.().credProps;
		if (credProps?.rk === false) {
			throw new PasskeyError(
				'not_discoverable',
				'Authenticator created a non-discoverable credential'
			);
		}

		const response = credential.response as AuthenticatorAttestationResponse;
		const transports = typeof response.getTransports === 'function' ? response.getTransports() : [];
		return {
			credentialId: bytesToHex(credential.rawId),
			attestationObjectHex: bytesToHex(response.attestationObject),
			clientDataJSONHex: bytesToHex(response.clientDataJSON),
			authenticatorAttachment: credential.authenticatorAttachment ?? '',
			transports: transports.join(',')
		};
	} catch (error) {
		throw classify(error);
	}
}

/** `navigator.credentials.get()` with no credential hint — "who are you?". */
export async function authenticate(): Promise<Assertion> {
	assertSupported();
	try {
		const credential = (await navigator.credentials.get({
			publicKey: {
				challenge: crypto.getRandomValues(new Uint8Array(32)),
				rpId: relyingPartyId(),
				userVerification: 'required'
			}
		})) as PublicKeyCredential | null;
		if (!credential) throw new PasskeyError('other', 'No credential returned');
		return parseAssertion(credential);
	} catch (error) {
		throw classify(error);
	}
}

/**
 * `navigator.credentials.get()` against exactly one credential, over a
 * challenge the core chose the label for.
 *
 * A previous pending request is aborted first: browsers refuse a second
 * concurrent ceremony with "a request is already pending", which would surface
 * as a spurious failure on a flow the person did not cancel.
 */
let pendingSign: AbortController | null = null;

/**
 * The parallel space's seam (spec 026 D18): a substitute signer installed by
 * the dev harness behind the RUNTIME dev gate, so every ceremony — sign,
 * signWithAny — is covered by one substitution. Production paths never install
 * one; without an override every call below reaches `navigator.credentials`.
 */
export interface PasskeyOverride {
	sign(challengeHex: string, credentialIds: string[] | null): Promise<Assertion>;
}
let override: PasskeyOverride | null = null;
export function setPasskeyOverride(next: PasskeyOverride | null): void {
	override = next;
}
export function hasPasskeyOverride(): boolean {
	return override !== null;
}

/** Abort the pending ceremony, if any (the core's `cancel_passkey_sign`). */
export function cancelSign(): void {
	pendingSign?.abort();
	pendingSign = null;
}

/**
 * Sign with ANY of a wallet's founding credentials — the provider picks. A
 * multi-key wallet passes every key so the person is never told the one
 * credential they hold "was not found" (Expo `webSign` semantics).
 */
export async function signWithAny(
	challengeHex: string,
	credentials: { id: string; transports?: string }[]
): Promise<Assertion> {
	if (override)
		return override.sign(
			challengeHex,
			credentials.map((c) => c.id)
		);
	assertSupported();
	pendingSign?.abort();
	const controller = new AbortController();
	pendingSign = controller;
	try {
		const credential = (await navigator.credentials.get({
			publicKey: {
				challenge: hexToBytes(challengeHex) as BufferSource,
				rpId: relyingPartyId(),
				userVerification: 'required',
				...(credentials.length > 0
					? {
							allowCredentials: credentials.map((c) => {
								const hints = (c.transports ?? '')
									.split(',')
									.map((value) => value.trim())
									.filter(Boolean) as AuthenticatorTransport[];
								return {
									type: 'public-key' as const,
									id: hexToBytes(c.id) as BufferSource,
									...(hints.length > 0 ? { transports: hints } : {})
								};
							})
						}
					: {})
			},
			signal: controller.signal
		})) as PublicKeyCredential | null;
		if (!credential) throw new PasskeyError('other', 'No credential returned');
		return parseAssertion(credential);
	} catch (error) {
		throw classify(error);
	} finally {
		if (pendingSign === controller) pendingSign = null;
	}
}

export async function sign(
	challengeHex: string,
	credentialId: string,
	/**
	 * WHERE the credential lives, as its authenticator reported at registration
	 * (`hybrid,internal`, `usb,nfc`, …), or empty when unknown.
	 *
	 * **Load-bearing, not a hint.** An `allowCredentials` entry with no
	 * transports leaves the platform to guess where to look, and Android's
	 * Credential Manager guesses REMOVABLE SECURITY KEY: a passkey living in
	 * Apple Passwords on another phone drew "Connect your security key", a dead
	 * end for somebody holding a phone and no key (device-found 2026-08-26).
	 * Browsers route on the same field.
	 */
	transports = ''
): Promise<Assertion> {
	if (override) return override.sign(challengeHex, [credentialId]);
	assertSupported();
	pendingSign?.abort();
	const controller = new AbortController();
	pendingSign = controller;
	const hints = transports
		.split(',')
		.map((value) => value.trim())
		.filter(Boolean) as AuthenticatorTransport[];

	try {
		const credential = (await navigator.credentials.get({
			publicKey: {
				challenge: hexToBytes(challengeHex) as BufferSource,
				rpId: relyingPartyId(),
				userVerification: 'required',
				allowCredentials: [
					{
						type: 'public-key',
						id: hexToBytes(credentialId) as BufferSource,
						...(hints.length > 0 ? { transports: hints } : {})
					}
				]
			},
			signal: controller.signal
		})) as PublicKeyCredential | null;
		if (!credential) throw new PasskeyError('other', 'No credential returned');
		return parseAssertion(credential);
	} catch (error) {
		throw classify(error);
	} finally {
		if (pendingSign === controller) pendingSign = null;
	}
}

function parseAssertion(credential: PublicKeyCredential): Assertion {
	const response = credential.response as AuthenticatorAssertionResponse;
	const assertion: Assertion = {
		credentialId: bytesToHex(credential.rawId),
		signatureHex: bytesToHex(response.signature),
		authenticatorDataHex: bytesToHex(response.authenticatorData),
		clientDataJSONHex: bytesToHex(response.clientDataJSON),
		authenticatorAttachment: credential.authenticatorAttachment ?? ''
	};
	if (response.userHandle && response.userHandle.byteLength > 0) {
		assertion.userIdHex = bytesToHex(response.userHandle);
	}
	return assertion;
}

function assertSupported(): void {
	if (!passkeySupported()) {
		throw new PasskeyError('not_supported', 'WebAuthn is not supported in this browser.');
	}
}

/**
 * The platform's error, in the core's vocabulary. This is the ONE judgement
 * call a shell makes, and it is deliberately narrow — everything unrecognised
 * becomes `other` carrying the platform's own words, which the core forwards
 * verbatim into the bug report rather than prettifying.
 */
function classify(error: unknown): PasskeyError {
	if (error instanceof PasskeyError) return error;
	const e = error as DOMException;
	if (e?.name === 'NotAllowedError' || e?.name === 'AbortError') {
		return new PasskeyError('cancelled', 'User cancelled the operation');
	}
	if (e?.name === 'NotSupportedError') {
		return new PasskeyError('not_supported', e.message || 'Authenticator not supported');
	}
	if (e?.name === 'InvalidStateError') {
		// With excludeCredentials set this means the chosen authenticator
		// already holds one of this wallet's founding keys.
		return new PasskeyError('other', "This authenticator already holds one of this wallet's keys");
	}
	return new PasskeyError('other', e?.message ?? 'Unknown WebAuthn error');
}

export function bytesToHex(buffer: ArrayBuffer | Uint8Array): string {
	const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
	let hex = '';
	for (const byte of bytes) hex += byte.toString(16).padStart(2, '0');
	return hex;
}

export function hexToBytes(hex: string): Uint8Array {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	const bytes = new Uint8Array(clean.length / 2);
	for (let i = 0; i < bytes.length; i++) {
		bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
	}
	return bytes;
}
