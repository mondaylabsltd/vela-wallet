import { ASSET_LINKS, associationResponse } from '$lib/app-links/association';

/**
 * `https://wallet.getvela.app/.well-known/assetlinks.json` — the Android App
 * Links statement, the Android lane's file verbatim
 * (`$lib/app-links/association`): its package and its signing certificates'
 * fingerprints, which only that lane may change. On the Worker, beside the
 * iOS association, so both are answered the same way.
 */
export const prerender = false;

export function GET(): Response {
	return associationResponse(ASSET_LINKS);
}
