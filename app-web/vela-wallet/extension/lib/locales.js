/**
 * Which of the packaged locales an extension page should open in.
 *
 * Shared by the service worker (which opens the wallet tab and the request
 * window) and the side panel's doorway page (which has to pick a locale before
 * it can navigate to a prerendered page at all). Pure: no extension API is
 * touched here, the caller passes the browser's UI language in.
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
 * The locale the SIDE PANEL opens in (G59): the wallet's pinned language
 * first, Chrome's UI language otherwise. "Not pinned" is the core's reading
 * of the stored value (`prefs::language`: empty, `system` or `auto`), and a
 * tag no page is packaged for falls back to Chrome's language rather than to
 * a page that does not exist. `panel-locale.test.ts` pins the correspondence.
 */
export function panelLocale(pinned, uiLanguage) {
	const tag = typeof pinned === 'string' ? pinned.trim() : '';
	if (tag && LOCALES.includes(tag)) return tag;
	return negotiate(uiLanguage);
}

export const walletPage = (locale) => `${locale}/wallet.html`;
export const requestPage = (locale, rid) =>
	rid === undefined
		? `${locale}/request.html`
		: `${locale}/request.html?rid=${encodeURIComponent(rid)}`;
