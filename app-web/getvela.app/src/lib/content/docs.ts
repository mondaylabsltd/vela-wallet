import type { Component } from 'svelte';
import type { DocMeta, MarkdownModule } from './types';
import { flatSidebar, DOCS_INDEX_SLUG, type SidebarItem } from './sidebar';
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';
import { seoConfig } from '$lib/seo';

/**
 * Docs content, keyed by (locale, slug) — spec 059, contracts/translation-store.md.
 *
 * English is the source and lives at `src/content/docs/<slug>.md`. A translation
 * lives at `src/content/docs/<tag>/<slug>.md`. No file means no translation,
 * which means FALLBACK: the English body renders at the localized URL under a
 * notice in the reader's own language (FR-017). There is no half-translated
 * state — a doc is a whole file, so it is either translated or it is not.
 */

/** Path (relative to the app root) where docs markdown lives. */
const DOCS_CONTENT_DIR = 'src/content/docs';

const modules = import.meta.glob<MarkdownModule<DocMeta>>('/src/content/docs/**/*.md', {
	eager: true
});

/** `/src/content/docs/zh/faq.md` → `{ locale: 'zh', slug: 'faq' }`. */
function parsePath(path: string): { locale: Locale; slug: string } | null {
	const rest = path.replace('/src/content/docs/', '').replace(/\.md$/, '');
	const parts = rest.split('/');

	if (parts.length === 1) return { locale: DEFAULT_LOCALE, slug: parts[0] };
	if (parts.length === 2) {
		const [tag, slug] = parts;
		// An unknown directory is not a locale — ignore it rather than inventing
		// a sixteenth language out of a typo.
		if (!(SUPPORTED_LOCALES as readonly string[]).includes(tag)) return null;
		if (tag === DEFAULT_LOCALE) return null; // English never lives in `en/`
		return { locale: tag as Locale, slug };
	}
	return null;
}

const byKey = new Map<string, MarkdownModule<DocMeta>>();
for (const [path, mod] of Object.entries(modules)) {
	const parsed = parsePath(path);
	if (parsed) byKey.set(`${parsed.locale}:${parsed.slug}`, mod);
}

export interface DocEntry {
	slug: string;
	/** The locale that was ASKED for. */
	locale: Locale;
	/** The locale actually rendered — `en` when the translation is missing. */
	renderedLocale: Locale;
	/** True when the reader asked for a language this doc does not have yet. */
	fallback: boolean;
	meta: DocMeta;
	component: Component;
}

/**
 * The doc to render for this reader. Falls back to English rather than 404ing:
 * a missing translation is a gap in our work, not a missing page (R4).
 */
export function getDoc(locale: Locale, slug: string): DocEntry | undefined {
	const translated = locale === DEFAULT_LOCALE ? undefined : byKey.get(`${locale}:${slug}`);
	const mod = translated ?? byKey.get(`${DEFAULT_LOCALE}:${slug}`);
	if (!mod) return undefined;

	return {
		slug,
		locale,
		renderedLocale: translated ? locale : DEFAULT_LOCALE,
		fallback: locale !== DEFAULT_LOCALE && !translated,
		meta: mod.metadata,
		component: mod.default
	};
}

/** Whether this doc genuinely exists in this locale. */
export function isDocTranslated(locale: Locale, slug: string): boolean {
	if (locale === DEFAULT_LOCALE) return byKey.has(`${DEFAULT_LOCALE}:${slug}`);
	return byKey.has(`${locale}:${slug}`);
}

/**
 * The locales an `hreflang` block may advertise for this doc (FR-018, FR-021).
 * A falling-back doc is an English page at a localized URL; listing it as a
 * translation is what earns a duplicate-content penalty.
 */
export function docLocales(slug: string): Locale[] {
	return SUPPORTED_LOCALES.filter((locale) => isDocTranslated(locale, slug));
}

/** Previous/next docs in sidebar order, for the footer pager. */
export function getAdjacentDocs(slug: string): { prev?: SidebarItem; next?: SidebarItem } {
	const index = flatSidebar.findIndex((item) => item.slug === slug);
	if (index === -1) return {};
	return { prev: flatSidebar[index - 1], next: flatSidebar[index + 1] };
}

/** Slugs handled by the `/docs/[...slug]` route (everything but the index). */
export function getDocSlugs(): string[] {
	return flatSidebar.map((item) => item.slug).filter((slug) => slug !== DOCS_INDEX_SLUG);
}

/**
 * GitHub "edit this file" URL for the markdown that ACTUALLY produced what the
 * reader is looking at (FR-019). Pointing a reader of a fallback page at a
 * translation file that does not exist — or a reader of the Japanese page at
 * the English source — is worse than having no link.
 */
export function getDocEditUrl(locale: Locale, slug: string): string {
	const { github, repoBranch, repoAppDir } = seoConfig;
	const rendered = isDocTranslated(locale, slug) ? locale : DEFAULT_LOCALE;
	const file =
		rendered === DEFAULT_LOCALE ? `${slug}.md` : `${rendered}/${slug}.md`;
	return `${github}/edit/${repoBranch}/${repoAppDir}/${DOCS_CONTENT_DIR}/${file}`;
}
