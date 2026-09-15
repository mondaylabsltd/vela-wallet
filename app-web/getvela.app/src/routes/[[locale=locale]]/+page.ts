import { PREFIXED_LOCALES } from '$lib/i18n/locales';
import type { EntryGenerator } from './$types';

/**
 * Prerender entries for the landing page in all fifteen locales.
 *
 * `{}` — with no `locale` key — is English: `resolve_route` drops an optional
 * param that is absent, so the entry becomes `/` rather than `/undefined`
 * (research §2). This is also why the wildcard `prerender.entries: '*'` is not
 * enough on its own: it STRIPS optional params, so it would only ever emit the
 * English page.
 */
export const entries: EntryGenerator = () => [{}, ...PREFIXED_LOCALES.map((locale) => ({ locale }))];
