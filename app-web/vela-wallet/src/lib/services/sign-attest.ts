/**
 * Displayed = signed, checked by a second implementation (spec 028 Phase 8).
 *
 * What a passkey signs is 32 bytes it cannot show. Between the confirm screen
 * (the core's view of the calls) and those bytes sits the shell's TypeScript
 * assembly — `safe-transaction.ts` — and until this module nothing checked
 * that the two agreed. Now, before any passkey prompt:
 *
 * 1. the core (`vela_core::user_op`, Rust) rebuilds the calldata from the
 *    calls the screen showed and refuses if the shell's bytes differ;
 * 2. the core computes the SafeOp / Safe-message hash itself and the shell's
 *    hash must match it to the byte;
 *
 * and after the prompt:
 *
 * 3. the assertion's `clientDataJSON` must carry exactly that hash as its
 *    challenge — the authenticator signed what was asked, not something a
 *    swapped override or a broken bridge substituted.
 *
 * Any disagreement throws {@link SignAttestError} and nothing is signed or
 * submitted. The check is fail-closed and free of I/O.
 */

import {
	attestSafeMessageHash,
	attestSafeOpHash,
	fromBase64Url,
	type AttestCalls,
	type AttestOp
} from '$lib/core/kernels';

export class SignAttestError extends Error {
	override readonly name = 'SignAttestError';
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
	if (a.length !== b.length) return false;
	let diff = 0;
	for (let i = 0; i < a.length; i += 1) diff |= a[i]! ^ b[i]!;
	return diff === 0;
}

function reason(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/**
 * The SafeOp hash both implementations agree on, or a refusal. `shellHash` is
 * what `calculateSafeOpHash` produced; `calls` is what the confirm showed
 * (`null` where the caller only has finished calldata).
 */
export function attestedSafeOpHash(
	op: AttestOp,
	chainId: number,
	calls: AttestCalls | null,
	shellHash: Uint8Array
): Uint8Array {
	let coreHash: Uint8Array;
	try {
		coreHash = attestSafeOpHash(op, calls, chainId);
	} catch (error) {
		throw new SignAttestError(`The core refused to attest this operation: ${reason(error)}`);
	}
	if (!sameBytes(coreHash, shellHash)) {
		throw new SignAttestError(
			'The shell and the core disagree on the operation to sign; nothing was signed.'
		);
	}
	return coreHash;
}

/** The EIP-1271 message hash both implementations agree on, or a refusal. */
export function attestedSafeMessageHash(
	originalHash: Uint8Array,
	chainId: number,
	safeAddress: string,
	shellHash: Uint8Array
): Uint8Array {
	let coreHash: Uint8Array;
	try {
		coreHash = attestSafeMessageHash(originalHash, chainId, safeAddress);
	} catch (error) {
		throw new SignAttestError(`The core refused to attest this message: ${reason(error)}`);
	}
	if (!sameBytes(coreHash, shellHash)) {
		throw new SignAttestError(
			'The shell and the core disagree on the message to sign; nothing was signed.'
		);
	}
	return coreHash;
}

/**
 * The authenticator signed `expected` and nothing else: `clientDataJSON` is a
 * `webauthn.get` whose challenge decodes to those bytes.
 */
export function assertChallengeSigned(clientDataJSON: Uint8Array, expected: Uint8Array): void {
	let parsed: { type?: unknown; challenge?: unknown };
	try {
		parsed = JSON.parse(new TextDecoder().decode(clientDataJSON)) as typeof parsed;
	} catch {
		throw new SignAttestError('The assertion carries no readable clientDataJSON; it is discarded.');
	}
	if (parsed.type !== 'webauthn.get') {
		throw new SignAttestError('The assertion is not a WebAuthn get; it is discarded.');
	}
	let challenge: Uint8Array;
	try {
		challenge =
			typeof parsed.challenge === 'string' ? fromBase64Url(parsed.challenge) : new Uint8Array(0);
	} catch {
		challenge = new Uint8Array(0);
	}
	if (!sameBytes(challenge, expected)) {
		throw new SignAttestError(
			'The passkey signed a different challenge than the one shown; the assertion is discarded.'
		);
	}
}
