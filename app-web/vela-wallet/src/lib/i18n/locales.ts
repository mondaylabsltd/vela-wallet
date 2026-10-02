/**
 * Locale registry + Accept-Language negotiation (spec 006, contracts/i18n-ssr.md).
 *
 * The 15 tags and the base-language mapping are copied from the RN app's
 * `src/i18n/shared.ts` (`SUPPORTED_LANGUAGES` / `detectSystemLanguage`) so web
 * and native resolve the same browser/device locale to the same catalog. If
 * that table changes, this one must change with it — `locales.test.ts` pins
 * the behavior.
 */

export const SUPPORTED_LOCALES = [
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
] as const;

export type Locale = (typeof SUPPORTED_LOCALES)[number];

export const FALLBACK_LOCALE: Locale = 'en';

const BY_LOWER = new Map<string, Locale>(SUPPORTED_LOCALES.map((l) => [l.toLowerCase(), l]));

export function isLocale(value: string): value is Locale {
	return BY_LOWER.get(value.toLowerCase()) !== undefined;
}

/** Canonical-case form of a supported tag, else undefined. */
export function toLocale(value: string): Locale | undefined {
	return BY_LOWER.get(value.toLowerCase());
}

/**
 * The locale a page's PATH names — its first segment, on the site (`/ja/wallet`)
 * and in the packaged extension (`/ja/wallet.html`, `/ja.html`) alike.
 *
 * A layout above `[locale]` cannot ask `page.params` for it: a page the
 * extension loads fresh has no route params until its first client navigation
 * (measured, Chrome for Testing 151: `page.params` is `{}` and `page.route.id`
 * null on `/ja/wallet.html`), so `page.params.locale ?? 'en'` read English
 * (issue 317).
 */
export function localeOfPath(pathname: string): Locale | undefined {
	const first = pathname.split('/')[1] ?? '';
	return toLocale(first.replace(/\.html$/, ''));
}

/** One parsed Accept-Language candidate. */
interface Candidate {
	tag: string;
	q: number;
	order: number;
}

function parseAcceptLanguage(header: string): Candidate[] {
	return header
		.split(',')
		.map((part, order): Candidate | null => {
			const [rawTag, ...params] = part.trim().split(';');
			const tag = rawTag?.trim();
			if (!tag) return null;
			let q = 1;
			for (const p of params) {
				const m = p.trim().match(/^q=([\d.]+)$/i);
				if (m) q = Number(m[1]);
			}
			return Number.isFinite(q) && q > 0 ? { tag, q, order } : null;
		})
		.filter((c): c is Candidate => c !== null)
		.sort((a, b) => b.q - a.q || a.order - b.order);
}

/** RN `detectSystemLanguage` table, applied to one BCP-47 tag. */
function matchTag(tag: string): Locale | undefined {
	const exact = toLocale(tag);
	if (exact) return exact;

	const subtags = tag.toLowerCase().split('-');
	const code = subtags[0];
	const script = subtags.find((s) => s === 'hans' || s === 'hant');
	const region = subtags[subtags.length - 1]?.toUpperCase();

	if (code === 'zh') {
		const traditional = script === 'hant' || region === 'TW' || region === 'HK' || region === 'MO';
		if (!traditional) return 'zh';
		return region === 'HK' || region === 'MO' ? 'zh-HK' : 'zh-TW';
	}
	if (code === 'id' || code === 'in') return 'id'; // 'in' = legacy Android code
	if (code === 'es') return 'es-MX'; // only Spanish variant shipped
	if (code === 'pt') return 'pt-BR'; // only Portuguese variant shipped
	return toLocale(code);
}

/**
 * Text direction registry. All 15 shipped locales are LTR; kept as data (not a
 * constant at call sites) so an RTL locale later only touches this file. The
 * engine remains the authority: `messages.test.ts` asserts `engine.dir()`
 * agrees for every locale.
 */
const RTL_LOCALES = new Set<Locale>();

export function textDirectionOf(locale: Locale): 'ltr' | 'rtl' {
	return RTL_LOCALES.has(locale) ? 'rtl' : 'ltr';
}

/** Deterministic Accept-Language → supported locale (contracts/i18n-ssr.md). */
export function negotiate(acceptLanguage: string | null | undefined): Locale {
	if (!acceptLanguage) return FALLBACK_LOCALE;
	for (const { tag } of parseAcceptLanguage(acceptLanguage)) {
		if (tag === '*') return FALLBACK_LOCALE;
		const match = matchTag(tag);
		if (match) return match;
	}
	return FALLBACK_LOCALE;
}
