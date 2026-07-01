/**
 * Fixed passkey fixtures for the "parallel space" test environment.
 *
 * This is the ONE thing that differs between the real app and the parallel-space
 * test app: instead of real device passkeys (Secure Enclave / Credential Manager /
 * navigator.credentials), signing uses THIS fixed set of P-256 keypairs. Everything
 * downstream is real and deterministic — the derived Safe addresses, and the on-chain
 * WebAuthn assertions that Safe's verifier accepts.
 *
 * A *set* of accounts (not just one) so the parallel space can exercise the whole
 * app: account switching, sending between two known accounts, split/sweep to known
 * recipients, and dApp flows where both parties are deterministic. Each account's
 * credential id is stable, so `Passkey.sign(challenge, credentialId)` picks the right
 * key exactly like the real module resolves a device credential.
 *
 * ⚠️  These private keys are throwaway TEST keys, committed on purpose. They never
 * guard real user funds: they are only wired in when the parallel space is explicitly
 * entered in a dev build (see `parallel-space.ts`, gated on `__DEV__`). The
 * `__setSignOverride` seam that installs them is a compile-time no-op in prod.
 *
 * Boundary: real space = real device passkeys. Parallel space = these fixtures.
 * Nothing else about the two environments differs.
 */
import { p256 } from '@noble/curves/p256';
import { sha256 } from '@noble/hashes/sha256';
import { computeAddress } from '@/services/safe-address';
import { fromHex, toHex, stripHexPrefix, concatBytes } from '@/services/hex';
import type { PasskeyAssertionResult, PasskeyRegistrationResult } from '@/modules/passkey';

// ---------------------------------------------------------------------------
// The fixed keyset (test-only)
// ---------------------------------------------------------------------------

export const FIXTURE_RP_ID = 'getvela.app';

/** ascii "vela-fixture-0N" → hex, used as the stable credential id / account id. */
function credId(n: number): string {
  return toHex(new TextEncoder().encode(`vela-fixture-0${n}`));
}

/**
 * Seed rows: fixed 32-byte P-256 private keys + display names. Each is a valid
 * scalar (< curve order). Add rows here to grow the keyset — nothing else changes.
 */
const SEED: { name: string; privHex: string }[] = [
  { name: 'Parallel One',   privHex: 'd80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5' },
  { name: 'Parallel Two',   privHex: '6e1ebe95f2f14d70b193aedbfe87c3d495943c19fb04a81c163cf92ae384c59f' },
  { name: 'Parallel Three', privHex: 'e66f17e63e4b6e1a6c8a31086d86bcb3172816bec70a5221576c1e2a2ae1f336' },
];

export interface FixtureAccount {
  /** Stable local credential id — also used as the wallet `account.id`. */
  id: string;
  name: string;
  privHex: string;
  /** Uncompressed P-256 public key `04 || x(32) || y(32)`. */
  publicKeyHex: string;
  /** Deterministic Safe address derived from the public key. */
  address: string;
}

/** The fixture accounts, derived once from {@link SEED}. */
export const FIXTURE_ACCOUNTS: FixtureAccount[] = SEED.map((row, i) => {
  const publicKeyHex = toHex(p256.getPublicKey(fromHex(row.privHex), false));
  return {
    id: credId(i + 1),
    name: row.name,
    privHex: row.privHex,
    publicKeyHex,
    address: computeAddress(publicKeyHex),
  };
});

/** The primary fixture account (index 0) — the default signer / active account. */
export const FIXTURE_ACCOUNT = FIXTURE_ACCOUNTS[0];

/** All fixture Safe addresses — the addresses to fund for opt-in on-chain tests. */
export const FIXTURE_ADDRESSES = FIXTURE_ACCOUNTS.map(a => a.address);

/** Look up a fixture account by its credential id (case-insensitive, 0x-tolerant). */
export function fixtureByCredentialId(id: string | null | undefined): FixtureAccount | undefined {
  if (!id) return undefined;
  const key = stripHexPrefix(id).toLowerCase();
  return FIXTURE_ACCOUNTS.find(a => a.id.toLowerCase() === key);
}

// ---------------------------------------------------------------------------
// Assertion builder (real WebAuthn signature over a fixture key)
// ---------------------------------------------------------------------------

/** base64url (no padding) — matches what browsers put in clientDataJSON.challenge. */
function base64url(bytes: Uint8Array): string {
  const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
  let out = '';
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out += CHARS[(n >> 18) & 63] + CHARS[(n >> 12) & 63] + CHARS[(n >> 6) & 63] + CHARS[n & 63];
  }
  const rem = bytes.length - i;
  if (rem === 1) {
    const n = bytes[i] << 16;
    out += CHARS[(n >> 18) & 63] + CHARS[(n >> 12) & 63];
  } else if (rem === 2) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8);
    out += CHARS[(n >> 18) & 63] + CHARS[(n >> 12) & 63] + CHARS[(n >> 6) & 63];
  }
  return out;
}

export interface MockAssertionOptions {
  /** Sign with a specific credential id (defaults to the primary fixture account). */
  credentialId?: string | null;
  /** Relying-party id whose hash goes into authenticatorData. */
  rpId?: string;
  /** Origin embedded in clientDataJSON. */
  origin?: string;
}

/**
 * Produce a genuine WebAuthn assertion over `challengeHex`, signed with the fixture
 * key selected by `opts.credentialId` (or the primary account). The output is
 * byte-for-byte what a real authenticator would emit for this key + challenge, so:
 *   - `verifySafeWebAuthn()` accepts it (field order, UV flag, authData length),
 *   - `derSignatureToRaw()` parses the DER signature, and
 *   - Safe's on-chain P-256 verifier validates it against that account's public key.
 *
 * signature = ECDSA_P256( sha256( authenticatorData || sha256(clientDataJSON) ) )
 */
export function buildMockAssertion(
  challengeHex: string,
  opts: MockAssertionOptions = {},
): PasskeyAssertionResult {
  const account = fixtureByCredentialId(opts.credentialId ?? undefined) ?? FIXTURE_ACCOUNT;
  const rpId = opts.rpId ?? FIXTURE_RP_ID;
  const origin = opts.origin ?? `https://${rpId}`;

  const challenge = fromHex(stripHexPrefix(challengeHex));

  const clientDataJSON =
    `{"type":"webauthn.get","challenge":"${base64url(challenge)}",` +
    `"origin":"${origin}","crossOrigin":false}`;
  const clientDataBytes = new TextEncoder().encode(clientDataJSON);

  // authenticatorData = rpIdHash(32) || flags(1: UP|UV) || signCount(4)
  const rpIdHash = sha256(new TextEncoder().encode(rpId));
  const authenticatorData = concatBytes(
    rpIdHash,
    new Uint8Array([0x05]), // 0x01 user-present | 0x04 user-verified
    new Uint8Array([0, 0, 0, 0]),
  );

  const clientDataHash = sha256(clientDataBytes);
  const signBase = sha256(concatBytes(authenticatorData, clientDataHash));

  // noble returns a low-s canonical signature by default (what the verifier wants).
  const sig = p256.sign(signBase, fromHex(account.privHex));

  return {
    credentialId: account.id,
    signatureHex: toHex(sig.toDERRawBytes()),
    authenticatorDataHex: toHex(authenticatorData),
    clientDataJSONHex: toHex(clientDataBytes),
  };
}

/**
 * Produce a fixture registration result whose attestation object embeds the account's
 * P-256 public key in COSE form, so `extractPublicKey()` recovers it. Used when the
 * parallel space runs the real onboarding flow instead of seeding a wallet directly.
 */
export function buildMockRegistration(opts: { credentialId?: string; rpId?: string; origin?: string } = {}): PasskeyRegistrationResult {
  const account = fixtureByCredentialId(opts.credentialId) ?? FIXTURE_ACCOUNT;
  const rpId = opts.rpId ?? FIXTURE_RP_ID;
  const origin = opts.origin ?? `https://${rpId}`;

  const clientDataJSON =
    `{"type":"webauthn.create","challenge":"${base64url(new Uint8Array(32))}",` +
    `"origin":"${origin}","crossOrigin":false}`;

  const attestationObject = buildFixtureAttestationObject(rpId, fromHex(account.id), account.publicKeyHex);

  return {
    credentialId: account.id,
    attestationObjectHex: toHex(attestationObject),
    clientDataJSONHex: toHex(new TextEncoder().encode(clientDataJSON)),
  };
}

/**
 * Minimal CBOR attestation object with fmt="none", carrying authData whose attested
 * credential data holds the account's P-256 public key as a COSE_Key. Matches the
 * shape `extractPublicKey()` (attestation-parser.ts) walks.
 */
function buildFixtureAttestationObject(rpId: string, credIdBytes: Uint8Array, pubHex: string): Uint8Array {
  const { x, y } = splitPubKey(pubHex);

  // COSE_Key: {1:2, 3:-7, -1:1, -2:x, -3:y}
  const cose = concatBytes(
    new Uint8Array([0xa5]),                     // map(5)
    new Uint8Array([0x01, 0x02]),               // 1: 2 (kty EC2)
    new Uint8Array([0x03, 0x26]),               // 3: -7 (alg ES256)
    new Uint8Array([0x20, 0x01]),               // -1: 1 (crv P-256)
    new Uint8Array([0x21, 0x58, 0x20]), x,      // -2: bytes(32) x
    new Uint8Array([0x22, 0x58, 0x20]), y,      // -3: bytes(32) y
  );

  const rpIdHash = sha256(new TextEncoder().encode(rpId));
  const flags = new Uint8Array([0x45]); // UP|UV|AT (0x01|0x04|0x40)
  const signCount = new Uint8Array([0, 0, 0, 0]);
  const aaguid = new Uint8Array(16);
  const credLen = new Uint8Array([(credIdBytes.length >> 8) & 0xff, credIdBytes.length & 0xff]);
  const authData = concatBytes(rpIdHash, flags, signCount, aaguid, credLen, credIdBytes, cose);

  // CBOR: {"fmt":"none","attStmt":{},"authData":<bstr>}
  const fmt = concatBytes(textStr('fmt'), textStr('none'));
  const attStmt = concatBytes(textStr('attStmt'), new Uint8Array([0xa0])); // map(0)
  const authKey = concatBytes(textStr('authData'), bstr(authData));
  return concatBytes(new Uint8Array([0xa3]), fmt, attStmt, authKey);
}

// tiny CBOR helpers (definite-length; sizes stay small enough for our fixtures)
function textStr(s: string): Uint8Array {
  const b = new TextEncoder().encode(s);
  return concatBytes(new Uint8Array([0x60 | b.length]), b);
}
function bstr(bytes: Uint8Array): Uint8Array {
  if (bytes.length < 24) return concatBytes(new Uint8Array([0x40 | bytes.length]), bytes);
  if (bytes.length < 256) return concatBytes(new Uint8Array([0x58, bytes.length]), bytes);
  return concatBytes(new Uint8Array([0x59, (bytes.length >> 8) & 0xff, bytes.length & 0xff]), bytes);
}
function splitPubKey(pubHex: string): { x: Uint8Array; y: Uint8Array } {
  const clean = stripHexPrefix(pubHex).replace(/^04/, '');
  return { x: fromHex(clean.slice(0, 64)), y: fromHex(clean.slice(64, 128)) };
}
