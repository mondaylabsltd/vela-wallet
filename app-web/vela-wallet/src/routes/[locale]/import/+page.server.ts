import type { EntryGenerator } from './$types';
import { SUPPORTED_LOCALES } from '#lib/i18n/locales.ts';

/**
 * The Welcome buttons stopped navigating here (spec 014 US2 — they swap the
 * panel / open the sheet in place), so the crawler no longer discovers this
 * placeholder. Keep it prerendered for direct URLs (research D3).
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));
