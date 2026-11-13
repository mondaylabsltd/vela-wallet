/**
 * The core's wasm, asked for by the DOCUMENT on the pages that cannot draw a
 * thing without it (perf-3).
 *
 * Left to `loadCore()`, the 5 MB artifact is requested only once every script
 * has arrived, the page has hydrated and a screen has mounted — on a cold
 * phone (Slow 4G, 4× CPU) that was 9–10 s after the document, and the wallet
 * stayed blank until the download that only then began had finished. A
 * `<link rel="preload">` in the prerendered head starts it with the page's
 * own scripts. `as="fetch"` + `crossorigin` is exactly the request
 * wasm-bindgen's `init` makes (`fetch(url)`: CORS mode, same-origin
 * credentials), so the response is reused rather than fetched twice.
 *
 * Only these routes: Welcome must not fetch any wasm until someone commits
 * to a flow (`welcome-ssr.e2e.ts`, `budgets.e2e.ts`), and the placeholder and
 * gallery pages never boot the core at all. The tag is written into the HTML
 * by `hooks.server.ts`, so a client navigation — whose screen loads the core
 * itself — never adds a second request for it.
 */
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

/** The routes that boot the core as they open. */
export const CORE_ROUTES: ReadonlySet<string> = new Set([
	'/[locale]/wallet',
	'/[locale]/settings',
	'/[locale]/contacts',
	'/[locale]/create',
	'/[locale]/request',
	'/[locale]/parallel'
]);

/** The `<link>` for `routeId`'s head, or `''` where the core is not wanted. */
export function corePreloadTag(routeId: string | null): string {
	if (routeId === null || !CORE_ROUTES.has(routeId)) return '';
	return `<link rel="preload" href="${WASM_URL}" as="fetch" type="application/wasm" crossorigin="anonymous" />`;
}
