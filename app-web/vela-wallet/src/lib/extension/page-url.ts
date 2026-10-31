/**
 * Addressing the app when it is a FILE, not a site (spec 027 D42).
 *
 * The extension packages the same prerendered pages the site serves, so a route
 * lives at a path plus `.html`: `/en/wallet` is the file `/en/wallet.html`, and
 * `/en` is `/en.html`. On the web the two are the same thing because a server
 * maps one to the other. Under `chrome-extension://` nothing does — measured:
 * an extension URL resolves neither a directory index (`/en/` and `/en` both
 * come back `ERR_FILE_NOT_FOUND`) nor an extensionless twin, and a twin cannot
 * even be written where SvelteKit has already made a directory for the route's
 * data.
 *
 * That is harmless for CLIENT navigation, which never asks for a document. It
 * is fatal for anything that leaves the page: a full navigation to `/en/wallet`
 * lands on `chrome-error://chromewebdata/`, and so does a reload after any
 * client navigation.
 *
 * So the app keeps its route paths everywhere and translates them at exactly
 * the two moments a document is actually fetched — a deliberate full
 * navigation, and the address bar after a client one.
 *
 * On the hosted site every function here is the identity function, decided by
 * the page's own origin. There is no build flag: the same bundle is correct in
 * both places.
 */

/** True when this document was loaded from the packaged extension. */
export function isPackagedApp(): boolean {
	return typeof location !== 'undefined' && location.protocol === __VELA_EXTENSION_PROTOCOL__;
}

/**
 * The document a route path is stored as. `/en/wallet` → `/en/wallet.html`.
 * Already-suffixed paths, and every path on the hosted site, are returned as
 * they arrived.
 */
export function packagedHref(routeHref: string): string {
	if (!isPackagedApp()) return routeHref;
	const [path, rest] = splitPath(routeHref);
	if (path.endsWith('.html') || path === '/' || path === '') return routeHref;
	return `${path.replace(/\/$/, '')}.html${rest}`;
}

/**
 * Rewrite the address bar to the document the current route is stored as, so a
 * reload finds a file. Called after navigation; a no-op on the hosted site and
 * whenever the URL already names a document.
 *
 * `history.state` is carried across verbatim — SvelteKit keeps its own
 * bookkeeping there, and replacing the URL is not meant to disturb it.
 */
export function normalizePackagedUrl(): void {
	if (!isPackagedApp()) return;
	const current = location.pathname;
	if (current.endsWith('.html') || current === '/') return;
	const next = `${current.replace(/\/$/, '')}.html${location.search}${location.hash}`;
	history.replaceState(history.state, '', next);
}

/** Split a href into its path and everything the query/hash adds. */
function splitPath(href: string): [string, string] {
	const cut = href.search(/[?#]/);
	return cut === -1 ? [href, ''] : [href.slice(0, cut), href.slice(cut)];
}
