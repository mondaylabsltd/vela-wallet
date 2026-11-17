import { negotiate } from '#lib/i18n/locales.ts';

/**
 * `/dapp` — Vela Connect's front door (spec 109): the address a site opens,
 * as a popup or with a redirect request in the fragment
 * (`docs/vela-connect/PROTOCOL.md` §2).
 *
 * The same hop `/` and `/pay` make: negotiate Accept-Language and 307 to the
 * prerendered `/<locale>/dapp`. The query is kept; the fragment — where a
 * redirect request travels, so no server sees it — never reaches here, and
 * the browser carries it across the redirect itself. Runs on the Worker, so
 * it stays wasm-free.
 */
export function GET({ request }: { request: Request }): Response {
	const locale = negotiate(request.headers.get('accept-language'));
	const { search } = new URL(request.url);
	return new Response(null, {
		status: 307,
		headers: {
			location: `/${locale}/dapp${search}`,
			vary: 'Accept-Language',
			'cache-control': 'private, no-store'
		}
	});
}
