/**
 * The social-preview cards, drawn by rust/vela-og (committed as
 * rust/pkg-web-og/vela_og.wasm; `node rust/scripts/build-web-site.mjs og`).
 *
 * Build-time only: vite.config.ts writes one PNG per page into the build and
 * draws them on request under `vite dev`; `bun run og:fonts` cuts the CJK
 * font subsets. Nothing here, and no image renderer, reaches the browser or
 * the Worker.
 */
import { existsSync, readFileSync } from 'node:fs';

const WASM = new URL('../../../../rust/pkg-web-og/vela_og.wasm', import.meta.url);
const FONTS = new URL('./fonts/', import.meta.url);

/**
 * @typedef {{ memory: WebAssembly.Memory, og_alloc(n: number): number, og_free(p: number, n: number): void,
 *   og_font(p: number, n: number, weight: number): number, og_fonts_clear(): void,
 *   og_check(p: number, n: number): number, og_render(p: number, n: number): number,
 *   og_subset(fp: number, fn: number, tp: number, tn: number): number, og_result_ptr(): number }} OgExports
 */

/** @type {OgExports | null} */
let exports = null;
/** The locale whose fonts are registered now. */
let fontsFor = /** @type {string | null} */ (null);

function wasm() {
	if (!exports) {
		const module = new WebAssembly.Module(readFileSync(WASM));
		exports = /** @type {OgExports} */ (
			/** @type {unknown} */ (new WebAssembly.Instance(module, {}).exports)
		);
	}
	return exports;
}

/**
 * Copy `bytes` into the module, run `f` with (ptr, len), free.
 * @template T
 * @param {Uint8Array} bytes
 * @param {(ptr: number, len: number) => T} f
 * @returns {T}
 */
function withBytes(bytes, f) {
	const e = wasm();
	const len = Math.max(1, bytes.length);
	const ptr = e.og_alloc(len);
	new Uint8Array(e.memory.buffer, ptr, bytes.length).set(bytes);
	try {
		return f(ptr, bytes.length);
	} finally {
		e.og_free(ptr, len);
	}
}

/** @param {number} n */
function result(n) {
	const e = wasm();
	return new Uint8Array(e.memory.buffer, e.og_result_ptr(), n & 0x7fffffff).slice();
}

/**
 * Register the committed fallback subsets for `lang` (scripts/og/fonts/): its
 * CJK pair, if it has one, then the symbols subset every locale shares (one
 * weight, drawn for both).
 */
function useFontsFor(/** @type {string} */ lang) {
	if (fontsFor === lang) return;
	const e = wasm();
	e.og_fonts_clear();
	/** @param {string} name @param {number} weight */
	const register = (name, weight) => {
		const file = new URL(name, FONTS);
		if (!existsSync(file)) return;
		const ok = withBytes(new Uint8Array(readFileSync(file)), (p, n) => e.og_font(p, n, weight));
		if (!ok) throw new Error(`og: ${file.pathname} is not a font the renderer can read`);
	};
	for (const weight of [400, 700]) register(`${lang}-${weight}.ttf`, weight);
	for (const weight of [400, 700]) register('symbols-400.ttf', weight);
	fontsFor = lang;
}

/** Forget the registered fonts (after `bun run og:fonts` rewrote them). */
export function reloadFonts() {
	fontsFor = null;
}

/**
 * @typedef {{ lang: string, site: string, domain: string, section?: string, title: string,
 *   description?: string, meta?: string }} OgCard
 */

/** @param {OgCard} card */
function request(card) {
	// Precomposed letters: the renderer places glyphs, it does not stack marks.
	const fields = [
		card.site,
		card.domain,
		card.section ?? '',
		card.title,
		card.description ?? '',
		card.meta ?? ''
	].map((f) => f.normalize('NFC'));
	if (fields.some((f) => f.includes('\0'))) throw new Error('og: a field contains NUL');
	return new TextEncoder().encode(fields.join('\0'));
}

/** @param {string} chars */
function describe(chars) {
	return [...chars]
		.map((c) => `${c} (U+${(c.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0')})`)
		.join(', ');
}

/**
 * The characters of a card no font has ('' when it can be drawn).
 * @param {OgCard} card
 * @returns {string}
 */
export function missingGlyphs(card) {
	useFontsFor(card.lang);
	const n = withBytes(request(card), (p, len) => wasm().og_check(p, len)) >>> 0;
	if (n === 0) return '';
	if (n === 0x80000000) throw new Error(`og: malformed request for "${card.title}"`);
	return new TextDecoder().decode(result(n));
}

/**
 * One 1200×630 PNG.
 * @param {OgCard} card
 * @returns {Uint8Array}
 */
export function renderOg(card) {
	useFontsFor(card.lang);
	const n = withBytes(request(card), (p, len) => wasm().og_render(p, len)) >>> 0;
	if (n === 0) throw new Error(`og: malformed request for "${card.title}"`);
	const bytes = result(n);
	if (n & 0x80000000) {
		throw new Error(
			`og: "${card.title}" (${card.lang}) uses characters its fonts lack: ` +
				`${describe(new TextDecoder().decode(bytes))}. Run \`bun run og:fonts\` ` +
				'(scripts/og/fonts/README.md).'
		);
	}
	return bytes;
}

/**
 * `font` (TrueType) cut down to the characters of `text`.
 * @param {Uint8Array} font
 * @param {string} text
 * @returns {Uint8Array}
 */
export function subsetFont(font, text) {
	const e = wasm();
	const n = withBytes(font, (fp, fn) =>
		withBytes(new TextEncoder().encode(text), (tp, tn) => e.og_subset(fp, fn, tp, tn))
	);
	if (n === 0 || n & 0x80000000) throw new Error('og: the font could not be subset');
	return result(n);
}
