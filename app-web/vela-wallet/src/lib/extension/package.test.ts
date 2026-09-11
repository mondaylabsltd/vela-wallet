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
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const APP_ROOT = join(import.meta.dirname, '../../..');
const DIST = join(APP_ROOT, 'extension/dist');
const MANIFEST = join(APP_ROOT, 'extension/manifest.json');

/** Every `.html` in the package. */
function pages(dir: string): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return pages(path);
		return name.endsWith('.html') ? [path] : [];
	});
}

/** An inline script is one with a body and no `src`. */
const INLINE_SCRIPT = /<script(?![^>]*\ssrc=)([^>]*)>([\s\S]*?)<\/script>/g;

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
		expect(typeof manifest.key).toBe('string');
		expect(manifest.key.length).toBeGreaterThan(300);
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

describe('the packaged pages', () => {
	const built = pages(DIST);

	it('exist — run `pnpm build:extension` before trusting this file', () => {
		expect(built.length).toBeGreaterThan(0);
	});

	it('sit beside no top-level `_` name, or Chrome installs none of them', () => {
		// Chrome refuses the PACKAGE, not the file: "Load unpacked" answers
		// "Cannot load extension with file or directory name _app. Filenames
		// starting with `_` are reserved for use by the system." — so this is a
		// the-extension-cannot-be-installed failure, not an untidy one. Kit's
		// `appDir` defaults to exactly that name, which is why the fix lives in
		// vite.config.ts rather than in a rename here. Only this assertion can
		// catch a regression: every extension e2e passed while the shipped
		// package was uninstallable by hand, because Playwright's
		// `--load-extension` accepts a name chrome://extensions rejects.
		const reserved = readdirSync(DIST).filter((name) => name.startsWith('_'));
		expect(reserved, 'top-level names Chrome reserves for itself').toEqual([]);
	});

	it('carry no inline script anywhere', () => {
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
		const artifacts = readdirSync(DIST).filter((name) => /^vela_core_bg\..*\.wasm$/.test(name));
		expect(artifacts).toHaveLength(1);
	});
});
