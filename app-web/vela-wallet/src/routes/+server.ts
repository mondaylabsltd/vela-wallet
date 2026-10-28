import { negotiate, pinnedLocale } from '$lib/i18n/locales';

/**
 * `/` — the only runtime route (contracts/i18n-ssr.md): a language the person
 * PINNED in Settings (the `vela_lang` cookie, set-12), else negotiate
 * Accept-Language, and 307 to the prerendered locale page. Runs on the
 * Cloudflare Worker, so it must stay wasm-free (see engine.server.ts).
 */
export function GET({ request }: { request: Request }): Response {
	const locale =
		pinnedLocale(request.headers.get('cookie')) ??
		negotiate(request.headers.get('accept-language'));
	// Preserve the query string — root-domain campaign links carry attribution
	// params (utm_*, ref) that must survive the locale hop.
	const { search } = new URL(request.url);
	return new Response(null, {
		status: 307,
		headers: {
			location: `/${locale}${search}`,
			vary: 'Accept-Language, Cookie',
			'cache-control': 'private, no-store'
		}
	});
}
