// What is SENT of a screenshot (078 round 3), in a real browser: a JPEG this
// device encoded — never the picked file — so a photo's EXIF (where and when
// it was taken) cannot ride along; the longest edge at most 1920.
import { describe, expect, it } from 'vitest';
import { MAX_SCREENSHOT_BYTES } from './bug-report';
import { MAX_EDGE, prepareScreenshot } from './screenshot-prep';

/** A canvas-drawn image of the given size, as `type`. */
async function drawn(width: number, height: number, type: string): Promise<Blob> {
	const canvas = new OffscreenCanvas(width, height);
	const context = canvas.getContext('2d') as OffscreenCanvasRenderingContext2D;
	const gradient = context.createLinearGradient(0, 0, width, height);
	gradient.addColorStop(0, '#e8572a');
	gradient.addColorStop(1, '#141412');
	context.fillStyle = gradient;
	context.fillRect(0, 0, width, height);
	return canvas.convertToBlob({ type, quality: 0.9 });
}

/** A JPEG with an EXIF APP1 segment spliced in after SOI — a phone photo's shape. */
async function withExif(width: number, height: number): Promise<Blob> {
	const jpeg = new Uint8Array(await (await drawn(width, height, 'image/jpeg')).arrayBuffer());
	const exifBody = new TextEncoder().encode('Exif\0\0GPS 51.5007N 0.1246W secret-location');
	const length = exifBody.length + 2;
	const app1 = new Uint8Array([0xff, 0xe1, length >> 8, length & 0xff, ...exifBody]);
	return new Blob([jpeg.subarray(0, 2), app1, jpeg.subarray(2)], { type: 'image/jpeg' });
}

/** The JPEG marker segments before the image data, by marker byte. */
function markers(bytes: Uint8Array): number[] {
	const found: number[] = [];
	let i = 2;
	while (i + 4 <= bytes.length && bytes[i] === 0xff) {
		const marker = bytes[i + 1];
		found.push(marker);
		if (marker === 0xda) break; // start of scan: the image data follows
		i += 2 + ((bytes[i + 2] << 8) | bytes[i + 3]);
	}
	return found;
}

const decode = (base64: string) => Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));

describe('prepareScreenshot', () => {
	it('sends a JPEG with no EXIF, even when the picked file carried one', async () => {
		const source = await withExif(640, 480);
		const sourceBytes = new Uint8Array(await source.arrayBuffer());
		expect(markers(sourceBytes)).toContain(0xe1);
		const prepared = await prepareScreenshot(source);
		const bytes = decode(prepared.base64);
		expect([...bytes.subarray(0, 3)]).toEqual([0xff, 0xd8, 0xff]);
		expect(markers(bytes)).not.toContain(0xe1);
		expect(new TextDecoder('latin1').decode(bytes)).not.toContain('secret-location');
	});

	it('re-encodes a PNG as a JPEG too — the round trip is the rule, not a size fix', async () => {
		const prepared = await prepareScreenshot(await drawn(300, 200, 'image/png'));
		expect([...decode(prepared.base64).subarray(0, 3)]).toEqual([0xff, 0xd8, 0xff]);
		expect(prepared.blob.type).toBe('image/jpeg');
		expect([prepared.width, prepared.height]).toEqual([300, 200]);
	});

	it('scales the longest edge down to 1920, and never up', async () => {
		const big = await prepareScreenshot(await drawn(3000, 1500, 'image/png'));
		expect(Math.max(big.width, big.height)).toBe(MAX_EDGE);
		const bitmap = await createImageBitmap(big.blob);
		expect([bitmap.width, bitmap.height]).toEqual([1920, 960]);
		expect(big.blob.size).toBeLessThanOrEqual(MAX_SCREENSHOT_BYTES);
	});

	it('refuses what the browser cannot decode — never sends it raw', async () => {
		const notAnImage = new Blob([new TextEncoder().encode('%PDF-1.4 not an image')], {
			type: 'image/heic'
		});
		await expect(prepareScreenshot(notAnImage)).rejects.toBeDefined();
	});
});
