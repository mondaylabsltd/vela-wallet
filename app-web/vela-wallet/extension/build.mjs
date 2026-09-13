#!/usr/bin/env node
/**
 * Assemble the loadable Chrome extension (spec 027 T311).
 *
 * `VELA_TARGET=extension vite build` puts the app's PRERENDERED pages into
 * `extension/dist` — the same pages the hosted site serves, because this
 * app resolves all 15 locales at build time through the wasm i18n engine and a
 * client-rendered shell would have no words (spec 027 D35, corrected). This
 * script takes that output and makes it installable:
 *
 *   1. drops what an extension has no door to (the fixture galleries, robots.txt);
 *   2. EXTERNALISES every inline `<script>`, because MV3 extension pages refuse
 *      inline script and a `sha256-` hash makes the extension fail to LOAD
 *      rather than helping — measured, not assumed;
 *   3. copies the manifest and the page-side scripts alongside;
 *   4. prints the package's size, which is a budget (SC-308).
 *
 * Usage: node extension/build.mjs [--skip-build]
 */
import { execFileSync } from 'node:child_process';
import { build as esbuild } from 'esbuild';
import {
	cpSync,
	existsSync,
	readFileSync,
	readdirSync,
	rmSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const APP = join(HERE, '..');
const DIST = join(HERE, 'dist');
const SITE = DIST;
/**
 * Where kit puts the client bundle — `kit.appDir`, which vite.config.ts moves
 * off its `_app` default for this target ONLY, because Chrome refuses to load
 * any extension holding a top-level name that starts with `_`. It is spelled
 * out here because this is the file that asserts the build ran: if the two ever
 * drift, the check below is what says so.
 */
const APP_DIR = 'app';

/** The page-side scripts, bundled from their module sources (below). */
const ENTRIES = ['inpage.js', 'content.js', 'background.js', 'panel.js'];
/** What must NOT be copied verbatim: build inputs and the bundler's own sources. */
const SKIP_COPY = new Set(['dist', 'build.mjs', 'README.md', 'lib', ...ENTRIES]);

/** Route trees the extension has no entry point for. */
const PRUNE = ['gallery', 'gallery.html'];
/** Files that only make sense to a crawler. */
const PRUNE_ROOT = ['robots.txt'];

function log(...parts) {
	console.log('[extension]', ...parts);
}

// ---------------------------------------------------------------------------
// 1. Build
// ---------------------------------------------------------------------------

if (!process.argv.includes('--skip-build')) {
	rmSync(DIST, { recursive: true, force: true });
	log('building the app for the extension target…');
	execFileSync('pnpm', ['exec', 'vite', 'build'], {
		cwd: APP,
		stdio: 'inherit',
		env: { ...process.env, VELA_TARGET: 'extension' }
	});
}
if (!existsSync(join(SITE, APP_DIR))) {
	console.error(
		`[extension] no build output at extension/dist/${APP_DIR} — run without --skip-build` +
			' (or check that vite.config.ts still sets kit.appDir for the extension target)'
	);
	process.exit(1);
}

// ---------------------------------------------------------------------------
// 2. Prune
// ---------------------------------------------------------------------------

let pruned = 0;
const prune = (path) => {
	if (!existsSync(path)) return;
	pruned += du(path);
	rmSync(path, { recursive: true, force: true });
};
for (const entry of readdirSync(SITE)) {
	const localeDir = join(SITE, entry);
	if (statSync(localeDir).isDirectory()) for (const name of PRUNE) prune(join(localeDir, name));
}
for (const name of PRUNE_ROOT) prune(join(SITE, name));
prune(join(SITE, 'dev'));
log(`pruned ${(pruned / 1e6).toFixed(1)} MB the extension has no door to`);

// ---------------------------------------------------------------------------
// 3. Externalise every inline script
// ---------------------------------------------------------------------------

/**
 * An inline script becomes a sibling file, NOT a bundled module.
 *
 * Two properties have to survive the move, and both do only because the file
 * lands in the same directory as its page:
 *   - the first script is render-blocking (it decides before first paint
 *     whether the launch animation plays); a classic `<script src>` still is,
 *     while `type="module"` would be deferred;
 *   - the second resolves `import("../app/…")` and `document.currentScript`
 *     against its own URL, so same directory means same resolution.
 */
function externalise(htmlPath) {
	const html = readFileSync(htmlPath, 'utf8');
	let index = 0;
	const base = htmlPath.slice(0, -'.html'.length).split('/').at(-1);
	const written = [];
	const next = html.replace(
		/<script(?![^>]*\ssrc=)([^>]*)>([\s\S]*?)<\/script>/g,
		(_all, attrs, body) => {
			index += 1;
			const name = `${base}.boot${index}.js`;
			writeFileSync(join(dirname(htmlPath), name), body);
			written.push(name);
			return `<script${attrs} src="${name}"></script>`;
		}
	);
	if (written.length) writeFileSync(htmlPath, next);
	return written.length;
}

const htmlFiles = [];
(function walk(dir) {
	for (const name of readdirSync(dir)) {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) walk(path);
		else if (name.endsWith('.html')) htmlFiles.push(path);
	}
})(SITE);

let scripts = 0;
for (const path of htmlFiles) scripts += externalise(path);
log(`externalised ${scripts} inline scripts across ${htmlFiles.length} pages`);

// ---------------------------------------------------------------------------
// 4. The extension's own files
// ---------------------------------------------------------------------------

/**
 * The page-side scripts are BUNDLED, not copied.
 *
 * MV3 content scripts are classic scripts: `import` is a syntax error there,
 * and only the service worker may declare `"type": "module"`. They are written
 * as modules anyway, because `lib/protocol.js` has to be one file that all
 * three sides agree on — and a shared constant copied three times is a
 * constant that will disagree three ways.
 *
 * Not minified, on purpose: what runs in a stranger's page should be readable
 * in their own dev tools.
 */
await esbuild({
	entryPoints: ENTRIES.map((name) => join(HERE, name)),
	outdir: DIST,
	bundle: true,
	format: 'iife',
	target: ['chrome116'],
	minify: false,
	sourcemap: false,
	logLevel: 'warning'
});
log(`bundled ${ENTRIES.join(', ')}`);

for (const name of readdirSync(HERE)) {
	if (SKIP_COPY.has(name)) continue;
	cpSync(join(HERE, name), join(DIST, name), { recursive: true });
}

// ---------------------------------------------------------------------------
// 5. The budget
// ---------------------------------------------------------------------------

function du(path) {
	if (!existsSync(path)) return 0;
	const stat = statSync(path);
	if (!stat.isDirectory()) return stat.size;
	return readdirSync(path).reduce((sum, name) => sum + du(join(path, name)), 0);
}

const total = du(DIST);
const files = htmlFiles.length;
log(`package: ${(total / 1e6).toFixed(1)} MB · ${files} pages · ${relative(APP, DIST)}`);
