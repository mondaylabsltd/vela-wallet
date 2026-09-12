/**
 * The decoder reads what the encoder wrote (spec 028 T424).
 *
 * Runs in a real browser, because everything here is browser machinery: a
 * canvas, `createImageBitmap`, a wasm decoder. A node test could only assert
 * the shape of the ladder; this asserts that the ladder WORKS.
 *
 * The pair matters more than either half. Phase 2 proved the card renders a
 * code; this proves the app can read one. Together they are the round trip a
 * person actually performs — someone shows a code, someone else scans it.
 */
import { describe, expect, it } from 'vitest';
import { encodeQr } from '$lib/wallet/qr';
import { CAMERA_FRAME_WIDTH, TRANSFORMS, ZBAR_SIZES, decodeImage } from './qr-decode';

const ADDRESS = '0xD400866e00B055B20752a826CD5C89b811de130b';

/** Draw a code the way the receive card draws it, with its quiet zone. */
function render(text: string, scale = 8, quiet = 4): HTMLCanvasElement {
	const { modules, path } = encodeQr(text);
	const side = (modules + quiet * 2) * scale;
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = side;
	const ctx = canvas.getContext('2d')!;
	ctx.fillStyle = '#fff';
	ctx.fillRect(0, 0, side, side);
	ctx.fillStyle = '#000';
	ctx.translate(quiet * scale, quiet * scale);
	ctx.scale(scale, scale);
	ctx.fill(new Path2D(path));
	return canvas;
}

describe('a rendered code can be read back', () => {
	it('decodes an address from an image the way a picked screenshot would', async () => {
		expect(await decodeImage(render(ADDRESS))).toBe(ADDRESS);
	});

	it('decodes a payment request whole', async () => {
		const link =
			'ethereum:0xD400866e00B055B20752a826CD5C89b811de130b@100/transfer' +
			'?address=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48&uint256=1000000';
		expect(await decodeImage(render(link))).toBe(link);
	});

	it('finds nothing in an image with no code, instead of inventing something', async () => {
		const blank = document.createElement('canvas');
		blank.width = blank.height = 400;
		const ctx = blank.getContext('2d')!;
		ctx.fillStyle = '#fff';
		ctx.fillRect(0, 0, 400, 400);
		expect(await decodeImage(blank)).toBeNull();
	});
});

describe('the ladder is the one that was measured', () => {
	it('descends from 1200 wide, the size a photo decodes at', () => {
		// `docs/qr-scanner-web.md`: a canvas downscale is a low-pass filter, and
		// 1200 wide is where JPEG noise and moiré are gone but about five pixels
		// per module remain. Changing this order is changing a measurement.
		expect([...ZBAR_SIZES]).toEqual([1200, 1000, 800, 600, 400]);
	});

	it('decodes a camera frame at 1000 wide, not at the sensor’s size', () => {
		expect(CAMERA_FRAME_WIDTH).toBe(1000);
	});

	it('inverts, because a code on a dark screen is the same code', () => {
		const pixels = new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 255]);
		TRANSFORMS.INVERT(pixels);
		expect([...pixels]).toEqual([255, 255, 255, 255, 0, 0, 0, 255]);
	});

	it('binarises on luminance, so a photographed grey becomes black or white', () => {
		// Mid grey below the threshold goes black; above it goes white.
		const dark = new Uint8ClampedArray([100, 100, 100, 255]);
		TRANSFORMS.BINARIZE(160)(dark);
		expect(dark[0]).toBe(0);
		const light = new Uint8ClampedArray([200, 200, 200, 255]);
		TRANSFORMS.BINARIZE(160)(light);
		expect(light[0]).toBe(255);
	});
});
