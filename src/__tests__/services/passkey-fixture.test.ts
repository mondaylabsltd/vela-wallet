/**
 * Golden test for the parallel-space passkey fixtures.
 *
 * Locks the derived Safe addresses (so a change to the derivation is caught) and
 * proves every fixture assertion is a real, Safe-compatible WebAuthn signature that
 * verifies against its own public key — i.e. the exact bytes the on-chain verifier
 * accepts.
 */
import { p256 } from '@noble/curves/p256';
import { sha256 } from '@noble/hashes/sha256';
import {
  FIXTURE_ACCOUNTS,
  FIXTURE_ACCOUNT,
  FIXTURE_ADDRESSES,
  buildMockAssertion,
  buildMockRegistration,
  fixtureByCredentialId,
} from '@/services/dev/passkey-fixture';
import { computeAddress } from '@/services/safe-address';
import { verifySafeWebAuthn } from '@/services/webauthn-verify';
import { derSignatureToRaw, extractPublicKey } from '@/services/attestation-parser';
import { fromHex, toHex, concatBytes, stripHexPrefix } from '@/services/hex';

describe('parallel-space passkey fixtures', () => {
  it('reveals a stable set of fixture accounts (addresses to fund)', () => {
    // eslint-disable-next-line no-console
    console.log('\n=== PARALLEL-SPACE FIXTURE ACCOUNTS ===');
    for (const a of FIXTURE_ACCOUNTS) {
      // eslint-disable-next-line no-console
      console.log(`${a.name.padEnd(16)} id=${a.id}  safe=${a.address}`);
    }
    expect(FIXTURE_ACCOUNTS.length).toBeGreaterThanOrEqual(3);
  });

  it('derives each Safe address deterministically from its public key', () => {
    for (const a of FIXTURE_ACCOUNTS) {
      expect(computeAddress(a.publicKeyHex)).toBe(a.address);
      expect(a.publicKeyHex.startsWith('04')).toBe(true);
      expect(a.address).toMatch(/^0x[0-9a-fA-F]{40}$/);
    }
  });

  // Golden lock: these are the addresses to fund on-chain. A change to the keyset
  // or the derivation must be a conscious edit here (and a re-fund), never silent.
  it('matches the frozen golden Safe addresses', () => {
    expect(FIXTURE_ACCOUNTS.map(a => a.address)).toEqual([
      '0xD400866e00B055B20752a826CD5C89b811de130b',
      '0x031d7D57c99CAF891e1C250554691Fd12D84772b',
      '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d',
    ]);
  });

  it('gives every account a distinct address, key and credential id', () => {
    const addrs = new Set(FIXTURE_ADDRESSES.map(a => a.toLowerCase()));
    const ids = new Set(FIXTURE_ACCOUNTS.map(a => a.id));
    const keys = new Set(FIXTURE_ACCOUNTS.map(a => a.publicKeyHex));
    expect(addrs.size).toBe(FIXTURE_ACCOUNTS.length);
    expect(ids.size).toBe(FIXTURE_ACCOUNTS.length);
    expect(keys.size).toBe(FIXTURE_ACCOUNTS.length);
  });

  it('produces Safe-compatible assertions whose DER signature parses', () => {
    const challenge = '0x' + 'ab'.repeat(32);
    for (const a of FIXTURE_ACCOUNTS) {
      const assertion = buildMockAssertion(challenge, { credentialId: a.id });
      expect(assertion.credentialId).toBe(a.id);
      expect(verifySafeWebAuthn(assertion).ok).toBe(true);
      expect(derSignatureToRaw(fromHex(assertion.signatureHex))).not.toBeNull();
    }
  });

  it('signs the exact WebAuthn digest the on-chain verifier checks', () => {
    const challenge = '0x' + 'cd'.repeat(32);
    const a = FIXTURE_ACCOUNTS[1];
    const assertion = buildMockAssertion(challenge, { credentialId: a.id });

    // Recompute signBase = sha256(authData || sha256(clientDataJSON)) and verify the
    // signature against the account's own public key — same math as Safe's verifier.
    const authData = fromHex(assertion.authenticatorDataHex);
    const clientData = fromHex(assertion.clientDataJSONHex);
    const signBase = sha256(concatBytes(authData, sha256(clientData)));
    const ok = p256.verify(
      fromHex(assertion.signatureHex),
      signBase,
      fromHex(a.publicKeyHex),
    );
    expect(ok).toBe(true);

    // The challenge must be embedded (base64url) in clientDataJSON.
    const json = new TextDecoder().decode(clientData);
    expect(json.startsWith('{"type":"webauthn.get","challenge":"')).toBe(true);
  });

  it('round-trips the public key through the registration attestation', () => {
    const reg = buildMockRegistration({ credentialId: FIXTURE_ACCOUNT.id });
    const parsed = extractPublicKey(fromHex(reg.attestationObjectHex));
    expect(parsed).not.toBeNull();
    const recovered = '04' + toHex(parsed!.x) + toHex(parsed!.y);
    expect(recovered.toLowerCase()).toBe(stripHexPrefix(FIXTURE_ACCOUNT.publicKeyHex).toLowerCase());
  });

  it('resolves accounts by credential id, tolerating 0x and case', () => {
    expect(fixtureByCredentialId(FIXTURE_ACCOUNT.id)).toBe(FIXTURE_ACCOUNT);
    expect(fixtureByCredentialId('0x' + FIXTURE_ACCOUNT.id.toUpperCase())).toBe(FIXTURE_ACCOUNT);
    expect(fixtureByCredentialId('deadbeef')).toBeUndefined();
    expect(fixtureByCredentialId(null)).toBeUndefined();
  });
});
