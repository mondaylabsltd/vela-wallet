/**
 * secp256k1 signing for TESTS ONLY — never imported by the app.
 *
 * Some tests play an outside multisig owner whose wallet signs a Safe hash
 * with an Ethereum key (`r ‖ s ‖ v`, as MetaMask returns it). The app never
 * signs with secp256k1 and the production core deliberately cannot sign at
 * all, and browsers' WebCrypto has no secp256k1, so this is the one place a
 * signature is made: RFC 6979 deterministic ECDSA, low-s, with the recovery
 * id — exactly what @noble/curves' `secp256k1.sign` returned, held to 108
 * signatures recorded from it before it was removed (`secp256k1.vectors.json`,
 * replayed by `secp256k1.test.ts`). The app's own code then verifies these
 * signatures through the core, so a wrong one fails the test it is in.
 *
 * Plain bigint arithmetic, affine points: slow and not constant-time, which a
 * test does not care about. HMAC-SHA256 comes from WebCrypto (browser and
 * Node), hence async.
 */

const P = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2fn;
const N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
const G = {
	x: 0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798n,
	y: 0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8n
};

type Point = { x: bigint; y: bigint } | null;
/** Bytes on a plain ArrayBuffer, which WebCrypto takes. */
type Bytes = Uint8Array<ArrayBuffer>;

const mod = (a: bigint, m: bigint) => ((a % m) + m) % m;

function inverse(a: bigint, m: bigint): bigint {
	let [r0, r1] = [mod(a, m), m];
	let [s0, s1] = [1n, 0n];
	while (r1 !== 0n) {
		const q = r0 / r1;
		[r0, r1] = [r1, r0 - q * r1];
		[s0, s1] = [s1, s0 - q * s1];
	}
	return mod(s0, m);
}

function add(a: Point, b: Point): Point {
	if (!a) return b;
	if (!b) return a;
	if (a.x === b.x && mod(a.y + b.y, P) === 0n) return null;
	const slope =
		a.x === b.x && a.y === b.y
			? mod(3n * a.x * a.x * inverse(2n * a.y, P), P)
			: mod((b.y - a.y) * inverse(b.x - a.x, P), P);
	const x = mod(slope * slope - a.x - b.x, P);
	return { x, y: mod(slope * (a.x - x) - a.y, P) };
}

function multiply(k: bigint, point: Point): Point {
	let result: Point = null;
	let addend = point;
	for (let e = k; e > 0n; e >>= 1n) {
		if (e & 1n) result = add(result, addend);
		addend = add(addend, addend);
	}
	return result;
}

const fromHex = (hex: string): Bytes =>
	Uint8Array.from((hex.replace(/^0x/, '').match(/../g) ?? []).map((b) => parseInt(b, 16)));
const toHex = (bytes: Uint8Array) =>
	Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
const toInt = (bytes: Uint8Array) => BigInt(`0x${toHex(bytes) || '0'}`);
const to32 = (n: bigint): Bytes => fromHex(n.toString(16).padStart(64, '0'));

function concat(...parts: Uint8Array[]): Bytes {
	const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
}

async function hmac(key: Bytes, data: Bytes): Promise<Bytes> {
	const k = await crypto.subtle.importKey('raw', key, { name: 'HMAC', hash: 'SHA-256' }, false, [
		'sign'
	]);
	return new Uint8Array(await crypto.subtle.sign('HMAC', k, data));
}

export interface Secp256k1Signature {
	r: bigint;
	s: bigint;
	/** 0 or 1 (2/3 only if R.x ≥ n, which RFC 6979 makes astronomically rare). */
	recovery: number;
}

/** ECDSA over the 32-byte `hashHex` with `keyHex`: RFC 6979, low-s, with the recovery id. */
export async function sign(hashHex: string, keyHex: string): Promise<Secp256k1Signature> {
	const d = toInt(fromHex(keyHex));
	if (d <= 0n || d >= N) throw new Error('secp256k1 test signer: not a private key');
	const hash = fromHex(hashHex);
	if (hash.length !== 32) throw new Error('secp256k1 test signer: a 32-byte hash');
	const z = mod(toInt(hash), N);
	const x = to32(d);
	const h1 = to32(z);

	// RFC 6979 §3.2, HMAC-SHA256, qlen = 256.
	let v: Bytes = new Uint8Array(32).fill(1);
	let k: Bytes = new Uint8Array(32);
	k = await hmac(k, concat(v, new Uint8Array([0]), x, h1));
	v = await hmac(k, v);
	k = await hmac(k, concat(v, new Uint8Array([1]), x, h1));
	v = await hmac(k, v);
	for (;;) {
		v = await hmac(k, v);
		const nonce = toInt(v);
		if (nonce >= 1n && nonce < N) {
			const R = multiply(nonce, G);
			if (R) {
				const r = mod(R.x, N);
				let s = mod(inverse(nonce, N) * (z + r * d), N);
				if (r !== 0n && s !== 0n) {
					let recovery = (R.y & 1n ? 1 : 0) | (R.x >= N ? 2 : 0);
					if (s > N >> 1n) {
						s = N - s;
						recovery ^= 1;
					}
					return { r, s, recovery };
				}
			}
		}
		k = await hmac(k, concat(v, new Uint8Array([0])));
		v = await hmac(k, v);
	}
}

/** `0x r ‖ s ‖ v` with v = 27 + recovery — what an Ethereum wallet returns. */
export async function signEthereum(hashHex: string, keyHex: string): Promise<string> {
	const { r, s, recovery } = await sign(hashHex, keyHex);
	return `0x${toHex(to32(r))}${toHex(to32(s))}${(27 + recovery).toString(16)}`;
}
