/**
 * The tab bar and the sidebar, fetched ahead of the tap.
 *
 * Every web destination is its own prerendered route, so a tap used to start
 * a round trip for the route's `__data.json` (a revalidation even when
 * nothing changed — the files are `max-age=0`) and, on a first visit, its JS
 * chunks. Measured at phone speed (4× CPU, Fast 4G) that was ~170 ms of a
 * 230–440 ms tap on every switch, while the native shells swap in a frame
 * (founder-found, 2026-09-26).
 *
 * Two halves, because SvelteKit keeps ONE preloaded page's data at a time:
 *  - the code for the other destinations once the page is idle, which lasts;
 *  - the data for the destination a finger is on, from `pointerdown` — the
 *    press is already ~100 ms of the wait by the time `click` fires.
 */
import { preloadCode, preloadData } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import { WEB_DESTINATIONS, type WebDestination } from './destinations';

function isDestination(id: string): id is WebDestination {
	return (WEB_DESTINATIONS as readonly string[]).includes(id);
}

/** The route a destination opens — the same href every route's `goto` uses. */
function destinationHref(id: WebDestination, locale: string): string {
	switch (id) {
		case 'wallet':
			return resolve('/[locale]/wallet', { locale });
		case 'contacts':
			return resolve('/[locale]/contacts', { locale });
		case 'settings':
			return resolve('/[locale]/settings', { locale });
	}
}

function hrefFor(id: string): string | undefined {
	const locale = page.params.locale;
	if (!locale || !isDestination(id)) return undefined;
	return destinationHref(id, locale);
}

/**
 * The route under a finger. A preload that fails is only a tap that waits as
 * it always did — the navigation itself reports anything real.
 */
export function preloadDestination(id: string): void {
	const href = hrefFor(id);
	if (href !== undefined) preloadData(href).catch(() => {});
}

/**
 * The other destinations' code, once the page has nothing better to do.
 * Returns the cancel for an unmount that comes first.
 */
export function warmDestinations(current: string | undefined): () => void {
	const run = () => {
		for (const id of WEB_DESTINATIONS) {
			if (id === current) continue;
			const href = hrefFor(id);
			if (href !== undefined) preloadCode(href).catch(() => {});
		}
	};
	// Safari has no `requestIdleCallback`; a beat after mount does the same job.
	if (typeof requestIdleCallback === 'function') {
		const handle = requestIdleCallback(run, { timeout: 3000 });
		return () => cancelIdleCallback(handle);
	}
	const handle = setTimeout(run, 1500);
	return () => clearTimeout(handle);
}
