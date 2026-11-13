import { error } from '@sveltejs/kit';
import {
	resolveMultisigMessages,
	resolveRequestMessages,
	resolveSigningMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * The window a dApp's request is answered in (spec 027 D34).
 *
 * Prerendered per locale like every route under `[locale]`. It exists only
 * inside the packaged extension — the hosted site has no dApp channel and
 * nothing ever navigates here — but it is built for both, because the
 * extension packages the same pages the site serves and a second build would
 * be a second implementation.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return {
		requestMessages: resolveRequestMessages(locale),
		// The sheet is 026's, and so are its words.
		signingMessages: resolveSigningMessages(locale),
		// A request to a multisig opens its proposal here, and an outside
		// member's wallet may be asked from here.
		multisigMessages: resolveMultisigMessages(locale),
		// Every artwork drawn here (the signer line, the consent card, a
		// proposal's members) opens the identicon viewer.
		identiconViewer: resolveWalletMessages(locale).identiconViewer
	};
};
