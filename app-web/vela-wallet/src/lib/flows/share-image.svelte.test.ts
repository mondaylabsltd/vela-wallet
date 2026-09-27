/**
 * 保存图片 produces a picture that still says the address (spec 028 Phase 9,
 * T488; recomposed 2026-09-27 after the WeChat card). The composed card is
 * asserted as text; the rasterised card is DECODED — the code has to survive
 * the network logo in its centre, the 2× draw and the PNG, or the image is
 * decoration.
 *
 * A `.svelte.test.ts`: the browser project, because a canvas is the
 * browser's to draw.
 *
 * `VITE_SHARE_CARD_REVIEW=<dir>` also writes the real export for a spread of
 * languages, networks and names into `<dir>` (relative to this file) — the
 * card is a picture, and a picture is reviewed by eye.
 */
import jsQR from 'jsqr';
import { commands } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import '$lib/tokens/tokens.css';
import { encodeShareQr } from '$lib/wallet/qr';
import {
	CARD_PALETTE,
	composeShareSvg,
	estimateWidth,
	fitHeadline,
	renderShareCanvas,
	SHARE_CARD
} from './share-image';
import type { ShareCardModel } from './model';

const ADDRESS = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
const model: ShareCardModel = {
	headline: '扫码向我转账',
	code: encodeShareQr(ADDRESS),
	name: '大表哥',
	lines: [ADDRESS.slice(0, 21), ADDRESS.slice(21)],
	networkNote: '仅支持 Ethereum 网络付款',
	networkMark: { ticker: 'ETH', badgeColor: 'rgb(98, 126, 234)' },
	identiconSvg:
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="30" fill="currentColor"/></svg>',
	wordmark: 'Vela Wallet'
};

const palette = { paper: 'rgb(255, 255, 255)', ink: 'rgb(26, 26, 24)' };

function decode(canvas: HTMLCanvasElement): string | null {
	const { data, width, height } = canvas
		.getContext('2d')!
		.getImageData(0, 0, canvas.width, canvas.height);
	return jsQR(data, width, height)?.data ?? null;
}

describe('the share image', () => {
	it('holds the token layer’s paper and ink', () => {
		// The card keeps these as values (the gallery draws it at prerender
		// time); they must still BE the tokens.
		const probe = document.createElement('div');
		document.body.appendChild(probe);
		const resolve = (token: string) => {
			probe.style.color = `var(${token})`;
			return getComputedStyle(probe).color;
		};
		const paper = resolve('--color-onAccent');
		const ink = resolve('--color-fixed-shadowInk');
		probe.style.color = CARD_PALETTE.paper;
		expect(getComputedStyle(probe).color).toBe(paper);
		probe.style.color = CARD_PALETTE.ink;
		expect(getComputedStyle(probe).color).toBe(ink);
		probe.remove();
	});

	it('is composed of the address, the code, the identicon, the network and the wordmark', () => {
		const svg = composeShareSvg(model, palette);
		expect(svg).toContain(model.lines[0]);
		expect(svg).toContain(model.lines[1]);
		expect(svg).toContain(`d="${model.code!.path}"`);
		expect(svg).toContain('<circle cx="32" cy="32"');
		expect(svg).toContain('Vela Wallet');
		expect(svg).toContain(model.networkNote);
		expect(svg).toContain(model.headline);
	});

	it('encodes at level H, so the logo plate has room', () => {
		// A plain address is 29 modules at M and 37 at H.
		expect(model.code!.modules).toBe(37);
	});

	it('wears the app icon on the icon’s own orange, and the field dips at the centre', () => {
		const svg = composeShareSvg(model, palette);
		expect(svg).toContain('fill="#f46d50"');
		expect(svg).not.toContain('#ff6a45');
		expect(svg).not.toContain('#E8572A');
		// The field's lower edge: a quadratic whose control point sits BELOW
		// the edge, so the orange bulges down into the white foot.
		const curve = /V([\d.]+) Q[\d.]+,([\d.]+) 0,([\d.]+) Z/.exec(svg);
		expect(curve).not.toBeNull();
		expect(Number(curve![2])).toBeGreaterThan(Number(curve![1]));
	});

	it('clips the identicon to a circle in the card’s own space, beside the address', () => {
		const svg = composeShareSvg(model, palette);
		// The clip group wraps the nested artwork; a clip-path ON the nested
		// <svg> would be read in its 64-unit space and blank it.
		expect(svg).toMatch(/<g clip-path="url\(#identicon-clip\)"><svg [^>]*viewBox="0 0 64 64"/);
		// The identicon is left of the address text, not in the code.
		const identiconX = Number(
			/<g clip-path="url\(#identicon-clip\)"><svg x="([\d.]+)"/.exec(svg)![1]
		);
		const addressX = Number(
			new RegExp(`<text x="([\\d.]+)"[^>]*>${model.lines[0]}<`).exec(svg)![1]
		);
		expect(identiconX + SHARE_CARD.identicon).toBeLessThanOrEqual(addressX);
	});

	it('puts the network logo in the code’s centre, and the lettered disc without one', () => {
		const withLogo = composeShareSvg(model, palette, '', 'data:image/png;base64,AAAA');
		const centre = SHARE_CARD.width / 2;
		expect(withLogo).toContain(
			`<image href="data:image/png;base64,AAAA" x="${centre - SHARE_CARD.logo / 2}"`
		);
		expect(withLogo).not.toContain('>ETH</text>');
		const without = composeShareSvg(model, palette);
		expect(without).toContain(`<circle cx="${centre}"`);
		expect(without).toContain('>ETH</text>');
	});

	it('sets a long headline on two lines that each fit, and a short one on one', () => {
		expect(fitHeadline('扫码向我转账', estimateWidth)).toEqual({
			size: 32,
			lines: ['扫码向我转账']
		});
		const long = fitHeadline('Отсканируйте, чтобы отправить мне крипто', estimateWidth);
		expect(long.lines).toHaveLength(2);
		for (const line of long.lines) {
			expect(estimateWidth(line, long.size, 700)).toBeLessThanOrEqual(SHARE_CARD.textWidth);
		}
	});

	it('cuts a name too long for its row, and never the address', () => {
		const svg = composeShareSvg(
			{ ...model, name: 'An account name far too long to sit beside the code' },
			palette
		);
		expect(svg).toContain('…</text>');
		expect(svg).toContain(model.lines[0]);
		expect(svg).toContain(model.lines[1]);
	});

	it('rasterises to a picture whose code decodes to the address', async () => {
		const canvas = await renderShareCanvas(model);
		expect(canvas.width).toBe(SHARE_CARD.width * SHARE_CARD.scale);
		expect(decode(canvas), 'the picture must BE the address, not resemble one').toBe(ADDRESS);
	});
});

const REVIEW_DIR = import.meta.env.VITE_SHARE_CARD_REVIEW as string | undefined;

describe.skipIf(!REVIEW_DIR)('the share image, written out for review', () => {
	const LOGOS = 'https://ethereum-data.getvela.app/chainlogos';
	const cases: { file: string; card: ShareCardModel }[] = [
		{
			file: 'zh-ethereum',
			card: { ...model, networkMark: { ...model.networkMark, logoUrls: [`${LOGOS}/eip155-1.png`] } }
		},
		{
			file: 'en-gnosis-webp',
			card: {
				...model,
				headline: 'Scan to Send Me Crypto',
				name: 'MultiTest',
				networkNote: 'Gnosis payments only',
				networkMark: {
					ticker: 'XDAI',
					badgeColor: 'rgb(0, 163, 144)',
					logoUrls: [`${LOGOS}/eip155-100.png`]
				}
			}
		},
		{
			file: 'ru-long-headline-bnb',
			card: {
				...model,
				headline: 'Отсканируйте, чтобы отправить мне крипто',
				name: 'Основной кошелёк',
				networkNote: 'Только платежи в сети BNB Smart Chain',
				networkMark: {
					ticker: 'BNB',
					badgeColor: 'rgb(240, 185, 11)',
					logoUrls: [`${LOGOS}/eip155-56.png`]
				}
			}
		},
		{
			file: 'de-long-name-tempo',
			card: {
				...model,
				headline: 'Scannen, um mir Krypto zu senden',
				name: 'Gemeinsames Haushaltskonto der Familie',
				networkNote: 'Nur Zahlungen über Tempo',
				networkMark: {
					ticker: 'USD',
					badgeColor: 'rgb(20, 20, 20)',
					logoUrls: [`${LOGOS}/eip155-4217.png`]
				}
			}
		},
		{ file: 'zh-no-logo', card: model }
	];

	it.each(cases)('$file', async ({ file, card }) => {
		let identiconSvg = card.identiconSvg;
		try {
			const { loadOnboardingCore } = await import('$lib/onboarding/core/wasm-client');
			await loadOnboardingCore();
			const { identiconSvgForClient } = await import('$lib/wallet/identicon');
			identiconSvg = identiconSvgForClient(ADDRESS);
		} catch {
			// The placeholder disc stands in; the layout is what is under review.
		}
		const canvas = await renderShareCanvas({ ...card, identiconSvg });
		expect(decode(canvas)).toBe(ADDRESS);
		const png = canvas.toDataURL('image/png').split(',')[1];
		await commands.writeFile(`${REVIEW_DIR}/${file}.png`, png, 'base64');
	});
});
