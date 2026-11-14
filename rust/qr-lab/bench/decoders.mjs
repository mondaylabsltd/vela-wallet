// The contestants, each behind one shape: decode(gray, w, h) -> string | null.
//
// zbar and jsqr are the exact versions the web app shipped (bench/package.json); the Rust ones are this lab's wasm shims.
// Everything runs as wasm/JS in the same V8, so times compare like for like.

import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
export const LAB = path.resolve(here, '..');
export const APP = path.resolve(LAB, '../../app-web/vela-wallet');
// The baseline decoders come from the lab's own package (bench/package.json):
// the exact versions the web app shipped before vela-qr.
const appRequire = createRequire(path.join(here, 'package.json'));

const zbar = await import(appRequire.resolve('@undecaf/zbar-wasm').replace(/main\.cjs$/, 'main.mjs'));
const jsQR = appRequire('jsqr');

/** A gray buffer as the RGBA a canvas would hand over (built outside any timing). */
export function toRgba(gray) {
	const out = new Uint8ClampedArray(gray.length * 4);
	for (let i = 0, j = 0; i < gray.length; i++, j += 4) {
		out[j] = out[j + 1] = out[j + 2] = gray[i];
		out[j + 3] = 255;
	}
	return out;
}

export const zbarDecoder = {
	name: 'zbar',
	/** Times the FIRST symbol zbar reported was not a QR code (qr-decode.ts takes found[0] as is). */
	nonQrFirst: [],
	async decode(gray, w, h) {
		// scanGrayBuffer copies into zbar's heap — the same copy scanImageData makes.
		const buf = gray.slice().buffer;
		const found = await zbar.scanGrayBuffer(buf, w, h);
		if (found.length === 0) return null;
		if (found[0].typeName !== 'ZBAR_QRCODE') this.nonQrFirst.push(`${found[0].typeName}:${found[0].decode().slice(0, 40)}`);
		return found[0].decode();
	}
};

export const jsqrDecoder = {
	name: 'jsqr',
	/** jsqr inverts itself (`attemptBoth`), so the harness never inverts for it. */
	invertsItself: true,
	needsRgba: true,
	async decode(rgba, w, h) {
		return jsQR(rgba, w, h, { inversionAttempts: 'attemptBoth' })?.data ?? null;
	},
	raw(rgba, w, h) {
		return jsQR(rgba, w, h, { inversionAttempts: 'attemptBoth' })?.data ?? null;
	}
};

function wasmDecoder(name, file) {
	const bytes = readFileSync(file);
	const module = new WebAssembly.Module(bytes);
	let inst = new WebAssembly.Instance(module, {});
	const utf8 = new TextDecoder();
	return {
		name,
		bytes: bytes.length,
		crashes: 0,
		async decode(gray, w, h) {
			const e = inst.exports;
			try {
				const ptr = e.qr_alloc(gray.length);
				new Uint8Array(e.memory.buffer, ptr, gray.length).set(gray);
				const n = e.qr_decode(ptr, w, h);
				const text = n >= 0 ? utf8.decode(new Uint8Array(e.memory.buffer, e.qr_result_ptr(), n)) : null;
				e.qr_free(ptr, gray.length);
				return text;
			} catch (err) {
				// A trap (panic=abort): the instance's heap may be mid-update, so
				// start a fresh one. Counted — a decoder that dies on input is a finding.
				this.crashes++;
				inst = new WebAssembly.Instance(module, {});
				return null;
			}
		}
	};
}

/** Every wasm-opt'd shim in `dir` (out/wasm/shim_<name>.wasm), sorted. */
export function rustDecoders(dir, only) {
	return readdirSync(dir)
		.filter((f) => /^shim_[a-z0-9_]+\.wasm$/.test(f))
		.sort()
		.map((f) => wasmDecoder(f.slice(5, -5).replace(/_/g, '-'), path.join(dir, f)))
		.filter((d) => !only || only.includes(d.name));
}

// The web's jsQR fallback transforms, verbatim from src/lib/services/qr-decode.ts.
export const INVERT = (d) => {
	for (let i = 0; i < d.length; i += 4) {
		d[i] = 255 - d[i];
		d[i + 1] = 255 - d[i + 1];
		d[i + 2] = 255 - d[i + 2];
	}
};
export const BINARIZE = (t) => (d) => {
	for (let i = 0; i < d.length; i += 4) {
		const lum = (d[i] * 77 + d[i + 1] * 150 + d[i + 2] * 29) >> 8;
		const v = lum <= t ? 0 : 255;
		d[i] = d[i + 1] = d[i + 2] = v;
	}
};
export const BIN_INVERT = (t) => (d) => {
	for (let i = 0; i < d.length; i += 4) {
		const lum = (d[i] * 77 + d[i + 1] * 150 + d[i + 2] * 29) >> 8;
		const v = lum <= t ? 255 : 0;
		d[i] = d[i + 1] = d[i + 2] = v;
	}
};
