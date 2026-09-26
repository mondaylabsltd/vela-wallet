/**
 * Screenshots for a bug report: which files are taken, and what is sent of
 * each (078 round 3; the founder ruled screenshots PUBLIC — they are shown
 * inline in the GitHub issue).
 *
 * ## What is sent is never the file that was picked
 *
 * Every image is decoded and drawn onto a canvas, then encoded again as a
 * JPEG. That round trip is the privacy guarantee, not an optimisation: a
 * photo's EXIF carries where and when it was taken, and a canvas carries no
 * metadata at all — so what leaves the device is pixels and nothing else,
 * even for a PNG that was already small. The pixels are the person's own
 * choice, and the sheet says, before Send, that they will be public.
 *
 * ## The steps, in order
 *
 * 1. decode (`createImageBitmap`, which decodes off the main thread; a file
 *    the browser cannot decode — HEIC in most of them — is refused, never
 *    sent raw);
 * 2. scale so the longest edge is at most {@link MAX_EDGE} — never up;
 * 3. encode JPEG at quality {@link QUALITY};
 * 4. if that is still over the endpoint's per-image cap, encode at
 *    {@link QUALITY_SMALLER}; then at a {@link SMALLER_EDGE} edge.
 */
import { MAX_SCREENSHOT_BYTES, MAX_SCREENSHOTS } from './bug-report';

export const MAX_EDGE = 1920;
export const SMALLER_EDGE = 1440;
export const QUALITY = 0.85;
export const QUALITY_SMALLER = 0.7;

/** What the file picker offers; the decoder is still the judge. */
export const SCREENSHOT_ACCEPT = 'image/png,image/jpeg,image/webp,image/*';

/** One image, ready to send: the JPEG it became, and a URL for its tile. */
export interface PreparedScreenshot {
	/** Plain base64 of the JPEG bytes — no `data:` prefix, no line breaks. */
	base64: string;
	/** The JPEG itself, for the tile (`URL.createObjectURL`). */
	blob: Blob;
	width: number;
	height: number;
}

/** Why a file was not taken. */
export type ScreenshotRefusal = 'limit' | 'unsupported';

/**
 * Which of `files` to take, given `current` already attached.
 *
 * Only images are candidates (a PDF or a text file is refused at once); of
 * those, the first `max − current` are taken in the order given and the rest
 * dropped with `limit`. `unsupported` wins the notice when both happened — it
 * is the one the person can do something about with the same files.
 */
export function chooseScreenshots(
	current: number,
	files: readonly File[],
	max: number = MAX_SCREENSHOTS
): { take: File[]; refusal: ScreenshotRefusal | null } {
	const images = files.filter((file) => file.type === '' || file.type.startsWith('image/'));
	const room = Math.max(0, max - current);
	const take = images.slice(0, room);
	const refusal: ScreenshotRefusal | null =
		images.length < files.length ? 'unsupported' : images.length > room ? 'limit' : null;
	return { take, refusal };
}

/** The size an image is drawn at: its longest edge at most `edge`, never larger. */
export function fitWithin(width: number, height: number, edge: number): [number, number] {
	const longest = Math.max(width, height);
	if (longest <= edge) return [width, height];
	const scale = edge / longest;
	return [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
}

/** Standard base64, padded, no line breaks — what the endpoint decodes. */
export function toBase64(bytes: Uint8Array): string {
	let binary = '';
	const CHUNK = 0x8000;
	for (let i = 0; i < bytes.length; i += CHUNK) {
		binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
	}
	return btoa(binary);
}

async function encode(
	bitmap: ImageBitmap,
	edge: number,
	quality: number
): Promise<{ blob: Blob; width: number; height: number }> {
	const [width, height] = fitWithin(bitmap.width, bitmap.height, edge);
	if (typeof OffscreenCanvas !== 'undefined') {
		const canvas = new OffscreenCanvas(width, height);
		const context = canvas.getContext('2d');
		if (!context) throw new Error('no 2d context');
		context.drawImage(bitmap, 0, 0, width, height);
		return { blob: await canvas.convertToBlob({ type: 'image/jpeg', quality }), width, height };
	}
	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('no 2d context');
	context.drawImage(bitmap, 0, 0, width, height);
	const blob = await new Promise<Blob | null>((resolve) =>
		canvas.toBlob(resolve, 'image/jpeg', quality)
	);
	if (!blob) throw new Error('encode failed');
	return { blob, width, height };
}

/**
 * Decode, scale and re-encode one image (see the module doc). Rejects when the
 * browser cannot decode it — the caller says `unsupported`, and the file is
 * never sent as it was.
 */
export async function prepareScreenshot(file: Blob): Promise<PreparedScreenshot> {
	const bitmap = await createImageBitmap(file);
	try {
		let result = await encode(bitmap, MAX_EDGE, QUALITY);
		if (result.blob.size > MAX_SCREENSHOT_BYTES) {
			result = await encode(bitmap, MAX_EDGE, QUALITY_SMALLER);
		}
		if (result.blob.size > MAX_SCREENSHOT_BYTES) {
			result = await encode(bitmap, SMALLER_EDGE, QUALITY_SMALLER);
		}
		const bytes = new Uint8Array(await result.blob.arrayBuffer());
		return { base64: toBase64(bytes), ...result };
	} finally {
		bitmap.close();
	}
}

/** Images on a clipboard or in a drop, as files. */
export function imageFilesOf(data: DataTransfer | null): File[] {
	if (!data) return [];
	const files: File[] = [];
	for (const item of Array.from(data.items ?? [])) {
		if (item.kind !== 'file') continue;
		const file = item.getAsFile();
		if (file) files.push(file);
	}
	if (files.length === 0 && data.files) files.push(...Array.from(data.files));
	return files;
}
