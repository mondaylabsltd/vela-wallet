import { PREFIXED_LOCALES } from '#lib/i18n/locales.ts';
import type { EntryGenerator } from './$types';

export const entries: EntryGenerator = () => [
	{},
	...PREFIXED_LOCALES.map((locale) => ({ locale }))
];
