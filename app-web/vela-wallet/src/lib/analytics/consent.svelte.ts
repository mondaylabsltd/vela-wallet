/**
 * "Share anonymous usage statistics" — OFF unless the person switches it on,
 * remembered on this device (§35, review 1-8: opt-in, decided 2026-11-08).
 *
 * localStorage, like the display preferences (`services/preferences.svelte.ts`)
 * and for their reason: it is read synchronously, at the moment a hit would be
 * sent. Only an explicit ON counts (`vela.analytics` = `on`); anything else —
 * no value, `off`, a value from before R21 — is off. Before R21 the default
 * was on and only `off` was ever written, so a browser that never chose carries
 * no value and gets the new default, off; one that had switched it off stays
 * off. Erase This Device sweeps the `vela.` namespace, so an erased browser
 * starts again from the default — off.
 *
 * Not a core preference: the phone and desktop apps carry no analytics, so
 * there is nothing for the core to keep consistent across shells.
 */
import { browser } from '$app/environment';

export const ANALYTICS_KEY = 'vela.analytics';
const ON = 'on';
const OFF = 'off';

/** The person switched it on, on this browser. Unreadable storage is no. */
function storedOptIn(): boolean {
	if (!browser) return false;
	try {
		return localStorage.getItem(ANALYTICS_KEY) === ON;
	} catch {
		return false;
	}
}

class AnalyticsConsent {
	/** What Settings shows. */
	enabled = $state(false);
	/**
	 * A choice made this visit, kept even where storage refuses it: a person
	 * who switched it off is never reported for the rest of the visit, and one
	 * who switched it on is, though their browser would not remember it.
	 */
	#chosen: boolean | null = null;

	/** Read the stored choice (idempotent; Settings calls it on mount). */
	boot(): void {
		this.enabled = this.allowed();
	}

	/** Checked at send time, so another tab's (or the side panel's) switch counts. */
	allowed(): boolean {
		if (this.#chosen !== null) return this.#chosen;
		return storedOptIn();
	}

	set(on: boolean): void {
		this.enabled = on;
		this.#chosen = on;
		if (!browser) return;
		try {
			localStorage.setItem(ANALYTICS_KEY, on ? ON : OFF);
		} catch {
			/* Blocked storage: the choice holds for this visit (`#chosen`). */
		}
	}
}

export const analyticsConsent = new AnalyticsConsent();
