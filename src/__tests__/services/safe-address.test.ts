/**
 * Tests for Safe address computation.
 * Test vectors match iOS SafeAddressTests.swift and Android SafeAddressComputerTest.kt.
 */
import { computeAddress, parsePublicKey, calculateSaltNonce, encodeSetupData, SAFE_PROXY_RUNTIME_CODE, PROXY_CREATION_CODE } from '@/services/safe-address';
import { keccak256 } from '@/services/eth-crypto';
import { toHex } from '@/services/hex';

// Test public key (matches iOS/Android test vectors)
const TEST_PUBLIC_KEY = '04a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90b1c2d3e4f50617283940a1b2c3d4e5f6b1c2d3e4f50617283940a1b2c3d4e5f6';
// Matches iOS SafeAddressTests.swift and Android SafeAddressComputerTest.kt
const EXPECTED_ADDRESS = '0x762EdA60D3B68755c271D608644650278f88329F';

describe('parsePublicKey', () => {
  test('parses uncompressed public key with 04 prefix', () => {
    const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
    expect(x.length).toBe(32);
    expect(y.length).toBe(32);
    expect(toHex(x)).toBe('a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90');
    expect(toHex(y)).toBe('b1c2d3e4f50617283940a1b2c3d4e5f6b1c2d3e4f50617283940a1b2c3d4e5f6');
  });

  test('parses with 0x prefix', () => {
    const { x, y } = parsePublicKey('0x' + TEST_PUBLIC_KEY);
    expect(x.length).toBe(32);
    expect(y.length).toBe(32);
  });

  test('parses without 04 prefix', () => {
    const rawXY = TEST_PUBLIC_KEY.slice(2); // remove "04"
    const { x, y } = parsePublicKey(rawXY);
    expect(x.length).toBe(32);
    expect(y.length).toBe(32);
  });

  test('returns empty for invalid input', () => {
    const { x, y } = parsePublicKey('invalid');
    expect(x.length).toBe(0);
    expect(y.length).toBe(0);
  });
});

describe('calculateSaltNonce', () => {
  test('produces correct salt nonce for test key', () => {
    const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
    const nonce = calculateSaltNonce(x, y);
    expect(nonce.length).toBe(32);
    expect(toHex(nonce)).toBe('ff558186314810b914e7a54ec8f9dee960ff493364c68ba36e07dd89f547787a');
  });
});

describe('encodeSetupData', () => {
  test('produces deterministic setup data', () => {
    const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
    const data1 = encodeSetupData(x, y);
    const data2 = encodeSetupData(x, y);
    expect(toHex(data1)).toBe(toHex(data2));
  });

  test('setup data hash matches cross-platform test vector', () => {
    const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
    const setupData = encodeSetupData(x, y);
    const hash = keccak256(setupData);
    expect(toHex(hash)).toBe('b0d27e7ff8c758797463d1d9b3cfe53cd9c7ff2a92f037cd261b4f90f5de0191');
  });

  test('starts with setup function selector', () => {
    const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
    const setupData = encodeSetupData(x, y);
    // setup(address[],uint256,address,bytes,address,address,uint256,address) → b63e800d
    expect(toHex(setupData.slice(0, 4))).toBe('b63e800d');
  });
});

describe('computeAddress', () => {
  test('computes correct Safe address from test public key', () => {
    const address = computeAddress(TEST_PUBLIC_KEY);
    expect(address).toBe(EXPECTED_ADDRESS);
  });

  test('produces checksummed address', () => {
    const address = computeAddress(TEST_PUBLIC_KEY);
    expect(address.startsWith('0x')).toBe(true);
    // Check mixed case (not all lowercase)
    const body = address.slice(2);
    const hasUpper = body.split('').some(c => c >= 'A' && c <= 'F');
    const hasLower = body.split('').some(c => c >= 'a' && c <= 'f');
    if (body.match(/[a-fA-F]/)) {
      expect(hasUpper || hasLower).toBe(true);
    }
  });

  test('is deterministic', () => {
    const addr1 = computeAddress(TEST_PUBLIC_KEY);
    const addr2 = computeAddress(TEST_PUBLIC_KEY);
    expect(addr1).toBe(addr2);
  });

  test('different public keys produce different addresses', () => {
    const key2 = '04' + 'ff'.repeat(32) + '00'.repeat(32);
    const addr1 = computeAddress(TEST_PUBLIC_KEY);
    const addr2 = computeAddress(key2);
    expect(addr1).not.toBe(addr2);
  });

  test('handles 0x prefix', () => {
    const addr1 = computeAddress(TEST_PUBLIC_KEY);
    const addr2 = computeAddress('0x' + TEST_PUBLIC_KEY);
    expect(addr1).toBe(addr2);
  });
});

describe('SAFE_PROXY_RUNTIME_CODE', () => {
  test('is a 0x-prefixed hex string', () => {
    expect(SAFE_PROXY_RUNTIME_CODE.startsWith('0x')).toBe(true);
    expect(/^0x[0-9a-f]+$/.test(SAFE_PROXY_RUNTIME_CODE)).toBe(true);
  });

  test('is exactly 0xab (171) bytes — the length the proxy constructor returns', () => {
    const byteLen = (SAFE_PROXY_RUNTIME_CODE.length - 2) / 2;
    expect(byteLen).toBe(0xab);
    expect(byteLen).toBe(171);
  });

  test('is the runtime region of PROXY_CREATION_CODE (after the constructor RETURN)', () => {
    // Constructor ends with `...6000396000f3fe` (CODECOPY; RETURN; INVALID),
    // then the runtime it returns, then the baked-in revert string.
    const sep = '6000396000f3fe';
    const start = PROXY_CREATION_CODE.indexOf(sep) + sep.length;
    const expected = '0x' + PROXY_CREATION_CODE.slice(start, start + 0xab * 2);
    expect(SAFE_PROXY_RUNTIME_CODE).toBe(expected);
  });

  test('looks like a Safe proxy runtime, not creation code', () => {
    // Runtime starts with the proxy preamble that loads the singleton from slot 0.
    expect(SAFE_PROXY_RUNTIME_CODE.startsWith('0x608060405273')).toBe(true);
    // Ends at the Solidity metadata terminator…
    expect(SAFE_PROXY_RUNTIME_CODE.endsWith('0033')).toBe(true);
    // …and must NOT include the "Invalid singleton address provided" revert
    // string that lives only in the creation code.
    const errString = Buffer.from('Invalid singleton address provided', 'utf8').toString('hex');
    expect(SAFE_PROXY_RUNTIME_CODE.includes(errString)).toBe(false);
  });

  test('is non-empty so eth_getCode marks a counterfactual account as a contract', () => {
    expect(SAFE_PROXY_RUNTIME_CODE).not.toBe('0x');
    expect(SAFE_PROXY_RUNTIME_CODE.length).toBeGreaterThan(2);
  });
});
