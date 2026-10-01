/**
 * The side panel opens in the wallet's pinned language (spec 082 T212, G59).
 *
 * The device pass pinned 简体中文 in the panel's own Settings, closed and
 * reopened the panel, and got English sheets: the doorway (`panel.js`) chose
 * the locale from Chrome's UI language only, so Settings read "Language
 * 简体中文" on an English page and the picker ticked English.
 *
 * `panelLocale` is the doorway's rule, pure. Which stored values mean "not
 * pinned" is the core's (`prefs::language`: empty, `system`, `auto`); the
 * doorway cannot load the core before its first navigation, so the
 * correspondence is pinned here against `prefsRead`.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { prefsRead } from '$lib/core/client';
import { PREF_KEYS } from '$lib/services/preferences.svelte';
import { LANGUAGE_KEY, LOCALES, panelLocale } from '../../../extension/lib/locales.js';

describe('the side panel’s locale (G59)', () => {
	it('is the pinned language when one is pinned', () => {
		expect(panelLocale('zh', 'en-US')).toBe('zh');
		expect(panelLocale('zh-TW', 'en-US')).toBe('zh-TW');
		expect(panelLocale(' ja ', 'en-US')).toBe('ja');
	});

	it('follows Chrome’s language when nothing is pinned', () => {
		expect(panelLocale(null, 'de-DE')).toBe('de');
		expect(panelLocale('auto', 'ko')).toBe('ko');
		expect(panelLocale('system', 'fr-FR')).toBe('fr');
		expect(panelLocale('', undefined)).toBe('en');
	});

	it('never opens a page the package does not have', () => {
		// A tag no page is prerendered for: Chrome's language decides instead.
		expect(panelLocale('xx-YY', 'pt-BR')).toBe('pt-BR');
		for (const pinned of LOCALES) expect(panelLocale(pinned, 'en')).toBe(pinned);
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
			const pinned = panelLocale(raw, 'ko') !== 'ko';
			expect(pinned, `"${raw}"`).toBe(core !== 'auto');
		}
	});
});
