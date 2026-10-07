import { beforeEach, describe, expect, it } from 'vitest';
import { rawResolve, resolveAppPromptMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { storeLinks, type StoreLinks } from './config';
import { APP_PROMPT_KEYS } from './messages';
import {
	DISMISSED_KEY,
	SUGGESTED_KEY,
	detectPlatform,
	dismissPrompt,
	notePromptShown,
	promptAllowed,
	promptPlan,
	resetPromptVisitForTests
} from './plan';

const APP_STORE = 'https://apps.apple.com/app/id0000000000';
const GOOGLE_PLAY = 'https://play.google.com/store/apps/details?id=app.getvela.wallet';
const NONE: StoreLinks = { appStore: null, googlePlay: null };
const BOTH: StoreLinks = { appStore: APP_STORE, googlePlay: GOOGLE_PLAY };

/** A Storage that remembers, or one that throws on every call. */
function memoryStorage(): Storage {
	const map = new Map<string, string>();
	return {
		get length() {
			return map.size;
		},
		clear: () => map.clear(),
		getItem: (key) => map.get(key) ?? null,
		key: (index) => [...map.keys()][index] ?? null,
		removeItem: (key) => void map.delete(key),
		setItem: (key, value) => void map.set(key, value)
	};
}
const BLOCKED = new Proxy({} as Storage, {
	get() {
		throw new Error('SecurityError');
	}
});

describe('the switch', () => {
	it('is off today: no store link, so no prompt on any platform, at any placement', () => {
		for (const placement of ['home', 'create_done', 'extension'] as const) {
			expect(storeLinks(placement)).toEqual(NONE);
			for (const platform of ['ios', 'android', 'desktop'] as const) {
				expect(promptPlan(platform, storeLinks(placement))).toBe(null);
			}
		}
	});
});

describe('promptPlan', () => {
	it('offers an iPhone the App Store only — and nothing without that link', () => {
		expect(promptPlan('ios', BOTH)).toEqual({
			platform: 'ios',
			stores: [{ store: 'app_store', url: APP_STORE }],
			qr: false
		});
		expect(promptPlan('ios', { appStore: null, googlePlay: GOOGLE_PLAY })).toBe(null);
	});

	it('offers an Android phone Google Play only — and nothing without that link', () => {
		expect(promptPlan('android', BOTH)).toEqual({
			platform: 'android',
			stores: [{ store: 'google_play', url: GOOGLE_PLAY }],
			qr: false
		});
		expect(promptPlan('android', { appStore: APP_STORE, googlePlay: null })).toBe(null);
	});

	it('offers a desktop both, with codes to scan, or whichever exists', () => {
		expect(promptPlan('desktop', BOTH)).toEqual({
			platform: 'desktop',
			stores: [
				{ store: 'app_store', url: APP_STORE },
				{ store: 'google_play', url: GOOGLE_PLAY }
			],
			qr: true
		});
		expect(promptPlan('desktop', { appStore: null, googlePlay: GOOGLE_PLAY })?.stores).toEqual([
			{ store: 'google_play', url: GOOGLE_PLAY }
		]);
	});

	it('draws no link that is not https', () => {
		expect(promptPlan('desktop', { appStore: 'http://example.com', googlePlay: 'nope' })).toBe(
			null
		);
	});
});

describe('detectPlatform', () => {
	const IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15';
	const IPAD_DESKTOP_MODE = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15';
	const ANDROID = 'Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 Chrome/131 Mobile';
	const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131';

	it('reads the phone a page is on', () => {
		expect(detectPlatform(IPHONE, 5, false)).toBe('ios');
		expect(detectPlatform(IPAD_DESKTOP_MODE, 5, false)).toBe('ios');
		expect(detectPlatform(ANDROID, 5, false)).toBe('android');
		expect(detectPlatform(MAC, 0, false)).toBe('desktop');
	});

	it('takes the extension for a desktop, always', () => {
		expect(detectPlatform(ANDROID, 5, true)).toBe('desktop');
	});
});

describe('promptAllowed', () => {
	beforeEach(() => resetPromptVisitForTests());

	it('never returns once closed, from either placement', () => {
		const storage = memoryStorage();
		expect(promptAllowed('home', storage)).toBe(true);
		dismissPrompt(storage);
		expect(storage.getItem(DISMISSED_KEY)).not.toBe(null);
		expect(promptAllowed('home', storage)).toBe(false);
		expect(promptAllowed('create_done', storage)).toBe(false);
	});

	it('suggests on the creation screen once per device', () => {
		const storage = memoryStorage();
		expect(promptAllowed('create_done', storage)).toBe(true);
		notePromptShown('create_done', storage);
		expect(storage.getItem(SUGGESTED_KEY)).not.toBe(null);
		resetPromptVisitForTests();
		expect(promptAllowed('create_done', storage)).toBe(false);
		// …while the home card still may, on a later visit.
		expect(promptAllowed('home', storage)).toBe(true);
	});

	it('does not ask again on the home the creation screen hands over to', () => {
		const storage = memoryStorage();
		notePromptShown('create_done', storage);
		expect(promptAllowed('home', storage)).toBe(false);
	});

	it('survives storage that throws', () => {
		expect(promptAllowed('home', BLOCKED)).toBe(true);
		expect(promptAllowed('extension', undefined)).toBe(true);
		expect(() => dismissPrompt(BLOCKED)).not.toThrow();
		expect(() => notePromptShown('create_done', BLOCKED)).not.toThrow();
	});
});

describe('the prompt speaks every locale', () => {
	it.each(SUPPORTED_LOCALES)('%s', (locale) => {
		for (const key of APP_PROMPT_KEYS) {
			const value = rawResolve(locale, key);
			expect(value, `${key} in ${locale}`).not.toBe(key);
			expect(value.trim()).not.toBe('');
		}
		const messages = resolveAppPromptMessages(locale);
		expect(messages.appStore).toBe('App Store');
		expect(messages.googlePlay).toBe('Google Play');
	});
});
