/**
 * Every extension surface opens in the wallet's pinned language (spec 082
 * T212, G59; spec 086, issue 317).
 *
 * G59: the device pass pinned 简体中文 in the panel's own Settings, closed and
 * reopened the panel, and got English sheets: the doorway (`panel.js`) chose
 * the locale from Chrome's UI language only. Issue 317: the side panel was
 * fixed, but the REQUEST WINDOW — the surface a page's request opens when it
 * carries no click — and the wallet tab were still opened by the worker in
 * Chrome's UI language; on a Chinese Chrome with English chosen, the signing
 * sheet was Chinese. The worker cannot read the pin, so both now open through
 * `open.html`, which asks the same rule.
 *
 * `surfaceLocale` is the doorways' rule, pure. Which stored values mean "not
 * pinned" is the core's (`prefs::language`: empty, `system`, `auto`); the
 * doorway cannot load the core before its first navigation, so the
 * correspondence is pinned here against `prefsRead`.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { prefsRead } from '$lib/core/client';
import { PREF_KEYS } from '$lib/services/preferences.svelte';
import {
	LANGUAGE_KEY,
	LOCALES,
	openDoor,
	openDoorTarget,
	panelLanguageStep,
	pinnedLanguage,
	surfaceLocale
} from '../../../extension/lib/locales.js';

describe('the locale every extension surface opens in (G59, issue 317)', () => {
	it('is the pinned language when one is pinned', () => {
		expect(surfaceLocale('zh', 'en-US')).toBe('zh');
		expect(surfaceLocale('zh-TW', 'en-US')).toBe('zh-TW');
		expect(surfaceLocale(' ja ', 'en-US')).toBe('ja');
	});

	it('follows Chrome’s language when nothing is pinned', () => {
		expect(surfaceLocale(null, 'de-DE')).toBe('de');
		expect(surfaceLocale('auto', 'ko')).toBe('ko');
		expect(surfaceLocale('system', 'fr-FR')).toBe('fr');
		expect(surfaceLocale('', undefined)).toBe('en');
	});

	it('never opens a page the package does not have', () => {
		// A tag no page is prerendered for: Chrome's language decides instead.
		expect(surfaceLocale('xx-YY', 'pt-BR')).toBe('pt-BR');
		for (const pinned of LOCALES) expect(surfaceLocale(pinned, 'en')).toBe(pinned);
	});

	it('reads the key the wallet writes', () => {
		expect(LANGUAGE_KEY).toBe(PREF_KEYS.language);
	});

	it('treats as "not pinned" exactly what the core reads as auto', () => {
		for (const raw of ['', 'system', 'auto', ' auto ', 'zh', 'zh-TW', 'ja']) {
			const core = (
				JSON.parse(prefsRead(JSON.stringify({ [PREF_KEYS.language]: raw }))) as {
					language: string;
				}
			).language;
			const pinned = surfaceLocale(raw, 'ko') !== 'ko';
			expect(pinned, `"${raw}"`).toBe(core !== 'auto');
		}
	});
});

describe('the doorways (issue 317)', () => {
	it('reads the pinned language where the wallet keeps it, and survives a denied storage', () => {
		const store = new Map<string, string>([[LANGUAGE_KEY, 'en']]);
		expect(pinnedLanguage({ getItem: (k: string) => store.get(k) ?? null })).toBe('en');
		expect(pinnedLanguage({ getItem: () => null })).toBeNull();
		expect(
			pinnedLanguage({
				getItem: () => {
					throw new Error('denied');
				}
			})
		).toBeNull();
	});

	it('the report: Chrome in Chinese, English chosen — every surface opens in English', () => {
		const locale = surfaceLocale('en', 'zh-CN');
		expect(locale).toBe('en');
		expect(
			openDoorTarget(new URL(`chrome-extension://x/${openDoor('7:p:1')}`).search, locale)
		).toBe('en/request.html?rid=7%3Ap%3A1');
		expect(openDoorTarget(new URL(`chrome-extension://x/${openDoor()}`).search, locale)).toBe(
			'en/wallet.html'
		);
	});

	it('first run, nothing chosen: Chrome’s language', () => {
		expect(surfaceLocale(null, 'zh-CN')).toBe('zh');
		expect(surfaceLocale(null, 'zh-TW')).toBe('zh-TW');
		expect(surfaceLocale(null, 'pt-PT')).toBe('pt-BR');
	});

	it('carries the request id through the doorway unchanged, whatever it holds', () => {
		for (const rid of ['12:abc', '12:a&b=c', '3:ünï', '9:?x#y']) {
			const search = new URL(`chrome-extension://x/${openDoor(rid)}`).search;
			const target = openDoorTarget(search, 'ja');
			expect(new URLSearchParams(target.split('?')[1]).get('rid'), rid).toBe(rid);
			expect(target.startsWith('ja/request.html?')).toBe(true);
		}
	});

	it('every packaged locale has both doors', () => {
		for (const locale of LOCALES) {
			expect(openDoorTarget('', locale)).toBe(`${locale}/wallet.html`);
			expect(openDoorTarget('?rid=1%3A2', locale)).toBe(`${locale}/request.html?rid=1%3A2`);
		}
	});
});

describe('an open side panel when the language changes elsewhere (issue 317)', () => {
	it('reopens in the new language once it is idle', () => {
		expect(
			panelLanguageStep({ pinned: 'en', uiLanguage: 'zh-CN', current: 'zh', busy: false })
		).toBe('reopen');
	});

	it('never redraws a sheet a person is reading, nor drops a request: it waits', () => {
		expect(
			panelLanguageStep({ pinned: 'en', uiLanguage: 'zh-CN', current: 'zh', busy: true })
		).toBe('wait');
	});

	it('stays when it already speaks the language a surface would open in', () => {
		expect(
			panelLanguageStep({ pinned: 'zh', uiLanguage: 'en-US', current: 'zh', busy: false })
		).toBe('stay');
		// Unpinned ("follow system") on a Chinese Chrome: Chinese, as it is.
		expect(
			panelLanguageStep({ pinned: 'auto', uiLanguage: 'zh-CN', current: 'zh', busy: true })
		).toBe('stay');
	});
});
