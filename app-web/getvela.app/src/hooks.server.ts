/**
 * Server hooks — CORS for `/api/*`, the `<html lang>` attribute, and the locale
 * alias redirects (spec 059).
 *
 * The web wallet is served from a different origin (e.g. https://wallet.getvela.app)
 * than these API routes (https://getvela.app/api/*), so browser calls are
 * cross-origin and need CORS headers + preflight handling. The native app isn't
 * affected (CORS is browser-only).
 *
 * Access is restricted to the getvela.app domain family (+ localhost for dev),
 * not "*": these endpoints are otherwise reachable by anyone via curl regardless
 * of CORS, so the real protection is each route's own rate-limit/token — but we
 * still scope the browser-allowed origins as defence in depth.
 */
import { redirect, type Handle } from '@sveltejs/kit';
import { DEFAULT_LOCALE, resolveAlias, splitLocalePath } from '$lib/i18n/locales';

/** Any https getvela.app (sub)domain, plus localhost/127.0.0.1 on any port for dev. */
function isAllowedOrigin(origin: string | null): boolean {
	if (!origin) return false;
	if (origin === 'https://getvela.app') return true;
	if (/^https:\/\/([a-z0-9-]+\.)+getvela\.app$/.test(origin)) return true;
	if (/^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(origin)) return true;
	return false;
}

function corsHeaders(origin: string): Record<string, string> {
	return {
		'Access-Control-Allow-Origin': origin,
		'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
		'Access-Control-Allow-Headers': 'Content-Type',
		'Access-Control-Max-Age': '86400',
		Vary: 'Origin'
	};
}

/**
 * One canonical URL per page (contracts/routing.md §Must 308).
 *
 * The route matcher accepts only the fourteen prefixes in canonical case, so a
 * mis-cased tag (`/pt-br/`), an alias (`/pt/`, `/zh-CN/`) or an unlisted region
 * (`/fr-CA/`) would otherwise 404. Each redirects to the tag that actually has a
 * catalog, permanently (308 preserves the method) and with the query string
 * intact, because campaign links carry `utm_*`.
 *
 * Deliberately NOT redirected:
 *  - any English path. `resolveAlias` only ever returns for a segment that looks
 *    like a language tag, and no route on this site is named one — but if a page
 *    called `/it` or `/id` is ever added, it would collide, and this comment is
 *    where to look.
 *  - `/en/…`. English has exactly one URL and it is unprefixed, so `/en/` is a
 *    404, not a redirect to `/` (FR-008).
 */
function aliasRedirect(url: URL): string | null {
	const [, first = ''] = url.pathname.split('/');
	if (!first) return null;

	// Already a canonical prefix, or plain English — nothing to do.
	if (splitLocalePath(url.pathname).locale !== DEFAULT_LOCALE) return null;

	const target = resolveAlias(first);
	if (!target || target === DEFAULT_LOCALE) return null;

	const rest = url.pathname.slice(first.length + 1);
	return `/${target}${rest}${url.search}`;
}

export const handle: Handle = async ({ event, resolve }) => {
	const isApi = event.url.pathname.startsWith('/api/');
	const origin = event.request.headers.get('origin');
	const allowed = isApi && isAllowedOrigin(origin);

	// Preflight: answer OPTIONS for allowed cross-origin API requests directly.
	if (isApi && event.request.method === 'OPTIONS') {
		return allowed
			? new Response(null, { status: 204, headers: corsHeaders(origin!) })
			: new Response(null, { status: 403 });
	}

	if (!isApi) {
		const to = aliasRedirect(event.url);
		if (to) redirect(308, to);
	}

	// `<html lang>` lives outside the app root, so it cannot be set from a
	// component. `transformPageChunk` runs during prerendering too, which is the
	// only reason a statically-built page can carry a per-locale lang (FR-014).
	const { locale } = splitLocalePath(event.url.pathname);
	const response = await resolve(event, {
		transformPageChunk: ({ html }) => html.replace('%lang%', locale)
	});

	if (allowed) {
		for (const [k, v] of Object.entries(corsHeaders(origin!))) {
			response.headers.set(k, v);
		}
	}
	return response;
};
