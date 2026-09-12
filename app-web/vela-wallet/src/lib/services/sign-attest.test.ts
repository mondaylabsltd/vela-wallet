/**
 * Displayed = signed, by a second implementation (spec 028 Phase 8).
 *
 * The shell's TypeScript assembly and the core's Rust assembly must agree on
 * the bytes a passkey signs — and when they do not, nothing is signed.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, test } from 'vitest';
import { attestSafeMessageHash, attestSafeOpHash, toBase64Url } from '$lib/core/kernels';
import {
	buildInBandFeeLeg,
	buildMultiSendExecuteCallData,
	calculateSafeOpHash,
	computeSafeMessageHash,
	encodeErc20Transfer
} from './safe-transaction';
import {
	assertChallengeSigned,
	attestedSafeMessageHash,
	attestedSafeOpHash,
	SignAttestError
} from './sign-attest';

const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const USDC = '0x' + 'a0b8'.repeat(10);
const RECIPIENT = '0x' + 'ab'.repeat(20);
const RELAY = '0x' + 'cd'.repeat(20);
const CHAIN = 100;

/** A send of 1 USDC plus a native fee leg, as the in-band path builds it. */
function shownBatch() {
	const inner = [{ to: USDC, value: '0x0', data: encodeErc20Transfer(RECIPIENT, 1_000_000n) }];
	const fee = { gasFeeToken: null, recipient: RELAY, amount: 12_345n };
	const callData = buildMultiSendExecuteCallData([
		...inner,
		buildInBandFeeLeg(fee.gasFeeToken, fee.recipient, fee.amount)
	]);
	return { inner, fee, callData };
}

function op(callData: Uint8Array) {
	return {
		sender: SAFE,
		nonce: '0x1a',
		initCode: new Uint8Array(0),
		callData,
		verificationGasLimit: 300_000n,
		callGasLimit: 250_000n,
		preVerificationGas: 110_000n,
		maxFeePerGas: 0n,
		maxPriorityFeePerGas: 0n,
		paymasterAndData: new Uint8Array(0),
		signature: new Uint8Array(0)
	};
}

describe('the SafeOp hash', () => {
	test('the shell and the core hash the same operation identically', () => {
		const { inner, fee, callData } = shownBatch();
		const userOp = op(callData);
		const shell = calculateSafeOpHash(userOp, CHAIN);
		const core = attestSafeOpHash(userOp, { inner, fee, alwaysMultiSend: true }, CHAIN);
		expect(core).toHaveLength(32);
		expect(Buffer.from(core).toString('hex')).toBe(Buffer.from(shell).toString('hex'));
		// And the hash-only attestation (finished calldata, nothing it was built from).
		expect(attestedSafeOpHash(userOp, CHAIN, null, shell)).toEqual(core);
	});

	test('calldata that is not the shown calls is refused', () => {
		const { inner, fee, callData } = shownBatch();
		const tampered = new Uint8Array(callData.length + 1);
		tampered.set(callData);
		const userOp = op(tampered);
		expect(() =>
			attestedSafeOpHash(
				userOp,
				CHAIN,
				{ inner, fee, alwaysMultiSend: true },
				calculateSafeOpHash(userOp, CHAIN)
			)
		).toThrow(SignAttestError);
	});

	test('a fee leg that is not the shown fee is refused', () => {
		const { inner, fee, callData } = shownBatch();
		const userOp = op(callData);
		const shownLess = { ...fee, amount: fee.amount - 1n };
		expect(() =>
			attestedSafeOpHash(
				userOp,
				CHAIN,
				{ inner, fee: shownLess, alwaysMultiSend: true },
				calculateSafeOpHash(userOp, CHAIN)
			)
		).toThrow(SignAttestError);
		const otherRelay = { ...fee, recipient: '0x' + 'ef'.repeat(20) };
		expect(() =>
			attestedSafeOpHash(
				userOp,
				CHAIN,
				{ inner, fee: otherRelay, alwaysMultiSend: true },
				calculateSafeOpHash(userOp, CHAIN)
			)
		).toThrow(SignAttestError);
	});

	test('a shell hash the core does not reproduce is refused', () => {
		const { callData } = shownBatch();
		const userOp = op(callData);
		const wrong = calculateSafeOpHash(userOp, CHAIN + 1);
		expect(() => attestedSafeOpHash(userOp, CHAIN, null, wrong)).toThrow(SignAttestError);
	});
});

describe('the Safe message hash', () => {
	test('agrees between the shell and the core', () => {
		const original = new Uint8Array(32).fill(7);
		const shell = computeSafeMessageHash(original, CHAIN, SAFE);
		const core = attestSafeMessageHash(original, CHAIN, SAFE);
		expect(Buffer.from(core).toString('hex')).toBe(Buffer.from(shell).toString('hex'));
		expect(attestedSafeMessageHash(original, CHAIN, SAFE, shell)).toEqual(core);
		expect(() => attestedSafeMessageHash(original, CHAIN + 1, SAFE, shell)).toThrow(
			SignAttestError
		);
	});
});

describe('the assertion', () => {
	const expected = new Uint8Array(32).fill(0x42);
	const clientData = (fields: Record<string, unknown>) =>
		new TextEncoder().encode(JSON.stringify(fields));

	test('over exactly the shown challenge passes', () => {
		expect(() =>
			assertChallengeSigned(
				clientData({
					type: 'webauthn.get',
					challenge: toBase64Url(expected),
					origin: 'https://getvela.app'
				}),
				expected
			)
		).not.toThrow();
	});

	test('over another challenge, another ceremony, or no JSON at all is discarded', () => {
		const other = new Uint8Array(32).fill(0x43);
		expect(() =>
			assertChallengeSigned(
				clientData({ type: 'webauthn.get', challenge: toBase64Url(other) }),
				expected
			)
		).toThrow(SignAttestError);
		expect(() =>
			assertChallengeSigned(
				clientData({ type: 'webauthn.create', challenge: toBase64Url(expected) }),
				expected
			)
		).toThrow(SignAttestError);
		expect(() => assertChallengeSigned(new Uint8Array([1, 2, 3]), expected)).toThrow(
			SignAttestError
		);
	});
});
