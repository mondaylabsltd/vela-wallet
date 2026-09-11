import { redirect } from '@sveltejs/kit';

/**
 * The payment-link bridge lives in the wallet app (served at <origin>/pay so it
 * works for the hosted wallet and self-hosted deployments alike). The marketing
 * domain only forwards any /pay links to the canonical hosted bridge, preserving
 * the query string — so there is a single bridge implementation to maintain.
 *
 * Status (2026-09-11, spec 039): the bridge page lived in the retired Expo
 * app and is owed to app-web/vela-wallet (spec 039 Part B). Until it lands,
 * the hostname's frozen Expo build still answers /pay; after the hostname
 * moves to the Worker it 404s unless the route has been built.
 */
export const prerender = false;

export function GET({ url }) {
  throw redirect(307, `https://wallet.getvela.app/pay${url.search}`);
}
