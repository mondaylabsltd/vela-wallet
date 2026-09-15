import { describe, expect, it } from 'vitest';
import { en } from './messages/en';
import { SUPPORTED_LOCALES } from './locales';
import { PAGE_NAMESPACES, __internals, catalog, namespaceState, translatedLocales } from './resolve';

const { isComplete } = __internals;

/**
 * The rule this file exists to protect: **a page is translated or it is not.**
 * A key-by-key merge would quietly ship a page with a Japanese heading and an
 * English third paragraph, which is the one outcome SC-006 forbids — worse than
 * either language on its own, and invisible in review because every key
 * resolves to *something*.
 */

describe('completeness is all-or-nothing per namespace (SC-006)', () => {
	it('accepts a translation that covers every leaf', () => {
		expect(isComplete({ a: 'x', b: { c: 'y' } }, { a: '一', b: { c: '二' } })).toBe(true);
	});

	it('rejects one missing leaf, however deep', () => {
		expect(isComplete({ a: 'x', b: { c: 'y' } }, { a: '一', b: {} })).toBe(false);
	});

	it('rejects an empty or whitespace string — present is not translated', () => {
		expect(isComplete({ a: 'x' }, { a: '' })).toBe(false);
		expect(isComplete({ a: 'x' }, { a: '   ' })).toBe(false);
	});

	it('rejects a shortened array — a 5-item FAQ as 4 items is a shorter page', () => {
		expect(isComplete(['a', 'b'], ['一'])).toBe(false);
		expect(isComplete(['a', 'b'], ['一', '二'])).toBe(true);
	});

	it('rejects a wrong-typed branch', () => {
		expect(isComplete({ a: { b: 'x' } }, { a: 'oops' })).toBe(false);
		expect(isComplete(['a'], { 0: 'a' })).toBe(false);
	});
});

describe('catalog()', () => {
	it('returns the source object itself for English', () => {
		expect(catalog('en')).toBe(en);
	});

	it('always resolves every key, in every locale, even untranslated ones', () => {
		for (const ns of PAGE_NAMESPACES) {
			for (const locale of ['ja', 'zh', 'ru'] as const) {
				expect(isComplete(en[ns], catalog(locale)[ns]), `${locale}/${ns}`).toBe(true);
			}
		}
	});

	// Picked at test time rather than hard-coded: as locales get translated, a
	// hard-coded example silently stops testing fallback and starts testing a
	// translation. The test needs a locale that is genuinely still English.
	const untranslated = SUPPORTED_LOCALES.find((l) => namespaceState('home', l) === 'fallback');

	it('has a locale left to demonstrate fallback with', () => {
		// If this ever fails, every locale is translated — delete these three
		// tests rather than weakening them.
		expect(untranslated, 'no untranslated locale remains').toBeDefined();
	});

	it('falls back to the whole English namespace, not a mixture', () => {
		// The fallback locale's home namespace is the English one, word for word,
		// not an English-shaped merge of translated fragments. The only difference
		// allowed is the link prefix (see the next test), so the comparison is
		// made with hrefs stripped.
		const strip = (v: unknown): unknown =>
			JSON.parse(JSON.stringify(v).replace(/href=\\"[^"]*\\"/g, 'href'));
		expect(strip(catalog(untranslated!).home)).toEqual(strip(en.home));
	});

	it('keeps a fallback page’s internal links inside the reader’s locale', () => {
		// An English page at /tr must not tip the reader back out to /docs — they
		// asked for Turkish and the rest of the site still has it.
		expect(catalog(untranslated!).home.compare.rows[3].vela).toContain(
			`href="/${untranslated}/docs/security-audits"`
		);
		expect(catalog('en').home.compare.rows[3].vela).toContain('href="/docs/security-audits"');
	});

	it('leaves external links and anchors alone', () => {
		expect(catalog(untranslated!).home.why.p1).toContain('href="https://account.base.app"');
	});

	it('keeps chrome and notice readable in a locale whose pages fall back', () => {
		// The point of the exception: the sentence saying "this is English" must
		// not itself be English.
		expect(catalog('zh').notice.fallback).not.toBe(en.notice.fallback);
		expect(catalog('zh').chrome.nav.docs).not.toBe(en.chrome.nav.docs);
	});
});

describe('the translation record drives hreflang (FR-018, FR-021)', () => {
	it('always contains English', () => {
		for (const ns of PAGE_NAMESPACES) {
			expect(translatedLocales(ns)).toContain('en');
		}
	});

	it('never advertises a locale whose page falls back', () => {
		for (const ns of PAGE_NAMESPACES) {
			for (const locale of translatedLocales(ns)) {
				expect(namespaceState(ns, locale), `${locale}/${ns}`).not.toBe('fallback');
			}
		}
	});

	it('marks English as the source, not as a translation', () => {
		expect(namespaceState('home', 'en')).toBe('source');
	});
});
