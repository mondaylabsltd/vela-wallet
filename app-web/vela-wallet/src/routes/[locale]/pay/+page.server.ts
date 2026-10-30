import { error } from '@sveltejs/kit';
import { resolvePayMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * A payment link's page (spec 039 Part B). Prerendered per locale like every
 * route under `[locale]`: its words resolve here, and the link itself — the
 * query — is read in the browser, where the core rules on it. Nothing here
 * may read `url.search`: a prerendered page has none.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return { pay: resolvePayMessages(locale) };
};
