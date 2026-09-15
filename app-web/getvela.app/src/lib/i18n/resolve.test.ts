import { describe, expect, it } from 'vitest';
import { en } from './messages/en';
import {
	PAGE_NAMESPACES,
	__internals,
	catalog,
	namespaceState,
	translatedLocales
} from './resolve';

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

	/**
	 * These used to pick, at test time, a locale whose `home` was still English,
	 * and assert the fallback through it. On 2026-09-15 the last four locales
	 * (vi, id, tr, zh-HK) were translated and no such locale exists any more —
	 * the guard test that watched for exactly this moment said to delete rather
	 * than weaken them, so the fallback-through-a-real-locale pair is gone.
	 *
	 * What is NOT gone: fallback itself is still the rule for the 16 docs pages,
	 * and the rule it rests on — a namespace is used only when every leaf of it
	 * is present — is asserted directly by the `isComplete` block above and by
	 * `namespaceState` below. What remains here is the link rewriting, which was
	 * never about fallback: an internal href is written English-relative in every
	 * catalog and must come back out under the reader's prefix.
	 */
	it('keeps a page’s internal links inside the reader’s locale', () => {
		// Found by its link rather than by its index: the table is edited far more
		// often than this test, and a row number would quietly stop testing
		// anything the first time a row is added above it.
		const linked = (locale: string) =>
			catalog(locale as never).home.compare.rows.find((r) => r.vela.includes('href="'))?.vela ?? '';
		expect(linked('en'), 'a row with an internal link').toContain('href="/docs/');
		expect(linked('zh')).toContain('href="/zh/docs/');
	});

	it('leaves external links and anchors alone', () => {
		expect(catalog('zh').home.why.p1).toContain('href="https://account.base.app"');
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
