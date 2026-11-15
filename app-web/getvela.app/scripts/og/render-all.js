/**
 * Draw every card for a build — only the ones that changed.
 *
 * A card's PNG is kept in node_modules/.cache/vela-og-cards/, named by the
 * SHA-256 of everything that decides its pixels: its text, the renderer
 * (rust/pkg-web-og/vela_og.wasm) and the fonts it may use. A card whose key
 * is already there is reused as is; the rest are drawn, one wasm instance per
 * CPU. Cards no build asked for are pruned, so the cache stays the size of
 * the site.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Worker } from 'node:worker_threads';

const WASM = new URL('../../../../rust/pkg-web-og/vela_og.wasm', import.meta.url);
const FONTS = new URL('./fonts/', import.meta.url);
const CACHE = fileURLToPath(new URL('../../node_modules/.cache/vela-og-cards/', import.meta.url));

/** @param {Uint8Array | string} data */
const sha256 = (data) => createHash('sha256').update(data).digest('hex');

/** @param {string} name */
function fileHash(name) {
	const url = new URL(name, FONTS);
	return existsSync(url) ? sha256(readFileSync(url)) : '-';
}

/**
 * @typedef {import('./render.js').OgCard & { path: string }} PathCard
 * @param {PathCard[]} cards
 * @returns {Promise<{ path: string, png: Uint8Array }[]>}
 */
export async function renderAll(cards) {
	const renderer = sha256(readFileSync(WASM));
	const symbols = fileHash('symbols-400.ttf');
	/** @type {Map<string, string>} */
	const fontsOf = new Map();
	/** @param {string} lang */
	const fontKey = (lang) => {
		if (!fontsOf.has(lang)) {
			fontsOf.set(lang, `${fileHash(`${lang}-400.ttf`)}:${fileHash(`${lang}-700.ttf`)}:${symbols}`);
		}
		return /** @type {string} */ (fontsOf.get(lang));
	};
	const keyed = cards.map((card) => ({
		card,
		key: sha256(JSON.stringify([renderer, fontKey(card.lang), card]))
	}));

	mkdirSync(CACHE, { recursive: true });
	const cached = keyed.filter(({ key }) => existsSync(join(CACHE, `${key}.png`)));
	const todo = keyed.filter(({ key }) => !existsSync(join(CACHE, `${key}.png`)));

	const drawn = await drawInWorkers(todo.map(({ card }) => card));
	const keyOf = new Map(todo.map(({ card, key }) => [card.path, key]));
	for (const { path, png } of drawn) writeFileSync(join(CACHE, `${keyOf.get(path)}.png`), png);

	// Keep only what this build uses.
	const used = new Set(keyed.map(({ key }) => `${key}.png`));
	for (const file of readdirSync(CACHE)) if (!used.has(file)) rmSync(join(CACHE, file));

	console.log(`og: ${cards.length} cards — ${todo.length} drawn, ${cached.length} unchanged`);
	return keyed.map(({ card, key }) => ({
		path: card.path,
		png: new Uint8Array(readFileSync(join(CACHE, `${key}.png`)))
	}));
}

/**
 * @param {PathCard[]} cards
 * @returns {Promise<{ path: string, png: Uint8Array }[]>}
 */
async function drawInWorkers(cards) {
	if (cards.length === 0) return [];
	const workers = Math.max(1, Math.min(availableParallelism(), Math.ceil(cards.length / 8)));
	// Grouped by language: each worker registers each locale's fonts once.
	const sorted = [...cards].sort((a, b) => a.lang.localeCompare(b.lang));
	const slices = Array.from({ length: workers }, (_, i) =>
		sorted.slice(
			Math.floor((i * sorted.length) / workers),
			Math.floor(((i + 1) * sorted.length) / workers)
		)
	);
	const parts = await Promise.all(
		slices.map(
			(slice) =>
				new Promise((resolve, reject) => {
					const worker = new Worker(new URL('./worker.js', import.meta.url), {
						workerData: { cards: slice }
					});
					worker.once('message', resolve);
					worker.once('error', reject);
					worker.once('exit', (code) => {
						if (code !== 0) reject(new Error(`og: render worker exited with ${code}`));
					});
				})
		)
	);
	return /** @type {{ path: string, png: Uint8Array }[]} */ (parts.flat());
}
