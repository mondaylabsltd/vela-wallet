/**
 * Tests for EIP-1271 compatible dApp signatures.
 *
 * Verifies that handlePersonalSign and handleSignTypedData return
 * full Safe WebAuthn contract signatures (not just raw P256 r+s),
 * so that on-chain isValidSignature verification works.
 */

// Mock react-native transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(), save: jest.fn(), remove: jest.fn(), syncNow: jest.fn(),
}));

// ── Realistic WebAuthn test fixtures ──────────────────────────────────────

// A valid DER-encoded P256 signature (as returned by WebAuthn)
const MOCK_DER_SIG_HEX =
  '3046022100' +
  'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2' +
  '022100' +
  'f1e2d3c4b5a6f1e2d3c4b5a6f1e2d3c4b5a6f1e2d3c4b5a6f1e2d3c4b5a6f1e2';

// authenticatorData: 37 bytes (rpIdHash(32) + flags(1) + counter(4))
const MOCK_AUTH_DATA_HEX =
  '49960de5880e8c687434170f6476605b8fe4aeb9a28632c7995cf3ba831d9763' + // rpIdHash
  '05' + // flags
  '00000001'; // counter

// A realistic clientDataJSON
const MOCK_CLIENT_DATA_JSON = JSON.stringify({
  type: 'webauthn.get',
  challenge: 'dGVzdC1jaGFsbGVuZ2U',
  origin: 'https://getvela.app',
  crossOrigin: false,
});
const MOCK_CLIENT_DATA_HEX = Buffer.from(MOCK_CLIENT_DATA_JSON).toString('hex');

const mockAssertionResult = {
  credentialId: 'mock-credential-id',
  signatureHex: MOCK_DER_SIG_HEX,
  authenticatorDataHex: MOCK_AUTH_DATA_HEX,
  clientDataJSONHex: MOCK_CLIENT_DATA_HEX,
};

// Mock passkey module to return our test assertion
jest.mock('@/modules/passkey', () => ({
  sign: jest.fn().mockResolvedValue(mockAssertionResult),
  getRelyingPartyId: jest.fn().mockReturnValue('getvela.app'),
  PasskeyAssertionResult: {},
}));

// Mock storage/public-key-index (not needed for signature format tests)
jest.mock('@/services/storage', () => ({
  findAccountByCredentialId: jest.fn().mockResolvedValue(null),
}));
jest.mock('@/services/public-key-index', () => ({
  queryRecord: jest.fn().mockRejectedValue(new Error('not needed')),
}));
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: jest.fn(),
}));

import { handlePersonalSign, handleSignTypedData, handleGenericSign, isSigningMethod } from '@/hooks/use-dapp-signing';
import { extractClientDataFields, buildUserOpSignature } from '@/services/safe-transaction';
import { derSignatureToRaw } from '@/services/attestation-parser';
import { fromHex, toHex } from '@/services/hex';
import type { Account } from '@/models/types';

const mockAccount: Account = {
  id: 'mock-credential-id',
  name: 'Test Account',
  safeAddress: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
  chainId: 137,
} as any;

describe('EIP-1271 contract signature format', () => {
  describe('extractClientDataFields', () => {
    test('extracts fields after challenge', () => {
      const clientDataJSON = new TextEncoder().encode(
        '{"type":"webauthn.get","challenge":"dGVzdA","origin":"https://getvela.app","crossOrigin":false}',
      );
      const fields = extractClientDataFields(clientDataJSON);
      expect(fields).toBe('"origin":"https://getvela.app","crossOrigin":false');
    });

    test('handles different challenge values', () => {
      const clientDataJSON = new TextEncoder().encode(
        '{"type":"webauthn.get","challenge":"abc123","origin":"https://example.com","crossOrigin":true}',
      );
      const fields = extractClientDataFields(clientDataJSON);
      expect(fields).toBe('"origin":"https://example.com","crossOrigin":true');
    });

    test('returns empty string for missing challenge', () => {
      const clientDataJSON = new TextEncoder().encode('{"type":"webauthn.get"}');
      const fields = extractClientDataFields(clientDataJSON);
      expect(fields).toBe('');
    });
  });

  describe('buildUserOpSignature structure', () => {
    test('produces signature with validity padding + contract sig header + dynamic data', () => {
      const authData = fromHex(MOCK_AUTH_DATA_HEX);
      const clientDataJSON = fromHex(MOCK_CLIENT_DATA_HEX);
      const clientDataFields = extractClientDataFields(clientDataJSON);

      const rawSig = derSignatureToRaw(fromHex(MOCK_DER_SIG_HEX));
      expect(rawSig).not.toBeNull();
      const sigR = rawSig!.slice(0, 32);
      const sigS = rawSig!.slice(32);

      const sig = buildUserOpSignature(authData, clientDataFields, sigR, sigS);

      // Minimum size: 12 (validity) + 32 (r/signer) + 32 (s/offset) + 1 (v) + 32 (dataLen) + dynamic
      expect(sig.length).toBeGreaterThan(109);

      // First 12 bytes = validity padding (zeros)
      expect(sig.slice(0, 12)).toEqual(new Uint8Array(12));

      // v byte at position 76 (12 + 32 + 32) should be 0x00 (contract sig type)
      expect(sig[76]).toBe(0x00);
    });

    test('signature is much longer than raw 64 bytes', () => {
      const authData = fromHex(MOCK_AUTH_DATA_HEX);
      const clientDataJSON = fromHex(MOCK_CLIENT_DATA_HEX);
      const clientDataFields = extractClientDataFields(clientDataJSON);

      const rawSig = derSignatureToRaw(fromHex(MOCK_DER_SIG_HEX));
      const sigR = rawSig!.slice(0, 32);
      const sigS = rawSig!.slice(32);

      const sig = buildUserOpSignature(authData, clientDataFields, sigR, sigS);

      // Full contract signature should be significantly longer than 65 bytes (raw r+s+v)
      expect(sig.length).toBeGreaterThan(200);
    });
  });

  describe('handlePersonalSign', () => {
    test('returns full contract signature, not raw P256 sig', async () => {
      const request = {
        id: 'test-1',
        method: 'personal_sign',
        params: ['0x48656c6c6f', '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c'], // "Hello"
      };

      const result = await handlePersonalSign(request, mockAccount);

      // Should be hex string starting with 0x
      expect(result).toMatch(/^0x[0-9a-f]+$/i);

      // Should NOT be 65 bytes (130 hex chars + 0x prefix = 132)
      // Old format was: 0x + rawSig(128 hex) + "00" = 132 chars
      expect(result.length).toBeGreaterThan(132);

      // Should NOT end with just "00" at the 65-byte boundary
      // The full contract signature is much longer
      const sigBytes = fromHex(result.slice(2));
      expect(sigBytes.length).toBeGreaterThan(200);

      // First 12 bytes should be validity padding (zeros)
      expect(sigBytes.slice(0, 12)).toEqual(new Uint8Array(12));

      // v byte at position 76 should be 0x00 (contract sig type)
      expect(sigBytes[76]).toBe(0x00);
    });
  });

  describe('handleSignTypedData', () => {
    test('returns full contract signature for EIP-712 typed data', async () => {
      const typedData = {
        types: {
          EIP712Domain: [
            { name: 'name', type: 'string' },
            { name: 'chainId', type: 'uint256' },
            { name: 'verifyingContract', type: 'address' },
          ],
          PermitSingle: [
            { name: 'details', type: 'PermitDetails' },
            { name: 'spender', type: 'address' },
            { name: 'sigDeadline', type: 'uint256' },
          ],
          PermitDetails: [
            { name: 'token', type: 'address' },
            { name: 'amount', type: 'uint160' },
            { name: 'expiration', type: 'uint48' },
            { name: 'nonce', type: 'uint48' },
          ],
        },
        domain: {
          name: 'Permit2',
          chainId: '137',
          verifyingContract: '0x000000000022d473030f116ddee9f6b43ac78ba3',
        },
        primaryType: 'PermitSingle',
        message: {
          details: {
            token: '0xc011a7e12a19f7b1f670d46f03b03f3342e82dfb',
            amount: '1461501637330902918203684832716283019655932542975',
            expiration: '1783609751',
            nonce: '0',
          },
          spender: '0x8b844f885672f333bc0042cb669255f93a4c1e6b',
          sigDeadline: '1781019551',
        },
      };

      const request = {
        id: 'test-2',
        method: 'eth_signTypedData_v4',
        params: ['0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c', JSON.stringify(typedData)],
      };

      const result = await handleSignTypedData(request, mockAccount);

      // Should be hex string
      expect(result).toMatch(/^0x[0-9a-f]+$/i);

      // Should be full contract signature (much longer than 65 bytes)
      const sigBytes = fromHex(result.slice(2));
      expect(sigBytes.length).toBeGreaterThan(200);

      // Validity padding + contract sig type check
      expect(sigBytes.slice(0, 12)).toEqual(new Uint8Array(12));
      expect(sigBytes[76]).toBe(0x00);
    });

    test('handles typed data passed as object (not string)', async () => {
      const request = {
        id: 'test-3',
        method: 'eth_signTypedData_v4',
        params: [
          '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
          {
            types: { EIP712Domain: [{ name: 'name', type: 'string' }] },
            domain: { name: 'Test' },
            primaryType: 'EIP712Domain',
            message: {},
          },
        ],
      };

      const result = await handleSignTypedData(request, mockAccount);
      expect(result).toMatch(/^0x[0-9a-f]+$/i);
      const sigBytes = fromHex(result.slice(2));
      expect(sigBytes.length).toBeGreaterThan(200);
    });
  });

  describe('handleGenericSign', () => {
    test('returns full contract signature, not raw DER signature', async () => {
      const request = {
        id: 'test-4',
        method: 'eth_sign',
        params: ['0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c', '0xdeadbeef'],
      };

      const result = await handleGenericSign(request, mockAccount);

      expect(result).toMatch(/^0x[0-9a-f]+$/i);
      const sigBytes = fromHex(result.slice(2));

      // Must be full contract sig, not just raw DER bytes
      expect(sigBytes.length).toBeGreaterThan(200);
      expect(sigBytes.slice(0, 12)).toEqual(new Uint8Array(12));
      expect(sigBytes[76]).toBe(0x00);
    });
  });

  describe('all signing functions share the same contract signature format', () => {
    test('personalSign, signTypedData, and genericSign all produce same structure', async () => {
      const personalReq = {
        id: 'cmp-1', method: 'personal_sign',
        params: ['0x48656c6c6f', '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c'],
      };
      const typedDataReq = {
        id: 'cmp-2', method: 'eth_signTypedData_v4',
        params: ['0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c', JSON.stringify({
          types: { EIP712Domain: [{ name: 'name', type: 'string' }] },
          domain: { name: 'Test' },
          primaryType: 'EIP712Domain',
          message: {},
        })],
      };
      const genericReq = {
        id: 'cmp-3', method: 'eth_sign',
        params: ['0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c', '0xdeadbeef'],
      };

      const [sig1, sig2, sig3] = await Promise.all([
        handlePersonalSign(personalReq, mockAccount),
        handleSignTypedData(typedDataReq, mockAccount),
        handleGenericSign(genericReq, mockAccount),
      ]);

      // All three should use the same mock assertion, so they should produce
      // identical contract signature structures (same authenticatorData + clientDataFields)
      const bytes1 = fromHex(sig1.slice(2));
      const bytes2 = fromHex(sig2.slice(2));
      const bytes3 = fromHex(sig3.slice(2));

      // All should have same length (same WebAuthn data)
      expect(bytes1.length).toBe(bytes2.length);
      expect(bytes2.length).toBe(bytes3.length);

      // All should have same validity padding and contract sig type
      for (const bytes of [bytes1, bytes2, bytes3]) {
        expect(bytes.slice(0, 12)).toEqual(new Uint8Array(12));
        expect(bytes[76]).toBe(0x00);
      }

      // Since all use the same mock assertion, the signatures should be identical
      expect(sig1).toBe(sig2);
      expect(sig2).toBe(sig3);
    });
  });

  describe('isSigningMethod', () => {
    test('wallet_sendCalls is classified as signing', () => {
      expect(isSigningMethod('wallet_sendCalls')).toBe(true);
    });
  });
});
