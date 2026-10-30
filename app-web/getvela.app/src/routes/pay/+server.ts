import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

/**
 * The payment-link bridge lives in the wallet app, so the marketing domain
 * only forwards `/pay` links to it, the query string kept whole — one bridge
 * to maintain (spec 039 Part B). The wallet's `/pay` negotiates the language,
 * opens its pay page, and the core rules on the link there.
 *
 * With the iOS or Android app installed, the OS opens `getvela.app/pay` in the
 * app (`.well-known/apple-app-site-association`, the Android lane's App
 * Links) and this never runs.
 */
export const prerender = false;

export const GET: RequestHandler = ({ url }) => {
	redirect(307, `https://wallet.getvela.app/pay${url.search}`);
};
