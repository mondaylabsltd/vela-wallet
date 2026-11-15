/** Ported from src/__tests__/services/safe-address.test.ts @ f9bcb278 — vitest; the core is initialised by the server-side wasm init. */
import '#lib/i18n/wasm-init.server.ts';
import { describe, expect, test } from 'vitest';
/**
 * Tests for Safe address computation.
 * Test vectors match iOS SafeAddressTests.swift and Android SafeAddressComputerTest.kt.
 */
import {
	computeAddress,
	parsePublicKey,
	calculateSaltNonce,
	encodeSetupData,
	SAFE_PROXY_RUNTIME_CODE,
	PROXY_CREATION_CODE
} from './kernels';
import { keccak256 } from './kernels';
import { toHex } from './kernels';

// Test public key (matches iOS/Android test vectors)
const TEST_PUBLIC_KEY =
	'04a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90b1c2d3e4f50617283940a1b2c3d4e5f6b1c2d3e4f50617283940a1b2c3d4e5f6';
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

	test('refuses invalid input instead of returning empty coordinates', () => {
		// Empty x/y was the oracle's silent failure mode — a caller that skipped the
		// length check would derive an address from nothing. The core throws.
		expect(() => parsePublicKey('invalid')).toThrow();
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
		const hasUpper = body.split('').some((c) => c >= 'A' && c <= 'F');
		const hasLower = body.split('').some((c) => c >= 'a' && c <= 'f');
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

// ---------------------------------------------------------------------------
// Multi-key founding sets (release gate: N=1 is byte-identical to single-key)
// ---------------------------------------------------------------------------

import {
	computeAddressMulti,
	computeSafeAddressMulti,
	computeWebauthnSignerAddress
} from './kernels';

// The pinned vectors from rust/crates/vela-core/src/safe.rs (multi_pinned_output_vectors).
const MULTI_KEYS = [
	'04' +
		'8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0' +
		'7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506',
	'04' +
		'0000000000000000000000000000000000000000000000000000000000000001' +
		'0000000000000000000000000000000000000000000000000000000000000002',
	'04' +
		'04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988' +
		'1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21'
];

describe('computeSafeAddressMulti', () => {
	test('N=1 is byte-identical to the single-key derivation (release gate)', () => {
		const { address, saltNonce, setupData } = computeSafeAddressMulti([TEST_PUBLIC_KEY]);
		expect(address).toBe(EXPECTED_ADDRESS);
		const { x, y } = parsePublicKey(TEST_PUBLIC_KEY);
		expect(toHex(saltNonce)).toBe(toHex(calculateSaltNonce(x, y)));
		expect(toHex(setupData)).toBe(toHex(encodeSetupData(x, y)));
		expect(computeAddressMulti([TEST_PUBLIC_KEY])).toBe(computeAddress(TEST_PUBLIC_KEY));
	});

	test('3-key founding set matches the pinned cross-layer vector', () => {
		expect(computeAddressMulti(MULTI_KEYS)).toBe('0x5AF6Cd8689C013192e157826f7C4574d7C2f9446');
	});

	test('2-key founding set matches the pinned cross-layer vector', () => {
		expect(computeAddressMulti([MULTI_KEYS[0], MULTI_KEYS[2]])).toBe(
			'0xaBeF0bf37A03a2Af821Cf409a52eB9C01524b2E0'
		);
	});

	test('keys[1..] order cannot move the address; the pin can', () => {
		// The extra keys are canonically sorted inside, so swapping them is a
		// no-op — but swapping the PINNED first key derives a different Safe.
		expect(computeAddressMulti([MULTI_KEYS[0], MULTI_KEYS[2], MULTI_KEYS[1]])).toBe(
			computeAddressMulti(MULTI_KEYS)
		);
		expect(computeAddressMulti([MULTI_KEYS[1], MULTI_KEYS[0], MULTI_KEYS[2]])).not.toBe(
			computeAddressMulti(MULTI_KEYS)
		);
	});

	test('duplicate founding keys are refused (undeployable address)', () => {
		expect(() => computeAddressMulti([MULTI_KEYS[0], MULTI_KEYS[0]])).toThrow();
	});
});

describe('computeWebauthnSignerAddress', () => {
	test('is deterministic and differs per key', () => {
		const a = computeWebauthnSignerAddress(MULTI_KEYS[0]);
		const b = computeWebauthnSignerAddress(MULTI_KEYS[1]);
		expect(a).toMatch(/^0x[0-9a-fA-F]{40}$/);
		expect(a).toBe(computeWebauthnSignerAddress(MULTI_KEYS[0]));
		expect(a).not.toBe(b);
	});
});
