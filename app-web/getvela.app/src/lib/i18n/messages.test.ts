import { describe, expect, it } from 'vitest';
import { SUPPORTED_LOCALES, DEFAULT_LOCALE, type Locale } from './locales';
import { en } from './messages/en';
import { __internals, PAGE_NAMESPACES } from './resolve';

const { TRANSLATIONS } = __internals;

/**
 * Structural checks on the translation files (spec 059, FR-026, FR-031).
 *
 * None of these can tell you whether a translation is GOOD — that is the R7
 * reading pass, and no test replaces it. What they catch is the class of
 * failure a bulk translation pass actually produces: a key that exists in no
 * English file, a link whose href was "translated", a placeholder dropped, an
 * `<a>` opened and never closed. Every one of those is invisible on the page
 * until somebody clicks it.
 */

type Json = string | Json[] | { [k: string]: Json };

function walk(
	source: unknown,
	value: unknown,
	path: string,
	visit: (path: string, english: string, translated: string) => void,
	onExtra: (path: string) => void
): void {
	if (typeof source === 'string') {
		if (typeof value === 'string') visit(path, source, value);
		return;
	}
	if (Array.isArray(source)) {
		if (!Array.isArray(value)) return;
		source.forEach((item, i) => walk(item, value[i], `${path}[${i}]`, visit, onExtra));
		return;
	}
	if (typeof source === 'object' && source !== null) {
		if (typeof value !== 'object' || value === null || Array.isArray(value)) return;
		const src = source as Record<string, unknown>;
		const val = value as Record<string, unknown>;
		for (const key of Object.keys(val)) {
			// `_`-prefixed keys are bookkeeping (see `_fingerprints`), not copy.
			if (key.startsWith('_')) continue;
			if (!(key in src)) onExtra(`${path}.${key}`);
		}
		for (const [key, v] of Object.entries(src)) {
			if (key in val) walk(v, val[key], `${path}.${key}`, visit, onExtra);
		}
	}
}

const localesWithFiles = SUPPORTED_LOCALES.filter(
	(l): l is Locale => l !== DEFAULT_LOCALE && TRANSLATIONS[l] !== undefined
);

describe('every translation file matches the English shape', () => {
	it('has at least one translation to check', () => {
		expect(localesWithFiles.length).toBeGreaterThan(0);
	});

	for (const locale of localesWithFiles) {
		it(`${locale} has no key that English does not have`, () => {
			const extras: string[] = [];
			walk(
				en as unknown as Json,
				TRANSLATIONS[locale] as unknown as Json,
				'',
				() => {},
				(p) => extras.push(p)
			);
			// An extra key is a stale key: the English string it translated is gone,
			// so the translation is dead weight that will never render.
			expect(extras, `stale keys in ${locale}.json`).toEqual([]);
		});
	}
});

describe('chrome and notice are complete in every locale (data-model §3)', () => {
	/**
	 * These two namespaces frame a page that fell back to English — including the
	 * sentence that says so. If they were allowed to fall back too, a reader who
	 * asked for Japanese would be told, in English, that the page is in English.
	 */
	const required = ['chrome', 'notice'] as const;

	for (const locale of localesWithFiles) {
		for (const ns of required) {
			it(`${locale}.${ns} covers every English key`, () => {
				const missing: string[] = [];
				const collect = (source: unknown, value: unknown, path: string) => {
					if (typeof source === 'string') {
						if (typeof value !== 'string' || value.trim() === '') missing.push(path);
						return;
					}
					if (typeof source === 'object' && source !== null && !Array.isArray(source)) {
						const val = (value ?? {}) as Record<string, unknown>;
						for (const [k, v] of Object.entries(source)) collect(v, val[k], `${path}.${k}`);
					}
				};
				collect(
					en[ns],
					(TRANSLATIONS[locale] as Record<string, unknown>)[ns],
					`${locale}.${ns}`
				);
				expect(missing).toEqual([]);
			});
		}
	}
});

describe('inline markup survives translation (FR-031)', () => {
	const hrefs = (s: string) => [...s.matchAll(/href="([^"]*)"/g)].map((m) => m[1]).sort();
	const tags = (s: string) => [...s.matchAll(/<\/?([a-z]+)/g)].map((m) => m[1]).sort();

	for (const locale of localesWithFiles) {
		it(`${locale} keeps every href and tag from the English value`, () => {
			const problems: string[] = [];
			walk(
				en as unknown as Json,
				TRANSLATIONS[locale] as unknown as Json,
				'',
				(path, english, translated) => {
					// A translator translates link TEXT. A changed href means a reader
					// is sent somewhere we did not intend, in one language only.
					const a = hrefs(english);
					const b = hrefs(translated);
					if (a.join('|') !== b.join('|')) {
						problems.push(`${path}: hrefs ${JSON.stringify(a)} → ${JSON.stringify(b)}`);
					}
					const ta = tags(english);
					const tb = tags(translated);
					if (ta.join('|') !== tb.join('|')) {
						problems.push(`${path}: tags ${JSON.stringify(ta)} → ${JSON.stringify(tb)}`);
					}
				},
				() => {}
			);
			expect(problems).toEqual([]);
		});
	}
});

describe('the honesty posture survives translation (FR-032, A02 FR-2/FR-3)', () => {
	/**
	 * The three claims that must never soften: we have no third-party audit of
	 * our own code, the app is alpha, and we cannot reach your keys. A
	 * translation that quietly upgrades "not audited" to "audited" is not a
	 * wording problem — it is a false statement about custody, in a language
	 * nobody on the team reads.
	 */
	const AUDIT_CLAIM = /audit/i;

	it('English still says the app code is not independently audited', () => {
		const text = JSON.stringify(en);
		expect(text).toMatch(AUDIT_CLAIM);
		// "audit is planned/coming" is forbidden outright by A02 FR-2.
		expect(text).not.toMatch(/audit is (planned|coming)/i);
		expect(text).not.toMatch(/planned audit/i);
	});

	for (const locale of localesWithFiles) {
		it(`${locale} does not promise an audit that does not exist`, () => {
			const text = JSON.stringify(TRANSLATIONS[locale]);
			expect(text).not.toMatch(/audit is (planned|coming)/i);
			expect(text).not.toMatch(/planned audit/i);
		});
	}
});

describe('every page namespace is either whole or absent', () => {
	for (const locale of localesWithFiles) {
		for (const ns of PAGE_NAMESPACES) {
			it(`${locale}.${ns} is not a fragment`, () => {
				const value = (TRANSLATIONS[locale] as Record<string, unknown>)[ns];
				// Absent is a legal state (R4) — the page falls back and says so.
				// Present-but-incomplete is the state that produces a half-translated
				// page, so it must not exist in a file at all. (vitest here runs with
				// requireAssertions, so the absent case asserts rather than returning.)
				const verdict = value === undefined || __internals.isComplete(en[ns], value);
				expect(verdict, `${locale}.${ns} is present but partial`).toBe(true);
			});
		}
	}
});

describe('the review record covers every locale (FR-028)', () => {
	it('names all fifteen, and only claims `reviewed` where a findings file exists', async () => {
		// `_comment` sits alongside the locale entries, so the cast goes through
		// `unknown` and the loop skips underscore keys.
		const review = (await import('./review.json')).default as unknown as Record<
			string,
			{ state: string; findings?: string }
		>;
		for (const locale of SUPPORTED_LOCALES) {
			expect(review[locale], `review.json is missing ${locale}`).toBeDefined();
		}
		expect(review[DEFAULT_LOCALE].state).toBe('source');
		for (const [locale, entry] of Object.entries(review)) {
			if (locale.startsWith('_')) continue;
			expect(['source', 'drafted', 'reviewed'], locale).toContain(entry.state);
			// "reviewed" is a claim about work a human did. It may only be made
			// alongside the findings file that proves it (FR-029, SC-011).
			if (entry.state === 'reviewed') {
				expect(entry.findings, `${locale} claims reviewed with no findings file`).toBeTruthy();
			}
		}
	});
});
