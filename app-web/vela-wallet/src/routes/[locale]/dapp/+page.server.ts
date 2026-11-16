import { error } from '@sveltejs/kit';
import {
	resolveConnectMessages,
	resolveRequestMessages,
	resolveSigningMessages,
	resolveWalletMessages
} from '#lib/i18n/engine.server.ts';
import { SUPPORTED_LOCALES, toLocale } from '#lib/i18n/locales.ts';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * Vela Connect's window (spec 109): the page a site opens to ask the web
 * wallet for an account, a signature or a transaction. Prerendered per
 * locale like every route under `[locale]`.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return {
		connectMessages: resolveConnectMessages(locale),
		// The consent card says what the extension's request window says.
		requestMessages: resolveRequestMessages(locale),
		// The sheet is 026's, and so are its words.
		signingMessages: resolveSigningMessages(locale),
		identiconViewer: resolveWalletMessages(locale).identiconViewer
	};
};
