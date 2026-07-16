import {
  decodeAbiParameters,
  hexToBytes,
  keccak256,
  numberToHex,
  type Hex,
} from 'viem';
import { describe, expect, it } from 'vitest';
import { SHARED_WEBAUTHN_OWNER } from './constants';
import { replaceSharedPrevalidatedSignature } from './relayer';
import {
  buildSafeContractSignature,
  extractClientDataFields,
  parseDerP256Signature,
  safeContractSignaturePayload,
} from './signatures';
import type { WebAuthnAssertion } from './types';

const challenge = keccak256('0x1234');
const challengeBytes = hexToBytes(challenge);

function base64Url(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function assertion(): WebAuthnAssertion {
  const clientData = `{"type":"webauthn.get","challenge":"${base64Url(challengeBytes)}","origin":"chrome-extension://test"}`;
  // DER: r=1, s=2. Both are in the P-256 scalar range.
  return {
    credentialId: '0xcafe',
    authenticatorDataHex: `0x${'11'.repeat(37)}` as Hex,
    clientDataJSONHex: `0x${Array.from(new TextEncoder().encode(clientData), (byte) => byte.toString(16).padStart(2, '0')).join('')}` as Hex,
    signatureHex: '0x3006020101020102',
  };
}

describe('Safe WebAuthn contract signatures', () => {
  it('parses DER and normalizes high-S signatures', () => {
    const parsed = parseDerP256Signature('0x3006020101020102');
    expect(parsed.r).toBe(1n);
    expect(parsed.s).toBe(2n);
  });

  it('uses the four-field ABI expected by the Safe WebAuthn signer', () => {
    const input = assertion();
    const signature = buildSafeContractSignature(input);
    const hex = signature.slice(2);

    expect(hex.slice(24, 64)).toBe(SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase());
    expect(BigInt(`0x${hex.slice(64, 128)}`)).toBe(65n);
    expect(hex.slice(128, 130)).toBe('00');

    const length = Number(BigInt(`0x${hex.slice(130, 194)}`));
    expect(length % 32).toBe(0); // Exact standard ABI size required by WebAuthn.castSignature.
    const dynamicEnd = 194 + length * 2;
    const decoded = decodeAbiParameters(
      [
        { type: 'bytes' },
        { type: 'bytes' },
        { type: 'uint256' },
        { type: 'uint256' },
      ],
      `0x${hex.slice(194, dynamicEnd)}` as Hex,
    );
    expect(decoded[0]).toBe(input.authenticatorDataHex);
    expect(decoded[1]).toBe(extractClientDataFields(input.clientDataJSONHex));
    expect(decoded[2]).toBe(1n);
    expect(decoded[3]).toBe(2n);
    expect(safeContractSignaturePayload(signature)).toBe(`0x${hex.slice(194, dynamicEnd)}`);
    expect(hex.length).toBe(dynamicEnd);
    expect(hex.slice(-2)).toBe('00'); // Protocol Kit may rewrite this padding byte to v=27.
    expect(numberToHex(length, { size: 32 })).toBe(`0x${hex.slice(130, 194)}`);

    // Safe's official WebAuthn.castSignature accepts at most the standard ABI
    // length: 192 fixed bytes plus the two aligned dynamic field lengths.
    const authLength = hexToBytes(input.authenticatorDataHex).length;
    const clientLength = hexToBytes(extractClientDataFields(input.clientDataJSONHex)).length;
    const aligned = (value: number) => Math.ceil(value / 32) * 32;
    expect(length).toBe(192 + aligned(authLength) + aligned(clientLength));
    expect(length).toBeLessThan(193 + aligned(authLength) + aligned(clientLength));

    // Protocol Kit's v-normalization changes only trailing ABI padding. The
    // signer fields still decode identically and the payload length stays valid.
    const protocolKitAdjusted = `${signature.slice(0, -2)}1b` as Hex;
    const adjustedDynamic = `0x${protocolKitAdjusted.slice(196)}` as Hex;
    const adjustedDecoded = decodeAbiParameters(
      [
        { type: 'bytes' },
        { type: 'bytes' },
        { type: 'uint256' },
        { type: 'uint256' },
      ],
      adjustedDynamic,
    );
    expect(adjustedDecoded).toEqual(decoded);
  });

  it('replaces a shared-owner prevalidated slot without corrupting other signatures', () => {
    const eoaSlot = `${'aa'.repeat(64)}1b`;
    const sharedSlot = `${SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0')}${'00'.repeat(32)}01`;
    const original = `0x${eoaSlot}${sharedSlot}` as Hex;
    const contractSignature = buildSafeContractSignature(assertion(), 130);
    const merged = replaceSharedPrevalidatedSignature(original, 2, contractSignature);
    const bytes = hexToBytes(merged);

    expect(bytes.slice(0, 65)).toEqual(hexToBytes(`0x${eoaSlot}`));
    expect(bytes[129]).toBe(0);
    expect(BigInt(`0x${merged.slice(2 + 65 * 2 + 64, 2 + 65 * 2 + 128)}`)).toBe(130n);
    expect(bytes.slice(130)).toEqual(hexToBytes(contractSignature).slice(65));
  });
});
