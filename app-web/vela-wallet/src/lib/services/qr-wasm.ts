/**
 * vela-qr (rust/vela-qr) — this repository's QR decoder — behind its plain C
 * ABI (rust/vela-qr/wasm). Environment-agnostic: the browser scanner
 * (`qr-decode.ts`) fetches the module, Node tests read it from disk; both hand
 * the bytes to {@link instantiateQr}.
 *
 * What a code *says* is decided here only as far as text goes: segments the
 * crate decoded (ASCII, UTF-8, …) are taken as they are, segments in a CJK or
 * legacy code page are decoded with the platform's `TextDecoder` — the tables
 * live in the browser, not in our module. What the text *means* is the core's
 * (`scan_resolved`).
 */

interface QrExports {
	memory: WebAssembly.Memory;
	qr_alloc(len: number): number;
	qr_free(ptr: number, len: number): void;
	qr_read_rgba(ptr: number, w: number, h: number): number;
	qr_read_luma(ptr: number, w: number, h: number): number;
	qr_result_ptr(): number;
}

/** One instantiated decoder. Re-instantiated from `module` if it ever traps. */
export interface QrDecoder {
	module: WebAssembly.Module;
	exports: QrExports;
}

/** What one read found. */
export type QrRead =
	/** The whole message, as text. */
	| { kind: 'text'; text: string }
	/**
	 * One symbol of a structured-append sequence: never the whole message, so
	 * never handed on as if it were. `text` is this part's, when readable.
	 */
	| { kind: 'part'; index: number; count: number; text: string | null }
	/** A code, but not text this platform can read (binary data, an unknown charset). */
	| { kind: 'unreadable' };

export async function instantiateQr(bytes: BufferSource): Promise<QrDecoder> {
	const module = await WebAssembly.compile(bytes);
	const instance = await WebAssembly.instantiate(module, {});
	return { module, exports: instance.exports as unknown as QrExports };
}

/**
 * The first code in an image, or `null` for none. `format` says what the
 * pixels are: RGBA (a canvas's `ImageData`) or one byte of luminance each.
 */
export function readQr(
	decoder: QrDecoder,
	pixels: Uint8Array | Uint8ClampedArray,
	width: number,
	height: number,
	format: 'rgba' | 'luma'
): QrRead | null {
	const expected = width * height * (format === 'rgba' ? 4 : 1);
	if (width <= 0 || height <= 0 || pixels.length !== expected) return null;
	const e = decoder.exports;
	try {
		const ptr = e.qr_alloc(expected);
		// Allocating may grow memory: view the buffer only after it.
		new Uint8Array(e.memory.buffer, ptr, expected).set(pixels);
		const n = (format === 'rgba' ? e.qr_read_rgba : e.qr_read_luma)(ptr, width, height);
		const result = n > 0 ? new Uint8Array(e.memory.buffer, e.qr_result_ptr(), n).slice() : null;
		e.qr_free(ptr, expected);
		return result ? parseQrResult(result) : null;
	} catch {
		// A trap leaves the instance's heap mid-update: start a fresh one. The
		// frame is lost, not the scanner.
		decoder.exports = new WebAssembly.Instance(decoder.module, {}).exports as unknown as QrExports;
		return null;
	}
}

/** The result format of `rust/vela-qr/wasm/src/lib.rs`. Exported for its unit. */
export function parseQrResult(bytes: Uint8Array): QrRead {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	let at = 0;
	const u8 = () => bytes[at++];
	const u32 = () => {
		const v = view.getUint32(at, true);
		at += 4;
		return v;
	};
	const take = (n: number) => {
		const s = bytes.subarray(at, at + n);
		at += n;
		return s;
	};
	const utf8 = new TextDecoder();

	if (u8() !== 1) return { kind: 'unreadable' };
	const part = u8() === 1 ? { index: u8(), count: u8() } : null;
	const texts: (string | null)[] = [];
	for (let segments = u32(); segments > 0; segments--) {
		const candidates: ({ text: string } | { label: string })[] = [];
		for (let n = u8(); n > 0; n--) {
			if (u8() === 0) candidates.push({ text: utf8.decode(take(u32())) });
			else candidates.push({ label: utf8.decode(take(u8())) });
		}
		const raw = take(u32());
		texts.push(segmentText(candidates, raw));
	}
	const text = texts.every((t) => t !== null) ? texts.join('') : null;
	if (part) return { kind: 'part', index: part.index, count: part.count, text };
	return text === null ? { kind: 'unreadable' } : { kind: 'text', text };
}

/** The first candidate that decodes the bytes strictly. */
function segmentText(
	candidates: ({ text: string } | { label: string })[],
	raw: Uint8Array
): string | null {
	for (const c of candidates) {
		if ('text' in c) return c.text;
		try {
			return new TextDecoder(c.label, { fatal: true }).decode(raw);
		} catch {
			// Not valid in that charset, or a label this runtime lacks: the next candidate.
		}
	}
	return null;
}
