/**
 * The device harness's signing page (spec 102 R8 e2e) — a page on this
 * machine's loopback that onboarding creates and signs in on, as "Use my own
 * signing page" does in the apps, and that every hand-off goes to in place of
 * the production page.
 *
 * Why it exists: the Safari package sends every ceremony and signature to a
 * page, and a simulator cannot reach a `sign.getvela.app` served from this
 * machine (no DNS mapping without root) — while a loopback page is a real
 * origin where the simulator's own authenticator makes and uses real passkeys
 * (for `localhost`). So the harness creates the wallet ON that page, and every
 * later signature the core routes to the official page goes to that page too
 * (r11: the sign hand-off named `sign.getvela.app` whatever the seed said, and
 * a local run stood on "Checking the page…").
 *
 * Who may set it, and where it counts — each a fence of its own:
 *
 *   - only the development package's `dev-seed.html` writes the key (`page=`);
 *   - only a DEVELOPMENT package honours it, told by its own manifest
 *     (`isDevelopmentManifest`: it names the harness's pages). The release
 *     package is the development one with those pages and that manifest entry
 *     removed (`extension/build.mjs`), so the same bundle in the release
 *     package reads no override — whatever a stale `localStorage` holds;
 *   - only a loopback ORIGIN with a port (`http(s)://127.0.0.1|localhost|[::1]:<port>`)
 *     is a page at all; anything else is refused, and the production page
 *     stays.
 */
import { isPackagedApp } from '$lib/extension/page-url';
import { isDevelopmentManifest } from '../../../extension/lib/safari-manifest.js';

export const HARNESS_PAGE_KEY = 'vela.dev.signingPage';

/** The production signing page — the core's `trusted_signer::DEFAULT_SIGNER_URL`. */
export const OFFICIAL_SIGNING_PAGE = 'https://sign.getvela.app/';

const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '[::1]']);

/**
 * `value` as a loopback page's base address (`<origin>/`), or `null`: http or
 * https, a loopback host, an explicit port, and nothing after the origin but
 * an optional `/` — no path, query, fragment or credentials.
 */
export function loopbackPage(value: unknown): string | null {
	if (typeof value !== 'string') return null;
	let url: URL;
	try {
		url = new URL(value);
	} catch {
		return null;
	}
	if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
	if (!LOOPBACK_HOSTS.has(url.hostname) || url.port === '') return null;
	if (url.username || url.password || url.search || url.hash || url.pathname !== '/') return null;
	// `new URL` keeps a trailing `?`/`#` out of `search`/`hash`: the text must
	// end at the origin, with at most its slash.
	if (!/^[a-z]+:\/\/[^/?#]+\/?$/i.test(value)) return null;
	return `${url.origin}/`;
}

/**
 * The harness page this package honours, from what it was given: `stored`
 * (the key's value) only in a development package, only when it is a
 * loopback page. `null`: none — the production page.
 */
export function chooseHarnessPage(stored: unknown, development: boolean): string | null {
	return development ? loopbackPage(stored) : null;
}

/**
 * The page a hand-off goes to: the route's `page`, except that the
 * production page is the harness page when this package honours one.
 * Another page the account chose for itself is never replaced.
 */
export function handoffPageFor(page: string, harness: string | null): string {
	if (harness === null) return page;
	let origin: string;
	try {
		origin = new URL(page).origin;
	} catch {
		return page;
	}
	return origin === new URL(OFFICIAL_SIGNING_PAGE).origin ? harness : page;
}

/** This package's manifest, or `null` outside a packaged extension page. */
function ownManifest(): unknown {
	try {
		const runtime = (globalThis as { chrome?: { runtime?: { getManifest?: () => unknown } } })
			.chrome?.runtime;
		return runtime?.getManifest?.() ?? null;
	} catch {
		return null;
	}
}

/** The harness page this document honours (development package only), or `null`. */
export function harnessSigningPage(): string | null {
	if (!isPackagedApp()) return null;
	if (!isDevelopmentManifest(ownManifest())) return null;
	try {
		return chooseHarnessPage(localStorage.getItem(HARNESS_PAGE_KEY), true);
	} catch {
		return null;
	}
}

/** {@link handoffPageFor} with this document's harness page. */
export function handoffPage(page: string): string {
	return handoffPageFor(page, harnessSigningPage());
}
