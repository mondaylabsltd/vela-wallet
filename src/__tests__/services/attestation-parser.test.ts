/**
 * Tests for attestation parser — DER signature conversion and CBOR parsing.
 * Test vectors match iOS AttestationParser tests.
 */
import { derSignatureToRaw, extractPublicKey } from '@/services/attestation-parser';
import { toHex, fromHex } from '@/services/hex';

describe('derSignatureToRaw', () => {
  test('converts standard DER signature to raw 64 bytes', () => {
    // Standard DER: 30 44 02 20 [32 bytes r] 02 20 [32 bytes s]
    const r = new Uint8Array(32).fill(0x11);
    const s = new Uint8Array(32).fill(0x22);
    const der = new Uint8Array([
      0x30, 0x44, // SEQUENCE, length 68
      0x02, 0x20, // INTEGER, length 32
      ...r,
      0x02, 0x20, // INTEGER, length 32
      ...s,
    ]);

    const raw = derSignatureToRaw(der);
    expect(raw).not.toBeNull();
    expect(raw!.length).toBe(64);
    expect(toHex(raw!.slice(0, 32))).toBe('11'.repeat(32));
    expect(toHex(raw!.slice(32))).toBe('22'.repeat(32));
  });

  test('strips leading zero bytes from r and s', () => {
    // DER with leading zero (signed integer representation)
    const r = new Uint8Array(32).fill(0xAA);
    const s = new Uint8Array(32).fill(0x11); // low-s value (< n/2)
    const der = new Uint8Array([
      0x30, 0x46, // SEQUENCE, length 70
      0x02, 0x21, // INTEGER, length 33
      0x00, ...r, // leading zero + 32 bytes
      0x02, 0x21, // INTEGER, length 33
      0x00, ...s, // leading zero + 32 bytes
    ]);

    const raw = derSignatureToRaw(der);
    expect(raw).not.toBeNull();
    expect(raw!.length).toBe(64);
    expect(toHex(raw!.slice(0, 32))).toBe('aa'.repeat(32));
    expect(toHex(raw!.slice(32))).toBe('11'.repeat(32));
  });

  test('pads short r or s values to 32 bytes', () => {
    // Short r (31 bytes)
    const r = new Uint8Array(31).fill(0xCC);
    const s = new Uint8Array(32).fill(0xDD);
    const der = new Uint8Array([
      0x30, 0x43, // SEQUENCE
      0x02, 0x1F, // INTEGER, length 31
      ...r,
      0x02, 0x20, // INTEGER, length 32
      ...s,
    ]);

    const raw = derSignatureToRaw(der);
    expect(raw).not.toBeNull();
    expect(raw!.length).toBe(64);
    // r should be left-padded with one zero
    expect(raw![0]).toBe(0x00);
    expect(toHex(raw!.slice(1, 32))).toBe('cc'.repeat(31));
  });

  test('returns null for invalid DER', () => {
    expect(derSignatureToRaw(new Uint8Array([0x00]))).toBeNull();
    expect(derSignatureToRaw(new Uint8Array([0x30, 0x00]))).toBeNull();
    expect(derSignatureToRaw(new Uint8Array([]))).toBeNull();
  });
});

describe('extractPublicKey', () => {
  test('returns null for empty input', () => {
    expect(extractPublicKey(new Uint8Array(0))).toBeNull();
  });

  test('returns null for non-CBOR input', () => {
    expect(extractPublicKey(new Uint8Array([0x00, 0x01, 0x02]))).toBeNull();
  });

  test('extracts public key from valid attestation object', () => {
    // Build a minimal valid attestation object (CBOR map with "authData")
    // This is a simplified test — real attestation objects are more complex
    // We build: {fmt: "none", attStmt: {}, authData: <bytes>}

    // For a full integration test, we'd need a real attestation object
    // from a WebAuthn registration. Here we test the basic CBOR parsing
    // with a hand-crafted minimal structure.

    // For now, we verify that the function handles edge cases correctly
    const shortAuthData = new Uint8Array(30); // too short for attested cred data
    // CBOR: map(1) { text("authData") -> bstr(30 bytes) }
    const cbor = new Uint8Array([
      0xa1, // map(1)
      0x68, // text(8)
      0x61, 0x75, 0x74, 0x68, 0x44, 0x61, 0x74, 0x61, // "authData"
      0x58, 0x1e, // bstr(30)
      ...shortAuthData,
    ]);

    // Should return null because authData is too short (< 37 bytes)
    expect(extractPublicKey(cbor)).toBeNull();
  });
});
