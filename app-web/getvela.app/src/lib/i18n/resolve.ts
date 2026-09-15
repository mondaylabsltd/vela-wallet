import { DEFAULT_LOCALE, SUPPORTED_LOCALES, pathFor, type Locale } from './locales';
import { en, type Messages, type PartialMessages } from './messages/en';

/**
 * Catalog resolution and the translation record (spec 059,
 * contracts/translation-store.md, data-model.md §TranslationRecord).
 *
 * The one idea worth holding on to: **fallback is per page, never per key.**
 * Merging a translation key-by-key over English is the obvious implementation
 * and it is wrong — it produces a page whose heading is Japanese and whose
 * third paragraph is English, which is worse than either language alone
 * (SC-006). So a page namespace is used only when it is COMPLETE; otherwise the
 * whole namespace falls back and the reader is told, in their own language,
 * that what follows is English.
 *
 * `chrome` and `notice` are the exception, and have to be: they are the frame
 * the fallback notice is drawn in. They merge per key, and a test requires them
 * to be complete in every locale so the merge never actually has work to do.
 */

/** Every `<tag>.json` beside `messages/en.ts`, loaded at build time. */
const files = import.meta.glob<PartialMessages>('./messages/*.json', {
	eager: true,
	import: 'default'
});

const TRANSLATIONS: Partial<Record<Locale, PartialMessages>> = {};
for (const [path, value] of Object.entries(files)) {
	const tag = path.replace('./messages/', '').replace('.json', '');
	if ((SUPPORTED_LOCALES as readonly string[]).includes(tag)) {
		TRANSLATIONS[tag as Locale] = value;
	}
}

/** The page namespaces — the localizable pages that are not docs. */
export const PAGE_NAMESPACES = ['home', 'about', 'roadmap'] as const;
export type PageNamespace = (typeof PAGE_NAMESPACES)[number];

/** Namespaces that must be complete in every locale (data-model §3). */
const REQUIRED_NAMESPACES = ['chrome', 'notice'] as const;

export type TranslationState = 'source' | 'translated' | 'fallback';

/**
 * Is every leaf of `source` present and non-empty in `value`?
 * Arrays must match in length — a five-item FAQ translated to four is not a
 * translated FAQ, it is a shorter page.
 */
function isComplete(source: unknown, value: unknown): boolean {
	if (Array.isArray(source)) {
		if (!Array.isArray(value) || value.length !== source.length) return false;
		return source.every((item, i) => isComplete(item, value[i]));
	}
	if (typeof source === 'object' && source !== null) {
		if (typeof value !== 'object' || value === null) return false;
		return Object.entries(source).every(([k, v]) =>
			isComplete(v, (value as Record<string, unknown>)[k])
		);
	}
	return typeof value === 'string' && value.trim().length > 0;
}

/** Per-key merge, used only for `chrome` and `notice`. */
function mergeRequired(source: unknown, value: unknown): unknown {
	if (Array.isArray(source)) {
		return Array.isArray(value) && value.length === source.length ? value : source;
	}
	if (typeof source === 'object' && source !== null) {
		const out: Record<string, unknown> = {};
		for (const [k, v] of Object.entries(source)) {
			out[k] = mergeRequired(v, (value as Record<string, unknown> | undefined)?.[k]);
		}
		return out;
	}
	return typeof value === 'string' && value.trim().length > 0 ? value : source;
}

/**
 * Internal links inside message HTML are written English-relative
 * (`href="/docs/security-audits"`), because a translator should never have to
 * think about URL prefixes — and if they did, half of them would get it wrong
 * in a way nothing would catch until a reader in Japanese landed on an English
 * page. The prefix is applied here instead, once, when the catalog is built.
 *
 * Only root-relative hrefs are touched. `https://…`, `#anchor` and `mailto:`
 * are left exactly as written.
 */
function localizeLinks(html: string, locale: Locale): string {
	return html.replace(/href="(\/[^"#]*)"/g, (_, path: string) => `href="${pathFor(locale, path)}"`);
}

/** Apply `localizeLinks` to every string in a tree, preserving its shape. */
function localizeTree(value: unknown, locale: Locale): unknown {
	if (typeof value === 'string') return localizeLinks(value, locale);
	if (Array.isArray(value)) return value.map((v) => localizeTree(v, locale));
	if (typeof value === 'object' && value !== null) {
		return Object.fromEntries(
			Object.entries(value).map(([k, v]) => [k, localizeTree(v, locale)])
		);
	}
	return value;
}

const cache = new Map<Locale, Messages>();

/**
 * The catalog a page renders from. Always complete — every key resolves to
 * something readable — but a page namespace is either wholly translated or
 * wholly English.
 */
export function catalog(locale: Locale): Messages {
	if (locale === DEFAULT_LOCALE) return en;

	const cached = cache.get(locale);
	if (cached) return cached;

	const translation = (TRANSLATIONS[locale] ?? {}) as Record<string, unknown>;
	const out = { ...en } as Record<string, unknown>;

	for (const ns of REQUIRED_NAMESPACES) {
		out[ns] = mergeRequired(en[ns], translation[ns]);
	}
	for (const ns of PAGE_NAMESPACES) {
		out[ns] = isComplete(en[ns], translation[ns]) ? translation[ns] : en[ns];
	}

	const result = localizeTree(out, locale) as Messages;
	cache.set(locale, result);
	return result;
}

/** Whether a non-docs page is genuinely translated in this locale. */
export function namespaceState(ns: PageNamespace, locale: Locale): TranslationState {
	if (locale === DEFAULT_LOCALE) return 'source';
	const candidate = (TRANSLATIONS[locale] as Record<string, unknown> | undefined)?.[ns];
	return isComplete(en[ns], candidate) ? 'translated' : 'fallback';
}

/**
 * The locales that genuinely have this page — the set an `hreflang` block may
 * advertise (FR-018, FR-021). A falling-back page is an English page at a
 * localized URL; saying otherwise is what earns a duplicate-content penalty.
 */
export function translatedLocales(ns: PageNamespace): Locale[] {
	return SUPPORTED_LOCALES.filter((l) => namespaceState(ns, l) !== 'fallback');
}

/** Exposed for the completeness tests and the status report. */
export const __internals = { isComplete, localizeLinks, TRANSLATIONS };
