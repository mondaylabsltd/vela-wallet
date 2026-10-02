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
 * Usage: node extension/build.mjs [--skip-build] [--zip]
 *
 * The package goes to `extension/dist`, or to `VELA_EXTENSION_DIST` (relative
 * to the app, or absolute) — the isolated e2e's own directory, so a test run
 * never rewrites the package a running browser has loaded (spec 082 RJ21).
 * `vite.config.ts` reads the same variable for the pages it prerenders.
 *
 * That package is the DEVELOPMENT one: it keeps the manifest's `key` (the
 * pinned id the e2e computes) and the parallel space (the fixture wallet the
 * e2e and the device pass enter through). Every build also derives the two
 * RELEASE packages from it (spec 094), beside it, with no developer pages —
 * the parallel space goes like the galleries (owner ruling 2026-10-02: release
 * artefacts carry no developer features):
 *
 *   - `extension/dist-release` (`<VELA_EXTENSION_DIST>-release`, or
 *     `VELA_EXTENSION_RELEASE_DIST`) — the GitHub release's "Load unpacked"
 *     package. It keeps `key`, so a tester's id stays the same from one
 *     version to the next;
 *   - `extension/dist-store` (`-store`, or `VELA_EXTENSION_STORE_DIST`) — the
 *     Chrome Web Store upload, without `key`: the store assigns the item's id
 *     and refuses a first upload whose manifest carries one. Nothing in the
 *     product depends on the id — the passkey's relying party is `getvela.app`
 *     by host permission.
 *
 * The package test reads all three.
 *
 * `--zip` zips the two release packages' CONTENTS (manifest.json at the root,
 * which is what "Load unpacked" and the Web Store both expect) into the app
 * directory: `vela-wallet-extension-<version>.zip` (the GitHub release) and
 * `vela-wallet-extension-<version>-chrome-web-store.zip` (the upload).
 *
 * The build is self-sufficient (spec 094 B2): it checks the design tokens and
 * copies the committed core wasm into `static/` first — the two steps
 * `pnpm build` runs before `vite build` — so a package can never reference a
 * wasm it does not carry.
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
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const APP = join(HERE, '..');
const DIST = process.env.VELA_EXTENSION_DIST
	? resolve(APP, process.env.VELA_EXTENSION_DIST)
	: join(HERE, 'dist');
const SITE = DIST;
/**
 * Where kit puts the client bundle — `kit.appDir`, which vite.config.ts moves
 * off its `_app` default for this target ONLY, because Chrome refuses to load
 * any extension holding a top-level name that starts with `_`. It is spelled
 * out here because this is the file that asserts the build ran: if the two ever
 * drift, the check below is what says so.
 */
const APP_DIR = 'app';

/**
 * The scripts this package ships, bundled from their module sources (below).
 *
 * `inpage.js` is not in this folder: the page-side provider is THE one every
 * Vela injects, and its home is the core crate (spec 070) — the desktop, iOS
 * and Android in-app browsers embed the same bytes through
 * `vela_core::app::dapp_rpc::provider_script`.
 */
const PROVIDER = join(
	HERE,
	'..',
	'..',
	'..',
	'rust',
	'crates',
	'vela-core',
	'provider',
	'inpage.js'
);
const ENTRIES = ['inpage.js', 'content.js', 'background.js', 'panel.js', 'open.js'];
const SOURCES = { 'inpage.js': PROVIDER };
/** What must NOT be copied verbatim: build inputs and the bundler's own sources. */
const SKIP_COPY = new Set([
	'dist',
	'dist-release',
	'dist-store',
	'build.mjs',
	'README.md',
	'release-notes.md',
	'lib',
	...ENTRIES
]);

/** Route trees the extension has no entry point for. */
const PRUNE = ['gallery', 'gallery.html'];
/**
 * Per-locale routes that are developer features: in the development package
 * (the e2e and the device pass enter the fixture wallet through them), never
 * in the store's (owner ruling 2026-10-02). The page, its externalised
 * scripts (`parallel.boot<n>.js`) and its data directory all go, so not even a
 * client navigation can reach it.
 */
const STORE_PRUNE = ['parallel'];
const storePruned = (name) =>
	STORE_PRUNE.some((route) => name === route || name.startsWith(`${route}.`));
/** Files that only make sense to a crawler. */
const PRUNE_ROOT = ['robots.txt'];

/** Where the two release packages go: beside the development one. */
const RELEASE_DIST = process.env.VELA_EXTENSION_RELEASE_DIST
	? resolve(APP, process.env.VELA_EXTENSION_RELEASE_DIST)
	: `${DIST}-release`;
const STORE_DIST = process.env.VELA_EXTENSION_STORE_DIST
	? resolve(APP, process.env.VELA_EXTENSION_STORE_DIST)
	: `${DIST}-store`;

const ZIP = process.argv.includes('--zip');

function log(...parts) {
	console.log('[extension]', ...parts);
}

// ---------------------------------------------------------------------------
// 1. Build
// ---------------------------------------------------------------------------

if (!process.argv.includes('--skip-build')) {
	// What `pnpm build` does before `vite build`, and this did not (spec 094
	// B2): a fresh checkout has no `static/vela_core_bg.<hash>.wasm` (it is a
	// gitignored copy), so the package referenced a core it did not carry and
	// every page that needs the core failed to load it.
	execFileSync('node', ['scripts/gen-tokens.mjs', '--check'], { cwd: APP, stdio: 'inherit' });
	execFileSync('node', ['scripts/sync-wasm.mjs'], { cwd: APP, stdio: 'inherit' });
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
		`[extension] no build output at ${relative(APP, DIST)}/${APP_DIR} — run without --skip-build` +
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
	entryPoints: Object.fromEntries(
		ENTRIES.map((name) => [name.replace(/\.js$/, ''), SOURCES[name] ?? join(HERE, name)])
	),
	outdir: DIST,
	bundle: true,
	format: 'iife',
	target: ['chrome122'],
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

// ---------------------------------------------------------------------------
// 6. The release packages, and the zips (--zip)
// ---------------------------------------------------------------------------

const manifest = JSON.parse(readFileSync(join(DIST, 'manifest.json'), 'utf8'));

/** The development package without its developer pages — and, for the store, without `key`. */
function deriveReleasePackage(target, { keepKey }) {
	rmSync(target, { recursive: true, force: true });
	cpSync(DIST, target, { recursive: true });
	let bytes = 0;
	for (const entry of readdirSync(target)) {
		const localeDir = join(target, entry);
		if (!statSync(localeDir).isDirectory()) continue;
		for (const name of readdirSync(localeDir).filter(storePruned)) {
			const path = join(localeDir, name);
			bytes += du(path);
			rmSync(path, { recursive: true, force: true });
		}
	}
	const released = { ...manifest };
	if (!keepKey) delete released.key;
	writeFileSync(join(target, 'manifest.json'), JSON.stringify(released, null, '\t') + '\n');
	log(
		`${keepKey ? 'release package (key kept)' : 'store package: no key'}, ` +
			`${(bytes / 1e3).toFixed(0)} kB of developer pages pruned · ` +
			`${(du(target) / 1e6).toFixed(1)} MB · ${relative(APP, target)}`
	);
}
deriveReleasePackage(RELEASE_DIST, { keepKey: true });
deriveReleasePackage(STORE_DIST, { keepKey: false });

if (ZIP) {
	const zipInto = (dir, name) => {
		const target = join(APP, name);
		rmSync(target, { force: true });
		// The CONTENTS of the package, manifest.json at the zip's root.
		execFileSync('zip', ['-qrX', target, '.', '-x', '*.DS_Store'], { cwd: dir, stdio: 'inherit' });
		log(`zip: ${name} (${(statSync(target).size / 1e6).toFixed(1)} MB)`);
	};
	zipInto(RELEASE_DIST, `vela-wallet-extension-${manifest.version}.zip`);
	zipInto(STORE_DIST, `vela-wallet-extension-${manifest.version}-chrome-web-store.zip`);
}
