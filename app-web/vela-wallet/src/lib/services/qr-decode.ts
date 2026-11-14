/**
 * Reading a QR code from a camera frame or a picked image (spec 028 T420).
 *
 * The decoder is vela-qr (rust/vela-qr), this repository's own: a port of
 * zxing-cpp's QR reader with a decode strategy on top — each size read
 * dark-on-light and light-on-dark, halved until too small (which reads
 * modules wider than the binarizer's window and smooths sensor noise), and a
 * small image doubled (modules under 2 px). It replaced `@undecaf/zbar-wasm`
 * + `jsqr`: measured in rust/qr-lab on 572 images, it reads 96.9% of picked
 * images and 97.2% of camera frames (zbar → jsQR: 84.3%; zbar on frames:
 * 86.4%), with no wrong reads, ~4 ms a frame, in ~31 KB of wasm (brotli) —
 * against 165 KB for the two packages.
 *
 * The ladder the old pipeline needed (zbar at 1200/1000/800/600/400 wide, then
 * jsQR with three pixel transforms) is gone: the crate rescales and inverts
 * itself. What is left here is only the canvas work a browser does best:
 * drawing a picture or a frame at a sensible size.
 *
 * LAZY: the module is fetched when a scanner opens, never before — nothing on
 * any other page pays for it (spec 028 D45, e2e/budgets.e2e.ts). It is served
 * from this origin; the repo fetches no code from third parties at runtime.
 */

import wasmUrl from '../../../../../rust/pkg-web-qr/vela_qr.wasm?url';
import { instantiateQr, readQr, type QrDecoder, type QrRead } from './qr-wasm';

/** A camera frame is read at this width — about five pixels a module for a phone-held code. */
export const CAMERA_FRAME_WIDTH = 1000;

/**
 * A picked image wider than this is drawn down to it first: a canvas downscale
 * is a low-pass filter, and the crate halves further itself. Measured: one
 * read at ≤ 1200 wide loses 2 of 572 images against the full old ladder of
 * sizes, and its slowest read drops from 94 ms to 25 ms (rust/qr-lab).
 */
export const STILL_MAX_WIDTH = 1200;

// ---------------------------------------------------------------------------
// The decoder, loaded on first use
// ---------------------------------------------------------------------------

let decoder: QrDecoder | null = null;
let loading: Promise<void> | null = null;

function load(): Promise<void> {
	if (decoder) return Promise.resolve();
	return (loading ??= fetch(wasmUrl)
		.then((response) => {
			if (!response.ok) throw new Error(`vela_qr.wasm: HTTP ${response.status}`);
			return response.arrayBuffer();
		})
		.then(instantiateQr)
		.then((d) => {
			decoder = d;
		})
		.catch(() => {
			// A failed fetch is not fatal and not final: the caller reports
			// "nothing found", and the next scanner to open tries again.
			loading = null;
		}));
}

/** Warm the decoder. Called when a scanner opens, never before. */
export async function loadDecoders(): Promise<void> {
	await load();
}

// ---------------------------------------------------------------------------
// Canvases — a scanner reads one per frame, so they are reused
// ---------------------------------------------------------------------------

let scratch: HTMLCanvasElement | null = null;

function context(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
	const ctx = canvas.getContext('2d', { willReadFrequently: true });
	if (!ctx) throw new Error('no 2d context');
	return ctx;
}

/**
 * The text a read produced, for a caller that acts on one message. One part
 * of a structured-append sequence is not the message, and data that is not
 * text has nothing to act on: both read as "no code".
 */
function message(read: QrRead | null): string | null {
	return read?.kind === 'text' ? read.text : null;
}

function readCanvas(canvas: HTMLCanvasElement): string | null {
	if (!decoder || canvas.width === 0 || canvas.height === 0) return null;
	const image = context(canvas).getImageData(0, 0, canvas.width, canvas.height);
	return message(readQr(decoder, image.data, image.width, image.height, 'rgba'));
}

/**
 * One camera frame — fast, because this runs continuously.
 *
 * `cropFactor` in (0, 1] takes a centred slice and draws it about a thousand
 * wide: the digital zoom for browsers with no hardware zoom, iOS Safari among
 * them.
 */
export async function decodeCameraFrame(
	canvas: HTMLCanvasElement,
	cropFactor = 1
): Promise<string | null> {
	await load();
	if (!decoder || canvas.width === 0 || canvas.height === 0) return null;
	const f = Math.min(1, Math.max(cropFactor, 0.001));
	const cw = Math.max(1, Math.round(canvas.width * f));
	const ch = Math.max(1, Math.round(canvas.height * f));
	const out = (scratch ??= document.createElement('canvas'));
	out.width = CAMERA_FRAME_WIDTH;
	out.height = Math.max(1, Math.round((CAMERA_FRAME_WIDTH * ch) / cw));
	context(out).drawImage(
		canvas,
		(canvas.width - cw) >> 1,
		(canvas.height - ch) >> 1,
		cw,
		ch,
		0,
		0,
		out.width,
		out.height
	);
	return readCanvas(out);
}

/**
 * A picked image — read once, at most {@link STILL_MAX_WIDTH} wide.
 */
export async function decodeImage(canvas: HTMLCanvasElement): Promise<string | null> {
	await load();
	if (!decoder || canvas.width === 0 || canvas.height === 0) return null;
	if (canvas.width <= STILL_MAX_WIDTH) return readCanvas(canvas);
	const out = (scratch ??= document.createElement('canvas'));
	out.width = STILL_MAX_WIDTH;
	out.height = Math.max(1, Math.round((STILL_MAX_WIDTH * canvas.height) / canvas.width));
	context(out).drawImage(canvas, 0, 0, out.width, out.height);
	return readCanvas(out);
}

/**
 * A picked file as a canvas. The browser decodes the image itself, which is why
 * Expo's pure-JS JPEG decoder has no counterpart here.
 */
export async function canvasFromFile(file: Blob): Promise<HTMLCanvasElement> {
	const bitmap = await createImageBitmap(file);
	const c = document.createElement('canvas');
	c.width = bitmap.width;
	c.height = bitmap.height;
	context(c).drawImage(bitmap, 0, 0);
	bitmap.close();
	return c;
}
