import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import cloudflareAdapter from '@sveltejs/adapter-cloudflare';
import staticAdapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

/**
 * `docs/design/onboarding/launch` is the repo-wide source of truth for animations
 * (spec 012 FR-001) and lives OUTSIDE this app. Aliasing it, plus opening it to
 * the dev server's fs allow-list, is what lets Vite emit the four `core` files
 * as hashed assets served from our own origin — no copy under app-web/, and no
 * third-party CDN at runtime.
 */
const LAUNCH_ANIMATIONS = fileURLToPath(new URL('../../docs/design/onboarding/launch', import.meta.url));

/**
 * Spec 027: the same application, built a second way.
 *
 * `VELA_TARGET=extension` builds the browser extension's shell instead of the
 * hosted site. Two kit options change and nothing else does:
 *
 * 1. the STATIC adapter, because a `chrome-extension://` page is a file, not a
 *    request — there is no Worker to run. Prerendering and pathname routing
 *    stay, and that is deliberate.
 * 2. `appDir`, away from its default `_app`. Chrome reserves every top-level
 *    name beginning with `_` inside an extension package, so it refuses the
 *    PACKAGE, not the file: "Cannot load extension with file or directory name
 *    _app. Filenames starting with `_` are reserved for use by the system."
 *    Nothing in the automated suite could have caught this — Playwright's
 *    `--load-extension` tolerates the reserved name, so the extension e2e ran
 *    green for weeks against a package no person could install by hand from
 *    chrome://extensions. The hosted site keeps `_app`: its asset URLs are
 *    public and cached, and no browser rule touches them.
 *
 * The obvious-looking alternative, `router.type: 'hash'` (which SvelteKit
 * documents for exactly this case), is NOT usable here: it forbids server files
 * anywhere in the route tree, and this app's entire i18n is build-time SSR —
 * every string in all 15 locales is resolved by the wasm engine inside
 * `+*.server.ts` load functions at prerender time, and only serialized strings
 * reach the client (CLAUDE.md hard rule 2). Hash routing would trade the
 * corpus for a router. So the extension packages the SAME prerendered pages the
 * site serves and enters them by file URL; the SPA router takes over from there
 * (spec 027 D35, corrected).
 *
 * What still has to be dealt with downstream: those prerendered pages carry
 * inline `<script>`, and MV3 extension pages refuse inline script outright —
 * with a `sha256-` hash making the extension fail to LOAD rather than helping.
 * `extension/build.mjs` externalises them.
 *
 * The hosted build reads none of this, so its output — and therefore its
 * budgets — cannot move because the extension exists.
 */
const EXTENSION_TARGET = process.env.VELA_TARGET === 'extension';

/** The one node suite that must meet an UNinitialized core (see `test.projects`). */
const CORE_LOADER_TEST = 'src/lib/core/client.test.ts';

/**
 * The version and commit every page of this build reports (spec 064 §3).
 *
 * VERSION is `extension/manifest.json`'s — the one declared version this app
 * has, and the one the release gate checks against the release branch's name.
 * (`package.json` says 0.0.1 and always has; nothing reads it.)
 *
 * COMMIT, first that answers: `VELA_GIT_COMMIT` (CI sets it from the commit it
 * checked out), `WORKERS_CI_COMMIT_SHA` (Cloudflare's build, which deploys the
 * web wallet from the `released` branch), `git rev-parse` (a developer's
 * machine), and otherwise the word `unknown` — never a constant that looks
 * like a commit.
 */
function buildIdentity(): { version: string; commit: string } {
	const manifest = JSON.parse(
		readFileSync(fileURLToPath(new URL('./extension/manifest.json', import.meta.url)), 'utf8')
	) as { version: string };
	const fromEnv = process.env.VELA_GIT_COMMIT || process.env.WORKERS_CI_COMMIT_SHA;
	let commit = fromEnv?.trim();
	if (!commit) {
		try {
			commit = execFileSync('git', ['rev-parse', 'HEAD'], { stdio: ['ignore', 'pipe', 'ignore'] })
				.toString()
				.trim();
		} catch {
			commit = '';
		}
	}
	return { version: manifest.version, commit: commit ? commit.slice(0, 7) : 'unknown' };
}
const BUILD = buildIdentity();

export default defineConfig({
	define: {
		__VELA_VERSION__: JSON.stringify(BUILD.version),
		__VELA_COMMIT__: JSON.stringify(BUILD.commit)
	},
	resolve: {
		alias: { $animations: LAUNCH_ANIMATIONS }
	},
	server: {
		fs: { allow: [LAUNCH_ANIMATIONS] }
	},
	/**
	 * The two QR decoders, pre-bundled (spec 028). Without this, vitest's browser
	 * project discovers them mid-run, re-optimises and RELOADS the test — which
	 * Vitest itself warns "may cause flaky behaviour or duplicated test runs".
	 * Naming them here is the fix it asks for.
	 */
	optimizeDeps: {
		include: ['@undecaf/zbar-wasm', 'jsqr']
	},
	plugins: [
		sveltekit({
			// Spec 038: poll the deployed version so a deploy mid-session turns
			// the next navigation into a full load (see +layout.svelte).
			version: { pollInterval: 60_000 },
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// `_app` → `app` for the extension only; see (2) above. This has to be
			// set here rather than fixed up in `extension/build.mjs`, because the
			// name is baked into every prerendered page's asset references.
			...(EXTENSION_TARGET ? { appDir: 'app' } : {}),
			adapter: EXTENSION_TARGET
				? staticAdapter({
						// The app is served from the extension's ROOT, not a
						// subdirectory. Its absolute references — `/vela_core_bg.<hash>.wasm`
						// above all — then resolve exactly as they do on the hosted
						// site, instead of needing a base-aware rewrite the hosted
						// build would have to carry too.
						pages: 'extension/dist',
						assets: 'extension/dist',
						// `/` is the hosted site's Accept-Language negotiation endpoint —
						// a dynamic route by nature, and meaningless inside an extension
						// that opens its pages by file URL. `strict` would demand a
						// fallback page for it; shipping a bogus shell the extension
						// never opens is worse than saying so here.
						strict: false
					})
				: cloudflareAdapter()
		})
	],
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**'],
					// The speed rules are the core's (spec 068); see the file.
					setupFiles: ['src/lib/core/test-setup.browser.ts']
				}
			},

			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}', CORE_LOADER_TEST],
					// The speed rules are the core's (spec 068); see the file.
					setupFiles: ['src/lib/core/test-setup.server.ts']
				}
			},
			{
				// The runtime loader's own contract is about a module that has NOT
				// been initialized yet (a failed load must not be cached), so it
				// runs without the core the setup above puts in place.
				extends: './vite.config.ts',
				test: {
					name: 'server-bare',
					environment: 'node',
					include: [CORE_LOADER_TEST]
				}
			}
		]
	}
});
