/**
 * "Share anonymous usage statistics" — on unless the person switches it off,
 * remembered on this device.
 *
 * localStorage, like the display preferences (`services/preferences.svelte.ts`)
 * and for their reason: it is read synchronously, at the moment a hit would be
 * sent. Only the OFF choice is written (`vela.analytics` = `off`); on is the
 * absence of it. Erase This Device sweeps the `vela.` namespace, so an erased
 * browser starts again from the default — the same as every other setting.
 *
 * Not a core preference: the phone and desktop apps carry no analytics, so
 * there is nothing for the core to keep consistent across shells.
 */
import { browser } from '$app/environment';

export const ANALYTICS_KEY = 'vela.analytics';
const OFF = 'off';

function storedOptOut(): boolean {
	if (!browser) return false;
	try {
		return localStorage.getItem(ANALYTICS_KEY) === OFF;
	} catch {
		return false;
	}
}

class AnalyticsConsent {
	/** What Settings shows. */
	enabled = $state(true);
	/**
	 * A choice made this visit, kept even where storage refuses it: a person
	 * who switched it off is not reported for the rest of the visit because
	 * their browser would not remember the switch.
	 */
	#chosen: boolean | null = null;

	/** Read the stored choice (idempotent; Settings calls it on mount). */
	boot(): void {
		this.enabled = this.allowed();
	}

	/** Checked at send time, so another tab's (or the side panel's) switch counts. */
	allowed(): boolean {
		if (this.#chosen === false) return false;
		return !storedOptOut();
	}

	set(on: boolean): void {
		this.enabled = on;
		this.#chosen = on;
		if (!browser) return;
		try {
			if (on) localStorage.removeItem(ANALYTICS_KEY);
			else localStorage.setItem(ANALYTICS_KEY, OFF);
		} catch {
			/* Blocked storage: the choice holds for this visit (`#chosen`). */
		}
	}
}

export const analyticsConsent = new AnalyticsConsent();
