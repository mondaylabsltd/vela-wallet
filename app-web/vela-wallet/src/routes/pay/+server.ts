import { negotiate } from '$lib/i18n/locales';

/**
 * `/pay` — a payment link's front door (spec 039 Part B), the address the
 * receive screen shares (`build_pay_link`, `wallet.getvela.app/pay?…`) and the
 * one getvela.app's `/pay` forwards to.
 *
 * The same hop `/` makes: negotiate Accept-Language and 307 to the
 * prerendered `/<locale>/pay`, the query kept whole — it IS the request. The
 * page reads it and the core rules on it; nothing here looks inside. Runs on
 * the Worker, so it stays wasm-free like `/`.
 *
 * The bare path is also what the iOS and Android apps claim as a universal /
 * app link: with the app installed the OS opens it there and this never runs.
 */
export function GET({ request }: { request: Request }): Response {
	const locale = negotiate(request.headers.get('accept-language'));
	const { search } = new URL(request.url);
	return new Response(null, {
		status: 307,
		headers: {
			location: `/${locale}/pay${search}`,
			vary: 'Accept-Language',
			'cache-control': 'private, no-store'
		}
	});
}
