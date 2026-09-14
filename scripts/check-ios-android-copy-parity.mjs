#!/usr/bin/env node
/**
 * The "what does the other phone SAY" ruler (spec 058).
 *
 * Events prove a machine is spoken to; they do not prove a screen draws the
 * thing. Copy does: every label, title and warning in this app comes from one
 * corpus, by key, so **a key Android resolves and iOS never mentions is a
 * sentence the iPhone cannot show** — which is usually a control, a state or a
 * whole sheet that is missing rather than a translation that is.
 *
 * Both clients are read in their own idiom:
 *   Android  `I18nKeys.kt`'s `const val NAME = "key.path"`, counted only when
 *            NAME is referenced from a file that is not `I18nKeys.kt`, plus any
 *            bare `"key.path"` literal elsewhere.
 *   iOS      the same two shapes: `I18nKeys.swift`'s `static let x = "…"` used
 *            elsewhere, plus `loc.t("…")` and bare literals.
 *
 * Keys are grouped by their first segment, which is the corpus's own area name
 * (`send.*`, `receive.*`, `settingsUi.*`, `explore.*`), so the table reads as
 * "which PART of the app has sentences one phone cannot say".
 *
 * Exit code stays 0: a key may be absent for a good reason (a platform word, a
 * control the founder removed). The table is the deliverable.
 *
 * Usage: node scripts/check-ios-android-copy-parity.mjs [--area send] [--list]
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const IOS = join(ROOT, 'app-ios/VelaWallet/VelaWallet');
const ANDROID = join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet');
const argv = process.argv.slice(2);
const area = argv.includes('--area') ? argv[argv.indexOf('--area') + 1] : null;
const list = argv.includes('--list') || Boolean(area);

function walk(dir, ext, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (!/\/build\/|node_modules/.test(p)) walk(p, ext, out);
		} else if (p.endsWith(ext)) out.push(p);
	}
	return out;
}

/**
 * The corpus itself decides what a key is. Shape alone does not: `chevron.right`
 * is an SF Symbol, `pancakeswap.finance` is a domain, and both look exactly like
 * a key path. Reading `en.json` and its namespace files means the ruler counts
 * only sentences that exist.
 */
const CORPUS = (() => {
	const dir = join(ROOT, 'rust/crates/vela-core/i18n/locales');
	const flat = (obj, prefix, out) => {
		for (const [k, v] of Object.entries(obj)) {
			const key = prefix + k;
			if (v && typeof v === 'object' && !Array.isArray(v)) flat(v, key + '.', out);
			else out.add(key);
		}
		return out;
	};
	const out = new Set();
	flat(JSON.parse(readFileSync(join(dir, 'en.json'), 'utf8')), '', out);
	for (const f of readdirSync(join(dir, 'en'))) {
		if (f.endsWith('.json')) flat(JSON.parse(readFileSync(join(dir, 'en', f), 'utf8')), '', out);
	}
	return out;
})();
const KEY = { test: (s) => CORPUS.has(s) };

/**
 * Keys this client can actually resolve: the constant table's entries whose
 * NAME something else references, plus literals written in place.
 */
function keysOf(files, tableFile, constRe) {
	const table = new Map(); // const name -> key path
	const tablePath = files.find((f) => f.endsWith(tableFile));
	if (tablePath) {
		for (const m of readFileSync(tablePath, 'utf8').matchAll(constRe)) {
			if (KEY.test(m[2])) table.set(m[1], m[2]);
		}
	}
	const rest = files
		// Fixture files are NOT excluded. On both phones the fixture layer is
		// where a screen's copy lives — the live builder patches data into a
		// fixture-shaped model — so dropping them would report every static
		// label as missing. Galleries and tests are dropped: a key that only a
		// review board or an assertion mentions is not a key a person can read.
		.filter((f) => f !== tablePath && !/Gallery|Preview|Tests?\.(swift|kt)$/.test(f))
		.map((f) => readFileSync(f, 'utf8'))
		.join('\n');
	const keys = new Set();
	for (const [name, path] of table) {
		if (new RegExp(`\\b${name}\\b`).test(rest)) keys.add(path);
	}
	for (const m of rest.matchAll(/"([^"\s]+)"/g)) if (KEY.test(m[1])) keys.add(m[1]);
	return keys;
}

const ios = keysOf(walk(IOS, '.swift'), 'I18nKeys.swift', /static let (\w+)\s*=\s*"([^"]+)"/g);
const android = keysOf(walk(ANDROID, '.kt'), 'I18nKeys.kt', /const val (\w+)\s*=\s*"([^"]+)"/g);

const areaOf = (k) => k.split('.')[0];
const rows = new Map();
const bump = (k, which) => {
	const a = areaOf(k);
	if (area && a !== area) return;
	if (!rows.has(a)) rows.set(a, { both: 0, androidOnly: [], iosOnly: [] });
	const r = rows.get(a);
	if (which === 'both') r.both++;
	else r[which].push(k);
};
for (const k of android) bump(k, ios.has(k) ? 'both' : 'androidOnly');
for (const k of ios) if (!android.has(k)) bump(k, 'iosOnly');

const sorted = [...rows].sort((a, b) => b[1].androidOnly.length - a[1].androidOnly.length);
console.log('| Area | Both | Android-only | iOS-only |');
console.log('| --- | --- | --- | --- |');
for (const [a, r] of sorted) {
	console.log(`| ${a} | ${r.both} | ${r.androidOnly.length} | ${r.iosOnly.length} |`);
}
console.log(
	`\nAndroid-only keys: ${sorted.reduce((n, [, r]) => n + r.androidOnly.length, 0)} · iOS-only: ${sorted.reduce((n, [, r]) => n + r.iosOnly.length, 0)}`
);
if (list) {
	for (const [a, r] of sorted) {
		if (!r.androidOnly.length && !r.iosOnly.length) continue;
		console.log(`\n## ${a}`);
		if (r.androidOnly.length) console.log('Android-only:\n  ' + r.androidOnly.sort().join('\n  '));
		if (r.iosOnly.length) console.log('iOS-only:\n  ' + r.iosOnly.sort().join('\n  '));
	}
}
