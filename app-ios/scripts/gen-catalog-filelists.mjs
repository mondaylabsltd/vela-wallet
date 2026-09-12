#!/usr/bin/env node
/**
 * Catalog build-declaration generator (spec 010-ios-catalog-bundling).
 *
 * Source  <-  assets/i18n/<lng>.json                                (repo root)
 * Output  ->  app-ios/scripts/catalogs-input.xcfilelist   (15 declared reads)
 *             app-ios/scripts/catalogs-output.xcfilelist  (15 declared writes)
 *
 * iOS does NOT keep a copy of the catalogs: the Xcode build phase
 * `Bundle locale catalogs` copies them straight from assets/i18n into
 * VelaWallet.app at build time (research D1/D2). These two .xcfilelists are the
 * declaration that makes that legal and correct — they are what grants the
 * sandboxed script read access outside SRCROOT, what makes the phase
 * incremental, and what defines "which locales exist" for iOS (research D3).
 *
 * This file replaces sync-catalogs.mjs and inherits its LOCALE_COUNT pin: the
 * locale list is derived from the files present in assets/i18n and pinned at
 * exactly 15, so a corpus change is a conscious decision here too. `--check`
 * fails when either list would change, and additionally when any catalog JSON
 * has been re-introduced under app-ios/ (the duplicate this spec removed).
 *
 * NEVER hand-edit the .xcfilelists — fix the corpus, rerun gen-i18n, rerun this.
 */

import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const APP_ROOT = join(import.meta.dirname, '..');
const REPO_ROOT = join(APP_ROOT, '..');
const SOURCE_DIR = join(REPO_ROOT, 'assets', 'i18n');
const INPUT_LIST = join(APP_ROOT, 'scripts', 'catalogs-input.xcfilelist');
const OUTPUT_LIST = join(APP_ROOT, 'scripts', 'catalogs-output.xcfilelist');

/** assets/i18n must carry exactly this many <lng>.json files (corpus pin). */
const LOCALE_COUNT = 15;

/**
 * SRCROOT is app-ios/VelaWallet, so ../../ reaches the repo root. Xcode expands
 * these settings when it resolves the list, and hands the resolved copy to the
 * build script — which is why no path is hardcoded on the shell side.
 */
const INPUT_PREFIX = '$(SRCROOT)/../../assets/i18n';
/**
 * TARGET_BUILD_DIR, not BUILT_PRODUCTS_DIR: the two diverge for install/archive
 * builds and only the former tracks the bundle being assembled (research D2).
 */
const OUTPUT_PREFIX = '$(TARGET_BUILD_DIR)/$(UNLOCALIZED_RESOURCES_FOLDER_PATH)';

function jsonFiles(dir) {
	let entries;
	try {
		entries = readdirSync(dir);
	} catch {
		return [];
	}
	return entries.filter((f) => f.endsWith('.json')).sort();
}

function loadLocales() {
	const files = jsonFiles(SOURCE_DIR);
	if (files.length !== LOCALE_COUNT) {
		throw new Error(
			`expected exactly ${LOCALE_COUNT} locale files in ${SOURCE_DIR}, found ${files.length}` +
				(files.length ? `:\n  ${files.join('\n  ')}` : '') +
				`\nif the corpus legitimately changed, update LOCALE_COUNT in ${basename(import.meta.filename)}`
		);
	}
	return files;
}

function render(prefix, files) {
	return `${files.map((f) => `${prefix}/${f}`).join('\n')}\n`;
}

function read(path) {
	try {
		return readFileSync(path, 'utf8');
	} catch {
		return null;
	}
}

/**
 * The duplicate this spec deleted must not come back.
 *
 * Two scoping rules, both load-bearing:
 *  - path-scoped to app-ios/, because the upstream corpus at
 *    rust/crates/vela-core/i18n/locales/ carries per-namespace files with the
 *    same basenames (its zh.json is a namespace, not a catalog);
 *  - basename-matched against the corpus, because app-ios legitimately tracks
 *    other JSON (Assets.xcassets/**\/Contents.json).
 */
function trackedCatalogsUnderAppIos(files) {
	const catalogNames = new Set(files);
	let tracked;
	try {
		tracked = execFileSync('git', ['ls-files', '--', 'app-ios'], {
			cwd: REPO_ROOT,
			encoding: 'utf8'
		});
	} catch {
		return []; // not a git checkout — the build-time gates still apply
	}
	return tracked
		.split('\n')
		.filter((p) => catalogNames.has(basename(p)))
		.sort();
}

function generate() {
	const files = loadLocales();
	writeFileSync(INPUT_LIST, render(INPUT_PREFIX, files));
	writeFileSync(OUTPUT_LIST, render(OUTPUT_PREFIX, files));
	console.log(`wrote ${files.length} entries -> ${basename(INPUT_LIST)}, ${basename(OUTPUT_LIST)}`);
}

function check() {
	const files = loadLocales();
	const drift = [];

	for (const [path, prefix] of [
		[INPUT_LIST, INPUT_PREFIX],
		[OUTPUT_LIST, OUTPUT_PREFIX]
	]) {
		const actual = read(path);
		if (actual === null) drift.push(`missing: ${basename(path)}`);
		else if (actual !== render(prefix, files)) drift.push(`stale:   ${basename(path)}`);
	}

	for (const path of trackedCatalogsUnderAppIos(files)) {
		drift.push(`tracked catalog copy under app-ios (must not exist): ${path}`);
	}

	if (drift.length) {
		console.error(
			`catalog declaration drift — run \`node app-ios/scripts/gen-catalog-filelists.mjs\`:\n  ${drift.join('\n  ')}`
		);
		process.exit(1);
	}
	console.log(`catalog file lists in sync (${files.length})`);
}

function main() {
	try {
		if (process.argv.includes('--check')) check();
		else generate();
	} catch (error) {
		// A locale-count mismatch is an expected outcome, not a crash — print the
		// message the reader needs, not a Node stack trace.
		console.error(error instanceof Error ? error.message : String(error));
		process.exit(1);
	}
}

if (process.argv[1] && import.meta.filename === process.argv[1]) {
	main();
}
