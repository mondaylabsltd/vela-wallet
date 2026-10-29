/**
 * What the home shows, the same in every document of this browser.
 *
 * The owner's report: the wallet in a tab and in the side panel, two totals
 * ($1,376.87 and $1,373.97) for one wallet. Each document runs its own core:
 * its own balance round, token cache and prices, read at its own moment —
 * and a round that settled in one never reached the other, nor did a display
 * currency chosen there, nor a network added there (all IndexedDB, which
 * raises no `storage` event). The account, the network space and the
 * privacy eye already followed (their own listeners); this is the rest.
 *
 * One BroadcastChannel, the house pattern (`contacts-sync.ts`,
 * `privacy-sync.ts`), three words:
 *
 *   - `holdings` — a round of THIS document's settled for an account and
 *     space: a document showing the same account reads again now, past its
 *     token cache, so the two figures come from the same moment. A read
 *     started because another document's settled is not announced in turn
 *     (`balance.svelte.ts`), so two documents never keep each other reading;
 *   - `currency` — the stored display currency was written: read it again;
 *   - `networks` — the stored custom networks or their settings were
 *     written: read them again.
 *
 * Nothing is decided here: each document's core reads the shared stores and
 * rules on them as it would on its own write.
 */

const CHANNEL = 'vela.walletSync';

export type WalletSyncMessage =
	| { topic: 'holdings'; address: string; space: string }
	| { topic: 'currency' }
	| { topic: 'networks' };

/** The part of `BroadcastChannel` this uses. */
interface SyncChannel {
	postMessage(message: unknown): void;
	addEventListener(type: 'message', listener: (event: { data: unknown }) => void): void;
	removeEventListener(type: 'message', listener: (event: { data: unknown }) => void): void;
}

let channel: SyncChannel | null | undefined;

function open(): SyncChannel | null {
	if (channel !== undefined) return channel;
	try {
		channel =
			typeof BroadcastChannel === 'function'
				? (new BroadcastChannel(CHANNEL) as unknown as SyncChannel)
				: null;
	} catch {
		channel = null;
	}
	return channel;
}

/** A message in the shape this module sends, or `null`. */
export function readSyncMessage(data: unknown): WalletSyncMessage | null {
	if (!data || typeof data !== 'object') return null;
	const m = data as Record<string, unknown>;
	if (m.topic === 'currency' || m.topic === 'networks') return { topic: m.topic };
	if (m.topic === 'holdings' && typeof m.address === 'string' && typeof m.space === 'string') {
		return { topic: 'holdings', address: m.address, space: m.space };
	}
	return null;
}

/** This document changed something the others show: they are told. */
export function announceWalletSync(message: WalletSyncMessage): void {
	try {
		open()?.postMessage(message);
	} catch {
		// A closed channel: nobody to tell. The stores hold it for the next load.
	}
}

/** Hear another document's word. Returns the unsubscribe. */
export function onWalletSyncElsewhere(listener: (message: WalletSyncMessage) => void): () => void {
	const target = open();
	if (!target) return () => {};
	const handler = (event: { data: unknown }) => {
		const message = readSyncMessage(event.data);
		if (message) listener(message);
	};
	target.addEventListener('message', handler);
	return () => target.removeEventListener('message', handler);
}

/** For tests: a channel of the test's own (`null`: none at all). */
export function useWalletSyncChannel(next: SyncChannel | null | undefined): void {
	channel = next;
}
