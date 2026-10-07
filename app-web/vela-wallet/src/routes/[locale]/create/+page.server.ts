import { error } from '@sveltejs/kit';
import type { EntryGenerator, PageServerLoad } from './$types';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import { resolveAppPromptMessages } from '$lib/i18n/engine.server';

/**
 * The create flow's own route (spec 019). Prerendered per locale like every
 * other page: the shell and its copy are static, and the only dynamic thing —
 * the state machine — arrives in the browser.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

/** The done screen's one-time "Get Vela on your phone" suggestion. */
export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return { appPrompt: resolveAppPromptMessages(locale) };
};
