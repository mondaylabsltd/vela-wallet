/**
 * EIP-55 on the /connect page (vis-20). That page runs in the outside
 * member's browser and carries no vela-core (the wasm is 5.6 MB, for one
 * address's letter case); the core's `checksumAddress` is the rule
 * everywhere else; this is the same EIP, pinned to the EIP's own vectors
 * in the test.
 */
import { keccak_256 } from '@noble/hashes/sha3';

/** `address` in its checksummed case, or as given when it is not an address. */
export function eip55(address: string): string {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address)) return address;
	const hex = address.slice(2).toLowerCase();
	const hash = keccak_256(new TextEncoder().encode(hex));
	let out = '0x';
	for (let i = 0; i < 40; i++) {
		const nibble = (hash[i >> 1] >> (i % 2 === 0 ? 4 : 0)) & 0xf;
		out += nibble >= 8 ? hex[i].toUpperCase() : hex[i];
	}
	return out;
}
