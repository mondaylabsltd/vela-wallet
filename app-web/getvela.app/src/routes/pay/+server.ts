import { redirect } from '@sveltejs/kit';

/**
 * The payment-link bridge lives in the wallet app (served at <origin>/pay so it
 * works for the hosted wallet and self-hosted deployments alike). The marketing
 * domain only forwards any /pay links to the canonical hosted bridge, preserving
 * the query string — so there is a single bridge implementation to maintain.
 *
 * Status (2026-09-11, spec 039): the bridge page lived in the retired Expo
 * app and is OWED to app-web/vela-wallet (spec 039 Part B). The hostname
 * moved to the Worker on 2026-09-11, where `/pay` does not exist yet, so a
 * redirect there would land on a 404. Until the route lands this forwards
 * to the wallet's front door with the query string kept — the wallet ignores
 * it today; when `/pay` ships, change the target back to `/pay${search}`.
 */
export const prerender = false;

export function GET({ url }) {
  throw redirect(307, `https://wallet.getvela.app/${url.search}`);
}
