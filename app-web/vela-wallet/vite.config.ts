import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import cloudflareAdapter from '@sveltejs/adapter-cloudflare';
import staticAdapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { withBrotliCoreAdapter } from './scripts/wasm-brotli.mjs';
import { createHash } from 'node:crypto';
import type { KitConfig } from '@sveltejs/kit';
import { CSP_DIRECTIVES } from './src/lib/security-headers';

/**
 * `docs/design/onboarding/launch` is the repo-wide source of truth for animations
 * (spec 012 FR-001) and lives OUTSIDE this app. Aliasing it, plus opening it to
 * the dev server's fs allow-list, is what lets Vite emit the four `core` files
 * as hashed assets served from our own origin — no copy under app-web/, and no
 * third-party CDN at runtime.
 */
const LAUNCH_ANIMATIONS = fileURLToPath(
	new URL('../../docs/design/onboarding/launch', import.meta.url)
);

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

/**
 * §35 (review 1-1): the hosted wallet's CSP, hash-based. SvelteKit hashes its
 * own inline scripts; `app.html`'s one render-blocking script (theme, text
 * scale, the launch and intro gates) is hashed here, from the file itself, so
 * an edit to it can never ship unhashed — the page would refuse to run it. The
 * extension's pages have the manifest's own CSP (`extension/manifest.json`),
 * and `extension/build.mjs` moves every inline script out to a file.
 */
function appHtmlScriptHash(): string {
	const html = readFileSync(fileURLToPath(new URL('./src/app.html', import.meta.url)), 'utf8');
	const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
	if (scripts.length !== 1)
		throw new Error(`app.html: expected one inline script, got ${scripts.length}`);
	return `sha256-${createHash('sha256').update(scripts[0][1], 'utf8').digest('base64')}`;
}

/**
 * §38 (review 0-5): the signing page's script, for every page the wallet opens
 * FROM THE BYTES ITS CHECK HASHED (`page-handoff`, `fromBytes`). Such a page is
 * a `blob:` document, and a blob document is held to its creator's policy as
 * well as its own — so without its script's hash here the page would open
 * and run nothing (measured in Chromium, Firefox and WebKit).
 *
 * Which pages is the core's word (`trustedSignerFromBytes`, read from the
 * shipped wasm); each page's bytes are the committed deployment's, hashed
 * again here so a file that is not its version fails the build, and the
 * script hash is read from that page's own `<meta>` CSP — the one value the
 * browser will compare. Only that one inline script is admitted, by hash:
 * nothing else of the page's policy is loosened, and the page's own
 * `default-src 'none'` still governs it.
 */
async function signingPageScriptHashes(): Promise<string[]> {
	const loader = pathToFileURL(
		fileURLToPath(new URL('../../rust/scripts/load-wasm-node.mjs', import.meta.url))
	).href;
	const { loadShippedCore } = (await import(/* @vite-ignore */ loader)) as {
		loadShippedCore: () => Promise<{ trustedSignerFromBytes?: () => string }>;
	};
	const core = await loadShippedCore();
	if (typeof core.trustedSignerFromBytes !== 'function') {
		throw new Error('the shipped core has no trustedSignerFromBytes — run build:wasm');
	}
	const versions = JSON.parse(core.trustedSignerFromBytes()) as string[];
	return versions.flatMap((version) => {
		const bytes = readFileSync(
			fileURLToPath(new URL(`../trusted-signer/dist/b/${version}/sign.html`, import.meta.url))
		);
		const served = createHash('sha256').update(bytes).digest('hex');
		if (served !== version) {
			throw new Error(`trusted-signer/dist/b/${version}/sign.html hashes to ${served}`);
		}
		const policy = /<meta http-equiv="Content-Security-Policy" content="([^"]+)"/.exec(
			bytes.toString('utf8')
		)?.[1];
		const scripts = policy
			?.split(';')
			.map((directive) => directive.trim().split(/\s+/))
			.find(([name]) => name === 'script-src')
			?.slice(1)
			.filter((token) => /^'sha256-[A-Za-z0-9+/]+=*'$/.test(token))
			.map((token) => token.slice(1, -1));
		if (!scripts?.length) throw new Error(`signing page ${version} names no script hash`);
		return scripts;
	});
}

const WALLET_CSP: NonNullable<KitConfig['csp']> = {
	mode: 'hash',
	directives: {
		...Object.fromEntries(Object.entries(CSP_DIRECTIVES).map(([k, v]) => [k, [...v]])),
		'script-src': [
			...CSP_DIRECTIVES['script-src'],
			appHtmlScriptHash(),
			...(EXTENSION_TARGET ? [] : await signingPageScriptHashes())
		]
	} as NonNullable<KitConfig['csp']>['directives']
};

/**
 * Where the extension build lands: `extension/dist` — the package Chrome
 * loads, the one a person or the device pass has open — unless
 * `VELA_EXTENSION_DIST` names another directory. The isolated e2e sets it
 * (`playwright.isolated.config.ts`), so a test run never rewrites the package
 * under a running browser (spec 082 RJ21, G66: the pre-opened side panel
 * failed its next navigation). `extension/build.mjs` reads the same variable.
 *
 * The Safari package's default is `extension/dist-safari`, where
 * `build.mjs --safari` looks for it — without this, `bun run
 * build:extension:safari` wrote Safari's pages over the Chrome package and
 * then failed for want of its own ("no build output at extension/dist-safari").
 */
const EXTENSION_OUT =
	process.env.VELA_EXTENSION_DIST ||
	(process.env.VELA_EXTENSION_BROWSER === 'safari' ? 'extension/dist-safari' : 'extension/dist');

/**
 * The scheme the extension's own pages are served from: `chrome-extension:`,
 * or `safari-web-extension:` when `extension/build.mjs --safari` builds the
 * Safari package (`VELA_EXTENSION_BROWSER=safari`). Every other build — the
 * hosted site, the tests, the Chrome package — gets `chrome-extension:`, so
 * their output is the bytes it was before the Safari target existed: the
 * constant replaces the literal it stands for.
 */
const EXTENSION_PROTOCOL =
	EXTENSION_TARGET && process.env.VELA_EXTENSION_BROWSER === 'safari'
		? 'safari-web-extension:'
		: 'chrome-extension:';

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
		__VELA_COMMIT__: JSON.stringify(BUILD.commit),
		// Which build this is: usage analytics report from the hosted wallet's
		// own origin in one, and only from `chrome-extension:` pages in the
		// other (`$lib/analytics/gate.ts`).
		__VELA_EXTENSION__: JSON.stringify(EXTENSION_TARGET),
		// §28: a TEST build's opt-in to production writes (the dev server's;
		// a development package's comes from its marker, extension/build.mjs).
		// Meaningless in any other build: only a test build installs the guard.
		__VELA_ALLOW_PRODUCTION_WRITES__: JSON.stringify(
			process.env.VELA_TARGET === 'extension'
				? ''
				: (process.env.VELA_ALLOW_PRODUCTION_WRITES ?? '')
		),
		// Which browser's package this is, by the scheme its pages load from
		// (`$lib/extension/page-url.ts`, `$lib/dapp/transport.ts`).
		__VELA_EXTENSION_PROTOCOL__: JSON.stringify(EXTENSION_PROTOCOL)
	},
	resolve: {
		alias: { $animations: LAUNCH_ANIMATIONS }
	},
	server: {
		fs: { allow: [LAUNCH_ANIMATIONS] },
		// The tests' own output (traces, screenshots) is no source: a dev server
		// watching it reloads the very page a live test is driving.
		watch: { ignored: ['**/e2e/**/.results/**', '**/e2e/**/.shots/**', '**/test-results/**'] }
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
			...(EXTENSION_TARGET ? { appDir: 'app' } : { csp: WALLET_CSP }),
			adapter: EXTENSION_TARGET
				? staticAdapter({
						// The app is served from the extension's ROOT, not a
						// subdirectory. Its absolute references — `/vela_core_bg.<hash>.wasm`
						// above all — then resolve exactly as they do on the hosted
						// site, instead of needing a base-aware rewrite the hosted
						// build would have to carry too.
						pages: EXTENSION_OUT,
						assets: EXTENSION_OUT,
						// `/` is the hosted site's Accept-Language negotiation endpoint —
						// a dynamic route by nature, and meaningless inside an extension
						// that opens its pages by file URL. `strict` would demand a
						// fallback page for it; shipping a bogus shell the extension
						// never opens is worse than saying so here.
						strict: false
					})
				: // The core's wasm at brotli 11, and the Worker that serves it (perf-4).
					withBrotliCoreAdapter(cloudflareAdapter())
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
					setupFiles: ['./vitest-setup-client.ts']
				}
			},

			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
