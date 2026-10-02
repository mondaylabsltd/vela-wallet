/**
 * The packaged extension's silent failure modes (spec 027 T312).
 *
 * Each was measured, and none of them raises an error anyone would notice: an
 * inline `<script>` simply never runs under MV3's extension-page CSP (and a
 * `sha256-` hash for it makes Chrome refuse to LOAD the extension at all), a
 * manifest without `'wasm-unsafe-eval'` compiles no wasm — which for this
 * product means every decision it makes is gone — and a top-level name
 * beginning with `_` makes Chrome reject the package on install, which no e2e
 * can see because Playwright loads extensions by a path that tolerates it.
 *
 * These read the BUILT package. When it is absent the tests say so rather than
 * passing quietly: a budget you skipped is not a budget you met.
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const APP_ROOT = join(import.meta.dirname, '../../..');
/**
 * The package under test: the one `extension/build.mjs` just wrote —
 * `VELA_EXTENSION_DIST` (relative to the app, or absolute) when set, as the
 * isolated e2e and the gates set it so a running browser's package is never
 * rewritten (spec 082 RJ21), else `extension/dist`. The same rule as the build
 * and `e2e/extension-helpers.ts`: reading `extension/dist` after a build that
 * went elsewhere passed a package nobody had checked.
 */
const DIST = process.env.VELA_EXTENSION_DIST
	? resolve(APP_ROOT, process.env.VELA_EXTENSION_DIST)
	: join(APP_ROOT, 'extension/dist');
/**
 * The Chrome Web Store package (spec 094), which every build derives beside the
 * development one — `build.mjs`'s rule: `VELA_EXTENSION_STORE_DIST`, else
 * `<development package>-store`.
 */
const STORE_DIST = process.env.VELA_EXTENSION_STORE_DIST
	? resolve(APP_ROOT, process.env.VELA_EXTENSION_STORE_DIST)
	: `${DIST}-store`;
/** The GitHub release's "Load unpacked" package — `key` kept, no developer pages. */
const RELEASE_DIST = process.env.VELA_EXTENSION_RELEASE_DIST
	? resolve(APP_ROOT, process.env.VELA_EXTENSION_RELEASE_DIST)
	: `${DIST}-release`;
const MANIFEST = join(APP_ROOT, 'extension/manifest.json');
/** The wasm the app's code names (`WASM_URL`), from the build's own source of it. */
const WASM_URL_MODULE = join(APP_ROOT, '../../rust/pkg-web/vela_core_wasm_url.js');

/** Every `.html` in the package. */
function pages(dir: string): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return pages(path);
		return name.endsWith('.html') ? [path] : [];
	});
}

/** Every file in the package, as paths relative to its root. */
function files(dir: string, prefix = ''): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		const relative = prefix ? `${prefix}/${name}` : name;
		return statSync(path).isDirectory() ? files(path, relative) : [relative];
	});
}

/** An inline script is one with a body and no `src`. */
const INLINE_SCRIPT = /<script(?![^>]*\ssrc=)([^>]*)>([\s\S]*?)<\/script>/g;
/** A core artifact named anywhere in a script: `vela_core_bg.<hash>.wasm`. */
const WASM_NAME = /vela_core_bg\.[0-9a-f]+\.wasm/g;

describe('the manifest', () => {
	const manifest = JSON.parse(readFileSync(MANIFEST, 'utf8'));

	it('declares wasm, or the core never compiles', () => {
		const csp = manifest.content_security_policy?.extension_pages ?? '';
		expect(csp, 'extension_pages CSP').toContain("'wasm-unsafe-eval'");
	});

	it('holds the host permission the passkey ceremony needs', () => {
		// Without it the ceremony cannot claim `getvela.app` as its relying
		// party, and the extension silently becomes a DIFFERENT wallet at a
		// different address (spec 027 D31).
		expect(manifest.host_permissions).toContain('https://getvela.app/*');
	});

	it('pins its own id, so the relying party and the tests address one origin', () => {
		// The DEVELOPMENT manifest only: the store package drops it (below).
		expect(typeof manifest.key).toBe('string');
		expect(manifest.key.length).toBeGreaterThan(300);
	});

	it('asks for the Chrome that lets an extension page use the getvela.app passkey (spec 094)', () => {
		// Below 122, an extension page may not claim a relying party it holds
		// host permission for: the passkey ceremony fails on a Chrome the
		// store would still have installed it on.
		expect(manifest.minimum_chrome_version).toBe('122');
	});

	it('stays out of incognito windows (spec 094)', () => {
		expect(manifest.incognito).toBe('not_allowed');
	});

	it('lets no web page reach into the package (spec 089)', () => {
		// The provider is a MAIN-world content script: Chrome injects it, and no
		// page ever needs to fetch it. Listed in `web_accessible_resources` it was
		// fetchable by every site at the pinned id — a free "is Vela installed?"
		// probe — for nothing. And no page may message the worker directly.
		expect(manifest.web_accessible_resources).toBeUndefined();
		expect(manifest.externally_connectable).toBeUndefined();
	});

	it('opens no action popup — a popup cannot survive a passkey prompt', () => {
		// Spec 027 D34: the popup is dismissed when focus moves to the
		// authenticator, mid-ceremony. The toolbar button opens a tab instead.
		expect(manifest.action?.default_popup).toBeUndefined();
	});

	it('answers a request in the side panel, which survives that prompt', () => {
		expect(manifest.permissions).toContain('sidePanel');
		expect(manifest.side_panel?.default_path).toBe('panel.html');
		expect(existsSync(join(APP_ROOT, 'extension', manifest.side_panel.default_path))).toBe(true);
	});

	it('wears the wallet mark, at every size Chrome asks for', () => {
		// The mark is docs/design/icon/app-icon.svg, rendered; without `icons` Chrome
		// shows a grey puzzle piece for the whole product.
		for (const size of ['16', '32', '48', '128']) {
			expect(manifest.icons?.[size], `icons.${size}`).toBeDefined();
			expect(existsSync(join(APP_ROOT, 'extension', manifest.icons[size]))).toBe(true);
			expect(manifest.action?.default_icon?.[size]).toBe(manifest.icons[size]);
		}
	});
});

/**
 * What both packages must be — the development one the e2e loads, and the one
 * uploaded to the store, which is derived from it (spec 094).
 */
describe.each([
	['development', DIST],
	['release', RELEASE_DIST],
	['store', STORE_DIST]
])('the %s package', (_name, dist) => {
	const built = pages(dist);

	it('exists — run `pnpm build:extension` before trusting this file', () => {
		expect(built.length).toBeGreaterThan(0);
	});

	it('sits beside no top-level `_` name, or Chrome installs none of it', () => {
		// Chrome refuses the PACKAGE, not the file: "Load unpacked" answers
		// "Cannot load extension with file or directory name _app. Filenames
		// starting with `_` are reserved for use by the system." — so this is a
		// the-extension-cannot-be-installed failure, not an untidy one. Kit's
		// `appDir` defaults to exactly that name, which is why the fix lives in
		// vite.config.ts rather than in a rename here. Only this assertion can
		// catch a regression: every extension e2e passed while the shipped
		// package was uninstallable by hand, because Playwright's
		// `--load-extension` accepts a name chrome://extensions rejects.
		const reserved = readdirSync(dist).filter((name) => name.startsWith('_'));
		expect(reserved, 'top-level names Chrome reserves for itself').toEqual([]);
	});

	it('carries no inline script anywhere', () => {
		const carriers = built.filter((path) => {
			const html = readFileSync(path, 'utf8');
			INLINE_SCRIPT.lastIndex = 0;
			return [...html.matchAll(INLINE_SCRIPT)].some((match) => match[2].trim().length > 0);
		});
		expect(carriers, 'pages whose scripts MV3 will refuse to run').toEqual([]);
	});

	it('ships the core artifact at the root the app addresses it by', () => {
		// `WASM_URL` is absolute (`/vela_core_bg.<hash>.wasm`), which is why the
		// app is packaged at the extension's ROOT rather than under a folder.
		const artifacts = readdirSync(dist).filter((name) => /^vela_core_bg\..*\.wasm$/.test(name));
		expect(artifacts).toHaveLength(1);
	});

	it('carries the very wasm its code names (spec 094 B2)', () => {
		// A package that names a core it does not carry loads every page and
		// then cannot decide anything: `pnpm build:extension` used to skip the
		// copy into `static/`, and the package shipped whichever wasm was
		// there — none on a fresh checkout. Asked of the BUILT code: every
		// artifact any script names must be a file at the root, and the one the
		// source names now is among them.
		const named = new Set<string>();
		for (const path of files(dist).filter((file) => file.endsWith('.js'))) {
			for (const match of readFileSync(join(dist, path), 'utf8').matchAll(WASM_NAME)) {
				named.add(match[0]);
			}
		}
		const current = /export const WASM_URL = '\/([^']+)'/.exec(
			readFileSync(WASM_URL_MODULE, 'utf8')
		)?.[1];
		expect(current, 'rust/pkg-web/vela_core_wasm_url.js names a wasm').toBeTruthy();
		expect([...named], 'the scripts name the current core').toContain(current);
		for (const name of named) {
			expect(existsSync(join(dist, name)), `${name} is in the package`).toBe(true);
		}
	});

	it('ships both doorways, each with its script beside it (issue 317)', () => {
		// The worker opens the request window and the wallet tab at `open.html`,
		// which picks the person's language; the side panel enters at
		// `panel.html`. A doorway missing from the package is a window that
		// opens on "file not found".
		for (const [page, script] of [
			['panel.html', 'panel.js'],
			['open.html', 'open.js']
		]) {
			expect(existsSync(join(dist, page)), page).toBe(true);
			expect(existsSync(join(dist, script)), script).toBe(true);
			expect(readFileSync(join(dist, page), 'utf8')).toContain(`<script src="${script}"></script>`);
		}
	});

	it('declares the version the source manifest does', () => {
		const packaged = JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8'));
		const source = JSON.parse(readFileSync(MANIFEST, 'utf8'));
		expect(packaged.version).toBe(source.version);
	});
});

describe('the development package', () => {
	const manifest = () => JSON.parse(readFileSync(join(DIST, 'manifest.json'), 'utf8'));

	it('keeps the pinned id the e2e computes', () => {
		expect(manifest().key).toBe(JSON.parse(readFileSync(MANIFEST, 'utf8')).key);
	});

	it('keeps the parallel space the e2e enters through', () => {
		expect(existsSync(join(DIST, 'en/parallel.html'))).toBe(true);
	});
});

/** Developer pages: the parallel space, a gallery, the `dev/` tree. */
const developerFiles = (dist: string) =>
	files(dist).filter((path) => /(^|\/)(parallel|gallery)(\.|\/)|^dev\//.test(path));

describe('the release package — the GitHub release’s "Load unpacked" (spec 094)', () => {
	const manifest = () => JSON.parse(readFileSync(join(RELEASE_DIST, 'manifest.json'), 'utf8'));

	it('keeps the pinned id, so a tester’s id stays the same from version to version', () => {
		expect(manifest().key).toBe(JSON.parse(readFileSync(MANIFEST, 'utf8')).key);
	});

	it('is otherwise the source manifest, field for field', () => {
		expect(manifest()).toEqual(JSON.parse(readFileSync(MANIFEST, 'utf8')));
	});

	it('carries no developer pages (owner ruling 2026-10-02)', () => {
		expect(developerFiles(RELEASE_DIST)).toEqual([]);
	});
});

describe('the store package (spec 094)', () => {
	const manifest = () => JSON.parse(readFileSync(join(STORE_DIST, 'manifest.json'), 'utf8'));

	it('carries no `key` — the store assigns the id and refuses an upload with one', () => {
		expect(manifest().key).toBeUndefined();
	});

	it('is otherwise the source manifest, field for field', () => {
		const source = JSON.parse(readFileSync(MANIFEST, 'utf8'));
		delete source.key;
		expect(manifest()).toEqual(source);
	});

	it('carries no developer pages — the parallel space or a gallery (owner ruling 2026-10-02)', () => {
		expect(developerFiles(STORE_DIST)).toEqual([]);
	});
});
