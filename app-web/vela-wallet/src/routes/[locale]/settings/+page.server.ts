/**
 * The settings route (spec 023) — the real screen, not a gallery state.
 *
 * Prerendered per locale like every other `[locale]` page: the body is the
 * fixture layer and its copy is static, so all of it resolves here at build
 * time. What is NOT resolved here is WHOSE settings these are — the account
 * block ships empty and the browser fills it from the session, for the same
 * reason the wallet route does (a prerendered fixture address would flash a
 * stranger's account for a frame after hydration).
 */
import { error } from '@sveltejs/kit';
import { resolveSettingsMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import { buildDesktopState, buildMobileState } from '$lib/settings/fixtures';
import { EMPTY_ACCOUNT } from '$lib/settings/identity';
import { webNavItems } from '$lib/wallet/destinations';
import { buildDesktopState as buildWalletDesktopState } from '$lib/wallet/fixtures';
import { identiconSvgFor } from '$lib/wallet/identicon.server';
import { EMPTY_HEADER } from '$lib/wallet/identity';
import type { EntryGenerator, PageServerLoad } from './$types';

export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);

	const messages = resolveSettingsMessages(locale);
	const walletMessages = resolveWalletMessages(locale);

	const home = buildMobileState('st1', messages, identiconSvgFor);
	const desktop = buildDesktopState('dst1', messages, identiconSvgFor);
	// The app sidebar is spec 015's, with 设置 selected instead of 钱包.
	const wallet = buildWalletDesktopState('d1', walletMessages, identiconSvgFor);
	const sidebar = {
		...wallet.sidebar,
		header: EMPTY_HEADER,
		// The network list is the wallet's filter (spec 028 Phase 9, RULING 2);
		// on this route there is nothing for it to filter.
		networks: [],
		// Three rows, not the board's four: the web has no 探索 (spec 022).
		nav: webNavItems(wallet.sidebar.nav).map((item) => ({
			...item,
			selected: item.id === 'settings'
		}))
	};

	return {
		// NOT `messages`: the layout already publishes the Welcome copy under
		// that name, and a page-level key of the same name would shadow it.
		settingsMessages: messages,
		home: { ...home, account: { ...home.account, ...EMPTY_ACCOUNT } },
		desktop: {
			...desktop,
			account: {
				...desktop.account,
				rows: desktop.account.rows.map((row) => (row.selected ? { ...row, ...EMPTY_ACCOUNT } : row))
			}
		},
		sidebar,
		/** 全部 in the sidebar's network list, which the live rows rebuild. */
		allNetworksLabel: walletMessages.networkFilter.allNetworks,
		/** The identicon viewer every artwork on this route opens. */
		identiconViewer: walletMessages.identiconViewer
	};
};
