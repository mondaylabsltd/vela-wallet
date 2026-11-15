/**
 * rust/vela-site-crypto — Keccak-256 and secp256k1 (k256) — behind its plain C
 * ABI. The module is committed (rust/pkg-web-site-crypto, built by
 * `node rust/scripts/build-web-site.mjs site-crypto`).
 *
 * Loading is asynchronous and happens once: `loadEvm()` fetches the module in
 * a browser (Vite fingerprints the `new URL(…, import.meta.url)` below into an
 * asset) and reads it from disk under Node or Bun. Everything after that is
 * synchronous. Calling a hash or key function before it has loaded throws —
 * a clear failure, never a wrong answer.
 */

interface CryptoExports {
	memory: WebAssembly.Memory;
	sc_alloc(len: number): number;
	sc_free(ptr: number, len: number): void;
	sc_keccak256(input: number, len: number, out: number): void;
	sc_address(key: number, out: number): number;
	sc_sign(key: number, hash: number, out: number): number;
}

const WASM_URL = new URL(
	'../../../../../rust/pkg-web-site-crypto/vela_site_crypto.wasm',
	import.meta.url
);

let exports: CryptoExports | null = null;
let loading: Promise<void> | null = null;

async function readModule(url: URL): Promise<BufferSource> {
	if (url.protocol === 'file:') {
		// Node / Bun (tests, scripts). Named through a variable so the browser
		// bundle never tries to resolve a Node built-in.
		const fsName = 'node:fs/promises';
		const fs = (await import(/* @vite-ignore */ fsName)) as typeof import('node:fs/promises');
		return new Uint8Array(await fs.readFile(url));
	}
	const response = await fetch(url);
	if (!response.ok) throw new Error(`vela_site_crypto.wasm: HTTP ${response.status}`);
	return response.arrayBuffer();
}

/** Instantiate from bytes already in hand. */
export async function instantiateEvm(bytes: BufferSource): Promise<void> {
	const { instance } = await WebAssembly.instantiate(bytes, {});
	exports = instance.exports as unknown as CryptoExports;
}

/** Load the module once; later calls return the same promise. */
export function loadEvm(): Promise<void> {
	if (exports) return Promise.resolve();
	loading ??= readModule(WASM_URL)
		.then(instantiateEvm)
		.catch((error) => {
			loading = null;
			throw error;
		});
	return loading;
}

export function evmLoaded(): boolean {
	return exports !== null;
}

function wasm(): CryptoExports {
	if (!exports) throw new Error('vela_site_crypto.wasm is not loaded yet (await loadEvm() first)');
	return exports;
}

/** Copy `data` into the module, run `f`, then wipe and free everything. */
function withBuffers<T>(
	inputs: Uint8Array[],
	outLen: number,
	f: (e: CryptoExports, ptrs: number[], out: number) => T
): { result: T; out: Uint8Array } {
	const e = wasm();
	const ptrs = inputs.map((b) => {
		const ptr = e.sc_alloc(Math.max(1, b.length));
		new Uint8Array(e.memory.buffer, ptr, b.length).set(b);
		return ptr;
	});
	const outPtr = e.sc_alloc(outLen);
	try {
		const result = f(e, ptrs, outPtr);
		// Allocating may have grown memory: view the buffer only now.
		const out = new Uint8Array(e.memory.buffer, outPtr, outLen).slice();
		return { result, out };
	} finally {
		// Secret keys pass through these buffers: zero them before freeing.
		inputs.forEach((b, i) => {
			new Uint8Array(e.memory.buffer, ptrs[i], b.length).fill(0);
			e.sc_free(ptrs[i], Math.max(1, b.length));
		});
		new Uint8Array(e.memory.buffer, outPtr, outLen).fill(0);
		e.sc_free(outPtr, outLen);
	}
}

export function keccak256Wasm(data: Uint8Array): Uint8Array {
	return withBuffers([data], 32, (e, [input], out) => e.sc_keccak256(input, data.length, out)).out;
}

/** The address of a 32-byte secret key, or `null` when it is not a valid key. */
export function secretKeyAddress(secret: Uint8Array): Uint8Array | null {
	if (secret.length !== 32) return null;
	const { result, out } = withBuffers([secret], 20, (e, [key], o) => e.sc_address(key, o));
	return result === 1 ? out : null;
}

/** `r ‖ s ‖ recovery id` (65 bytes), RFC 6979, low-s; `null` for an invalid key. */
export function signPrehash(secret: Uint8Array, hash: Uint8Array): Uint8Array | null {
	if (secret.length !== 32 || hash.length !== 32) return null;
	const { result, out } = withBuffers([secret, hash], 65, (e, [key, h], o) => e.sc_sign(key, h, o));
	return result === 1 ? out : null;
}
