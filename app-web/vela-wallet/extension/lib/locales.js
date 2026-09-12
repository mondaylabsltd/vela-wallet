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

export const walletPage = (locale) => `${locale}/wallet.html`;
export const requestPage = (locale, rid) =>
	rid === undefined
		? `${locale}/request.html`
		: `${locale}/request.html?rid=${encodeURIComponent(rid)}`;
