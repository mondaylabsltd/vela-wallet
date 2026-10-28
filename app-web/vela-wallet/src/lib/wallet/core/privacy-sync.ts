/**
 * Balance privacy across this browser's tabs (home-12).
 *
 * The choice is written to the shared store (`vela.balanceHidden`, IndexedDB),
 * which tells no other tab — so one tab hid the balance and the next one, open
 * beside it, kept showing it. A tab that writes the choice says so on a
 * BroadcastChannel; every other tab that shows the other value takes the
 * person's latest choice. Nothing is decided here: the eye's flip, its
 * persistence and first-write-wins stay the core's (`PrivacyToggled`).
 */

const CHANNEL = 'vela.balancePrivacy';

/** The part of `BroadcastChannel` this uses. */
interface PrivacyChannel {
	postMessage(message: unknown): void;
	addEventListener(type: 'message', listener: (event: { data: unknown }) => void): void;
}

let channel: PrivacyChannel | null | undefined;

function open(): PrivacyChannel | null {
	if (channel === undefined) {
		channel =
			typeof BroadcastChannel === 'function'
				? (new BroadcastChannel(CHANNEL) as unknown as PrivacyChannel)
				: null;
	}
	return channel;
}

/** This tab wrote the person's choice: the other tabs are told. */
export function announcePrivacy(hidden: boolean): void {
	try {
		open()?.postMessage({ hidden });
	} catch {
		// best-effort: the store holds the choice for the next load either way
	}
}

/**
 * Hear another tab's choice. `apply` is called with it; the caller flips its
 * own view only when it differs.
 */
export function onPrivacyElsewhere(apply: (hidden: boolean) => void): void {
	open()?.addEventListener('message', (event) => {
		const data = event.data as { hidden?: unknown } | null;
		if (typeof data?.hidden === 'boolean') apply(data.hidden);
	});
}

/** For tests: a channel of the test's own. */
export function usePrivacyChannel(next: PrivacyChannel | null): void {
	channel = next;
}
