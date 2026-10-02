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
import jsQR from 'jsqr';
import { beforeAll, describe, expect, it } from 'vitest';
import { loadCore, PaymentRequestCore } from '$lib/core/client';
import type { PaymentRequestEvent } from '$lib/core/generated/PaymentRequestEvent';
import type { PaymentRequestView } from '$lib/core/generated/PaymentRequestView';
import { encodeQr } from '$lib/wallet/qr';
import {
	CAMERA_FRAME_WIDTH,
	TRANSFORMS,
	ZBAR_SIZES,
	canvasFromFile,
	decodeImage
} from './qr-decode';
import receiveCodeOn from './__fixtures__/receive-code-on.png?url';
import receiveCodeOff from './__fixtures__/receive-code-off.png?url';

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

/** The receive code's value, from the real core: the bare address, or the URI for Polygon. */
function coreCode(includeNetwork: boolean): string {
	const me = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
	const core = new PaymentRequestCore();
	try {
		const events: PaymentRequestEvent[] = [
			{ type: 'start', account: me, recipient: me, base_url: 'https://getvela.app/pay' },
			{
				type: 'asset_picked',
				chain_id: 137,
				token_address: null,
				symbol: 'POL',
				decimals: 18,
				network_name: 'Polygon'
			},
			{ type: 'include_network_changed', include: includeNetwork }
		];
		let view: PaymentRequestView | null = null;
		for (const event of events) {
			view = (JSON.parse(core.dispatch(JSON.stringify(event))) as { view: PaymentRequestView })
				.view;
		}
		return view!.qr_value;
	} finally {
		core.free();
	}
}

/**
 * Spec 090: Vela reads its OWN receive code from a picture — the code cut from
 * a 1080×2400 Android screenshot (modules ~24 px across), as a person would
 * pick it from the album after a chat app passed it along.
 *
 * jsQR on the picture as it is finds nothing (asserted): its local binarizer
 * window sits inside one module. `decodeImage` reads it because its own
 * measured photo ladder shrinks first (zbar at 1200/1000/800/600/400 wide,
 * then jsQR) — the same cure as the core's `still_qr_sizes`, which Android
 * climbs; the web needs no second ladder.
 */
describe('Vela reads its own receive code from a screenshot (spec 090)', () => {
	beforeAll(() => loadCore());

	for (const [name, url, includeNetwork] of [
		['network on', receiveCodeOn, true],
		['bare address', receiveCodeOff, false]
	] as const) {
		it(name, async () => {
			const canvas = await canvasFromFile(await (await fetch(url)).blob());
			const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height);
			expect(
				jsQR(pixels.data, canvas.width, canvas.height, { inversionAttempts: 'attemptBoth' })
			).toBeNull();
			expect(await decodeImage(canvas)).toBe(coreCode(includeNetwork));
		});
	}
});
