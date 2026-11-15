import type { EntryGenerator } from './$types';
import { SUPPORTED_LOCALES } from '#lib/i18n/locales.ts';

/** Prerender one Welcome page per supported locale; sub-pages export their own entries. */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));
