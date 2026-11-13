import type { Handle } from '@sveltejs/kit';
import { toLocale, textDirectionOf, FALLBACK_LOCALE } from '$lib/i18n/locales';
import { corePreloadTag } from '$lib/core/preload';
import { withWalletHeaders } from '$lib/security-headers';

/**
 * Stamp `<html lang dir>` from the route's [locale] param (app.html carries
 * %lang%/%dir% placeholders). Deliberately wasm-free: this runs on the
 * deployed Worker for non-prerendered responses, so direction comes from the
 * static registry in locales.ts — `messages.test.ts` pins that registry to the
 * engine's `dir()` so the two cannot drift.
 *
 * And the core's wasm preload, on the routes that boot the core (perf-3): a
 * URL string, still no wasm in the Worker.
 *
 * And §35's headers (`$lib/security-headers`) on every response the Worker
 * renders — the prerendered pages and assets carry them from `_headers`.
 */
export const handle: Handle = async ({ event, resolve }) => {
	const locale = toLocale(event.params.locale ?? '') ?? FALLBACK_LOCALE;
	const response = await resolve(event, {
		transformPageChunk: ({ html }) =>
			html
				.replace('%lang%', locale)
				.replace('%dir%', textDirectionOf(locale))
				.replace('%vela.corePreload%', corePreloadTag(event.route.id))
	});
	return withWalletHeaders(response);
};
