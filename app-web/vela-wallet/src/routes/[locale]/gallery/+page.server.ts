/**
 * Gallery root: component boards + links to full screen states (spec 015
 * FR-004, extended by spec 018 with the contacts vocabulary). Prerendered for
 * the review locales; not linked from any user-facing page. All resolution
 * (strings, identicons) happens here at build time.
 */
import { error } from '@sveltejs/kit';
import {
	resolveContactsMessages,
	resolveIntroMessages,
	resolveSettingsMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { toLocale } from '$lib/i18n/locales';
import {
	buildDesktopState,
	buildMobileState,
	DESKTOP_STATES,
	IDENTICON_BOARD_SEEDS,
	MOBILE_STATES
} from '$lib/wallet/fixtures';
import {
	addMenu,
	buildDesktopState as buildContactsDesktopState,
	buildMobileState as buildContactsMobileState,
	CONTACT_BOARD_SEEDS,
	contactContextMenu,
	DESKTOP_STATES as CONTACTS_DESKTOP_STATES,
	groupContextMenu,
	groupMenuMobile,
	headerDropdown,
	MOBILE_STATES as CONTACTS_MOBILE_STATES
} from '$lib/contacts/fixtures';
import {
	buildMobileState as buildSettingsMobileState,
	DESKTOP_STATES as SETTINGS_DESKTOP_STATES,
	MOBILE_STATES as SETTINGS_MOBILE_STATES,
	unreachableBoards,
	webVenue
} from '$lib/settings/fixtures';
import { boardCheckTime } from '$lib/settings/board-check';
import { integrityLineModel } from '$lib/settings/venue';
import { fill } from '$lib/wallet/messages';
import { identiconSvgFor } from '$lib/wallet/identicon.server';
import type { WalletHomeModel } from '$lib/wallet/model';
import type { EntryGenerator, PageServerLoad } from './$types';

/** Review locales (spec US4: zh mocks + en). Other locales 404 here. */
export const entries: EntryGenerator = () => [{ locale: 'zh' }, { locale: 'en' }];

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	const messages = resolveWalletMessages(locale);
	const contactsMessages = resolveContactsMessages(locale);
	const settingsMessages = resolveSettingsMessages(locale);
	const identicon = identiconSvgFor;

	const models: Partial<Record<string, WalletHomeModel>> = {};
	for (const state of ['h1s', 'h2', 'h3', 'h4', 'h5', 'h6', 'h7'] as const) {
		models[state] = buildMobileState(state, messages, identicon);
	}

	const c1 = buildContactsMobileState('c1', contactsMessages, identicon);
	const c1s = buildContactsMobileState('c1s', contactsMessages, identicon);
	const c1f = buildContactsMobileState('c1f', contactsMessages, identicon);
	const c2 = buildContactsMobileState('c2', contactsMessages, identicon);
	const c2s = buildContactsMobileState('c2s', contactsMessages, identicon);
	const c3 = buildContactsMobileState('c3', contactsMessages, identicon);
	const c4 = buildContactsMobileState('c4', contactsMessages, identicon);
	const dc1 = buildContactsDesktopState('dc1', contactsMessages, identicon);
	const dc4 = buildContactsDesktopState('dc4', contactsMessages, identicon);

	// One settings state feeds most of the component board: every primitive the
	// forty mocks use appears somewhere in ST1b's model or in the sub-pages it
	// carries, so the board never has to invent data of its own.
	const st1b = buildSettingsMobileState('st1b', settingsMessages, identicon);
	const st9b = buildSettingsMobileState('st9b', settingsMessages, identicon);
	const st10b = buildSettingsMobileState('st10b', settingsMessages, identicon);
	const st10c = buildSettingsMobileState('st10c', settingsMessages, identicon);
	const sr1 = buildSettingsMobileState('sr1', settingsMessages, identicon);
	const sr2b = buildSettingsMobileState('sr2b', settingsMessages, identicon);
	// Spec 102: where you review and sign, and the pages this device trusts.
	const st17 = buildSettingsMobileState('st17', settingsMessages, identicon);
	const st17b = buildSettingsMobileState('st17b', settingsMessages, identicon);
	const st18 = buildSettingsMobileState('st18', settingsMessages, identicon);
	const st18b = buildSettingsMobileState('st18b', settingsMessages, identicon);
	/**
	 * The web's own reading of the same two accounts: it opens no page, so the
	 * page rows are disabled with their reason and nothing is offered (P2-09,
	 * D-16; P2-11).
	 */
	const web = (which: 'app' | 'own') => webVenue(settingsMessages, which);
	/** Every integrity state the core can name, each in words (spec 102 R6). */
	const integrity = (
		[
			['checking', ''],
			['matches', '0ba8ee8c'],
			['trusted', '3f9a1c22'],
			['unchecked', ''],
			['askTrust', '9be01d44'],
			['mismatch', '7d41e0b9'],
			['blocked', '7d41e0b9'],
			['couldNotCheck', ''],
			['noVersion', ''],
			['allBlocked', '']
		] as const
	).flatMap(([state, version]) => {
		const line = integrityLineModel(
			{ key: `componentsUi.signing.integrity.${state}`, version },
			boardCheckTime(locale),
			settingsMessages
		);
		return line === undefined ? [] : [{ state, line }];
	});

	return {
		messages,
		contactsMessages,
		/** Spec 020: the first-run intro's copy, for the slide board. */
		intro: resolveIntroMessages(locale),
		settingsMessages,
		settingsMobileStates: SETTINGS_MOBILE_STATES,
		settingsDesktopStates: SETTINGS_DESKTOP_STATES,
		settings: {
			sections: st1b.sections,
			account: st1b.account,
			appearance: st1b.appearance,
			networks: st1b.networks,
			networkDetail: st9b.networkDetail,
			storage: st1b.storage,
			about: st1b.about,
			erase: st1b.erase,
			languageSheet: st1b.languageSheet,
			currencySheet: st1b.currencySheet,
			numberSheet: st1b.numberSheet,
			signOutSheet: st1b.signOutSheet,
			clearCachesSheet: st1b.clearCachesSheet,
			checks: { compatible: st10b.addNetwork, incompatible: st10c.addNetwork },
			banner: sr1.rpcBanner,
			// Spec 092's list, and PR 3 note 4's token-list state beside it.
			unreachable: unreachableBoards(settingsMessages),
			rpcFixFailing: st1b.rpcFix,
			rpcFixRestored: sr2b.rpcFix,
			balanceDetail: st1b.balanceDetail,
			relayer: st1b.relayer,
			indexDown: st1b.indexDown,
			rpcProviders: st1b.rpcProviders,
			endpoints: st1b.endpoints,
			feedback: st1b.feedback,
			venue: {
				app: st17.venue,
				own: st17b.venue,
				webApp: web('app'),
				webOwn: web('own')
			},
			signingPages: st18.signingPages,
			signingPagesRefused: st18b.signingPages,
			integrity
		},
		models,
		mobileStates: MOBILE_STATES,
		desktopStates: DESKTOP_STATES,
		contactsMobileStates: CONTACTS_MOBILE_STATES,
		contactsDesktopStates: CONTACTS_DESKTOP_STATES,
		sidebar: buildDesktopState('d1', messages, identicon).sidebar,
		board: IDENTICON_BOARD_SEEDS.map((seed) => ({
			seed: seed === '' ? '(empty)' : seed,
			svg: identicon(seed)
		})),
		contacts: {
			list: c1.list,
			swipe: c1s.list,
			filtered: c1f.list,
			detail: c2.detail,
			confirm: c2s.confirm,
			empty: c3.empty,
			group: c4.group,
			rail: dc1.rail,
			railDrop: { ...dc4.rail, dropTarget: dc4.rail.groups[1]?.name },
			searchEmpty: {
				title: fill(contactsMessages.noResults, { query: 'zzz' }),
				caption: contactsMessages.emptyHint,
				primary: contactsMessages.addContact,
				secondary: contactsMessages.importFile
			},
			menus: {
				add: addMenu(contactsMessages),
				group: groupMenuMobile(contactsMessages),
				header: headerDropdown(contactsMessages),
				groupContext: groupContextMenu(contactsMessages),
				contactContext: contactContextMenu(contactsMessages)
			},
			board: CONTACT_BOARD_SEEDS.map((seed) => ({
				seed: seed === '' ? '(empty)' : seed,
				svg: identicon(seed)
			}))
		}
	};
};
