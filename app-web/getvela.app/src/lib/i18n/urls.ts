import { docLocales, getDocSlugs } from '$lib/content/docs';
import { DOCS_INDEX_SLUG } from '$lib/content/sidebar';
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, pathFor, type Locale } from './locales';
import { namespaceState, type PageNamespace } from './resolve';

/**
 * The site's localized URL set, in one place (spec 059, contracts/head-and-sitemap.md).
 *
 * This exists so that the three things that must agree — what a page renders,
 * which `hreflang` alternates it emits, and what the sitemap lists — are
 * computed from ONE function rather than three lists that drift. A sitemap
 * advertising a translation the router will serve in English is the specific
 * failure this prevents.
 */

export interface LocalizedPage {
	/** Stable id for tests and reports. */
	id: string;
	/** The unprefixed English path. */
	englishPath: string;
	/** Locales that genuinely have this page — never includes a fallback. */
	locales: Locale[];
	changefreq: string;
	priority: string;
}

/** Pages that live in the locale subtree, with their real translation state. */
export function localizedPages(): LocalizedPage[] {
	const fromNamespace = (
		id: string,
		ns: PageNamespace,
		englishPath: string,
		changefreq: string,
		priority: string
	): LocalizedPage => ({
		id,
		englishPath,
		locales: localesFor(ns),
		changefreq,
		priority
	});

	const pages: LocalizedPage[] = [
		fromNamespace('home', 'home', '/', 'weekly', '1.0'),
		fromNamespace('getStarted', 'getStarted', '/get-started', 'monthly', '0.9'),
		fromNamespace('about', 'about', '/about', 'monthly', '0.7'),
		fromNamespace('roadmap', 'roadmap', '/roadmap', 'monthly', '0.6'),
		// The docs index renders `introduction` at the bare /docs path.
		{
			id: `docs:${DOCS_INDEX_SLUG}`,
			englishPath: '/docs',
			locales: docLocales(DOCS_INDEX_SLUG),
			changefreq: 'weekly',
			priority: '0.8'
		}
	];

	for (const slug of getDocSlugs()) {
		pages.push({
			id: `docs:${slug}`,
			englishPath: `/docs/${slug}`,
			locales: docLocales(slug),
			changefreq: 'monthly',
			priority: '0.6'
		});
	}

	return pages;
}

/**
 * Locales whose version of this namespace is real. English is always in the
 * list: it is the source, and `x-default` points at it.
 */
function localesFor(ns: PageNamespace): Locale[] {
	const out: Locale[] = [DEFAULT_LOCALE];
	for (const locale of SUPPORTED_LOCALES) {
		if (locale === DEFAULT_LOCALE) continue;
		if (namespaceState(ns, locale) === 'translated') out.push(locale);
	}
	return out;
}

/** Every URL this page has, keyed by locale. */
export function urlsFor(page: LocalizedPage): { locale: Locale; path: string }[] {
	return page.locales.map((locale) => ({ locale, path: pathFor(locale, page.englishPath) }));
}

/** Pages that exist only in English and have no alternates (R5). */
export const ENGLISH_ONLY_PAGES: { path: string; changefreq: string; priority: string }[] = [
	{ path: '/blog', changefreq: 'weekly', priority: '0.8' },
	{ path: '/privacy', changefreq: 'yearly', priority: '0.3' },
	{ path: '/terms', changefreq: 'yearly', priority: '0.3' }
];
