/**
 * Hex ⇄ bytes, the few conversions the site needs. Strict on input: a value
 * that is not `0x` + an even number of hex digits is refused, never "repaired".
 */

export type Hex = `0x${string}`;

const HEX_RE = /^0x([0-9a-fA-F]{2})*$/;

export function isHex(value: unknown): value is Hex {
	return typeof value === 'string' && HEX_RE.test(value);
}

export function hexToBytes(hex: string): Uint8Array {
	if (!isHex(hex)) throw new Error(`not hex: ${String(hex).slice(0, 20)}`);
	const body = hex.slice(2);
	const out = new Uint8Array(body.length / 2);
	for (let i = 0; i < out.length; i++) out[i] = parseInt(body.slice(i * 2, i * 2 + 2), 16);
	return out;
}

export function bytesToHex(bytes: Uint8Array): Hex {
	let s = '0x';
	for (const b of bytes) s += b.toString(16).padStart(2, '0');
	return s as Hex;
}

export function concatBytes(...parts: Uint8Array[]): Uint8Array {
	const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
	let o = 0;
	for (const p of parts) {
		out.set(p, o);
		o += p.length;
	}
	return out;
}

/** UTF-8 text → hex. */
export function stringToHex(text: string): Hex {
	return bytesToHex(new TextEncoder().encode(text));
}

/** Hex → UTF-8 text. Invalid sequences become U+FFFD (callers that must know
 *  decode with `fatal: true` themselves). */
export function hexToString(hex: string): string {
	return new TextDecoder().decode(hexToBytes(hex));
}

/** A non-negative integer as minimal big-endian bytes (0 → empty). */
export function bigintToBytes(value: bigint): Uint8Array {
	if (value < 0n) throw new Error('negative integer');
	if (value === 0n) return new Uint8Array(0);
	let h = value.toString(16);
	if (h.length % 2) h = `0${h}`;
	return hexToBytes(`0x${h}`);
}

export function bytesToBigint(bytes: Uint8Array): bigint {
	return bytes.length === 0 ? 0n : BigInt(bytesToHex(bytes));
}

/** A quantity as JSON-RPC wants it: `0x`-prefixed, no leading zeros. */
export function toQuantity(value: bigint | number): Hex {
	return `0x${BigInt(value).toString(16)}`;
}

/** A JSON-RPC quantity (`0x…`) back to a bigint; anything else throws. */
export function fromQuantity(value: unknown): bigint {
	if (typeof value !== 'string' || !/^0x[0-9a-fA-F]+$/.test(value)) {
		throw new Error(`not a quantity: ${String(value).slice(0, 20)}`);
	}
	return BigInt(value);
}
