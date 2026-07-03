/**
 * Tests for P-256 public key recovery from WebAuthn assertion signatures.
 *
 * Property-tested against real keys: Node's WebCrypto generates genuine P-256
 * keypairs and ECDSA signatures, and recovery must reproduce the exported
 * public key exactly.
 */

import { createHash, webcrypto } from 'node:crypto';
import { sha256 } from '@/services/sha256';
import {
  recoverPublicKeyFromAssertions,
  type RecoverableAssertion,
} from '@/services/p256-recovery';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const toHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

/** Wrap a P1363 raw (r||s) signature into DER, as WebAuthn assertions use. */
function rawSigToDer(raw: Uint8Array): Uint8Array {
  const encodeInt = (bytes: Uint8Array): number[] => {
    let i = 0;
    while (i < bytes.length - 1 && bytes[i] === 0) i++; // strip leading zeros
    let body = Array.from(bytes.slice(i));
    if (body[0] & 0x80) body = [0, ...body]; // keep it a positive DER integer
    return [0x02, body.length, ...body];
  };
  const r = encodeInt(raw.slice(0, 32));
  const s = encodeInt(raw.slice(32, 64));
  return new Uint8Array([0x30, r.length + s.length, ...r, ...s]);
}

interface TestCredential {
  publicKeyHex: string; // uncompressed 04||x||y
  sign: (label: string) => Promise<RecoverableAssertion>;
}

/** Create a real P-256 credential whose sign() produces WebAuthn-shaped assertions. */
async function makeCredential(seed: string): Promise<TestCredential> {
  const keyPair = await webcrypto.subtle.generateKey(
    { name: 'ECDSA', namedCurve: 'P-256' },
    true,
    ['sign', 'verify'],
  );
  const rawKey = new Uint8Array(await webcrypto.subtle.exportKey('raw', keyPair.publicKey));

  return {
    publicKeyHex: toHex(rawKey),
    async sign(label: string): Promise<RecoverableAssertion> {
      // WebAuthn signs authenticatorData || sha256(clientDataJSON)
      const authData = new TextEncoder().encode(`authdata-${seed}-${label}-`.padEnd(37, 'x'));
      const clientDataJSON = new TextEncoder().encode(
        JSON.stringify({ type: 'webauthn.get', challenge: `${seed}-${label}` }),
      );
      const clientDataHash = new Uint8Array(createHash('sha256').update(clientDataJSON).digest());
      const message = new Uint8Array([...authData, ...clientDataHash]);
      const rawSig = new Uint8Array(
        await webcrypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, keyPair.privateKey, message),
      );
      return {
        signatureHex: toHex(rawSigToDer(rawSig)),
        authenticatorDataHex: toHex(authData),
        clientDataJSONHex: toHex(clientDataJSON),
      };
    },
  };
}

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

describe('sha256', () => {
  test('matches FIPS 180-4 vectors', () => {
    expect(toHex(sha256(new Uint8Array(0)))).toBe(
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    );
    expect(toHex(sha256(new TextEncoder().encode('abc')))).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });

  test('matches Node crypto across sizes (incl. padding boundaries)', () => {
    for (const size of [1, 31, 55, 56, 63, 64, 65, 127, 128, 1000]) {
      const data = new Uint8Array(size).map((_, i) => (i * 7 + size) & 0xff);
      expect(toHex(sha256(data))).toBe(createHash('sha256').update(data).digest('hex'));
    }
  });
});

// ---------------------------------------------------------------------------
// Public key recovery
// ---------------------------------------------------------------------------

describe('recoverPublicKeyFromAssertions', () => {
  test('recovers the exact public key from two assertions (5 independent keys)', async () => {
    for (let i = 0; i < 5; i++) {
      const credential = await makeCredential(`key${i}`);
      const first = await credential.sign('first');
      const second = await credential.sign('second');
      expect(recoverPublicKeyFromAssertions(first, second)).toBe(credential.publicKeyHex);
    }
  });

  test('recovery is order-independent', async () => {
    const credential = await makeCredential('order');
    const first = await credential.sign('a');
    const second = await credential.sign('b');
    expect(recoverPublicKeyFromAssertions(second, first)).toBe(credential.publicKeyHex);
  });

  test('returns null for assertions from different credentials', async () => {
    const alice = await makeCredential('alice');
    const bob = await makeCredential('bob');
    expect(
      recoverPublicKeyFromAssertions(await alice.sign('x'), await bob.sign('y')),
    ).toBeNull();
  });

  test('returns null when the same assertion is passed twice (one signature is never enough)', async () => {
    const credential = await makeCredential('replay');
    const assertion = await credential.sign('only');
    expect(recoverPublicKeyFromAssertions(assertion, assertion)).toBeNull();
  });

  test('returns null on malformed DER signatures', async () => {
    const credential = await makeCredential('garbage');
    const good = await credential.sign('good');
    const bad = { ...(await credential.sign('bad')), signatureHex: 'deadbeef' };
    expect(recoverPublicKeyFromAssertions(good, bad)).toBeNull();
  });

  test('returns null when a signature was tampered with', async () => {
    const credential = await makeCredential('tamper');
    const first = await credential.sign('a');
    const second = await credential.sign('b');
    // Flip one byte inside the DER body of the second signature
    const bytes = first.signatureHex.split('');
    bytes[12] = bytes[12] === '0' ? '1' : '0';
    const tampered = { ...second, signatureHex: bytes.join('') };
    expect(recoverPublicKeyFromAssertions(first, tampered)).toBeNull();
  });
});
