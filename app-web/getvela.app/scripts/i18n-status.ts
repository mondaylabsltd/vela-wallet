/**
 * `bun run i18n:status` — the translation report (spec 059, FR-026, FR-028).
 *
 * Prints, per locale: how many of the localizable pages are genuinely
 * translated, how many fall back to English, how many translations are STALE
 * (the English changed after they were written), and the locale's review state.
 *
 * It reads files off disk rather than going through Vite, so it works without a
 * build and can be run in CI. Deliberately exits 0 even when locales are
 * incomplete: a missing translation is a legal state (R4) and must never block
 * an English fix from shipping (FR-027). Pass `--gate` to make it exit non-zero
 * on a STALE entry, which is the one state that means something is now wrong
 * rather than merely unfinished.
 */
import { createHash } from 'node:crypto';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { en } from '../src/lib/i18n/messages/en.ts';
import { SUPPORTED_LOCALES, DEFAULT_LOCALE, type Locale } from '../src/lib/i18n/locales.ts';

const here = (p: string) => fileURLToPath(new URL(`../${p}`, import.meta.url));

const MESSAGES = 'src/lib/i18n/messages';
const DOCS = 'src/content/docs';
const PAGE_NAMESPACES = ['home', 'about', 'roadmap', 'getStarted', 'chainSetup'] as const;

type Review = { state: 'source' | 'drafted' | 'reviewed'; date?: string };
const review: Record<string, Review> = JSON.parse(
	readFileSync(here('src/lib/i18n/review.json'), 'utf8')
);

function translation(locale: Locale): Record<string, unknown> {
	const file = here(`${MESSAGES}/${locale}.json`);
	return existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : {};
}

/** Every leaf of `source` present and non-empty in `value`. */
function isComplete(source: unknown, value: unknown): boolean {
	if (Array.isArray(source)) {
		if (!Array.isArray(value) || value.length !== source.length) return false;
		return source.every((item, i) => isComplete(item, value[i]));
	}
	if (typeof source === 'object' && source !== null) {
		if (typeof value !== 'object' || value === null) return false;
		return Object.entries(source).every(([k, v]) =>
			isComplete(v, (value as Record<string, unknown>)[k])
		);
	}
	return typeof value === 'string' && value.trim().length > 0;
}

/**
 * The fingerprint of an English namespace or doc. A translation records the
 * fingerprint it was made from; when they differ the translation is STALE —
 * present, renderable, and quietly describing a page that has since changed.
 */
function fingerprint(value: unknown): string {
	return createHash('sha256').update(JSON.stringify(value)).digest('hex').slice(0, 12);
}

const docSlugs = readdirSync(here(DOCS))
	.filter((f) => f.endsWith('.md'))
	.map((f) => f.replace(/\.md$/, ''))
	.sort();

function docExists(locale: Locale, slug: string): boolean {
	return existsSync(here(`${DOCS}/${locale}/${slug}.md`));
}

interface Row {
	locale: string;
	pages: number;
	translated: number;
	fallback: number;
	stale: number;
	staleNames: string[];
	state: string;
	date: string;
}

const totalPages = PAGE_NAMESPACES.length + docSlugs.length;
const rows: Row[] = [];

for (const locale of SUPPORTED_LOCALES) {
	if (locale === DEFAULT_LOCALE) {
		rows.push({
			locale,
			pages: totalPages,
			translated: totalPages,
			fallback: 0,
			stale: 0,
			staleNames: [],
			state: 'source',
			date: ''
		});
		continue;
	}

	const t = translation(locale);
	const fingerprints = (t._fingerprints ?? {}) as Record<string, string>;
	let translated = 0;
	let stale = 0;
	const staleNames: string[] = [];

	for (const ns of PAGE_NAMESPACES) {
		if (!isComplete(en[ns], t[ns])) continue;
		translated++;
		const recorded = fingerprints[ns];
		if (recorded && recorded !== fingerprint(en[ns])) {
			stale++;
			staleNames.push(ns);
		}
	}
	for (const slug of docSlugs) {
		if (docExists(locale, slug)) translated++;
	}

	const r = review[locale] ?? { state: 'drafted' };
	rows.push({
		locale,
		pages: totalPages,
		translated,
		fallback: totalPages - translated,
		stale,
		staleNames,
		state: r.state,
		date: r.date ?? ''
	});
}

const pad = (s: string, n: number) => s.padEnd(n);
console.log(
	`\n${pad('locale', 8)}${pad('pages', 7)}${pad('translated', 12)}${pad('fallback', 10)}${pad('stale', 7)}${pad('review', 10)}date`
);
console.log('-'.repeat(62));
for (const r of rows) {
	console.log(
		pad(r.locale, 8) +
			pad(String(r.pages), 7) +
			pad(String(r.translated), 12) +
			pad(String(r.fallback), 10) +
			pad(String(r.stale), 7) +
			pad(r.state, 10) +
			r.date
	);
}

const staleRows = rows.filter((r) => r.stale > 0);
if (staleRows.length) {
	console.log('\nSTALE — the English changed after these were translated:');
	for (const r of staleRows) console.log(`  ${r.locale}: ${r.staleNames.join(', ')}`);
}

const reviewed = rows.filter((r) => r.state === 'reviewed').map((r) => r.locale);
console.log(
	`\n${docSlugs.length} docs + ${PAGE_NAMESPACES.length} pages per locale · ` +
		`reviewed: ${reviewed.length ? reviewed.join(', ') : 'none'}`
);
console.log(
	'A locale short of 100% is not a failure — the missing pages render English\n' +
		'under a notice in the reader’s own language (R4). Only STALE means something\n' +
		'is now wrong rather than merely unfinished.\n'
);

if (process.argv.includes('--gate') && staleRows.length) process.exit(1);
