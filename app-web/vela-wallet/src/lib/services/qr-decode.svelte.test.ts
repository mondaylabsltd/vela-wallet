/**
 * The decoder reads what the encoder wrote (spec 028 T424).
 *
 * Runs in a real browser, because everything here is browser machinery: a
 * canvas, `createImageBitmap`, the vela-qr wasm module fetched by URL.
 *
 * The pair matters more than either half. Phase 2 proved the card renders a
 * code; this proves the app can read one. Together they are the round trip a
 * person actually performs — someone shows a code, someone else scans it.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { loadCore, PaymentRequestCore } from '$lib/core/client';
import type { PaymentRequestEvent } from '$lib/core/generated/PaymentRequestEvent';
import type { PaymentRequestView } from '$lib/core/generated/PaymentRequestView';
import { encodeQr } from '$lib/wallet/qr';
import {
	CAMERA_FRAME_WIDTH,
	STILL_MAX_WIDTH,
	canvasFromFile,
	decodeCameraFrame,
	decodeImage
} from './qr-decode';
import receiveCodeOn from './__fixtures__/receive-code-on.png?url';
import receiveCodeOff from './__fixtures__/receive-code-off.png?url';

const ADDRESS = '0xD400866e00B055B20752a826CD5C89b811de130b';

// `encodeQr` is the core's encoder.
beforeAll(() => loadCore());

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

describe('the sizes are the ones that were measured', () => {
	it('decodes a camera frame at 1000 wide, not at the sensor’s size', () => {
		expect(CAMERA_FRAME_WIDTH).toBe(1000);
	});

	it('reads a picked image at most 1200 wide, once', () => {
		// rust/qr-lab: one read at ≤ 1200 wide loses 2 of 572 images against the
		// old five-size ladder and cuts the slowest read from 94 ms to 25 ms.
		expect(STILL_MAX_WIDTH).toBe(1200);
	});
});

describe('what a camera and an album hand over', () => {
	it('reads a code shown light-on-dark', async () => {
		const canvas = render(ADDRESS);
		const ctx = canvas.getContext('2d')!;
		// `render` left its module-unit transform on the context.
		ctx.setTransform(1, 0, 0, 1, 0, 0);
		ctx.globalCompositeOperation = 'difference';
		ctx.fillStyle = '#fff';
		ctx.fillRect(0, 0, canvas.width, canvas.height);
		expect(await decodeImage(canvas)).toBe(ADDRESS);
	});

	it('reads a camera frame: a code in a wider, larger picture', async () => {
		const code = render(ADDRESS, 6);
		const frame = document.createElement('canvas');
		frame.width = 1920;
		frame.height = 1080;
		const ctx = frame.getContext('2d')!;
		ctx.fillStyle = '#888';
		ctx.fillRect(0, 0, frame.width, frame.height);
		ctx.drawImage(code, 700, 250);
		expect(await decodeCameraFrame(frame)).toBe(ADDRESS);
	});

	it('reads a photo wider than 1200 by drawing it down first', async () => {
		const code = render(ADDRESS, 30);
		const photo = document.createElement('canvas');
		photo.width = 3024;
		photo.height = 4032;
		const ctx = photo.getContext('2d')!;
		ctx.fillStyle = '#fff';
		ctx.fillRect(0, 0, photo.width, photo.height);
		ctx.drawImage(code, 600, 1200);
		expect(await decodeImage(photo)).toBe(ADDRESS);
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
 * Its modules are wider than a binarizer's local window — what made jsQR,
 * the old fallback, find nothing here. vela-qr reads it because it halves the
 * image until the modules fit — the same cure as the core's `still_qr_sizes`,
 * which Android climbs.
 */
describe('Vela reads its own receive code from a screenshot (spec 090)', () => {
	beforeAll(() => loadCore());

	for (const [name, url, includeNetwork] of [
		['network on', receiveCodeOn, true],
		['bare address', receiveCodeOff, false]
	] as const) {
		it(name, async () => {
			const canvas = await canvasFromFile(await (await fetch(url)).blob());
			expect(await decodeImage(canvas)).toBe(coreCode(includeNetwork));
		});
	}
});
