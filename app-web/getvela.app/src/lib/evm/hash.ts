/**
 * Keccak-256 and what is built on it: EIP-55 checksums, selectors, CREATE2.
 *
 * The hash itself is `@noble/hashes` — audited, dependency-free, and the same
 * implementation viem used. Everything around it is ours and small enough to
 * read in one sitting.
 */
import { keccak_256 } from '@noble/hashes/sha3.js';
import { bytesToHex, concatBytes, hexToBytes, type Hex } from './hex';

export type Address = `0x${string}`;

export function keccak256(data: Hex | Uint8Array): Hex {
	return bytesToHex(keccak_256(typeof data === 'string' ? hexToBytes(data) : data));
}

export function keccak256Bytes(data: Uint8Array): Uint8Array {
	return keccak_256(data);
}

const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

/**
 * EIP-55 checksummed form of an address. Throws on anything that is not 20
 * bytes of hex, and on a mixed-case input whose checksum is wrong (a mistyped
 * address must not be silently "corrected" into a different one).
 */
export function getAddress(address: string): Address {
	if (!ADDRESS_RE.test(address)) throw new Error(`not an address: ${address.slice(0, 44)}`);
	const lower = address.slice(2).toLowerCase();
	const hash = keccak_256(new TextEncoder().encode(lower));
	let out = '0x';
	for (let i = 0; i < 40; i++) {
		const nibble = (hash[i >> 1] >> (i % 2 === 0 ? 4 : 0)) & 0x0f;
		out += nibble >= 8 ? lower[i].toUpperCase() : lower[i];
	}
	const body = address.slice(2);
	const mixed = body !== body.toLowerCase() && body !== body.toUpperCase();
	if (mixed && out !== address) throw new Error(`bad address checksum: ${address}`);
	return out as Address;
}

export function isAddress(address: string): boolean {
	try {
		getAddress(address);
		return true;
	} catch {
		return false;
	}
}

/** The 4-byte selector of a canonical signature, e.g. `SINGLETON()`. */
export function selectorOf(signature: string): Hex {
	return keccak256(new TextEncoder().encode(signature)).slice(0, 10) as Hex;
}

/** Where CREATE2 puts `initCode` deployed by `from` with `salt`. */
export function create2Address(from: Address, salt: Hex, initCodeHash: Hex): Address {
	const saltBytes = hexToBytes(salt);
	const codeHash = hexToBytes(initCodeHash);
	if (saltBytes.length !== 32 || codeHash.length !== 32) {
		throw new Error('CREATE2 salt and init-code hash must be 32 bytes each');
	}
	const digest = keccak_256(
		concatBytes(Uint8Array.of(0xff), hexToBytes(getAddress(from)), saltBytes, codeHash)
	);
	return getAddress(bytesToHex(digest.subarray(12)));
}
