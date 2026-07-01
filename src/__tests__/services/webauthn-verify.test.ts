/**
 * Tests for webauthn-verify — local Safe-contract compatibility checks on a
 * passkey assertion (US 1.4). Pure logic, mirrors the on-chain WebAuthn
 * verifier: clientDataJSON prefix, trailing `}`, authenticatorData length,
 * and the User Verification (UV) flag.
 *
 * This is a P0 security gap: the guard runs BEFORE a signature is trusted, so
 * an incompatible provider (e.g. field-reordered clientDataJSON) must be
 * rejected with a human-readable reason rather than silently passing through.
 */
import { verifySafeWebAuthn } from '@/services/webauthn-verify';
import { toHex } from '@/services/hex';
import type { PasskeyAssertionResult } from '@/modules/passkey';

const REQUIRED_PREFIX = '{"type":"webauthn.get","challenge":"';

/** UTF-8 encode a string to a plain hex payload (no 0x). */
function hexOf(s: string): string {
  return toHex(new TextEncoder().encode(s));
}

/**
 * Build an authenticatorData hex blob: 32-byte rpIdHash + 1 flags byte +
 * 4-byte counter = 37 bytes by default. `len` lets us make it too short.
 */
function authDataHex(flags: number, len = 37): string {
  const arr = new Uint8Array(len);
  if (len > 32) arr[32] = flags;
  return toHex(arr);
}

/** Minimal assertion — verifySafeWebAuthn only reads the two hex fields. */
function assertion(clientDataJSON: string, authHex: string): PasskeyAssertionResult {
  return {
    signatureHex: '00',
    authenticatorDataHex: authHex,
    clientDataJSONHex: hexOf(clientDataJSON),
  } as PasskeyAssertionResult;
}

const UV = 0x04; // User Verification flag bit
const UP = 0x01; // User Presence flag bit
const validClientData = `${REQUIRED_PREFIX}aGVsbG8","origin":"https://vela.app"}`;

describe('verifySafeWebAuthn', () => {
  describe('accepts compatible assertions', () => {
    test('valid clientDataJSON + UV flag set → ok', () => {
      const res = verifySafeWebAuthn(assertion(validClientData, authDataHex(UV)));
      expect(res.ok).toBe(true);
      expect(res.reason).toBeUndefined();
    });

    test('UP + UV both set → ok (real authenticators set both)', () => {
      const res = verifySafeWebAuthn(assertion(validClientData, authDataHex(UP | UV)));
      expect(res.ok).toBe(true);
    });
  });

  describe('rejects clientDataJSON field-order incompatibility', () => {
    test('challenge-before-type ordering → rejected with reason', () => {
      // Xiaomi/other password managers emit fields in a different order.
      const reordered = '{"challenge":"aGVsbG8","type":"webauthn.get","origin":"x"}';
      const res = verifySafeWebAuthn(assertion(reordered, authDataHex(UV)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('field order incompatible');
    });

    test('wrong type (webauthn.create) → rejected', () => {
      const created = '{"type":"webauthn.create","challenge":"aGVsbG8"}';
      const res = verifySafeWebAuthn(assertion(created, authDataHex(UV)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('field order incompatible');
    });

    test('leading whitespace breaks the exact prefix → rejected', () => {
      const res = verifySafeWebAuthn(assertion(` ${validClientData}`, authDataHex(UV)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('field order incompatible');
    });
  });

  describe('rejects malformed clientDataJSON', () => {
    test('does not end with } → rejected', () => {
      // Correct prefix but truncated (no closing brace).
      const truncated = `${REQUIRED_PREFIX}aGVsbG8"`;
      const res = verifySafeWebAuthn(assertion(truncated, authDataHex(UV)));
      expect(res.ok).toBe(false);
      expect(res.reason).toBe('clientDataJSON does not end with }');
    });
  });

  describe('rejects authenticatorData problems', () => {
    test('authenticatorData shorter than 33 bytes → rejected', () => {
      const res = verifySafeWebAuthn(assertion(validClientData, authDataHex(UV, 20)));
      expect(res.ok).toBe(false);
      expect(res.reason).toBe('authenticatorData too short');
    });

    test('UV flag not set (UP only) → rejected with flags in reason', () => {
      const res = verifySafeWebAuthn(assertion(validClientData, authDataHex(UP)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('User Verification flag not set');
      expect(res.reason).toContain('0x1');
    });

    test('no flags set (0x00) → rejected', () => {
      const res = verifySafeWebAuthn(assertion(validClientData, authDataHex(0x00)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('User Verification flag not set');
    });
  });

  describe('check ordering', () => {
    test('clientDataJSON prefix is checked before UV flag', () => {
      // Both are wrong; the prefix reason must win (checked first).
      const reordered = '{"challenge":"x","type":"webauthn.get"}';
      const res = verifySafeWebAuthn(assertion(reordered, authDataHex(0x00)));
      expect(res.ok).toBe(false);
      expect(res.reason).toContain('field order incompatible');
    });
  });
});
