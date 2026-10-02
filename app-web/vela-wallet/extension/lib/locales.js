/**
 * Which of the packaged locales an extension page opens in — decided ONCE, here,
 * for every surface (spec 086, issue 317).
 *
 * Every page the wallet has is prerendered per locale, so a surface has to pick
 * its locale before it can navigate to a page at all. The service worker cannot
 * read the wallet's pinned language (`localStorage` does not exist in a worker),
 * so it never picks: the side panel, the request window and the wallet tab all
 * open through a doorway page (`panel.html`, `open.html`) that reads the pinned
 * language and asks `surfaceLocale`. Pure but for `pinnedLanguage`'s read: the
 * caller passes the browser's UI language in.
 */
export const LOCALES = [
	'en',
	'zh',
	'zh-TW',
	'zh-HK',
	'ja',
	'ko',
	'vi',
	'id',
	'tr',
	'es-MX',
	'pt-BR',
	'fr',
	'de',
	'ru',
	'it'
];

export function negotiate(tag) {
	if (!tag) return 'en';
	if (LOCALES.includes(tag)) return tag;
	const base = tag.split('-')[0];
	return LOCALES.find((l) => l === base || l.startsWith(`${base}-`)) ?? 'en';
}

/**
 * Where the wallet keeps the language a person pinned (`localStorage`, the
 * Expo key the preferences store writes — `PREF_KEYS.language`).
 */
export const LANGUAGE_KEY = 'vela.language';

/**
 * The language a person pinned in the wallet, read where the wallet keeps it:
 * the extension origin's `localStorage`, which every extension PAGE shares
 * (and the worker has no access to). `null` when storage is denied.
 *
 * @param {{ getItem(key: string): string | null } | undefined} storage
 */
export function pinnedLanguage(storage = globalThis.localStorage) {
	try {
		return storage?.getItem(LANGUAGE_KEY) ?? null;
	} catch {
		return null; // storage denied — Chrome's language decides
	}
}

/**
 * The locale EVERY extension surface opens in (G59 for the side panel; spec
 * 086, issue 317 for the request window and the wallet tab, which the worker
 * used to open in Chrome's UI language whatever the person had chosen): the
 * wallet's pinned language first, Chrome's UI language otherwise. "Not
 * pinned" is the core's reading of the stored value (`prefs::language`:
 * empty, `system` or `auto`), and a tag no page is packaged for falls back to
 * Chrome's language rather than to a page that does not exist.
 * `panel-locale.test.ts` pins the correspondence.
 */
export function surfaceLocale(pinned, uiLanguage) {
	const tag = typeof pinned === 'string' ? pinned.trim() : '';
	if (tag && LOCALES.includes(tag)) return tag;
	return negotiate(uiLanguage);
}

/**
 * What an open side panel does when the pinned language changes in another
 * document (issue 317: a panel open across the change kept drawing its sheets
 * in the old language). `'stay'` — it is already in the locale a surface
 * opens in now; `'wait'` — it owes a request or shows something over the
 * wallet, and a sheet a person is reading is never redrawn under them;
 * `'reopen'` — it goes back through its doorway, in the new locale.
 */
export function panelLanguageStep({ pinned, uiLanguage, current, busy }) {
	if (surfaceLocale(pinned, uiLanguage) === current) return 'stay';
	return busy ? 'wait' : 'reopen';
}

export const walletPage = (locale) => `${locale}/wallet.html`;
export const requestPage = (locale, rid) =>
	rid === undefined
		? `${locale}/request.html`
		: `${locale}/request.html?rid=${encodeURIComponent(rid)}`;

/** The side panel's doorway (`side_panel.default_path`). */
export const PANEL_DOOR = 'panel.html';

/**
 * The doorway the worker opens the request window and the wallet tab at: the
 * page then picks the locale (`open.js`), because the worker cannot.
 */
export const openDoor = (rid) =>
	rid === undefined ? 'open.html' : `open.html?rid=${encodeURIComponent(rid)}`;

/**
 * Where `open.html` sends its page: the request window to its request, the
 * toolbar's tab to the wallet — in `locale`. `search` is the doorway's own
 * query (`?rid=…`).
 */
export function openDoorTarget(search, locale) {
	const rid = new URLSearchParams(search).get('rid');
	return rid ? requestPage(locale, rid) : walletPage(locale);
}
