import { DEFAULT_LOCALE, toLocale, type Locale } from '$lib/i18n/locales';
import type { LayoutLoad } from './$types';

/**
 * The localized subtree (spec 059, plan stage A).
 *
 * `prerender = true` covers every page below this file — landing, about,
 * roadmap and all eleven docs — in all fifteen locales. They are static pages;
 * making the Worker render them per request would cost money and latency for
 * output that never varies (research §9).
 *
 * An ABSENT `locale` param means English: `[[locale]]` is optional, and
 * `resolve_route` drops the segment when the param is missing, so this one
 * subtree serves `/` and `/ja` from the same files without English moving
 * (FR-009).
 */
export const prerender = true;

export const load: LayoutLoad = ({ params }) => {
	// The matcher has already rejected anything that is not one of the fourteen
	// prefixes, so this can only fail when the param is absent — i.e. English.
	const locale: Locale = toLocale(params.locale) ?? DEFAULT_LOCALE;
	return { locale };
};
