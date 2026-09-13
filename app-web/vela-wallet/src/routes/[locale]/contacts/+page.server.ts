import { error } from '@sveltejs/kit';
import {
	resolveContactsMessages,
	resolveSettingsMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import { webNavItems } from '$lib/wallet/destinations';
import { buildDesktopState as buildWalletDesktopState } from '$lib/wallet/fixtures';
import { identiconSvgFor } from '$lib/wallet/identicon.server';
import { EMPTY_HEADER } from '$lib/wallet/identity';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * The address book's own route (spec 024) — the tab that used to swallow its
 * tap. Prerendered per locale like every route under `[locale]`; the page
 * ships only its copy, because the book itself lives in the browser and the
 * core has not ruled until it loads there. The guard lives in the page, as on
 * /wallet and /settings.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);

	// The wide layout's app sidebar is spec 015's, with 通讯录 selected — the
	// same one /settings carries. The header ships empty and the browser fills
	// it from the session, for the reason the wallet route gives.
	const walletMessages = resolveWalletMessages(locale);
	const wallet = buildWalletDesktopState('d1', walletMessages, identiconSvgFor);
	const sidebar = {
		...wallet.sidebar,
		header: EMPTY_HEADER,
		// The network list is the wallet's filter (spec 028 Phase 9, RULING 2);
		// on this route there is nothing for it to filter.
		networks: [],
		nav: webNavItems(wallet.sidebar.nav).map((item) => ({
			...item,
			selected: item.id === 'contacts'
		}))
	};

	return {
		// Page-unique key — `messages` would shadow the layout's Welcome copy.
		contactsMessages: resolveContactsMessages(locale),
		sidebar,
		/** 全部 in the sidebar's network list, which the live rows rebuild. */
		allNetworksLabel: walletMessages.networkFilter.allNetworks,
		/** The identicon viewer every artwork on this route opens. */
		identiconViewer: walletMessages.identiconViewer,
		/** The account switcher behind the sidebar header's name. */
		accountsMessages: resolveSettingsMessages(locale).accounts
	};
};
