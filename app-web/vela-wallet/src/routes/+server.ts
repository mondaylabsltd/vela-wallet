import { negotiate } from '$lib/i18n/locales';

/**
 * `/` — the only runtime route (contracts/i18n-ssr.md): negotiate
 * Accept-Language and 307 to the prerendered locale page. Runs on the
 * Cloudflare Worker, so it must stay wasm-free (see engine.server.ts).
 *
 * set-12: a language the person PINNED in Settings lives in `localStorage`,
 * which this Worker cannot read, and spec 006 keeps cookies out of the
 * wallet — so the locale page it lands on honours the pin itself, before its
 * first paint (`pinnedLocaleTarget`, inline in `app.html`).
 */
export function GET({ request }: { request: Request }): Response {
	const locale = negotiate(request.headers.get('accept-language'));
	// Preserve the query string — root-domain campaign links carry attribution
	// params (utm_*, ref) that must survive the locale hop.
	const { search } = new URL(request.url);
	return new Response(null, {
		status: 307,
		headers: {
			location: `/${locale}${search}`,
			vary: 'Accept-Language',
			'cache-control': 'private, no-store'
		}
	});
}
