/**
 * ENS namehash (EIP-137), shared by the identity waterfall and the Chainlink
 * fiat feeds (both Expo files carried their own copy — one here).
 *
 *   namehash("")    = 0x00…00
 *   namehash("a.b") = keccak256(namehash("b") ‖ keccak256("a"))
 *
 * `keccak256` is the core's kernel: callers run after `loadCore()`.
 */
import { keccak256 } from '#lib/core/client.ts';

export function namehash(name: string): string {
	let node: Uint8Array = new Uint8Array(32);
	if (name) {
		const labels = name.split('.');
		for (let i = labels.length - 1; i >= 0; i--) {
			const labelHash = keccak256(new TextEncoder().encode(labels[i]));
			const combined = new Uint8Array(64);
			combined.set(node, 0);
			combined.set(labelHash, 32);
			node = keccak256(combined);
		}
	}
	return bytesToHex(node);
}

export function bytesToHex(bytes: Uint8Array): string {
	let hex = '0x';
	for (const b of bytes) hex += b.toString(16).padStart(2, '0');
	return hex;
}
