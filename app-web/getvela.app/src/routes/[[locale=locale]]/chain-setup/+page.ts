import { PREFIXED_LOCALES } from '$lib/i18n/locales';
import type { EntryGenerator } from './$types';

/** Prerendered in every locale like the other pages; all the work happens in the browser. */
export const entries: EntryGenerator = () => [
	{},
	...PREFIXED_LOCALES.map((locale) => ({ locale }))
];
