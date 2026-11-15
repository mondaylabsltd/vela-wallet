/**
 * `bun run og:fonts` — cut the card fallback fonts (scripts/og/fonts/) to
 * exactly the characters the cards use, with rust/vela-og's subsetter.
 *
 * Two passes. First each CJK locale's pair, from its Noto Sans region font,
 * holding every character of that locale's cards. Then one symbols subset all
 * locales share, from Noto Sans Symbols 2, holding whatever is still missing
 * (✕ and the like, which Inter lacks). A character no pinned font has is
 * reported, and the build will refuse the card until it is reworded.
 *
 * The full fonts are those pinned in fonts.json: downloaded once into
 * node_modules/.cache/vela-og-fonts/, refused if their SHA-256 differs from
 * the pin — on download and on every reuse. The build fetches nothing; it
 * reads only the committed subsets.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Locale } from '../../src/lib/i18n/locales';
import { cardText, ogCards } from '../../src/lib/content/og-cards';
import { missingGlyphs, reloadFonts, subsetFont } from './render.js';

interface Pinned {
	url: string;
	sha256: string;
	bytes: number;
}
type Entry = { family: string; '400': Pinned; '700'?: Pinned };
type Manifest = { fonts: Record<string, Entry>; symbols: Entry };

const APP = join(import.meta.dirname, '../..');
const OUT = join(import.meta.dirname, 'fonts');
const CACHE = join(APP, 'node_modules/.cache/vela-og-fonts');
const manifest: Manifest = JSON.parse(
	readFileSync(join(import.meta.dirname, 'fonts.json'), 'utf8')
);

const sha256 = (b: Uint8Array) => createHash('sha256').update(b).digest('hex');

async function pinned(font: Pinned): Promise<Uint8Array> {
	const file = join(CACHE, `${font.sha256}.ttf`);
	if (existsSync(file)) {
		const bytes = new Uint8Array(readFileSync(file));
		if (sha256(bytes) === font.sha256) return bytes;
		rmSync(file);
	}
	console.log(`  downloading ${font.url} (${(font.bytes / 1e6).toFixed(1)} MB)`);
	const res = await fetch(font.url);
	if (!res.ok) throw new Error(`${font.url}: HTTP ${res.status}`);
	const bytes = new Uint8Array(await res.arrayBuffer());
	const got = sha256(bytes);
	if (got !== font.sha256) {
		throw new Error(`${font.url}: SHA-256 ${got}, pinned ${font.sha256} — refusing it`);
	}
	mkdirSync(CACHE, { recursive: true });
	writeFileSync(file, bytes);
	return bytes;
}

/** Write `bytes` to `name` (or remove it when there is nothing to hold). */
function write(
	name: string,
	family: string,
	text: string,
	bytes: Uint8Array | null,
	previous?: Buffer | null
): void {
	const out = join(OUT, name);
	if (!bytes) {
		if (existsSync(out)) rmSync(out);
		return;
	}
	const before = previous !== undefined ? previous : existsSync(out) ? readFileSync(out) : null;
	writeFileSync(out, bytes);
	const chars = new Set(text.replace(/\s/g, '')).size;
	const changed = !before || !Buffer.from(bytes).equals(before);
	console.log(
		`${name.padEnd(16)} ${family.padEnd(20)} ${String(chars).padStart(5)} characters ` +
			`${(bytes.length / 1024).toFixed(0).padStart(4)} KB${changed ? '  (updated)' : ''}`
	);
}

// Drafts included: `vite dev` draws their cards too.
const cards = ogCards(APP, true);

// Pass 1: each CJK locale's pair.
for (const [lang, entry] of Object.entries(manifest.fonts)) {
	const text = cardText(cards, lang as Locale);
	const used = cards.some((c) => c.lang === lang);
	for (const weight of ['400', '700'] as const) {
		const source = entry[weight];
		const cut = used && source ? subsetFont(await pinned(source), text) : null;
		write(`${lang}-${weight}.ttf`, entry.family, text, cut);
	}
}

// Pass 2: what is still missing anywhere, from the symbols font.
const symbolsFile = join(OUT, 'symbols-400.ttf');
const previousSymbols = existsSync(symbolsFile) ? readFileSync(symbolsFile) : null;
write('symbols-400.ttf', '', '', null);
reloadFonts();
const missing = [...new Set(cards.flatMap((c) => [...missingGlyphs(c)]))].join('');
const symbols = missing ? subsetFont(await pinned(manifest.symbols['400']), missing) : null;
write('symbols-400.ttf', manifest.symbols.family, missing, symbols, previousSymbols);

// Anything no pinned font has: say so now, not at build time.
reloadFonts();
const still = cards.map((c) => [c.path, missingGlyphs(c)] as const).filter(([, m]) => m);
for (const [path, chars] of still) console.error(`no font has ${chars} (in ${path}) — reword it`);
if (still.length) process.exit(1);
