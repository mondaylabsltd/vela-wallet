import { error } from '@sveltejs/kit';
import { DEFAULT_LOCALE, PREFIXED_LOCALES, toLocale } from '$lib/i18n/locales';
import type { EntryGenerator, PageLoad } from './$types';
import { getDoc, getDocSlugs } from '$lib/content/docs';

/**
 * The (locale × slug) product: ten slugs — `introduction` is served at `/docs`
 * by the sibling route — times fifteen locales. `{ slug }` with no `locale` key
 * is the English entry (research §2).
 */
export const entries: EntryGenerator = () =>
	getDocSlugs().flatMap((slug) => [
		{ slug },
		...PREFIXED_LOCALES.map((locale) => ({ locale, slug }))
	]);

export const load: PageLoad = ({ params }) => {
	const locale = toLocale(params.locale) ?? DEFAULT_LOCALE;
	const doc = getDoc(locale, params.slug);
	if (!doc) {
		error(404, 'Documentation page not found');
	}
	// `fallback` travels in the load data so the page can decide whether to show
	// the notice from the DOC's real state, not from the locale.
	return { slug: doc.slug, meta: doc.meta, fallback: doc.fallback };
};
