/**
 * Server hooks — CORS for the /api/* routes.
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
import type { Handle } from '@sveltejs/kit';

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

	const response = await resolve(event);

	if (allowed) {
		for (const [k, v] of Object.entries(corsHeaders(origin!))) {
			response.headers.set(k, v);
		}
	}
	return response;
};
