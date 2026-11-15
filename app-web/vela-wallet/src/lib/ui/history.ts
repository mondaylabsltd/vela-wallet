/**
 * The browser history entries the app writes itself (ob-16): a flow step the
 * Back button unwinds, a URL stripped of a one-time code. Through SvelteKit's
 * shallow routing, not `history.*` directly — the router keeps its own index
 * in each entry, and a raw `history.pushState` both bypassed it and drew
 * "Avoid using `history.pushState(...)`" on every step in development.
 *
 * A shallow `goto` is asynchronous, which SvelteKit 2's `pushState` and
 * `replaceState` were not. Two things follow:
 *
 * - The writes are made one at a time, in the order they were asked for, each
 *   against the address the one before it left — as synchronous calls were. A
 *   query dropped and a flow step pushed straight after it must not push the
 *   address that still carries the query.
 * - A refusal arrives as a rejection: before the router has started (a page's
 *   first mount), and for an address no route matches — the packaged
 *   extension's `/en/wallet.html`, a file rather than a route. The platform's
 *   own call then does the same job, as it always did.
 */
import { goto } from '$app/navigation';
import { page } from '$app/state';

let written: Promise<void> = Promise.resolve();

/** Run `write` once every earlier write has landed. */
function inTurn(write: () => Promise<void>): Promise<void> {
	written = written.then(write);
	return written;
}

/** One more Back for the browser to unwind, at the same URL. */
export function markHistory(state: App.PageState): Promise<void> {
	if (typeof history === 'undefined') return Promise.resolve();
	return inTurn(() =>
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- the current page's own address
		goto('', { shallow: true, state }).catch(() => history.pushState(state, ''))
	);
}

/** The current entry's URL becomes `url` (same page; its state is kept). */
export function replaceUrl(url: string): Promise<void> {
	if (typeof history === 'undefined') return Promise.resolve();
	return inTurn(() =>
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- the current page's own address, minus a query or a fragment
		goto(url, { shallow: true, replace: true, state: page.state }).catch(() =>
			history.replaceState(history.state, '', url)
		)
	);
}
