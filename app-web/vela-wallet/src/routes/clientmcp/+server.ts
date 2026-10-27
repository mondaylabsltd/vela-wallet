import { negotiate } from '$lib/i18n/locales';

/**
 * `/clientmcp` — spec 106: where clientmcp.dev sends the browser back after
 * the person signs in (the return address registered there). A runtime
 * route, like `/`, and as wasm-free: it hands the query to the settings page
 * in the fragment (`#clientmcp?code=…&state=…`), so the one-time code is read
 * by the page that started the sign-in and never travels to a server again.
 * The page itself moves on to the language the sign-in started in.
 */
export function GET({ request }: { request: Request }): Response {
	const locale = negotiate(request.headers.get('accept-language'));
	const { search } = new URL(request.url);
	return new Response(null, {
		status: 303,
		headers: {
			location: `/${locale}/settings#clientmcp${search}`,
			vary: 'Accept-Language',
			'cache-control': 'private, no-store',
			'referrer-policy': 'no-referrer'
		}
	});
}
