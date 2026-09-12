/**
 * Reading a QR code from a camera frame or a picked image (spec 028 T420).
 *
 * Ported from src/components/QRScanner.tsx @ 28d25ae9 — the WEB half of it, and
 * the preprocessing ladder is the whole point. `docs/qr-scanner-web.md` recorded
 * what it cost to find:
 *
 *   - **iOS Safari has no `BarcodeDetector`**, so the platform most likely to be
 *     scanning a wallet's code cannot use the browser's own decoder;
 *   - **jsQR alone cannot read a real camera frame or photo.** It is fine on a
 *     clean digital screenshot and fails on JPEG noise, moiré and phone lighting;
 *   - **zbar compiled to WASM works, but only after a downscale.** A canvas
 *     `drawImage` downscale IS a low-pass filter: it smooths the noise and moiré
 *     and sharpens the code's edges. 1200 wide for a photo, 1000 for a video
 *     frame — about five pixels per module, where speed and accuracy meet.
 *
 * Both decoders are LAZY: nothing here is fetched until a scanner opens, which
 * is what keeps 239 KB of wasm and 257 KB of javascript off every other page.
 *
 * One deliberate divergence from Expo: it loads zbar from a CDN, because Metro
 * cannot import a module that uses `import.meta`. Vite can, and this repo does
 * not fetch code from third parties at runtime — the same rule that made the
 * launch animations local assets (spec 028 D45).
 *
 * `image-decode.ts` is NOT ported. It is a pure-JS JPEG decoder that exists
 * because native has no canvas; a browser decodes images itself.
 */

/** jsQR reads a code either way round; a wallet code may be shown inverted. */
const JSQR_OPTS = { inversionAttempts: 'attemptBoth' as const };

// ---------------------------------------------------------------------------
// Pixel transforms — the jsQR fallback's only tools
// ---------------------------------------------------------------------------

const INVERT = (d: Uint8ClampedArray): void => {
	for (let i = 0; i < d.length; i += 4) {
		d[i] = 255 - d[i];
		d[i + 1] = 255 - d[i + 1];
		d[i + 2] = 255 - d[i + 2];
	}
};

/** Luminance thresholds use the integer weights the Expo port used. */
const BINARIZE =
	(t: number) =>
	(d: Uint8ClampedArray): void => {
		for (let i = 0; i < d.length; i += 4) {
			const lum = (d[i] * 77 + d[i + 1] * 150 + d[i + 2] * 29) >> 8;
			const v = lum <= t ? 0 : 255;
			d[i] = d[i + 1] = d[i + 2] = v;
		}
	};

const BIN_INVERT =
	(t: number) =>
	(d: Uint8ClampedArray): void => {
		for (let i = 0; i < d.length; i += 4) {
			const lum = (d[i] * 77 + d[i + 1] * 150 + d[i + 2] * 29) >> 8;
			const v = lum <= t ? 255 : 0;
			d[i] = d[i + 1] = d[i + 2] = v;
		}
	};

// ---------------------------------------------------------------------------
// Reusable canvases — a scanner runs this per frame
// ---------------------------------------------------------------------------

let canvasA: HTMLCanvasElement | null = null;
let canvasB: HTMLCanvasElement | null = null;

function scratch(slot: 'A' | 'B'): HTMLCanvasElement {
	if (slot === 'A') return (canvasA ??= document.createElement('canvas'));
	return (canvasB ??= document.createElement('canvas'));
}

function context(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
	const ctx = canvas.getContext('2d', { willReadFrequently: true });
	if (!ctx) throw new Error('no 2d context');
	return ctx;
}

/** Downscale — which is also the low-pass filter the ladder depends on. */
function drawScaled(
	src: CanvasImageSource & { width: number; height: number },
	targetW: number,
	slot: 'A' | 'B' = 'A'
): HTMLCanvasElement {
	const c = scratch(slot);
	c.width = targetW;
	c.height = Math.round((targetW * src.height) / src.width);
	context(c).drawImage(src, 0, 0, c.width, c.height);
	return c;
}

// ---------------------------------------------------------------------------
// The decoders, loaded on first use
// ---------------------------------------------------------------------------

type ZbarScan = (data: ImageData) => Promise<{ decode(): string }[]>;
let zbarScan: ZbarScan | null = null;
let zbarLoading: Promise<void> | null = null;

function loadZbar(): Promise<void> {
	if (zbarScan) return Promise.resolve();
	return (zbarLoading ??= import('@undecaf/zbar-wasm')
		.then((m) => {
			zbarScan = m.scanImageData as unknown as ZbarScan;
		})
		.catch(() => {
			// A missing decoder is not fatal: jsQR still reads a clean screenshot,
			// and the caller reports "nothing found" rather than an error nobody
			// can act on.
			zbarScan = null;
		}));
}

type JsQrFn = (
	data: Uint8ClampedArray,
	width: number,
	height: number,
	options?: { inversionAttempts: string }
) => { data: string } | null;
let jsQr: JsQrFn | null = null;
let jsQrLoading: Promise<void> | null = null;

function loadJsQr(): Promise<void> {
	if (jsQr) return Promise.resolve();
	return (jsQrLoading ??= import('jsqr').then((m) => {
		jsQr = (m.default ?? m) as unknown as JsQrFn;
	}));
}

/** Warm both decoders. Called when a scanner opens, never before. */
export async function loadDecoders(): Promise<void> {
	await Promise.all([loadZbar(), loadJsQr()]);
}

// ---------------------------------------------------------------------------
// The ladder
// ---------------------------------------------------------------------------

async function tryZbar(canvas: HTMLCanvasElement): Promise<string | null> {
	if (!zbarScan) return null;
	const image = context(canvas).getImageData(0, 0, canvas.width, canvas.height);
	try {
		const found = await zbarScan(image);
		if (found.length > 0) return found[0].decode();
	} catch {
		/* a decoder that throws on one frame is a frame, not a failure */
	}
	// A code shown on a dark screen is the same code inverted.
	INVERT(image.data);
	try {
		const found = await zbarScan(image);
		if (found.length > 0) return found[0].decode();
	} catch {
		/* as above */
	}
	return null;
}

function tryJsQr(
	canvas: HTMLCanvasElement,
	targetW: number,
	transform: (d: Uint8ClampedArray) => void
): string | null {
	if (!jsQr) return null;
	const c = drawScaled(canvas, Math.min(canvas.width, targetW), 'B');
	const image = context(c).getImageData(0, 0, c.width, c.height);
	transform(image.data);
	return jsQr(image.data, c.width, c.height, JSQR_OPTS)?.data ?? null;
}

/**
 * One camera frame — fast, because this runs continuously.
 *
 * `cropFactor` in (0, 1] takes a centred slice and upscales it to about a thousand wide: the
 * digital zoom for browsers with no hardware zoom, iOS Safari among them.
 */
export async function decodeCameraFrame(
	canvas: HTMLCanvasElement,
	cropFactor = 1
): Promise<string | null> {
	await loadZbar();
	if (cropFactor >= 0.999) return tryZbar(drawScaled(canvas, 1000));
	const cw = Math.max(1, Math.round(canvas.width * cropFactor));
	const ch = Math.max(1, Math.round(canvas.height * cropFactor));
	const cx = (canvas.width - cw) >> 1;
	const cy = (canvas.height - ch) >> 1;
	const out = scratch('A');
	out.width = 1000;
	out.height = Math.max(1, Math.round((1000 * ch) / cw));
	context(out).drawImage(canvas, cx, cy, cw, ch, 0, 0, out.width, out.height);
	return tryZbar(out);
}

/**
 * A picked image — thorough, because it happens once and a person is waiting
 * for an answer rather than watching a viewfinder.
 *
 * zbar first at every size it likes, then jsQR with the three transforms that
 * rescue a clean screenshot zbar refuses (a screenshot's quiet zone is often
 * cropped away, which binarising or inverting restores).
 */
export async function decodeImage(canvas: HTMLCanvasElement): Promise<string | null> {
	await loadDecoders();
	const width = canvas.width;
	const sizes = [1200, 1000, 800, 600, 400].filter((s) => s < width);

	for (const size of sizes) {
		const found = await tryZbar(drawScaled(canvas, size));
		if (found) return found;
	}
	for (const size of [width, ...sizes]) {
		for (const transform of [BIN_INVERT(160), INVERT, BINARIZE(160)]) {
			const found = tryJsQr(canvas, size, transform);
			if (found) return found;
		}
	}
	return null;
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

/** Exported for the unit that pins the ladder's order and its transforms. */
export const TRANSFORMS = { INVERT, BINARIZE, BIN_INVERT };
export const ZBAR_SIZES = [1200, 1000, 800, 600, 400] as const;
export const CAMERA_FRAME_WIDTH = 1000;
