/**
 * `GET /api/downloads` — what the latest Release actually carries (spec 065, A4).
 *
 * The get-started page is prerendered, so it cannot know this at build time and
 * must not pretend to: it renders its rows from this answer. 503 means "no list
 * now and none remembered"; the page then keeps every row a plain link, and
 * `/download/<platform>` decides per click.
 */
import { loadRelease, manifestOf } from '$lib/server/downloads';
import { depsFor } from '$lib/server/downloads-edge';
import type { RequestHandler } from './$types';

export const prerender = false;

export const GET: RequestHandler = async (event) => {
	try {
		const manifest = manifestOf(await loadRelease(depsFor(event)));
		return new Response(JSON.stringify(manifest), {
			headers: { 'Content-Type': 'application/json', 'Cache-Control': 'public, max-age=300' }
		});
	} catch {
		return new Response(JSON.stringify({ error: 'release list unavailable' }), {
			status: 503,
			headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }
		});
	}
};
