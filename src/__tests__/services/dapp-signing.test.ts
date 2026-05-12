/**
 * Tests for DApp signing logic.
 *
 * Tests the pure routing/classification functions and verifies
 * personal_sign prefix construction.
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
jest.mock('@/modules/passkey', () => ({}));

import { isSigningMethod } from '@/hooks/use-dapp-signing';
import { keccak256 } from '@/services/eth-crypto';
import { fromHex, toHex } from '@/services/hex';

function computePersonalSignHash(msgBytes: Uint8Array): Uint8Array {
  const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
  const combined = new Uint8Array(prefix.length + msgBytes.length);
  combined.set(prefix);
  combined.set(msgBytes, prefix.length);
  return keccak256(combined);
}

describe('dapp-signing', () => {
  describe('isSigningMethod', () => {
    test('identifies eth_sendTransaction as signing', () => {
      expect(isSigningMethod('eth_sendTransaction')).toBe(true);
    });

    test('identifies personal_sign as signing', () => {
      expect(isSigningMethod('personal_sign')).toBe(true);
    });

    test('identifies eth_sign as signing', () => {
      expect(isSigningMethod('eth_sign')).toBe(true);
    });

    test('identifies signTypedData variants as signing', () => {
      expect(isSigningMethod('eth_signTypedData')).toBe(true);
      expect(isSigningMethod('eth_signTypedData_v3')).toBe(true);
      expect(isSigningMethod('eth_signTypedData_v4')).toBe(true);
    });

    test('does not flag read-only methods', () => {
      expect(isSigningMethod('eth_accounts')).toBe(false);
      expect(isSigningMethod('eth_chainId')).toBe(false);
      expect(isSigningMethod('eth_call')).toBe(false);
      expect(isSigningMethod('eth_getBalance')).toBe(false);
      expect(isSigningMethod('eth_blockNumber')).toBe(false);
      expect(isSigningMethod('net_version')).toBe(false);
    });
  });

  describe('personal_sign hash construction', () => {
    // Verify the EIP-191 prefix construction is correct
    test('constructs correct EIP-191 hash for "Hello"', () => {
      const msg = 'Hello';
      const msgBytes = new TextEncoder().encode(msg);

      // EIP-191: "\x19Ethereum Signed Message:\n" + len(msg) + msg
      const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
      const combined = new Uint8Array(prefix.length + msgBytes.length);
      combined.set(prefix);
      combined.set(msgBytes, prefix.length);
      const hash = keccak256(combined);

      // Hash should be 32 bytes and deterministic
      expect(hash.length).toBe(32);
      // Verify the prefix is correct: "\x19Ethereum Signed Message:\n5"
      const prefixStr = new TextDecoder().decode(prefix);
      expect(prefixStr).toBe('\x19Ethereum Signed Message:\n5');
    });

    test('text vs hex-encoded message produce same hash', () => {
      // personal_sign sends hex-encoded messages; when decoded they should match raw text
      const hexMsg = '0x48656c6c6f'; // "Hello" in hex
      const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
      const msgBytesFromHex = fromHex(clean);
      const msgBytesFromText = new TextEncoder().encode('Hello');

      // Both should produce the same EIP-191 hash
      const hashFromHex = computePersonalSignHash(msgBytesFromHex);
      const hashFromText = computePersonalSignHash(msgBytesFromText);

      expect(toHex(hashFromHex)).toBe(toHex(hashFromText));
    });

    test('handles multi-digit length correctly', () => {
      // 100-byte message: length string should be "100"
      const msgBytes = new Uint8Array(100);
      const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
      const prefixStr = new TextDecoder().decode(prefix);
      expect(prefixStr).toBe('\x19Ethereum Signed Message:\n100');
    });

    test('handles empty message', () => {
      const msgBytes = new Uint8Array(0);
      const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
      const combined = new Uint8Array(prefix.length);
      combined.set(prefix);
      const hash = keccak256(combined);
      expect(hash.length).toBe(32);
    });
  });
});
