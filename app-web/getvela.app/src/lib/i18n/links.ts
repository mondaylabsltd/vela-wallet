/**
 * The site's internal links, as `<a href>` values: a path from `pathFor` or
 * `switchTo`, passed through SvelteKit's `resolve()` so it carries the base
 * path. Components link with these, never with `pathFor` directly, and
 * `svelte/no-navigation-without-resolve` holds them to it.
 *
 * `locales.ts` stays free of `$app/*`: the Vite config and the build scripts
 * import it, and there is no SvelteKit runtime there.
 */
import { resolve } from '$app/paths';
import type { PathnameWithSearchOrHash, ResolvedPathname } from '$app/types';
import { pathFor, switchTo, type Locale } from './locales.ts';

/**
 * An internal path, `/`-prefixed as the rest of the site writes it, possibly
 * with `?search` or `#hash`. The cast is the one place a computed path meets
 * `resolve()`'s route types; every path reaching it is built by `pathFor`.
 */
function resolvePath(path: string): ResolvedPathname {
	return resolve(path.replace(/^\//, '') as PathnameWithSearchOrHash);
}

/** `localHref('ja', '/docs/faq')` → the link to `/ja/docs/faq`. */
export function localHref(locale: Locale, path: string): ResolvedPathname {
	return resolvePath(pathFor(locale, path));
}

/** The link to the page at `currentPathname`, in another language. */
export function switchHref(locale: Locale, currentPathname: string): ResolvedPathname {
	return resolvePath(switchTo(locale, currentPathname));
}
