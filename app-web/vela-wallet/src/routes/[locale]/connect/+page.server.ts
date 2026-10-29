import { error } from '@sveltejs/kit';
import { resolveMultisigMessages, resolveSigningMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * Vela's connect page: opened in the browser that holds an outside owner's
 * wallet (a link, or Vela's code scanned with a wallet app), it is the Wallet
 * side of a WalletPair session and forwards Vela's requests to that wallet
 * (`$lib/multisig/external`) — and, when its link names the transaction,
 * approves, executes or rejects it on its own (r4C-2, r4C-9). Prerendered per
 * locale like every route under `[locale]`; the signing sheet's words draw a
 * call's review as Vela's own sheet does (cs-12).
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	return {
		multisigMessages: resolveMultisigMessages(locale),
		signingMessages: resolveSigningMessages(locale)
	};
};
