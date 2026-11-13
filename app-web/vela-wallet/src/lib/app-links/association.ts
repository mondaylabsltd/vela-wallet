/**
 * The app-link association files `wallet.getvela.app` serves, so the iOS and
 * Android apps may open this host's links (a `/pay` link above all, and the
 * fragment-gated multisig links) instead of the browser.
 *
 * ONE source of truth each, owned by the platform that consumes it:
 *
 * - `app-ios/universal-links/wallet.getvela.app.apple-app-site-association.json`
 *   (the iOS lane writes it; its tests check it against the app's entitlement)
 * - `app-android/vela-wallet/app-links/assetlinks.json` (the Android lane
 *   writes it, with the signing certificates' fingerprints)
 *
 * They are imported here as text — bundled into the Worker at build time, so
 * the bytes served are the file's bytes and cannot drift from it. Nobody edits
 * a copy under app-web; there is none. Served from the Worker as
 * `application/json`, with no redirect, as Apple's and Google's verifiers
 * require (`src/routes/.well-known/*`).
 */
import appleAppSiteAssociation from '../../../../../app-ios/universal-links/wallet.getvela.app.apple-app-site-association.json?raw';
import assetLinks from '../../../../../app-android/vela-wallet/app-links/assetlinks.json?raw';

export const APPLE_APP_SITE_ASSOCIATION: string = appleAppSiteAssociation;
export const ASSET_LINKS: string = assetLinks;

/**
 * The file, as the verifiers want it: 200, `application/json`, the exact
 * bytes. An hour's cache — Apple's CDN and Google's verifier re-fetch on their
 * own schedule, and a fixed association should not wait a day to reach them.
 */
export function associationResponse(body: string): Response {
	return new Response(body, {
		status: 200,
		headers: {
			'content-type': 'application/json',
			'cache-control': 'public, max-age=3600'
		}
	});
}
