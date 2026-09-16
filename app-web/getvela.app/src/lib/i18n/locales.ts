/**
 * The locale registry (spec 059, R2 + contracts/routing.md).
 *
 * The fifteen tags and the endonyms are the SAME ONES THE WALLET USES —
 * `app-web/vela-wallet/src/lib/i18n/locales.ts` (`SUPPORTED_LOCALES`) and
 * `app-web/vela-wallet/src/lib/settings/fixtures.ts` (`LOCALE_ENDONYMS`).
 * `locales.test.ts` reads both of those files off disk and fails if this one
 * drifts, so there is no "keep these in sync" comment to ignore: it is a test.
 *
 * Where this file deliberately differs from the wallet's: the wallet negotiates
 * `Accept-Language` at `/` and 307s. This site does not (R3) — English lives at
 * the unprefixed root so crawlers and shared links always get what the URL says.
 * The negotiation below is used only by the client-side offer banner.
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

/** English is the source language and the only one without a URL segment. */
export const DEFAULT_LOCALE: Locale = 'en';

/** The fourteen tags that appear as a path prefix. `en` never does (FR-008). */
export const PREFIXED_LOCALES: readonly Locale[] = SUPPORTED_LOCALES.filter(
	(l) => l !== DEFAULT_LOCALE
);

export interface LocaleInfo {
	/** Canonical tag — also the URL segment, for everything but `en`. */
	tag: Locale;
	/** The language's name IN that language. Never translated (FR-012). */
	endonym: string;
	/** `og:locale` value (FR-023). */
	ogLocale: string;
	/** Writing direction. All fifteen are `ltr`; recorded so a sixteenth can't sneak in. */
	dir: 'ltr' | 'rtl';
}

export const LOCALES: Record<Locale, LocaleInfo> = {
	en: { tag: 'en', endonym: 'English', ogLocale: 'en_US', dir: 'ltr' },
	zh: { tag: 'zh', endonym: '简体中文', ogLocale: 'zh_CN', dir: 'ltr' },
	'zh-TW': { tag: 'zh-TW', endonym: '繁體中文（台灣）', ogLocale: 'zh_TW', dir: 'ltr' },
	'zh-HK': { tag: 'zh-HK', endonym: '繁體中文（香港）', ogLocale: 'zh_HK', dir: 'ltr' },
	ja: { tag: 'ja', endonym: '日本語', ogLocale: 'ja_JP', dir: 'ltr' },
	ko: { tag: 'ko', endonym: '한국어', ogLocale: 'ko_KR', dir: 'ltr' },
	vi: { tag: 'vi', endonym: 'Tiếng Việt', ogLocale: 'vi_VN', dir: 'ltr' },
	id: { tag: 'id', endonym: 'Bahasa Indonesia', ogLocale: 'id_ID', dir: 'ltr' },
	tr: { tag: 'tr', endonym: 'Türkçe', ogLocale: 'tr_TR', dir: 'ltr' },
	'es-MX': { tag: 'es-MX', endonym: 'Español (México)', ogLocale: 'es_MX', dir: 'ltr' },
	'pt-BR': { tag: 'pt-BR', endonym: 'Português (Brasil)', ogLocale: 'pt_BR', dir: 'ltr' },
	fr: { tag: 'fr', endonym: 'Français', ogLocale: 'fr_FR', dir: 'ltr' },
	de: { tag: 'de', endonym: 'Deutsch', ogLocale: 'de_DE', dir: 'ltr' },
	ru: { tag: 'ru', endonym: 'Русский', ogLocale: 'ru_RU', dir: 'ltr' },
	it: { tag: 'it', endonym: 'Italiano', ogLocale: 'it_IT', dir: 'ltr' }
};

const BY_LOWER = new Map<string, Locale>(SUPPORTED_LOCALES.map((l) => [l.toLowerCase(), l]));

/**
 * Tags that are NOT catalogs but must still land somewhere deliberate — a hand
 * written link, a browser sending a regional variant, an old campaign URL. Each
 * one 308s to its target (contracts/routing.md §Must 308); none of them is ever
 * served content of its own, or the same page would have two URLs.
 */
const ALIASES: Record<string, Locale> = {
	pt: 'pt-BR',
	'pt-pt': 'pt-BR',
	es: 'es-MX',
	'es-es': 'es-MX',
	'zh-cn': 'zh',
	'zh-hans': 'zh',
	'zh-sg': 'zh',
	'zh-hant': 'zh-TW',
	'zh-mo': 'zh-TW'
};

/** True for a canonical tag in any casing. */
export function isLocale(value: string): value is Locale {
	return BY_LOWER.has(value.toLowerCase());
}

/** A canonical tag in canonical casing, or undefined. Exact tags only — no aliases. */
export function toLocale(value: string | undefined | null): Locale | undefined {
	if (!value) return undefined;
	return BY_LOWER.get(value.toLowerCase());
}

/**
 * What a non-canonical tag should redirect to, or undefined if it is already
 * canonical / unknown. Three layers, most specific first:
 *   1. wrong case            `/PT-BR/` → `pt-BR`
 *   2. the alias table       `/pt/`    → `pt-BR`
 *   3. base-language fallback `/fr-CA/` → `fr`
 * A value that resolves to itself returns undefined — there is nothing to do.
 */
export function resolveAlias(value: string): Locale | undefined {
	const lower = value.toLowerCase();

	const exact = BY_LOWER.get(lower);
	if (exact) return exact === value ? undefined : exact;

	const aliased = ALIASES[lower];
	if (aliased) return aliased;

	const base = lower.split('-')[0];
	const byBase = BY_LOWER.get(base);
	if (byBase) return byBase;

	return undefined;
}

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
				if (m) q = Number.parseFloat(m[1]);
			}
			return { tag, q: Number.isFinite(q) ? q : 0, order };
		})
		.filter((c): c is Candidate => c !== null && c.q > 0)
		.sort((a, b) => b.q - a.q || a.order - b.order);
}

/**
 * The reader's best-matching locale, for the OFFER BANNER ONLY (FR-013).
 * Nothing on this site redirects on the strength of this value — R3.
 * Accepts an `Accept-Language` header string or a `navigator.languages` array.
 */
export function negotiate(input: string | readonly string[] | null | undefined): Locale {
	if (!input) return DEFAULT_LOCALE;

	const tags =
		typeof input === 'string'
			? parseAcceptLanguage(input).map((c) => c.tag)
			: [...input].filter(Boolean);

	for (const tag of tags) {
		if (tag === '*') return DEFAULT_LOCALE;
		const exact = toLocale(tag);
		if (exact) return exact;
		const aliased = resolveAlias(tag);
		if (aliased) return aliased;
	}
	return DEFAULT_LOCALE;
}

/** URL segment for a locale — empty for English, which has no prefix. */
export function localeSegment(locale: Locale): string {
	return locale === DEFAULT_LOCALE ? '' : locale;
}

/**
 * Split a pathname into its locale and the rest. `/ja/docs/faq` → `{ locale:
 * 'ja', path: '/docs/faq' }`; `/docs/faq` → `{ locale: 'en', path: '/docs/faq' }`.
 * A prefix that is not a canonical tag is NOT a locale — `/en/…` included — so
 * the caller sees the unmodified path and the router 404s it (FR-010).
 */
export function splitLocalePath(pathname: string): { locale: Locale; path: string } {
	const [, first = '', ...rest] = pathname.split('/');
	// Canonical case only: `/PT-BR/` is a redirect, not a page.
	if (first && (SUPPORTED_LOCALES as readonly string[]).includes(first) && first !== DEFAULT_LOCALE) {
		return { locale: first as Locale, path: '/' + rest.join('/') };
	}
	return { locale: DEFAULT_LOCALE, path: pathname };
}

/**
 * The URL of `path` (an English, unprefixed path) in `locale`.
 * `pathFor('ja', '/docs/faq')` → `/ja/docs/faq`; `pathFor('en', …)` → unchanged.
 */
export function pathFor(locale: Locale, path: string): string {
	const clean = path.startsWith('/') ? path : `/${path}`;
	if (locale === DEFAULT_LOCALE) return clean;
	return clean === '/' ? `/${locale}` : `/${locale}${clean}`;
}

/** The same page the reader is on, in another language (FR-012). */
export function switchTo(locale: Locale, currentPathname: string): string {
	return pathFor(locale, splitLocalePath(currentPathname).path);
}
