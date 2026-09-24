/**
 * `bun run i18n:stamp` — record which English text each translation was made
 * from (spec 059, FR-026).
 *
 * Why this exists: on 2026-09-15 two hero facts were reworded in English while
 * translations of the old wording were being written. Nothing caught it — the
 * files were structurally perfect and said something the page no longer said.
 * A fingerprint per namespace turns that silent drift into a line in
 * `i18n:status`.
 *
 * Run it after finishing a translation, never before: stamping an untranslated
 * or half-finished namespace records agreement that does not exist.
 */
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { en } from '../src/lib/i18n/messages/en.ts';
import { SUPPORTED_LOCALES, DEFAULT_LOCALE } from '../src/lib/i18n/locales.ts';
import { docFingerprint, withSource } from './doc-fingerprint.ts';

const here = (p: string) => fileURLToPath(new URL(`../${p}`, import.meta.url));
const PAGE_NAMESPACES = ['home', 'about', 'roadmap', 'getStarted', 'chainSetup'] as const;

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

const fingerprint = (v: unknown) =>
	createHash('sha256').update(JSON.stringify(v)).digest('hex').slice(0, 12);

for (const locale of SUPPORTED_LOCALES) {
	if (locale === DEFAULT_LOCALE) continue;
	const file = here(`src/lib/i18n/messages/${locale}.json`);
	if (!existsSync(file)) continue;

	const data = JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
	const stamps: Record<string, string> = {};
	for (const ns of PAGE_NAMESPACES) {
		if (isComplete(en[ns], data[ns])) stamps[ns] = fingerprint(en[ns]);
	}

	if (Object.keys(stamps).length === 0) {
		delete data._fingerprints;
	} else {
		data._fingerprints = stamps;
	}
	writeFileSync(file, JSON.stringify(data, null, '\t') + '\n');
	console.log(`${locale}: stamped ${Object.keys(stamps).join(', ') || '(nothing translated)'}`);
}

// Docs (spec 080): every translated doc records the English file it was made
// from. Same discipline as above — stamp only a translation that is finished
// and re-aligned, never one you have not read against the current English.
const DOCS = 'src/content/docs';
for (const locale of SUPPORTED_LOCALES) {
	if (locale === DEFAULT_LOCALE) continue;
	const dir = here(`${DOCS}/${locale}`);
	if (!existsSync(dir)) continue;
	let count = 0;
	for (const file of readdirSync(dir).filter((f) => f.endsWith('.md'))) {
		const english = here(`${DOCS}/${file}`);
		if (!existsSync(english)) continue;
		const path = `${dir}/${file}`;
		const before = readFileSync(path, 'utf8');
		const after = withSource(before, docFingerprint(english));
		if (after !== before) writeFileSync(path, after);
		count++;
	}
	console.log(`${locale}: stamped ${count} docs`);
}
