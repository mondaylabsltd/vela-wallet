/**
 * The round trip (spec 028 T410 — SC-401).
 *
 * This file is the whole point of the feature. Spec 021's placeholder shipped
 * onto a live receive screen because nothing ever asked the rendered code to be
 * READ — "a QR appeared" is exactly the assertion a decorative pattern passes.
 *
 * So these do not check that a code was produced. They rasterise what the app
 * would draw and hand it to an INDEPENDENT decoder (`jsqr`, which knows nothing
 * about the encoder), and demand the original text back.
 */
import jsQR from 'jsqr';
import { describe, expect, it } from 'vitest';
import { buildQrPath, encodeQr } from './qr';

const ADDRESS = '0xD400866e00B055B20752a826CD5C89b811de130b';
/** An EIP-681 request with an amount, an asset and a chain — the dense case. */
const PAYMENT_LINK =
	'ethereum:0xD400866e00B055B20752a826CD5C89b811de130b@100/transfer' +
	'?address=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48&uint256=1000000';

/**
 * Draw the code the way the card draws it — one path over dark modules — and
 * hand back pixels. The quiet zone is deliberate: a code rendered flush to its
 * own edge is one a decoder legitimately refuses, and the card must not.
 */
function rasterise(text: string, scale = 8, quietModules = 4) {
	const { modules, path } = encodeQr(text);
	const dark = new Set<string>();
	// Replay the path's `Mx yh<run>` runs, which is what a renderer draws.
	for (const [, x, y, run] of path.matchAll(/M(\d+) (\d+)h(\d+)/g)) {
		for (let i = 0; i < Number(run); i++) dark.add(`${Number(x) + i},${y}`);
	}
	const side = (modules + quietModules * 2) * scale;
	const pixels = new Uint8ClampedArray(side * side * 4).fill(255);
	for (let y = 0; y < modules; y++) {
		for (let x = 0; x < modules; x++) {
			if (!dark.has(`${x},${y}`)) continue;
			for (let dy = 0; dy < scale; dy++) {
				for (let dx = 0; dx < scale; dx++) {
					const px = (x + quietModules) * scale + dx;
					const py = (y + quietModules) * scale + dy;
					const at = (py * side + px) * 4;
					pixels[at] = pixels[at + 1] = pixels[at + 2] = 0;
				}
			}
		}
	}
	return { pixels, side, modules };
}

const readBack = (text: string) => {
	const { pixels, side } = rasterise(text);
	return jsQR(pixels, side, side)?.data ?? null;
};

describe('what the receive card renders can be read back', () => {
	it('returns the address, exactly', () => {
		expect(readBack(ADDRESS)).toBe(ADDRESS);
	});

	it('carries a payment request whole — amount, asset and chain survive', () => {
		expect(readBack(PAYMENT_LINK)).toBe(PAYMENT_LINK);
	});

	it('is a real code at the size the card was drawn for', () => {
		// 021 chose 29 modules for the receive card. A plain address encodes to
		// exactly that at error-correction M, so the drawn geometry was picked
		// against a real code rather than a pattern.
		expect(encodeQr(ADDRESS).modules).toBe(29);
	});

	it('grows for a denser payload instead of truncating it', () => {
		// The card is a fixed 344px, so a payment link is a smaller module — but
		// it must still be the WHOLE link, which the round trip above proves.
		expect(encodeQr(PAYMENT_LINK).modules).toBeGreaterThan(29);
	});
});

describe('the path the card draws', () => {
	it('merges a run of dark modules into one horizontal stroke', () => {
		// Four darks in a row become `h4`, not four rects. Per-cell rendering
		// leaves hairline gridlines from pixel rounding, and a code with
		// gridlines photographs badly — which is how most people read it.
		const row = [0, 1, 1, 1, 1];
		expect(buildQrPath(row, 5)).toBe('M1 0h4v1h-4z');
	});

	it('draws nothing for a blank matrix', () => {
		expect(buildQrPath([0, 0, 0, 0], 2)).toBe('');
	});
});
