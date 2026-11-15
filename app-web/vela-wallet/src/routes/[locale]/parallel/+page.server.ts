import { error } from '@sveltejs/kit';
import { SUPPORTED_LOCALES, toLocale } from '#lib/i18n/locales.ts';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * The parallel space's own door (spec 026 US4). Prerendered per locale like
 * every route under `[locale]`, and it ships no copy at all: the page is a
 * developer switch, worded in English on purpose so it can never be mistaken
 * for product chrome, and its words never enter the corpus.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return {};
};
