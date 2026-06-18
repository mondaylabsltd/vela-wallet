import { redirect } from '@sveltejs/kit';

/**
 * The payment-link bridge lives in the wallet app (served at <origin>/pay so it
 * works for the hosted wallet and self-hosted deployments alike). The marketing
 * domain only forwards any /pay links to the canonical hosted bridge, preserving
 * the query string — so there is a single bridge implementation to maintain.
 */
export const prerender = false;

export function GET({ url }) {
  throw redirect(307, `https://wallet.getvela.app/pay${url.search}`);
}
