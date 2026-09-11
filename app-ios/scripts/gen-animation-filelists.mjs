#!/usr/bin/env node
/**
 * Launch-animation build-declaration generator (spec 012-launch-animation-lottie).
 *
 * Source  <-  docs/design/onboarding/launch/*-core-*.json                (repo root)
 * Output  ->  app-ios/scripts/animations-input.xcfilelist    (declared reads)
 *             app-ios/scripts/animations-output.xcfilelist   (declared writes)
 *
 * iOS keeps NO copy of the animations: the Xcode build phase
 * `Bundle launch animations` copies them straight from docs/design/onboarding/launch
 * into VelaWallet.app at build time. This is the arrangement spec 010 introduced
 * for locale catalogs, applied to a second asset family — and the reason the two
 * .xcfilelists exist at all is that they are what grants the sandboxed build
 * script (`ENABLE_USER_SCRIPT_SANDBOXING`) read access outside SRCROOT, what
 * makes the phase incremental, and what defines "which animations exist" for iOS.
 *
 * Only the `core` framings ship. The `full` pair stays in the design directory
 * as the reference that pins the apps' box ratio and is never loaded
 * (spec 012 research D0/D3).
 *
 * Unlike the catalog generator there is NO count pin: adding a second animation
 * must not require editing a build file (FR-004). What is pinned instead is the
 * shape — every shipped file must be a `-core-` framing with a legal name — so a
 * stray file cannot ride along unnoticed.
 *
 * NEVER hand-edit the .xcfilelists — add the asset, rerun this.
 */

import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const APP_ROOT = join(import.meta.dirname, '..');
const REPO_ROOT = join(APP_ROOT, '..');
const SOURCE_DIR = join(REPO_ROOT, 'docs', 'design', 'onboarding', 'launch');
const INPUT_LIST = join(APP_ROOT, 'scripts', 'animations-input.xcfilelist');
const OUTPUT_LIST = join(APP_ROOT, 'scripts', 'animations-output.xcfilelist');

/**
 * Same rule the repo-wide linter enforces (contracts/portable-subset.md).
 * The animation NAME is a field, not the literal `launch` — hardcoding it meant
 * a second animation could not be added without editing this file, which is a
 * build-configuration edit and exactly what FR-004 forbids.
 */
const NAME_RE = /^vela-wallet-([a-z0-9]+(?:-[a-z0-9]+)*?)-(phone|desktop)-(core|full)-(dark|light)\.json$/;

/** SRCROOT is app-ios/VelaWallet, so ../../ reaches the repo root. */
const INPUT_PREFIX = '$(SRCROOT)/../../docs/design/onboarding/launch';
/**
 * TARGET_BUILD_DIR, not BUILT_PRODUCTS_DIR: the two diverge for install/archive
 * builds and only the former tracks the bundle being assembled (spec 010 D2).
 */
const OUTPUT_PREFIX = '$(TARGET_BUILD_DIR)/$(UNLOCALIZED_RESOURCES_FOLDER_PATH)';

function shippedAnimations() {
	let entries;
	try {
		entries = readdirSync(SOURCE_DIR);
	} catch {
		throw new Error(
			`launch animation directory not found: ${SOURCE_DIR}\n` +
				'docs/design/onboarding/launch is the source of truth for every app (spec 012 FR-001).'
		);
	}
	const json = entries.filter((f) => f.endsWith('.json')).sort();
	const illegal = json.filter((f) => !NAME_RE.test(f));
	if (illegal.length) {
		throw new Error(
			`animation filenames must match vela-wallet-{name}-{phone|desktop}-{core|full}-{dark|light}.json:\n  ${illegal.join('\n  ')}`
		);
	}
	const shipped = json.filter((f) => NAME_RE.exec(f)[3] === 'core');
	if (!shipped.length) {
		throw new Error(`no \`-core-\` framings found in ${SOURCE_DIR} — nothing would ship`);
	}
	return shipped;
}

const render = (prefix, files) => `${files.map((f) => `${prefix}/${f}`).join('\n')}\n`;

function read(path) {
	try {
		return readFileSync(path, 'utf8');
	} catch {
		return null;
	}
}

/**
 * The duplicate this arrangement exists to prevent must not come back.
 *
 * Basename-matched against the real animations and path-scoped to app-ios/,
 * exactly as the catalog generator does it: app-ios legitimately tracks other
 * JSON (Assets.xcassets/**\/Contents.json).
 */
function trackedAnimationsUnderAppIos(files) {
	const names = new Set(files);
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
		.filter((p) => names.has(basename(p)))
		.sort();
}

function generate() {
	const files = shippedAnimations();
	writeFileSync(INPUT_LIST, render(INPUT_PREFIX, files));
	writeFileSync(OUTPUT_LIST, render(OUTPUT_PREFIX, files));
	console.log(`wrote ${files.length} entries -> ${basename(INPUT_LIST)}, ${basename(OUTPUT_LIST)}`);
}

function check() {
	const files = shippedAnimations();
	const drift = [];

	for (const [path, prefix] of [
		[INPUT_LIST, INPUT_PREFIX],
		[OUTPUT_LIST, OUTPUT_PREFIX]
	]) {
		const actual = read(path);
		if (actual === null) drift.push(`missing: ${basename(path)}`);
		else if (actual !== render(prefix, files)) drift.push(`stale:   ${basename(path)}`);
	}

	for (const path of trackedAnimationsUnderAppIos(files)) {
		drift.push(`tracked animation copy under app-ios (must not exist): ${path}`);
	}

	if (drift.length) {
		console.error(
			`animation declaration drift — run \`node app-ios/scripts/gen-animation-filelists.mjs\`:\n  ${drift.join('\n  ')}`
		);
		process.exit(1);
	}
	console.log(`animation file lists in sync (${files.length})`);
}

function main() {
	try {
		if (process.argv.includes('--check')) check();
		else generate();
	} catch (error) {
		// A naming or corpus problem is an expected outcome, not a crash — print
		// the message the reader needs, not a Node stack trace.
		console.error(error instanceof Error ? error.message : String(error));
		process.exit(1);
	}
}

if (process.argv[1] && import.meta.filename === process.argv[1]) {
	main();
}
