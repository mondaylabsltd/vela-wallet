/**
 * `GET /download/<platform>` — the stable entry per platform (spec 065, A3).
 *
 * The page links here and nowhere else, so it never names a version: a new
 * Release moves every button by itself. A platform the Release does not carry
 * (the macOS images, for the hours before they are signed and attached) answers
 * with the page saying so — never a 404, and never a phone package, because no
 * such platform id exists.
 */
import { isPlatformId } from '$lib/downloads/platforms';
import { serveDownload, unavailable } from '$lib/server/downloads';
import { depsFor } from '$lib/server/downloads-edge';
import type { RequestHandler } from './$types';

export const prerender = false;

export const GET: RequestHandler = async (event) => {
	const id = event.params.platform;
	if (!isPlatformId(id)) return unavailable(id, event.request);
	return serveDownload(id, event.request, depsFor(event));
};
