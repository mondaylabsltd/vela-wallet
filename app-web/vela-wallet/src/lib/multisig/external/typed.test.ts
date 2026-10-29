/**
 * r4C-7: the connect page reads a typed-data request as a SafeTx only when it
 * is exactly the core's own typed data for that SafeTx; r4C-16: Safe's
 * `eth_sign` form, as the core checks it — the bare `personal_sign` answer is
 * no approval of the safeTxHash, the `v + 4` one is, and is told apart; the
 * review's hashes make the safeTxHash.
 */
import { describe, expect, it } from 'vitest';
import '$lib/i18n/wasm-init.server';
import { secp256k1 } from '@noble/curves/secp256k1.js';
import { keccak_256 } from '@noble/hashes/sha3.js';
import {
	ecdsaForm,
	safeTxHash,
	safeTxHashes,
	safeTxTypedData,
	verifyEcdsaApproval,
	type SafeTx
} from '../core/kernel';
import { ethSignForm, readSafeTxRequest } from './typed';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const TX: SafeTx = {
	to: '0x000000000000000000000000000000000000dEaD',
	value: '0x2386f26fc10000',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x3'
};

const KEY = new Uint8Array(32).fill(7);
const hexOf = (b: Uint8Array) => `0x${Buffer.from(b).toString('hex')}`;
const bytesOf = (h: string) => Uint8Array.from(Buffer.from(h.replace(/^0x/, ''), 'hex'));
const OWNER = (() => {
	const pub = secp256k1.getPublicKey(KEY, false).slice(1);
	return hexOf(keccak_256(pub).slice(12));
})();

/** `r ‖ s ‖ v` (v 27/28) over a 32-byte digest. */
function sign(digest: Uint8Array): string {
	const sig = secp256k1.sign(digest, KEY, { prehash: false }) as unknown as {
		r: bigint;
		s: bigint;
		recovery: number;
	};
	const word = (n: bigint) => n.toString(16).padStart(64, '0');
	return `0x${word(sig.r)}${word(sig.s)}${(27 + sig.recovery).toString(16)}`;
}

/** What `personal_sign` signs for a 32-byte message: the EIP-191 hash. */
function personalDigest(hash: string): Uint8Array {
	const prefix = new TextEncoder().encode('\x19Ethereum Signed Message:\n32');
	return keccak_256(new Uint8Array([...prefix, ...bytesOf(hash)]));
}

describe('reading a typed-data request as a SafeTx', () => {
	it("is the core's own typed data, field for field", () => {
		const typed = safeTxTypedData(5042002, SAFE, TX);
		const read = readSafeTxRequest(JSON.stringify(typed));
		expect(read).not.toBeNull();
		expect(read!.chainId).toBe(5042002);
		expect(safeTxHash(read!.chainId, read!.safe, read!.tx)).toBe(safeTxHash(5042002, SAFE, TX));
		// An object, as some sides send it, reads the same.
		expect(readSafeTxRequest(typed)).toEqual(read);
	});

	it('anything else is not one: a changed field, another domain, another type', () => {
		const typed = safeTxTypedData(5042002, SAFE, TX) as Record<string, never>;
		const changed = JSON.parse(JSON.stringify(typed));
		changed.types.SafeTx.pop();
		expect(readSafeTxRequest(changed)).toBeNull();
		const named = JSON.parse(JSON.stringify(typed));
		named.domain.name = 'Not Safe';
		expect(readSafeTxRequest(named)).toBeNull();
		const other = JSON.parse(JSON.stringify(typed));
		other.primaryType = 'Permit';
		expect(readSafeTxRequest(other)).toBeNull();
		expect(readSafeTxRequest('not json')).toBeNull();
		expect(readSafeTxRequest(undefined)).toBeNull();
	});
});

describe("Safe's eth_sign form (r4C-16)", () => {
	const hash = safeTxHash(5042002, SAFE, TX);

	it('v + 4 over the EIP-191 hash is an approval, told apart from typed data', () => {
		const personal = sign(personalDigest(hash));
		// The bare personal_sign answer is no approval of the safeTxHash.
		expect(() => verifyEcdsaApproval(hash, { owner: OWNER, signature: personal })).toThrow();
		const form = ethSignForm(personal);
		expect(form.slice(-2)).toMatch(/^(1f|20)$/);
		const checked = verifyEcdsaApproval(hash, { owner: OWNER, signature: form });
		expect(checked.owner.toLowerCase()).toBe(OWNER);
		expect(checked.signature.toLowerCase()).toBe(form.toLowerCase());
		expect(ecdsaForm(form)).toBe('ethSign');
		// The typed form, over the safeTxHash itself.
		const typed = sign(bytesOf(hash));
		expect(verifyEcdsaApproval(hash, { owner: OWNER, signature: typed }).owner.toLowerCase()).toBe(
			OWNER
		);
		expect(ecdsaForm(typed)).toBe('typedData');
	});

	it('ethSignForm takes v 27/28 or 0/1, and nothing else', () => {
		const rs = '11'.repeat(64);
		expect(ethSignForm(`0x${rs}1b`)).toBe(`0x${rs}1f`);
		expect(ethSignForm(`0x${rs}01`)).toBe(`0x${rs}20`);
		expect(() => ethSignForm(`0x${rs}1f`)).toThrow();
		expect(() => ethSignForm('0x1234')).toThrow();
	});
});

describe('the hashes a hardware wallet shows (r4C-7)', () => {
	it('domain and message hash make the safeTxHash', () => {
		const h = safeTxHashes(5042002, SAFE, TX);
		expect(h.safeTxHash).toBe(safeTxHash(5042002, SAFE, TX));
		const joined = new Uint8Array([
			0x19,
			0x01,
			...bytesOf(h.domainHash),
			...bytesOf(h.messageHash)
		]);
		expect(hexOf(keccak_256(joined))).toBe(h.safeTxHash);
		expect(h.ethSignHash).toBe(hexOf(personalDigest(h.safeTxHash)));
	});
});
