/**
 * Solidity ABI encoding and decoding — only the types this site reads and
 * writes: `uintN`, `bool`, `address`, `bytesN`, `bytes`, `string`, arrays
 * (`T[]`, `T[k]`) and tuples. Anything else is refused loudly rather than
 * guessed at.
 *
 * Decoding is strict, because every byte it sees comes from an RPC node or a
 * record anyone may write: offsets and lengths are bounds-checked before
 * they are followed, and values with dirty high bits (an address, a `uint32`,
 * a `bool` that is not 0/1) are rejected — the same answer Solidity's own
 * decoder gives, so we never show a value the contract itself would refuse.
 *
 * Shape of decoded values: `uintN` up to 48 bits is a `number`, wider is a
 * `bigint`; addresses come back EIP-55 checksummed; a tuple whose components
 * are all named is an object, otherwise an array.
 */
import { getAddress, keccak256 } from './hash';
import { bigintToBytes, bytesToBigint, bytesToHex, concatBytes, hexToBytes, type Hex } from './hex';

export interface AbiParameter {
	readonly name?: string;
	readonly type: string;
	readonly components?: readonly AbiParameter[];
}

export interface AbiFunction {
	readonly type: 'function';
	readonly name: string;
	readonly stateMutability: 'pure' | 'view' | 'nonpayable' | 'payable';
	readonly inputs: readonly AbiParameter[];
	readonly outputs: readonly AbiParameter[];
}

export type Abi = readonly AbiFunction[];

// ── Types ────────────────────────────────────────────────────────────────────

type Kind =
	| { k: 'uint'; bits: number }
	| { k: 'bool' }
	| { k: 'address' }
	| { k: 'fixedBytes'; size: number }
	| { k: 'bytes' }
	| { k: 'string' }
	| { k: 'array'; of: AbiParameter; length: number | null }
	| { k: 'tuple'; components: readonly AbiParameter[] };

function kindOf(p: AbiParameter): Kind {
	const arr = /^(.*)\[(\d*)\]$/.exec(p.type);
	if (arr) {
		const length = arr[2] === '' ? null : Number(arr[2]);
		if (length !== null && !(length > 0)) throw new Error(`unsupported ABI type ${p.type}`);
		return { k: 'array', of: { ...p, type: arr[1] }, length };
	}
	if (p.type === 'tuple') {
		if (!p.components) throw new Error('tuple without components');
		return { k: 'tuple', components: p.components };
	}
	if (p.type === 'address') return { k: 'address' };
	if (p.type === 'bool') return { k: 'bool' };
	if (p.type === 'bytes') return { k: 'bytes' };
	if (p.type === 'string') return { k: 'string' };
	const uint = /^uint(\d*)$/.exec(p.type);
	if (uint) {
		const bits = uint[1] === '' ? 256 : Number(uint[1]);
		if (bits >= 8 && bits <= 256 && bits % 8 === 0) return { k: 'uint', bits };
	}
	const fixed = /^bytes(\d+)$/.exec(p.type);
	if (fixed) {
		const size = Number(fixed[1]);
		if (size >= 1 && size <= 32) return { k: 'fixedBytes', size };
	}
	throw new Error(`unsupported ABI type ${p.type}`);
}

function isDynamic(p: AbiParameter): boolean {
	const kind = kindOf(p);
	switch (kind.k) {
		case 'bytes':
		case 'string':
			return true;
		case 'array':
			return kind.length === null || isDynamic(kind.of);
		case 'tuple':
			return kind.components.some(isDynamic);
		default:
			return false;
	}
}

/** Bytes a static parameter occupies in the head. */
function staticSize(p: AbiParameter): number {
	const kind = kindOf(p);
	if (kind.k === 'array' && kind.length !== null) return kind.length * staticSize(kind.of);
	if (kind.k === 'tuple') return kind.components.reduce((n, c) => n + staticSize(c), 0);
	return 32;
}

function headSize(p: AbiParameter): number {
	return isDynamic(p) ? 32 : staticSize(p);
}

/** The canonical type as it appears in a function signature. */
function canonicalType(p: AbiParameter): string {
	if (p.type.startsWith('tuple')) {
		return `(${(p.components ?? []).map(canonicalType).join(',')})${p.type.slice(5)}`;
	}
	return p.type === 'uint' ? 'uint256' : p.type;
}

export function functionSignature(fn: AbiFunction): string {
	return `${fn.name}(${fn.inputs.map(canonicalType).join(',')})`;
}

export function functionSelector(fn: AbiFunction): Hex {
	return keccak256(new TextEncoder().encode(functionSignature(fn))).slice(0, 10) as Hex;
}

// ── Encoding ─────────────────────────────────────────────────────────────────

function word(value: bigint): Uint8Array {
	const b = bigintToBytes(value);
	if (b.length > 32) throw new Error('integer wider than 32 bytes');
	const out = new Uint8Array(32);
	out.set(b, 32 - b.length);
	return out;
}

function padRight(bytes: Uint8Array): Uint8Array {
	const out = new Uint8Array(Math.ceil(bytes.length / 32) * 32);
	out.set(bytes);
	return out;
}

function toBigint(value: unknown): bigint {
	if (typeof value === 'bigint') return value;
	if (typeof value === 'number' && Number.isSafeInteger(value)) return BigInt(value);
	throw new Error(`not an integer: ${String(value)}`);
}

function encodeValue(p: AbiParameter, value: unknown): Uint8Array {
	const kind = kindOf(p);
	switch (kind.k) {
		case 'uint': {
			const v = toBigint(value);
			if (v < 0n || v >> BigInt(kind.bits) !== 0n) throw new Error(`${p.type} out of range`);
			return word(v);
		}
		case 'bool':
			if (typeof value !== 'boolean') throw new Error('bool expected');
			return word(value ? 1n : 0n);
		case 'address': {
			const b = hexToBytes(getAddress(String(value)));
			const out = new Uint8Array(32);
			out.set(b, 12);
			return out;
		}
		case 'fixedBytes': {
			const b = hexToBytes(String(value));
			if (b.length !== kind.size) throw new Error(`${p.type} expects ${kind.size} bytes`);
			return padRight(b);
		}
		case 'bytes':
		case 'string': {
			const b =
				kind.k === 'string' ? new TextEncoder().encode(String(value)) : hexToBytes(String(value));
			return concatBytes(word(BigInt(b.length)), padRight(b));
		}
		case 'array': {
			if (!Array.isArray(value)) throw new Error(`${p.type} expects an array`);
			if (kind.length !== null && value.length !== kind.length) {
				throw new Error(`${p.type} expects ${kind.length} items`);
			}
			const body = encodeParameters(
				value.map(() => kind.of),
				value
			);
			return kind.length === null ? concatBytes(word(BigInt(value.length)), body) : body;
		}
		case 'tuple': {
			const values = Array.isArray(value)
				? value
				: kind.components.map((c) => (value as Record<string, unknown>)[c.name ?? '']);
			return encodeParameters(kind.components, values);
		}
	}
}

function encodeParameters(params: readonly AbiParameter[], values: readonly unknown[]): Uint8Array {
	if (params.length !== values.length) {
		throw new Error(`expected ${params.length} values, got ${values.length}`);
	}
	let tailOffset = params.reduce((n, p) => n + headSize(p), 0);
	const heads: Uint8Array[] = [];
	const tails: Uint8Array[] = [];
	params.forEach((p, i) => {
		const encoded = encodeValue(p, values[i]);
		if (isDynamic(p)) {
			heads.push(word(BigInt(tailOffset)));
			tails.push(encoded);
			tailOffset += encoded.length;
		} else {
			heads.push(encoded);
		}
	});
	return concatBytes(...heads, ...tails);
}

export function encodeAbiParameters(
	params: readonly AbiParameter[],
	values: readonly unknown[]
): Hex {
	return bytesToHex(encodeParameters(params, values));
}

// ── Decoding ─────────────────────────────────────────────────────────────────

/**
 * Reads with a budget. An honest encoding is read at most once per byte —
 * each head word, each length, each tail — so reading more than the data's
 * own size means offsets alias one another: a hostile answer built to make a
 * few kilobytes decode into gigabytes. That is refused, not decoded.
 */
class Reader {
	private budget: number;

	constructor(readonly data: Uint8Array) {
		this.budget = data.length;
	}

	private spend(n: number): void {
		this.budget -= n;
		if (this.budget < 0) throw new Error('ABI data reads its own bytes more than once');
	}

	word(pos: number): Uint8Array {
		if (pos < 0 || pos + 32 > this.data.length) throw new Error('ABI data out of bounds');
		this.spend(32);
		return this.data.subarray(pos, pos + 32);
	}

	/** An offset or a length: must point inside the data. */
	size(pos: number): number {
		const v = bytesToBigint(this.word(pos));
		if (v > BigInt(this.data.length)) throw new Error('ABI offset out of bounds');
		return Number(v);
	}

	slice(pos: number, len: number): Uint8Array {
		if (pos + len > this.data.length) throw new Error('ABI data out of bounds');
		this.spend(len);
		return this.data.subarray(pos, pos + len);
	}
}

function allZero(bytes: Uint8Array): boolean {
	return bytes.every((b) => b === 0);
}

function decodeValue(r: Reader, p: AbiParameter, pos: number): unknown {
	const kind = kindOf(p);
	switch (kind.k) {
		case 'uint': {
			const v = bytesToBigint(r.word(pos));
			if (v >> BigInt(kind.bits) !== 0n) throw new Error(`${p.type} has dirty high bits`);
			return kind.bits <= 48 ? Number(v) : v;
		}
		case 'bool': {
			const v = bytesToBigint(r.word(pos));
			if (v > 1n) throw new Error('bool is not 0 or 1');
			return v === 1n;
		}
		case 'address': {
			const w = r.word(pos);
			if (!allZero(w.subarray(0, 12))) throw new Error('address has dirty high bits');
			return getAddress(bytesToHex(w.subarray(12)));
		}
		case 'fixedBytes': {
			const w = r.word(pos);
			if (!allZero(w.subarray(kind.size))) throw new Error(`${p.type} has dirty low bits`);
			return bytesToHex(w.subarray(0, kind.size));
		}
		case 'bytes':
		case 'string': {
			const len = r.size(pos);
			const b = r.slice(pos + 32, len);
			return kind.k === 'string' ? new TextDecoder().decode(b) : bytesToHex(b);
		}
		case 'array': {
			if (kind.length !== null) {
				return decodeParameters(r, Array(kind.length).fill(kind.of), pos);
			}
			const len = r.size(pos);
			// Every element takes at least one head word: a length the data
			// cannot hold is refused before anything is allocated for it.
			if (len * 32 > r.data.length - pos - 32) throw new Error('ABI array length out of bounds');
			return decodeParameters(r, Array(len).fill(kind.of), pos + 32);
		}
		case 'tuple': {
			const values = decodeParameters(r, kind.components, pos);
			if (kind.components.every((c) => c.name)) {
				return Object.fromEntries(kind.components.map((c, i) => [c.name, values[i]]));
			}
			return values;
		}
	}
}

function decodeParameters(r: Reader, params: readonly AbiParameter[], base: number): unknown[] {
	const out: unknown[] = [];
	let head = base;
	for (const p of params) {
		if (isDynamic(p)) {
			out.push(decodeValue(r, p, base + r.size(head)));
			head += 32;
		} else {
			out.push(decodeValue(r, p, head));
			head += staticSize(p);
		}
	}
	return out;
}

export function decodeAbiParameters(params: readonly AbiParameter[], data: Hex): unknown[] {
	return decodeParameters(new Reader(hexToBytes(data)), params, 0);
}

// ── Functions ────────────────────────────────────────────────────────────────

function findFunction(abi: Abi, name: string): AbiFunction {
	const fn = abi.find((f) => f.type === 'function' && f.name === name);
	if (!fn) throw new Error(`function ${name} not in ABI`);
	return fn;
}

export function encodeFunctionData(
	abi: Abi,
	functionName: string,
	args: readonly unknown[] = []
): Hex {
	const fn = findFunction(abi, functionName);
	return `${functionSelector(fn)}${encodeAbiParameters(fn.inputs, args).slice(2)}` as Hex;
}

/** A call's return data: the single output as itself, several as an array. */
export function decodeFunctionResult(abi: Abi, functionName: string, data: Hex): unknown {
	const fn = findFunction(abi, functionName);
	if (fn.outputs.length > 0 && (data === '0x' || !data)) {
		throw new Error(`${functionName} returned no data (no contract at the address?)`);
	}
	const values = decodeAbiParameters(fn.outputs, data);
	return fn.outputs.length === 1 ? values[0] : values;
}

/** Decode calldata against the ABI's functions, matched by selector. */
export function decodeFunctionData(abi: Abi, data: Hex): { functionName: string; args: unknown[] } {
	const selector = data.slice(0, 10).toLowerCase();
	const fn = abi.find((f) => f.type === 'function' && functionSelector(f) === selector);
	if (!fn) throw new Error(`no function with selector ${selector}`);
	return { functionName: fn.name, args: decodeAbiParameters(fn.inputs, `0x${data.slice(10)}`) };
}
