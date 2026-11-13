/**
 * The address book's word between tabs (set-1).
 *
 * The book lives in IndexedDB, which raises no `storage` event, so a tab that
 * writes it says so on a BroadcastChannel and every other tab's book reads the
 * stores again (the core's `store_changed`). The core does the rest: with the
 * store shared (`use_shared_store`) every change re-reads before it writes,
 * so two tabs merge into one book instead of each writing back its own.
 *
 * A channel never hears its own posts, so one instance per tab both posts and
 * listens. Where BroadcastChannel is missing the book still merges on write;
 * it only stops refreshing on its own.
 */

const CHANNEL = 'vela.contacts';

let channel: BroadcastChannel | null | undefined;

function open(): BroadcastChannel | null {
	if (channel !== undefined) return channel;
	try {
		channel = typeof BroadcastChannel === 'function' ? new BroadcastChannel(CHANNEL) : null;
	} catch {
		channel = null;
	}
	return channel;
}

/** This tab wrote (or cleared) the book's stores: the others read them again. */
export function announceContactsChanged(): void {
	try {
		open()?.postMessage('changed');
	} catch {
		// A closed channel: nobody to tell.
	}
}

/** Called whenever ANOTHER tab changed the book. Returns the unsubscribe. */
export function onContactsChangedElsewhere(listener: () => void): () => void {
	const target = open();
	if (!target) return () => {};
	const handler = () => listener();
	target.addEventListener('message', handler);
	return () => target.removeEventListener('message', handler);
}

/** Test seam: forget the channel so a test can install its own. */
export function resetContactsSyncForTests(): void {
	channel?.close();
	channel = undefined;
}
