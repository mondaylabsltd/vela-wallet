/**
 * Local WebAuthn compatibility verification — mirrors Safe contract checks.
 *
 * Verifies the passkey provider's assertion response is compatible with
 * Safe's on-chain WebAuthn verifier. The contract rejects responses where:
 *  1. clientDataJSON doesn't start with {"type":"webauthn.get","challenge":"
 *  2. clientDataJSON doesn't end with }
 *  3. authenticatorData UV flag is not set
 *
 * P256 signature math is NOT checked here — if the device's secure enclave
 * produced the signature, it is mathematically correct. The real compatibility
 * issue is the clientDataJSON field order (e.g. Xiaomi Password Manager).
 */

import { fromHex } from './hex';
import type { PasskeyAssertionResult } from '@/modules/passkey';

const REQUIRED_PREFIX = '{"type":"webauthn.get","challenge":"';

export interface VerifyResult {
  ok: boolean;
  /** Human-readable reason on failure */
  reason?: string;
}

/**
 * Verify a WebAuthn assertion is compatible with Safe contracts.
 */
export function verifySafeWebAuthn(
  assertion: PasskeyAssertionResult,
): VerifyResult {
  const clientDataBytes = fromHex(assertion.clientDataJSONHex);
  const clientDataJSON = new TextDecoder().decode(clientDataBytes);
  const authData = fromHex(assertion.authenticatorDataHex);

  // 1. clientDataJSON must start with {"type":"webauthn.get","challenge":"
  if (!clientDataJSON.startsWith(REQUIRED_PREFIX)) {
    const actual = clientDataJSON.slice(0, 60);
    return {
      ok: false,
      reason: `clientDataJSON field order incompatible.\nExpected: ${REQUIRED_PREFIX}...\nGot: ${actual}...`,
    };
  }

  // 2. clientDataJSON must end with }
  if (!clientDataJSON.endsWith('}')) {
    return { ok: false, reason: 'clientDataJSON does not end with }' };
  }

  // 3. authenticatorData must be at least 33 bytes
  if (authData.length < 33) {
    return { ok: false, reason: 'authenticatorData too short' };
  }

  // 4. UV flag must be set (bit 2 of flags byte at index 32)
  const flags = authData[32];
  if ((flags & 0x04) !== 0x04) {
    return {
      ok: false,
      reason: `User Verification flag not set (flags=0x${flags.toString(16)})`,
    };
  }

  return { ok: true };
}
