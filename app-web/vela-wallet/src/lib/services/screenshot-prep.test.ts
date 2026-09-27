// The rules for WHICH screenshots are taken (078 round 3). The image work
// itself needs a real browser: `screenshot-prep.svelte.test.ts`.
import { describe, expect, it } from 'vitest';
import { chooseScreenshots, fitWithin, toBase64 } from './screenshot-prep';

const file = (name: string, type: string) => new File([new Uint8Array([1, 2, 3])], name, { type });
const png = (n: number) => Array.from({ length: n }, (_, i) => file(`s${i}.png`, 'image/png'));

describe('chooseScreenshots — at most five, images only', () => {
	it('takes everything while there is room', () => {
		const files = png(3);
		expect(chooseScreenshots(0, files)).toEqual({ take: files, refusal: null });
	});

	it('takes the first that fit, in order, and says why the rest were not', () => {
		const files = png(4);
		const { take, refusal } = chooseScreenshots(3, files);
		expect(take).toEqual(files.slice(0, 2));
		expect(refusal).toBe('limit');
		expect(chooseScreenshots(5, png(1))).toEqual({ take: [], refusal: 'limit' });
	});

	it('refuses what is not an image, and still takes the images beside it', () => {
		const pdf = file('doc.pdf', 'application/pdf');
		const [a, b] = png(2);
		expect(chooseScreenshots(0, [a, pdf, b])).toEqual({ take: [a, b], refusal: 'unsupported' });
	});

	it('leaves an untyped file to the decoder rather than guessing', () => {
		const untyped = file('shot', '');
		expect(chooseScreenshots(0, [untyped])).toEqual({ take: [untyped], refusal: null });
	});
});

describe('fitWithin — the longest edge at most 1920, never larger', () => {
	it('scales a large image down, keeping its shape', () => {
		expect(fitWithin(4000, 3000, 1920)).toEqual([1920, 1440]);
		expect(fitWithin(1170, 2532, 1920)).toEqual([887, 1920]);
	});

	it('never scales a small one up', () => {
		expect(fitWithin(800, 600, 1920)).toEqual([800, 600]);
	});
});

describe('toBase64 — standard alphabet, padded, no line breaks', () => {
	it('round-trips any bytes, including large ones', () => {
		const bytes = new Uint8Array(100_000).map((_, i) => (i * 31) % 256);
		const encoded = toBase64(bytes);
		expect(encoded).not.toMatch(/[\s-_]/);
		expect(encoded.length % 4).toBe(0);
		expect(Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0))).toEqual(bytes);
	});
});
