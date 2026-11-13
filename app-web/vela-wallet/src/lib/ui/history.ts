/**
 * The browser history entries the app writes itself (ob-16): a flow step the
 * Back button unwinds, a URL stripped of a one-time code. Through SvelteKit's
 * shallow routing, not `history.*` directly — the router keeps its own index
 * in each entry, and a raw `history.pushState` both bypassed it and drew
 * "Avoid using `history.pushState(...)`" on every step in development.
 *
 * Before the router has started (a page's first mount) SvelteKit refuses the
 * call; the platform's own then does the same job, as it always did.
 */
import { pushState, replaceState } from '$app/navigation';
import { page } from '$app/state';

/** One more Back for the browser to unwind, at the same URL. */
export function markHistory(state: App.PageState): void {
	if (typeof history === 'undefined') return;
	try {
		pushState('', state);
	} catch {
		history.pushState(state, '');
	}
}

/** The current entry's URL becomes `url` (same page; its state is kept). */
export function replaceUrl(url: string): void {
	if (typeof history === 'undefined') return;
	try {
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- the current page's own address, minus a query or a fragment
		replaceState(url, page.state);
	} catch {
		history.replaceState(history.state, '', url);
	}
}
