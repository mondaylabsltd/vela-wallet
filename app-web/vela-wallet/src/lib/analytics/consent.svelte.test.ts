/**
 * §35 (review 1-8): usage statistics are opt-in. Off unless the person
 * switched them on on this browser — a browser that never chose (before R21
 * the default was on and nothing was written for it) gets the new default,
 * off; one that had switched them off stays off.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { ANALYTICS_KEY, analyticsConsent } from './consent.svelte';

afterEach(() => localStorage.removeItem(ANALYTICS_KEY));

describe('usage statistics consent', () => {
	// One module, one visit: the order below is the visit's.
	it('is off by default, on only by an explicit choice, and remembers it', () => {
		// Never chose — the pre-R21 "on" had no stored value: off now.
		localStorage.removeItem(ANALYTICS_KEY);
		analyticsConsent.boot();
		expect(analyticsConsent.enabled).toBe(false);
		expect(analyticsConsent.allowed()).toBe(false);

		// The pre-R21 opt-out stays off.
		localStorage.setItem(ANALYTICS_KEY, 'off');
		expect(analyticsConsent.allowed()).toBe(false);

		// Anything but an explicit "on" is off.
		localStorage.setItem(ANALYTICS_KEY, 'yes');
		expect(analyticsConsent.allowed()).toBe(false);

		// Another tab (or the side panel) switched it on: read at send time.
		localStorage.setItem(ANALYTICS_KEY, 'on');
		expect(analyticsConsent.allowed()).toBe(true);
		analyticsConsent.boot();
		expect(analyticsConsent.enabled).toBe(true);

		// This visit's own switch: written, and it wins over storage.
		analyticsConsent.set(false);
		expect(localStorage.getItem(ANALYTICS_KEY)).toBe('off');
		expect(analyticsConsent.allowed()).toBe(false);
		analyticsConsent.set(true);
		expect(localStorage.getItem(ANALYTICS_KEY)).toBe('on');
		expect(analyticsConsent.allowed()).toBe(true);
		analyticsConsent.set(false);
		localStorage.setItem(ANALYTICS_KEY, 'on');
		expect(analyticsConsent.allowed()).toBe(false);
	});
});
