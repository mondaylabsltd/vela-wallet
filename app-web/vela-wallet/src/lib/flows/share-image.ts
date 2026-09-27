/**
 * 保存图片 — the receive share card as a PNG (spec 028 Phase 9, T488; redrawn
 * in Phase 10, and again on 2026-09-27 to the WeChat Pay collection card the
 * founder holds it against).
 *
 * The composition, top to bottom: the app icon's orange field with the
 * headline and, under it, the one network this address may be paid on; a
 * white sheet with generous orange all round, holding the code — the
 * NETWORK's logo in its centre, where a payer's eye lands before it scans —
 * and under the code the account itself: its identicon on the left, the name
 * and the whole address in two mono lines beside it; then the field closes
 * over a white foot in one downward curve, and the app icon and the wordmark
 * stand on the white.
 *
 * Three things ride together on purpose (`liveShareCard`): the address in
 * readable text so a person can check it without a scanner, the code so a
 * camera can, and the account's identicon — DERIVED from the address, so a
 * card someone doctored to swap the address carries artwork that no longer
 * matches it. The identicon sits beside the address it is checked against.
 *
 * The code is encoded at level H (`encodeShareQr`): the logo plate covers
 * about 7% of it, and a picture that travels through chat apps is
 * recompressed on the way. Every platform draws this card to the geometry
 * in `SHARE_CARD` — desktop `flows/share_card.rs`, Android
 * `ShareCardArtwork.kt`, iOS `ShareCardArtwork.swift` copy it by hand.
 *
 * A saved image has to be pixels, and pixels come from a document of their
 * own. So the card is composed here as an SVG string with the app's faces
 * embedded, drawn to a canvas at 2× and handed over as a file. The
 * chain logo is fetched and EMBEDDED as a data URI — an SVG drawn through an
 * `<img>` may not reach across origins — and a logo that cannot be fetched
 * falls back to the lettered disc.
 *
 * Audit-whitelisted (tokens.test.ts): `@font-face` needs `font-family:`, and
 * the card is a render product, not product UI. The field is the APP ICON's
 * own orange (founder, 2026-08-15: the icon's #F46D50, not the UI accent), so
 * it is read from the icon asset; paper and ink are the token layer's
 * mode-invariant values (`CARD_PALETTE`).
 *
 * The gallery's R4 (`ShareCard.svelte`) draws this same document inline, so
 * the card on screen and the card saved are one composition.
 */
import jakarta700 from '@fontsource/plus-jakarta-sans/files/plus-jakarta-sans-latin-700-normal.woff2?url';
import jakarta500 from '@fontsource/plus-jakarta-sans/files/plus-jakarta-sans-latin-500-normal.woff2?url';
import plexMono from '@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-400-normal.woff2?url';
import { APP_ICON } from '$lib/ui/brand-mark';
import { saveBlob } from '$lib/services/file-io';
import type { ShareCardModel } from './model';

/**
 * The card's geometry in CSS px at 1× — the one spec all four platforms
 * draw. Proportions are the WeChat card's: a sheet about two thirds of the
 * width, the code about 70% of the sheet, the curve dipping about a fifteenth
 * of the width, and a brand line about half the width.
 */
export const SHARE_CARD = {
	width: 480,
	top: 52,
	/** Headline: one line at 32 when it fits, shrinking to 26, then two lines. */
	headlineSize: 32,
	headlineMinSize: 26,
	headlineLeading: 1.25,
	/** Widest a line of text on the orange may run. */
	textWidth: 400,
	noteGap: 10,
	noteSize: 15,
	noteLine: 20,
	sheetGap: 28,
	sheetWidth: 320,
	sheetRadius: 20,
	sheetPad: 48,
	sheetPadBottom: 40,
	qr: 224,
	/** The white plate the network logo sits on, in the code's centre. */
	plate: 60,
	plateRadius: 16,
	logo: 44,
	identityGap: 26,
	identicon: 48,
	identityTextGap: 12,
	nameSize: 17,
	nameLine: 22,
	addressSize: 12.5,
	addressLine: 17,
	nameAddressGap: 3,
	/** Sheet bottom to where the curve leaves the card's edges. */
	curveGap: 52,
	/** How far the curve dips at the centre. */
	curveDepth: 32,
	/** The curve's lowest point to the card's bottom. */
	foot: 112,
	icon: 52,
	iconGap: 12,
	wordmarkSize: 32,
	/** Rasterised at 2× so the code stays crisp on a phone screen. */
	scale: 2
} as const;

const SANS =
	"'Vela Card Sans', 'Plus Jakarta Sans', 'PingFang SC', 'Hiragino Sans GB', 'Hiragino Sans', 'Apple SD Gothic Neo', 'Microsoft YaHei', 'Malgun Gothic', 'Noto Sans CJK SC', 'Noto Sans SC', system-ui, sans-serif";
const MONO = "'Vela Card Mono', 'IBM Plex Mono', ui-monospace, 'SF Mono', Menlo, monospace";

/** The card's paper and ink. */
export interface Palette {
	paper: string;
	ink: string;
}

/**
 * The token layer's `--color-onAccent` and `--color-fixed-shadowInk` — the
 * same value in both modes, which is why the card may hold them as values: the
 * gallery draws it at prerender time, where there is no computed style to
 * read. The browser test pins these to the token layer.
 */
export const CARD_PALETTE: Palette = { paper: '#FFFFFF', ink: '#1A1A18' };

/** Width of `text` in px, in the card's sans (or mono) at `weight` and `size`. */
export type Measure = (text: string, size: number, weight: number, mono?: boolean) => number;

/**
 * A measure for when there is no canvas (a test, a server): per-character
 * advances close to the embedded faces, wide for CJK. The browser save path
 * measures with a canvas and the real faces.
 */
export const estimateWidth: Measure = (text, size, weight, mono = false) => {
	if (mono) return Array.from(text).length * size * 0.6;
	let em = 0;
	for (const ch of text) {
		const code = ch.codePointAt(0) ?? 0;
		if (code >= 0x2e80) em += 1;
		else if (ch === ' ') em += 0.27;
		else if (/[A-Z]/.test(ch)) em += 0.68;
		else if (/[mw]/.test(ch)) em += 0.86;
		else if (/[iljtf.,'!|:;]/.test(ch)) em += 0.3;
		else em += 0.57;
	}
	return em * size * (weight >= 700 ? 1.04 : 1);
};

/**
 * The headline set to the card: one line at the largest size from 32 down to
 * 26 that fits, else two lines split where the halves come out closest in
 * width (at a space when there is one, anywhere in CJK), shrunk until the
 * longer half fits.
 */
export function fitHeadline(text: string, measure: Measure): { size: number; lines: string[] } {
	const { headlineSize: max, headlineMinSize: min, textWidth } = SHARE_CARD;
	const whole = measure(text, max, 700);
	if (whole <= textWidth) return { size: max, lines: [text] };
	const shrunk = Math.floor((max * textWidth) / whole);
	if (shrunk >= min) return { size: shrunk, lines: [text] };

	const chars = Array.from(text);
	const hasSpace = chars.includes(' ');
	let best: [string, string] = [text, ''];
	let bestWidth = Infinity;
	for (let i = 1; i < chars.length; i++) {
		if (hasSpace && chars[i] !== ' ') continue;
		const first = chars.slice(0, i).join('').trim();
		const second = chars.slice(i).join('').trim();
		if (first === '' || second === '') continue;
		const width = Math.max(measure(first, max, 700), measure(second, max, 700));
		if (width < bestWidth) {
			best = [first, second];
			bestWidth = width;
		}
	}
	const size = Math.min(max, Math.floor((max * textWidth) / bestWidth));
	return { size, lines: best };
}

/** `text` cut to `width` with an ellipsis, or whole when it fits. */
export function truncate(text: string, width: number, fits: (candidate: string) => number): string {
	if (fits(text) <= width) return text;
	const chars = Array.from(text);
	while (chars.length > 1 && fits(`${chars.join('')}…`) > width) chars.pop();
	return `${chars.join('').trimEnd()}…`;
}

function escape(text: string): string {
	return text
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}

/** A nested `<svg>` placed at x/y with a size, from a root `<svg …>` string. */
function place(svg: string, x: number, y: number, size: number): string {
	if (!svg.startsWith('<svg')) return '';
	return svg.replace(/^<svg\b/, `<svg x="${x}" y="${y}" width="${size}" height="${size}"`);
}

/** The baseline that centres a line of `size` text on `centre`. */
function baseline(centre: number, size: number): number {
	return round(centre + size * 0.35);
}

function round(n: number): number {
	return Math.round(n * 100) / 100;
}

/**
 * The card as an SVG document. Pure apart from what `measure` reports, so a
 * test can assert what it says and decode the code it carries. `logo` is the
 * network's logo as a data URI when one could be fetched.
 */
export function composeShareSvg(
	model: ShareCardModel,
	palette: Palette,
	fonts = '',
	logo: string | null = null,
	measure: Measure = estimateWidth
): string {
	const C = SHARE_CARD;
	const W = C.width;
	const field = APP_ICON.plate.fill;
	const { paper, ink } = palette;

	// The orange: headline, then the network it may be paid on.
	const headline = fitHeadline(model.headline, measure);
	const headlineLine = headline.size * C.headlineLeading;
	const headlineText = headline.lines
		.map(
			(line, i) =>
				`<text x="${W / 2}" y="${baseline(C.top + headlineLine * (i + 0.5), headline.size)}" text-anchor="middle" font-family="${SANS}" font-size="${headline.size}" font-weight="700" fill="${paper}">${escape(line)}</text>`
		)
		.join('\n');
	const noteTop = C.top + headlineLine * headline.lines.length + C.noteGap;
	const noteWidth = measure(model.networkNote, C.noteSize, 500);
	const noteSize =
		noteWidth <= C.textWidth
			? C.noteSize
			: Math.max(11, Math.floor((C.noteSize * C.textWidth) / noteWidth));
	const note = `<text x="${W / 2}" y="${baseline(noteTop + C.noteLine / 2, noteSize)}" text-anchor="middle" font-family="${SANS}" font-size="${noteSize}" font-weight="500" fill="${paper}">${escape(model.networkNote)}</text>`;

	// The sheet and its code.
	const sheetX = (W - C.sheetWidth) / 2;
	const sheetY = noteTop + C.noteLine + C.sheetGap;
	const qrX = (W - C.qr) / 2;
	const qrY = sheetY + C.sheetPad;
	const modules = model.code?.modules ?? 0;
	const code =
		model.code === undefined
			? ''
			: `<svg x="${qrX}" y="${qrY}" width="${C.qr}" height="${C.qr}" viewBox="0 0 ${modules} ${modules}" shape-rendering="crispEdges"><path d="${model.code.path}" fill="${ink}"/></svg>`;
	const cx = W / 2;
	const cy = qrY + C.qr / 2;
	const plate = `<rect x="${cx - C.plate / 2}" y="${cy - C.plate / 2}" width="${C.plate}" height="${C.plate}" rx="${C.plateRadius}" fill="${paper}"/>`;
	const r = C.logo / 2;
	const mark =
		logo === null
			? `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${model.networkMark.badgeColor}"/>
<text x="${cx}" y="${baseline(cy, 14)}" text-anchor="middle" font-family="${SANS}" font-size="14" font-weight="700" fill="${paper}">${escape(model.networkMark.ticker)}</text>`
			: `<image href="${logo}" x="${cx - r}" y="${cy - r}" width="${C.logo}" height="${C.logo}" clip-path="url(#network-clip)" preserveAspectRatio="xMidYMid slice"/>
<circle cx="${cx}" cy="${cy}" r="${r - 0.5}" fill="none" stroke="${ink}" stroke-opacity="0.08"/>`;

	// The account: identicon left, name and the whole address beside it.
	const idTop = qrY + C.qr + C.identityGap;
	const textHeight = C.nameLine + C.nameAddressGap + C.addressLine * 2;
	const textRoom = C.qr - C.identicon - C.identityTextGap;
	const name = truncate(model.name, textRoom, (t) => measure(t, C.nameSize, 700));
	const blockWidth =
		C.identicon +
		C.identityTextGap +
		Math.min(
			textRoom,
			Math.max(
				measure(name, C.nameSize, 700),
				...model.lines.map((line) => measure(line, C.addressSize, 400, true))
			)
		);
	const idX = round(cx - blockWidth / 2);
	const idY = idTop + (textHeight - C.identicon) / 2;
	const idCentre = { x: idX + C.identicon / 2, y: idY + C.identicon / 2 };
	// The clip rides on a `<g>` in the CARD's coordinates: put on the nested
	// `<svg>` it is read in the artwork's own 64-unit space and clips
	// everything away.
	const identicon = `<g clip-path="url(#identicon-clip)">${place(model.identiconSvg, idX, idY, C.identicon)}</g>`;
	const textX = idX + C.identicon + C.identityTextGap;
	const nameText = `<text x="${textX}" y="${baseline(idTop + C.nameLine / 2, C.nameSize)}" font-family="${SANS}" font-size="${C.nameSize}" font-weight="700" fill="${ink}">${escape(name)}</text>`;
	const addressTop = idTop + C.nameLine + C.nameAddressGap;
	const addressText = model.lines
		.map(
			(line, i) =>
				`<text x="${textX}" y="${baseline(addressTop + C.addressLine * (i + 0.5), C.addressSize)}" font-family="${MONO}" font-size="${C.addressSize}" fill="${ink}" fill-opacity="0.5">${escape(line)}</text>`
		)
		.join('\n');
	const sheetBottom = idTop + textHeight + C.sheetPadBottom;

	// The field closes over the foot in one curve that dips at the centre —
	// the WeChat card's direction — and the brand stands on the white.
	const edge = sheetBottom + C.curveGap;
	const lowest = edge + C.curveDepth;
	const H = lowest + C.foot;
	const fieldPath = `<path d="M0,0 H${W} V${edge} Q${W / 2},${edge + C.curveDepth * 2} 0,${edge} Z" fill="${field}"/>`;
	const wordmarkWidth = measure(model.wordmark, C.wordmarkSize, 700);
	const brandX = round(W / 2 - (C.icon + C.iconGap + wordmarkWidth) / 2);
	const brandCentre = lowest + C.foot / 2;
	const brandY = brandCentre - C.icon / 2;
	const icon = `<svg x="${brandX}" y="${brandY}" width="${C.icon}" height="${C.icon}" viewBox="${APP_ICON.viewBox}"><rect x="${APP_ICON.plate.x}" y="${APP_ICON.plate.y}" width="${APP_ICON.plate.size}" height="${APP_ICON.plate.size}" rx="${APP_ICON.plate.rx}" fill="${APP_ICON.plate.fill}"/>${APP_ICON.paths.map((path) => `<path d="${path.d}" fill="${path.fill}"/>`).join('')}</svg>`;
	const wordmark = `<text x="${brandX + C.icon + C.iconGap}" y="${baseline(brandCentre, C.wordmarkSize)}" font-family="${SANS}" font-size="${C.wordmarkSize}" font-weight="700" fill="${ink}">${escape(model.wordmark)}</text>`;

	return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<style>${fonts}</style>
<defs>
<clipPath id="identicon-clip"><circle cx="${idCentre.x}" cy="${idCentre.y}" r="${C.identicon / 2}"/></clipPath>
<clipPath id="network-clip"><circle cx="${cx}" cy="${cy}" r="${r}"/></clipPath>
</defs>
<rect width="${W}" height="${H}" fill="${paper}"/>
${fieldPath}
${headlineText}
${note}
<rect x="${sheetX}" y="${sheetY}" width="${C.sheetWidth}" height="${sheetBottom - sheetY}" rx="${C.sheetRadius}" fill="${paper}"/>
${code}
${plate}
${mark}
${identicon}
${nameText}
${addressText}
${icon}
${wordmark}
</svg>`;
}

/** The card's pixel size, read back from a composed document. */
function sizeOf(svg: string): { width: number; height: number } {
	const match = /^<svg [^>]*width="([\d.]+)" height="([\d.]+)"/.exec(svg);
	return { width: Number(match?.[1] ?? SHARE_CARD.width), height: Number(match?.[2] ?? 0) };
}

interface Face {
	family: string;
	url: string;
	weight: number;
}

const FACES: Face[] = [
	{ family: 'Vela Card Sans', url: jakarta700, weight: 700 },
	{ family: 'Vela Card Sans', url: jakarta500, weight: 500 },
	{ family: 'Vela Card Mono', url: plexMono, weight: 400 }
];

/**
 * Each face twice over: as an `@font-face` the SVG document carries (an
 * `<img>` cannot load fonts of its own), and registered with this document
 * so the canvas measures text in the face that will draw it.
 */
async function loadFace(face: Face): Promise<string> {
	try {
		const res = await fetch(face.url);
		if (!res.ok) return '';
		const buffer = await res.arrayBuffer();
		try {
			const font = new FontFace(face.family, buffer, { weight: String(face.weight) });
			document.fonts.add(await font.load());
		} catch {
			// Measured in a fallback face: a line may sit a little off-centre.
		}
		const bytes = new Uint8Array(buffer);
		let binary = '';
		for (let i = 0; i < bytes.length; i += 0x8000) {
			binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
		}
		return `@font-face{font-family:'${face.family}';font-weight:${face.weight};src:url(data:font/woff2;base64,${btoa(binary)}) format('woff2');}`;
	} catch {
		// No face: the system's sans and mono stand in. The card is still the card.
		return '';
	}
}

/** A canvas measure in the faces `loadFace` registered. */
function canvasMeasure(): Measure {
	const ctx = document.createElement('canvas').getContext('2d');
	if (ctx === null) return estimateWidth;
	return (text, size, weight, mono = false) => {
		ctx.font = `${weight} ${size}px ${mono ? MONO : SANS}`;
		return ctx.measureText(text).width;
	};
}

/**
 * The network's logo as bytes the card can carry. One fetch per URL per
 * session; a refusal (no CORS, no logo, no network) is remembered as null,
 * and the lettered disc stands in.
 */
const logoBytes = new Map<string, Promise<string | null>>();

export function logoDataUri(url: string | undefined): Promise<string | null> {
	if (url === undefined || url === '') return Promise.resolve(null);
	let pending = logoBytes.get(url);
	if (pending === undefined) {
		pending = (async () => {
			try {
				const res = await fetch(url, { mode: 'cors' });
				if (!res.ok) return null;
				const blob = await res.blob();
				return await new Promise<string | null>((resolve) => {
					const reader = new FileReader();
					reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : null);
					reader.onerror = () => resolve(null);
					reader.readAsDataURL(blob);
				});
			} catch {
				return null;
			}
		})();
		logoBytes.set(url, pending);
	}
	return pending;
}

/** The card drawn to pixels — its own document, so a canvas can read it. */
export async function renderShareCanvas(model: ShareCardModel): Promise<HTMLCanvasElement> {
	const [faces, logo] = await Promise.all([
		Promise.all(FACES.map(loadFace)),
		logoDataUri(model.networkMark.logoUrls?.[0])
	]);
	const svg = composeShareSvg(model, CARD_PALETTE, faces.join(''), logo, canvasMeasure());
	const { width, height } = sizeOf(svg);
	const image = new Image();
	image.decoding = 'async';
	await new Promise<void>((resolve, reject) => {
		image.onload = () => resolve();
		image.onerror = () => reject(new Error('share image: the card did not render'));
		image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
	});
	const canvas = document.createElement('canvas');
	canvas.width = Math.round(width * SHARE_CARD.scale);
	canvas.height = Math.round(height * SHARE_CARD.scale);
	const ctx = canvas.getContext('2d');
	if (ctx === null) throw new Error('share image: no 2d context');
	ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
	return canvas;
}

/** Compose, rasterise, hand over. Resolves false when the browser refused. */
export async function saveShareImage(model: ShareCardModel, fileName: string): Promise<boolean> {
	try {
		const canvas = await renderShareCanvas(model);
		const blob = await new Promise<Blob | null>((resolve) =>
			canvas.toBlob((result) => resolve(result), 'image/png')
		);
		if (blob === null) return false;
		await saveBlob(fileName, blob);
		return true;
	} catch {
		return false;
	}
}
