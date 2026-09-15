import { PREFIXED_LOCALES } from '$lib/i18n/locales';
import type { EntryGenerator } from './$types';

/** `/docs` (the introduction) in all fifteen locales. */
export const entries: EntryGenerator = () => [{}, ...PREFIXED_LOCALES.map((locale) => ({ locale }))];
