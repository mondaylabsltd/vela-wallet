import { APPLE_APP_SITE_ASSOCIATION, associationResponse } from '$lib/app-links/association';

/**
 * `https://wallet.getvela.app/.well-known/apple-app-site-association` — the
 * iOS universal-link association, the iOS lane's file verbatim
 * (`$lib/app-links/association`). On the Worker, never prerendered: a static
 * file with no extension would be served as `application/octet-stream`.
 */
export const prerender = false;

export function GET(): Response {
	return associationResponse(APPLE_APP_SITE_ASSOCIATION);
}
