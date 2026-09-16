import { describe, expect, it } from 'vitest';
import { docLocales, getDocSlugs, isDocTranslated } from '$lib/content/docs';
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, pathFor } from './locales';
import { namespaceState } from './resolve';
import { ENGLISH_ONLY_PAGES, localizedPages, urlsFor } from './urls';

/**
 * The `hreflang` / sitemap invariants (spec 059, contracts/head-and-sitemap.md).
 *
 * These are cheap to state and expensive to get wrong: an alternate set that is
 * not reciprocal, or that advertises a page which renders in English, is how a
 * multilingual site loses ranking instead of gaining it — and neither failure
 * is visible by looking at the page.
 */

describe('the alternate set advertises what exists (FR-018, FR-021)', () => {
	const pages = localizedPages();

	it('covers every localizable page', () => {
		const ids = pages.map((p) => p.id);
		expect(ids).toContain('home');
		expect(ids).toContain('getStarted');
		expect(ids).toContain('about');
		expect(ids).toContain('roadmap');
		expect(ids).toContain('docs:introduction');
		for (const slug of getDocSlugs()) expect(ids).toContain(`docs:${slug}`);
	});

	it('always contains English — it is the source and the x-default', () => {
		for (const page of pages) {
			expect(page.locales, page.id).toContain(DEFAULT_LOCALE);
		}
	});

	it('never lists a locale whose page would render in English', () => {
		for (const page of pages) {
			for (const locale of page.locales) {
				if (locale === DEFAULT_LOCALE) continue;
				const real = page.id.startsWith('docs:')
					? isDocTranslated(locale, page.id.slice('docs:'.length))
					: namespaceState(page.id as 'home', locale) === 'translated';
				expect(real, `${page.id} claims ${locale}`).toBe(true);
			}
		}
	});

	it('is reciprocal by construction — every URL of a page shares one set', () => {
		for (const page of pages) {
			const urls = urlsFor(page);
			expect(urls.map((u) => u.locale)).toEqual(page.locales);
			// Each URL is the same English path under its own prefix, so any two
			// versions of a page necessarily list each other.
			for (const u of urls) {
				expect(u.path).toBe(pathFor(u.locale, page.englishPath));
			}
		}
	});

	it('only ever names a supported locale', () => {
		for (const page of pages) {
			for (const locale of page.locales) {
				expect(SUPPORTED_LOCALES).toContain(locale);
			}
		}
	});

	it('gives every page a unique id and a unique English path', () => {
		const ids = pages.map((p) => p.id);
		const paths = pages.map((p) => p.englishPath);
		expect(new Set(ids).size).toBe(ids.length);
		expect(new Set(paths).size).toBe(paths.length);
	});
});

describe('English-only pages stay English-only (R5)', () => {
	it('lists blog and the legal pages, and nothing localized', () => {
		const paths = ENGLISH_ONLY_PAGES.map((p) => p.path);
		expect(paths).toEqual(['/blog', '/privacy', '/terms']);
	});

	it('does not overlap the localized set — one page, one owner', () => {
		const localized = new Set(localizedPages().map((p) => p.englishPath));
		for (const page of ENGLISH_ONLY_PAGES) {
			expect(localized.has(page.path), page.path).toBe(false);
		}
	});
});

describe('docs translation state', () => {
	it('reports English as present for every doc in the sidebar', () => {
		for (const slug of getDocSlugs()) {
			expect(isDocTranslated(DEFAULT_LOCALE, slug), slug).toBe(true);
			expect(docLocales(slug), slug).toContain(DEFAULT_LOCALE);
		}
	});

	it('does not claim a translation for a locale with no file', () => {
		// `xx` is not a locale at all; the guard is that nothing invents one.
		for (const slug of getDocSlugs()) {
			for (const locale of docLocales(slug)) {
				expect(SUPPORTED_LOCALES).toContain(locale);
			}
		}
	});
});
