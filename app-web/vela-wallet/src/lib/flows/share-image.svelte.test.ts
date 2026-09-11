/**
 * 保存图片 produces a picture that still says the address (spec 028 Phase 9,
 * T488). The composed card is asserted as text; the rasterised card is
 * DECODED — the code has to survive the identicon in its centre, the 2×
 * draw and the PNG, or the image is decoration.
 *
 * A `.svelte.test.ts`: the browser project, because a canvas is the
 * browser's to draw.
 */
import jsQR from 'jsqr';
import { describe, expect, it } from 'vitest';
import '$lib/tokens/tokens.css';
import { encodeQr } from '$lib/wallet/qr';
import { composeShareSvg, renderShareCanvas } from './share-image';
import type { ShareCardModel } from './model';

const ADDRESS = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
const model: ShareCardModel = {
	headline: 'Scan to send me tokens',
	code: encodeQr(ADDRESS),
	name: 'My Wallet',
	lines: [ADDRESS.slice(0, 21), ADDRESS.slice(21)],
	networkNote: 'Ethereum and 11 more networks',
	networkMark: { ticker: 'ETH', badgeColor: 'rgb(90, 124, 246)' },
	identiconSvg:
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="30" fill="currentColor"/></svg>',
	wordmark: 'Vela Wallet'
};

const palette = {
	accent: 'rgb(232, 87, 42)',
	onAccent: 'rgb(255, 255, 255)',
	ink: 'rgb(26, 26, 24)',
	border: 'rgb(236, 235, 228)',
	mark: 'rgb(110, 107, 98)'
};

describe('the share image', () => {
	it('is composed of the address, the code, the identicon and the wordmark', () => {
		const svg = composeShareSvg(model, palette);
		expect(svg).toContain(model.lines[0]);
		expect(svg).toContain(model.lines[1]);
		expect(svg).toContain(`d="${model.code!.path}"`);
		expect(svg).toContain('<circle cx="32" cy="32"');
		expect(svg).toContain('Vela Wallet');
		expect(svg).toContain(model.networkNote);
	});

	it('clips the identicon to a circle in the card’s own space and wears the app icon', () => {
		const svg = composeShareSvg(model, palette);
		// The clip group wraps the nested artwork; a clip-path ON the nested
		// <svg> would be read in its 64-unit space and blank the centre.
		expect(svg).toMatch(/<g clip-path="url\(#identicon-clip\)"><svg [^>]*viewBox="0 0 64 64"/);
		// The canonical app icon (docs/design/icon/app-icon.svg), not the in-app sailboat.
		expect(svg).toContain('fill="#f46d50"');
		expect(svg).not.toContain('#ff6a45');
	});

	it('embeds a fetched network logo, and falls back to the lettered disc without one', () => {
		const withLogo = composeShareSvg(model, palette, '', 'data:image/png;base64,AAAA');
		expect(withLogo).toContain('<image href="data:image/png;base64,AAAA"');
		expect(withLogo).not.toContain('>ETH</text>');
		const without = composeShareSvg(model, palette);
		expect(without).toContain('>ETH</text>');
	});

	it('rasterises to a picture whose code decodes to the address', async () => {
		const canvas = await renderShareCanvas(model);
		expect(canvas.width).toBeGreaterThan(canvas.height / 2);
		const ctx = canvas.getContext('2d')!;
		const { data, width, height } = ctx.getImageData(0, 0, canvas.width, canvas.height);
		const decoded = jsQR(data, width, height)?.data ?? null;
		expect(decoded, 'the picture must BE the address, not resemble one').toBe(ADDRESS);
	});
});
