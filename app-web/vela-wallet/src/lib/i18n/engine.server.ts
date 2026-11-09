/**
 * vela-core i18n engine — BUILD-TIME ONLY seam (spec 006, contracts/i18n-ssr.md).
 *
 * Runs the real Rust resolver (the artefact spec 005 proved against i18next
 * with zero divergences) over the real generated catalogs (`assets/i18n/`).
 * Every `[locale]` page is prerendered, so this module executes in Node during
 * `vite build` (and in the dev server / vitest) — never on the deployed
 * Cloudflare Worker, which cannot compile wasm from bytes. The e2e suite
 * asserts the built `_worker.js` contains no `WASM_BASE64`.
 *
 * Loading mirrors `src/i18n/index.web.ts` (RN web): the wasm is read from the
 * committed `public/` asset and `initSync`'d at import (`wasm-init.server.ts`);
 * catalogs are statically imported raw, so resolution is synchronous.
 */
import {
	fiatSymbol,
	I18n as WasmI18n,
	i18nPluralSuffixes
} from '../../../../../rust/pkg-web/vela_core.js';
import './wasm-init.server';
import { FALLBACK_LOCALE, type Locale } from './locales';
import { FLOW_KEYS, FLOW_PLURAL_KEYS, type FlowMessages, type WelcomeMessages } from './messages';
import {
	BALANCE_INTERNAL_KEYS,
	BALANCE_SAID_KEYS,
	type WalletMessages
} from '$lib/wallet/messages';
import type { ContactsMessages } from '$lib/contacts/messages';
import { INTRO_KEYS } from '$lib/intro/slides';
import { FEE_REASON_KEYS, WALLET_FLOW_KEYS, type WalletFlowMessages } from '$lib/flows/messages';
import type { ExploreMessages } from '$lib/explore/messages';
import type { RequestMessages } from '$lib/dapp/messages';
import type { AgentMessages } from '$lib/agent/messages';
import type { NetworkSpaceMessages } from '$lib/network-space/messages';
import type { ExtensionMessages } from '$lib/extension/messages';
import { MULTISIG_SHAPE, type MultisigMessages } from '$lib/multisig/messages';
import {
	CERTAIN_KEYS,
	SIM_SAID_KEYS,
	SIM_COULD_NOT_CHECK_KEY,
	SIM_WILL_FAIL_KEY,
	SIM_WILL_FAIL_REASON_KEY,
	type HandoffMessages,
	type SigningMessages
} from '$lib/signing/messages';
import { CLEAR_TERMS, INTENT_TERMS } from '$lib/signing/terms';
import {
	BACKUP_EXPLAIN_KEYS,
	BACKUP_ROW_KEYS,
	BALANCE_STATUS_KEYS,
	DRAWN_CURRENCY_CODES,
	NET_HINT_KEYS,
	NET_RPC_FIELD_KEYS,
	NET_STOP_KEYS,
	UNREACHABLE_LINE_KEYS,
	VENUE_BLOCK_KEYS,
	type BackupExplainKey,
	type BackupRowKey,
	type NetHintKey,
	type NetRpcFieldKey,
	type NetStopKey,
	type SettingsMessages,
	type VenueBlockKey
} from '$lib/settings/messages';
import type { AppPromptMessages } from '$lib/app-prompt/messages';
import { PAY_MESSAGE_KEYS, type PayMessages } from '$lib/pay/link';
import type { PluralCopy } from './plural';

/** Generated runtime catalogs (gen-i18n.mjs stage 4), one per locale. */
const CATALOGS = import.meta.glob('../../../../../assets/i18n/*.json', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

/**
 * The on-demand namespaces (gen-i18n's `ON_DEMAND`, round 5): `home.multisig`
 * is left out of the main catalog every native shell holds — none draws it —
 * and written as `assets/i18n/multisig/<lng>.json`. This app draws it (the
 * multisig screens, the connect page, the send flow's token note), and
 * resolves at build time, so it merges each locale's back in here.
 */
const ON_DEMAND = import.meta.glob('../../../../../assets/i18n/*/*.json', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

function deepMerge(
	a: Record<string, unknown>,
	b: Record<string, unknown>
): Record<string, unknown> {
	const out: Record<string, unknown> = { ...a };
	for (const [key, value] of Object.entries(b)) {
		const here = out[key];
		out[key] =
			value && typeof value === 'object' && here && typeof here === 'object'
				? deepMerge(here as Record<string, unknown>, value as Record<string, unknown>)
				: value;
	}
	return out;
}

function catalogBytes(locale: string): Uint8Array {
	const entry = Object.entries(CATALOGS).find(([path]) => path.endsWith(`/${locale}.json`));
	if (!entry) throw new Error(`no generated catalog for locale "${locale}" in assets/i18n/`);
	const parts = Object.entries(ON_DEMAND).filter(([path]) => path.endsWith(`/${locale}.json`));
	if (parts.length === 0) return new TextEncoder().encode(entry[1]);
	let doc = JSON.parse(entry[1]) as Record<string, unknown>;
	for (const [, raw] of parts) doc = deepMerge(doc, JSON.parse(raw) as Record<string, unknown>);
	return new TextEncoder().encode(JSON.stringify(doc));
}

const engine = new WasmI18n(catalogBytes(FALLBACK_LOCALE));

/**
 * Resolve `key` in `locale`. The engine echoes the key when nothing matches —
 * surface that as an error: a Welcome page must never ship raw keys.
 */
function t(locale: Locale, key: string): string {
	const value = engine.t(key);
	if (value === key) throw new Error(`i18n key "${key}" did not resolve for locale "${locale}"`);
	return value;
}

/** Make `locale` active, loading its catalog on first use. `en` ships in the constructor. */
function activate(locale: Locale): void {
	if (locale !== FALLBACK_LOCALE && !engine.residentLocales().includes(locale)) {
		engine.loadCatalog(locale, catalogBytes(locale));
	}
	engine.changeLanguage(locale);
}

export function textDirection(locale: Locale): string {
	activate(locale);
	return engine.dir();
}

/** The serializable strings the Welcome page renders (data-model.md PageMessages). */
export function resolveWelcomeMessages(locale: Locale): WelcomeMessages {
	activate(locale);
	return {
		metaTitle: t(locale, 'onboarding.welcomeWeb.meta.title'),
		metaDescription: t(locale, 'onboarding.welcomeWeb.meta.description'),
		tagline: t(locale, 'onboarding.welcomeWeb.tagline'),
		heroTitle: t(locale, 'onboarding.welcome.heroTitle'),
		heroTitleFit: t(locale, 'onboarding.welcome.heroTitleFit'),
		heroSubtitle: t(locale, 'onboarding.welcome.heroSubtitle'),
		createWallet: t(locale, 'onboarding.welcome.createWallet'),
		alreadyHaveWallet: t(locale, 'onboarding.welcome.alreadyHaveWallet'),
		linkWaiting: t(locale, 'onboarding.welcomeWeb.linkWaiting'),
		linkBroken: t(locale, 'onboarding.welcomeWeb.linkBroken')
	};
}

/**
 * The serialized onboarding-flow copy (spec 014, T025): every key the
 * create/login panels can resolve, as raw templates — `{{var}}` fills happen
 * client-side from frozen presentation state. `t` throws on key echo, so a
 * missing corpus key fails the prerender instead of shipping a dotted key.
 */
export function resolveFlowMessages(locale: Locale): FlowMessages {
	activate(locale);
	// A plural key has no bare value, so it ships as the forms THIS locale has
	// — the core's categories for it, e.g. `_one`/`_other` in en, `_other`
	// alone in zh, four in ru — and the screen picks one by count.
	const plural = FLOW_PLURAL_KEYS.flatMap((key) =>
		i18nPluralSuffixes(locale).map((suffix) => `${key}${suffix}`)
	);
	return Object.fromEntries(
		[...FLOW_KEYS, ...plural].map((key) => [key, t(locale, key)])
	) as FlowMessages;
}

/**
 * The first-run intro's copy (spec 020). Same shape as the flow's: dotted key →
 * resolved template, filled client-side, so the carousel can be handed one map
 * instead of a field per string.
 */
export function resolveIntroMessages(locale: Locale): Readonly<Record<string, string>> {
	activate(locale);
	return Object.fromEntries(INTRO_KEYS.map((key) => [key, t(locale, key)]));
}

/**
 * The "Get Vela on your phone" prompt (the wallet home and the creation
 * screen). The two store names are product names, not prose: the corpus has
 * no key for them and should not.
 */
export function resolveAppPromptMessages(locale: Locale): AppPromptMessages {
	activate(locale);
	return {
		title: t(locale, 'home.getApp.title'),
		body: t(locale, 'home.getApp.body'),
		scanHint: t(locale, 'home.getApp.scanHint'),
		close: t(locale, 'onboarding.common.close'),
		appStore: 'App Store',
		googlePlay: 'Google Play'
	};
}

/** The `/pay` page's words (spec 039 Part B): a refused link, and no wallet yet. */
export function resolvePayMessages(locale: Locale): PayMessages {
	activate(locale);
	const out = {} as PayMessages;
	for (const name of Object.keys(PAY_MESSAGE_KEYS) as (keyof PayMessages)[]) {
		out[name] = t(locale, PAY_MESSAGE_KEYS[name]);
	}
	return out;
}

/** The serializable strings the wallet screens render (spec 015, research.md D3). */
export function resolveWalletMessages(locale: Locale): WalletMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		nav: {
			wallet: k('componentsUi.mainNav.wallet'),
			contacts: k('componentsUi.mainNav.contacts'),
			explore: k('componentsUi.mainNav.explore'),
			settings: k('componentsUi.mainNav.settings')
		},
		balance: {
			totalLabel: k('home.totalBalance'),
			liveIndicator: k('home.liveIndicator'),
			said: Object.fromEntries(BALANCE_SAID_KEYS.map((key) => [key, k(key)])),
			stale: k('home.balanceStale'),
			unpriced: k('home.balanceUnpriced'),
			unpricedTotal: k('home.totalUnpriced'),
			allUnpriced: k('home.allUnpriced'),
			currencyUnavailable: k('home.currencyUnavailable'),
			nodeFallback: k('home.nodeFallbackNotice'),
			nodeFallbackOpen: k('home.nodeFallbackOpen'),
			syncing: k('home.syncingLine'),
			syncStalled: k('home.syncStalledLine'),
			syncRetry: k('home.balanceDetailRetry'),
			unreachable: k('onboarding.common.networkBody'),
			noPrice: k('home.balanceDetailNoPrice'),
			a11yHide: k('home.a11yHideBalance'),
			a11yShow: k('home.a11yShowBalance'),
			lastUpdated: k('home.lastUpdated'),
			updating: k('home.updating'),
			refreshBalance: k('home.refreshBalance'),
			ago: {
				now: k('time.now'),
				minutes: k('time.minutesShort'),
				hours: k('time.hoursShort')
			}
		},
		actions: {
			receive: k('componentsUi.dock.receive'),
			send: k('componentsUi.dock.send'),
			scan: k('componentsUi.dock.scan')
		},
		sections: {
			activity: k('home.tabActivity'),
			assets: k('assets.sectionTitle'),
			all: k('history.filterAll'),
			add: k('assets.addToken')
		},
		activity: {
			sent: k('history.labelSent'),
			received: k('history.labelReceived'),
			dapp: k('history.txLabelDappTx'),
			contractCall: k('componentsUi.signing.intentContractCall'),
			intents: Object.fromEntries(
				INTENT_TERMS.map((term) => [term, k(`componentsUi.signing.${term}`)])
			),
			dappRowTitle: k('history.dappRowTitle'),
			unlimited: k('componentsUi.signingApprove.unlimitedValue'),
			today: k('componentsUi.dayGroup.today'),
			yesterday: k('componentsUi.dayGroup.yesterday'),
			toName: k('history.toName'),
			fromName: k('history.fromName'),
			emptyTitle: k('home.emptyNoActivity'),
			emptyCaption: k('home.emptySubtitle'),
			pending: k('componentsTx.detail.statusPending'),
			failed: k('componentsTx.detail.statusFailed'),
			notSent: k('componentsTx.receipt.statusNotSent'),
			assetsCount: k('componentsTx.receipt.assetsCount'),
			// 087 F04: the corpus's bare "Unknown", for a record nothing settles.
			unknown: k('componentsUi.signing.intentUnknown'),
			emptyTitleNetwork: k('home.emptyNoActivityNetwork'),
			unreadOne: k('home.activityUnreadOne'),
			unreadMany: k('home.activityUnreadMany'),
			tokenUnverified: k('home.multisig.token.unverified'),
			tokenLookalike: k('home.multisig.token.lookalike')
		},
		assets: {
			emptyTitle: k('assets.emptyTitle'),
			emptyCaption: k('assets.emptySubtext'),
			unreachableOne: k('assets.unreachableOne'),
			unreachableMany: k('assets.unreachableMany'),
			tokenListUnreachable: k('assets.tokenListUnreachable'),
			ownNodeUnreachable: k('assets.ownNodeUnreachable'),
			unreachableBody: k('assets.unreachableBody'),
			lastSeen: k('assets.lastSeen'),
			internal: Object.fromEntries(BALANCE_INTERNAL_KEYS.map((key) => [key, k(key)]))
		},
		networkFilter: {
			pillAll: k('componentsUi.networkFilter.pillAll'),
			sheetTitle: k('componentsUi.networkFilter.selectChain'),
			allNetworks: k('componentsUi.networkFilter.allNetworks'),
			only: k('home.networkFilterOnly'),
			showEvery: k('componentsUi.networkFilter.showEvery')
		},
		sidebar: {
			networks: k('settingsModals.network.modalTitle')
		},
		receive: {
			title: k('receive.title'),
			addressLabel: k('receive.addressLabel'),
			copyAddress: k('componentsUi.identiconViewer.copyAddress'),
			qrCaption: k('componentsUi.qrPlaceholder.caption'),
			warningTitle: k('receive.warningTitle'),
			warningReminder: k('receive.warningReminder'),
			networksLine: k('receive.networksLine'),
			networkDetail: k('receive.networkDetail')
		},
		assetDetail: {
			send: k('tokenDetail.send'),
			receive: k('tokenDetail.receive'),
			labelName: k('tokenDetail.labelName'),
			labelPrice: k('tokenDetail.labelPrice'),
			priceValue: k('tokenDetail.priceValue'),
			labelContract: k('tokenDetail.labelContract'),
			labelDecimals: k('tokenDetail.labelDecimals'),
			labelTransactions: k('tokenDetail.labelTransactions'),
			viewOnExplorer: k('tokenDetail.viewOnExplorer'),
			nativeToken: k('addToken.labelNativeToken')
		},
		identiconViewer: {
			title: k('componentsUi.identiconViewer.title'),
			caption: k('componentsUi.identiconViewer.caption'),
			copyAddress: k('componentsUi.identiconViewer.copyAddress'),
			copied: k('componentsUi.identiconViewer.copied'),
			close: k('componentsUi.identiconViewer.close'),
			a11yOpen: k('componentsUi.identiconViewer.a11yOpen')
		},
		signOut: {
			title: k('settings.signOut.title'),
			descMany: k('settings.signOut.descMany'),
			keeps: k('settings.signOut.keeps'),
			warning: k('settings.signOut.warning'),
			button: k('settings.signOut.button'),
			anyway: k('settings.signOut.anyway'),
			cancel: k('settings.signOut.cancel')
		},
		close: k('componentsUi.identiconViewer.close')
	};
}

/** The serializable strings the contacts screens render (spec 018, D3). */
export function resolveContactsMessages(locale: Locale): ContactsMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		title: k('contacts.title'),
		searchPlaceholder: k('contacts.searchPlaceholder'),
		sectionGroups: k('contacts.sectionGroups'),
		sectionContacts: k('contacts.sectionContacts'),
		manage: k('contacts.manage'),
		allContacts: k('contacts.allContacts'),
		countPeople: k('contacts.countPeople'),
		groupMembers: k('contacts.groupMembers'),
		membersCount: k('contacts.membersCount'),
		groupNew: k('contacts.groupNew'),
		groupEdit: k('contacts.groupEdit'),
		groupRename: k('contacts.groupRename'),
		groupDelete: k('contacts.groupDelete'),
		moveGroup: k('contacts.moveGroup'),
		addMember: k('contacts.addMember'),
		addContact: k('contacts.addContact'),
		addTitle: k('contacts.addTitle'),
		saveToContacts: k('contacts.saveToContacts'),
		editTitle: k('contacts.editTitle'),
		sectionFavorites: k('contacts.sectionFavorites'),
		favoriteAdd: k('contacts.favoriteAdd'),
		favoriteRemove: k('contacts.favoriteRemove'),
		noteLabel: k('contacts.noteLabel'),
		notePlaceholder: k('contacts.notePlaceholder'),
		nameLabel: k('contacts.nameLabel'),
		namePlaceholder: k('contacts.namePlaceholder'),
		addressPlaceholder: k('contacts.addressPlaceholder'),
		save: k('contacts.save'),
		invalidAddress: k('contacts.invalidAddress'),
		invalidChecksum: k('contacts.invalidChecksum'),
		groupNameLabel: k('contacts.groupNameLabel'),
		groupNamePlaceholder: k('contacts.groupNamePlaceholder'),
		edit: k('contacts.edit'),
		empty: k('contacts.empty'),
		emptyHint: k('contacts.emptyHint'),
		noResults: k('contacts.noResults'),
		batchSend: k('contacts.batchSend'),
		batchSendHint: k('contacts.batchSendHint'),
		batchSendHintTitled: k('contacts.batchSendHintTitled'),
		batchSendNeedsMembers: k('contacts.batchSendNeedsMembers'),
		importFile: k('contacts.importFile'),
		importAll: k('contacts.importAll'),
		importGroup: k('contacts.importGroup'),
		exportTitle: k('contacts.exportTitle'),
		exportAll: k('contacts.exportAll'),
		exportGroup: k('contacts.exportGroup'),
		recentActivity: k('contacts.recentActivity'),
		viewAllActivity: k('contacts.viewAllActivity'),
		addressLabel: k('contacts.addressLabel'),
		copyAddress: k('componentsUi.identiconViewer.copyAddress'),
		send: k('componentsUi.dock.send'),
		deleteContact: k('contacts.deleteContact'),
		delete: k('contacts.delete'),
		deleteTitle: k('contacts.deleteTitle'),
		deleteBody: k('contacts.deleteBody'),
		cancel: k('contacts.cancel'),
		importDoneTitle: k('contacts.importDoneTitle'),
		importDoneBody: k('contacts.importDoneBody'),
		importDoneInvalid: k('contacts.importDoneInvalid'),
		importFailTitle: k('contacts.importFailTitle'),
		importFailBody: k('contacts.importFailBody'),
		importFailEncoding: k('contacts.importFailEncoding'),
		importFailEmpty: k('contacts.importFailEmpty'),
		exportBody: k('contacts.exportBody'),
		groupDeleteBody: k('contacts.groupDeleteBody'),
		groupMembersLabel: k('contacts.groupMembersLabel'),
		groupNoContacts: k('contacts.groupNoContacts'),
		copied: k('componentsUi.identiconViewer.copied'),
		noActivity: k('history.emptyTitle'),
		done: k('common.done'),
		duplicateAddress: k('contacts.duplicateAddress'),
		updateExisting: k('contacts.updateExisting'),
		groupPickEmpty: k('contacts.groupPickEmpty'),
		saveFailed: k('contacts.saveFailed'),
		back: k('onboarding.common.back'),
		memberCount: pluralCopy(locale, 'contacts.memberCount'),
		batchSendHintCount: pluralCopy(locale, 'contacts.batchSendHintCount'),
		batchSendHintTitledCount: pluralCopy(locale, 'contacts.batchSendHintTitledCount'),
		activity: {
			sent: k('history.labelSent'),
			received: k('history.labelReceived'),
			today: k('componentsUi.dayGroup.today'),
			yesterday: k('componentsUi.dayGroup.yesterday'),
			all: k('history.filterAll')
		},
		shell: {
			navWallet: k('componentsUi.mainNav.wallet'),
			navContacts: k('componentsUi.mainNav.contacts'),
			navExplore: k('componentsUi.mainNav.explore'),
			navSettings: k('componentsUi.mainNav.settings'),
			networksTitle: k('settingsModals.network.modalTitle'),
			allNetworks: k('componentsUi.networkFilter.allNetworks'),
			close: k('componentsUi.identiconViewer.close')
		}
	};
}

/**
 * Spec 102: a venue refusal's lines, by the corpus key the core names
 * (`VenueBlock::key()`) — Settings and both sheets carry the same three.
 */
function venueBlockWords(k: (key: string) => string): Record<VenueBlockKey, string> {
	return Object.fromEntries(VENUE_BLOCK_KEYS.map((key) => [key, k(key)])) as Record<
		VenueBlockKey,
		string
	>;
}

/**
 * Spec 102 (D4): the hand-off card's words — every signing sheet's (the page,
 * everywhere: 2026-10-30), the multisig's, and the gallery's boards.
 */
export function resolveHandoffMessages(locale: Locale): HandoffMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	const table = (keys: string[]) => Object.fromEntries(keys.map((key) => [key, k(key)]));
	return {
		title: k('componentsUi.signing.handoffTitle'),
		// A key ceremony's own titles (`Ceremony::title_key`, spec 102 R8).
		ceremonies: table([
			'componentsUi.signing.ceremonyCreate',
			'componentsUi.signing.ceremonySignIn',
			'componentsUi.signing.ceremonyConfirm'
		]),
		// The labels a `KeyLabel.label_key` can name (the core's
		// `CONFIRM_WITH_LABEL` / `NEW_KEY_ON_LABEL`).
		keyLabels: table([
			'componentsUi.signing.confirmWithLabel',
			'componentsUi.signing.newKeyOnLabel'
		]),
		open: k('componentsUi.signing.openSigner'),
		waiting: k('componentsUi.signing.trustedSignerWaiting'),
		waitingHint: k('componentsUi.signing.trustedSignerWaitingHint'),
		reopen: k('componentsUi.signing.trustedSignerReopen'),
		official: k('settings.signing.pageOfficial'),
		integrity: resolveSettingsMessages(locale).integrity,
		places: table([
			'onboarding.create.methodPlatformTitle',
			'onboarding.create.methodHybridTitle',
			'onboarding.create.methodSecurityKeyTitle'
		]),
		unanswered: k('componentsUi.signing.trustedSignerUnanswered'),
		mismatch: k('componentsUi.signing.trustedSignerMismatch'),
		refused: k('componentsUi.signing.trustedSignerRefused')
	};
}

/** The serializable strings the settings screens render (spec 023). */
export function resolveSettingsMessages(locale: Locale): SettingsMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		title: k('settings.title'),
		nav: {
			wallet: k('componentsUi.mainNav.wallet'),
			contacts: k('componentsUi.mainNav.contacts'),
			explore: k('componentsUi.mainNav.explore'),
			settings: k('componentsUi.mainNav.settings')
		},
		sections: {
			account: k('settings.sections.account'),
			appearance: k('settings.sections.appearance'),
			localization: k('settings.sections.localization'),
			advanced: k('settings.sections.advanced'),
			community: k('settings.sections.community'),
			privacy: k('settings.sections.privacy'),
			security: k('settings.sections.security')
		},
		account: {
			switch: k('settings.account.switch'),
			contactsSubtitle: k('settings.account.contactsSubtitle'),
			openInSafe: k('settings.account.openInSafe')
		},
		contacts: k('componentsUi.mainNav.contacts'),
		feedback: { title: k('settings.feedback.title'), subtitle: k('settings.feedback.subtitle') },
		analytics: {
			title: k('settings.analytics.title'),
			subtitle: k('settings.analytics.subtitle')
		},
		refuseBlind: {
			title: k('settings.refuseBlind.title'),
			subtitle: k('settings.refuseBlind.subtitle')
		},
		sites: {
			title: k('settings.sites.title'),
			subtitle: k('settings.sites.subtitle'),
			listLabel: k('settings.sites.listLabel'),
			empty: k('settings.sites.empty')
		},
		agent: {
			title: k('settings.agent.title'),
			subtitle: k('settings.agent.subtitle'),
			description: k('settings.agent.description'),
			enable: k('settings.agent.enable'),
			bridge: k('settings.agent.bridge'),
			bridgeHint: k('settings.agent.bridgeHint'),
			bridgeInvalid: k('settings.agent.bridgeInvalid'),
			token: k('settings.agent.token'),
			tokenHint: k('settings.agent.tokenHint'),
			endpoint: k('settings.agent.endpoint'),
			endpointHint: k('settings.agent.endpointHint'),
			statusOff: k('settings.agent.statusOff'),
			statusConnecting: k('settings.agent.statusConnecting'),
			statusOnline: k('settings.agent.statusOnline'),
			statusOffline: k('settings.agent.statusOffline'),
			statusRefused: k('settings.agent.statusRefused'),
			statusElsewhere: k('settings.agent.statusElsewhere'),
			useHere: k('settings.agent.useHere'),
			lastAction: k('settings.agent.lastAction'),
			lastDone: k('settings.agent.lastDone'),
			lastRefused: k('settings.agent.lastRefused'),
			lastWaiting: k('settings.agent.lastWaiting'),
			connect: k('settings.agent.connect'),
			connectHint: k('settings.agent.connectHint'),
			connecting: k('settings.agent.connecting'),
			connectedAs: k('settings.agent.connectedAs'),
			connectAgent: k('settings.agent.connectAgent'),
			connectAgentHint: k('settings.agent.connectAgentHint'),
			reconnect: k('settings.agent.reconnect'),
			disconnect: k('settings.agent.disconnect'),
			ownBridge: k('settings.agent.ownBridge'),
			linkRefused: k('settings.agent.linkRefused'),
			linkUnavailable: k('settings.agent.linkUnavailable'),
			linkExpired: k('settings.agent.linkExpired'),
			linkNotOurs: k('settings.agent.linkNotOurs'),
			linkNetwork: k('settings.agent.linkNetwork'),
			linkFailed: k('settings.agent.linkFailed'),
			linkReturn: k('settings.agent.linkReturn'),
			agentAddress: k('settings.agent.agentAddress'),
			agentAddressHint: k('settings.agent.agentAddressHint'),
			show: k('settings.agent.show'),
			hide: k('settings.agent.hide'),
			copy: k('settings.agent.copy'),
			copied: k('settings.agent.copied'),
			share: k('settings.agent.share'),
			clientId: k('settings.agent.clientId'),
			wallets: k('settings.agent.wallets'),
			walletsHint: k('settings.agent.walletsHint'),
			walletsNone: k('settings.agent.walletsNone')
		},
		appearance: {
			themeTitle: k('settings.appearance.themeTitle'),
			themeLight: k('settings.appearance.themeLight'),
			themeDark: k('settings.appearance.themeDark'),
			themeAuto: k('settings.appearance.themeAuto'),
			textScale: k('settings.appearance.textScale')
		},
		language: {
			title: k('language.title'),
			pickerTitle: k('language.pickerTitle'),
			pickerSubtitle: k('language.pickerSubtitle'),
			followSystem: k('language.followSystem'),
			contributeNote: k('language.contributeNote'),
			contributeCta: k('language.contributeCta')
		},
		localization: {
			currencyTitle: k('settings.localization.currencyTitle'),
			autoExample: k('settings.localization.autoExample'),
			numberTitle: k('settings.localization.numberTitle'),
			numberSubtitle: k('settings.localization.numberSubtitle'),
			dateTitle: k('settings.localization.dateTitle'),
			dateSubtitle: k('settings.localization.dateSubtitle'),
			timeTitle: k('settings.localization.timeTitle'),
			timeSubtitle: k('settings.localization.timeSubtitle')
		},
		formatNote: {
			system: k('settings.formatNote.system'),
			indian: k('settings.formatNote.indian'),
			h24: k('settings.formatNote.h24'),
			h12: k('settings.formatNote.h12')
		},
		currency: {
			title: k('componentsUi.currency.title'),
			searchPlaceholder: k('componentsUi.currency.searchPlaceholder'),
			symbols: Object.fromEntries(
				DRAWN_CURRENCY_CODES.map((code) => [code, fiatSymbol(code, locale)])
			)
		},
		advanced: {
			networksTitle: k('settings.advanced.networksTitle'),
			networksSubtitle: k('settings.advanced.networksSubtitle'),
			addNetworkTitle: k('settings.advanced.addNetworkTitle'),
			addNetworkSubtitle: k('settings.advanced.addNetworkSubtitle'),
			endpointsTitle: k('settings.advanced.endpointsTitle'),
			endpointsSubtitle: k('settings.advanced.endpointsSubtitle'),
			feeSpeedTitle: k('settings.advanced.feeSpeedTitle'),
			feeSpeedSubtitle: k('settings.advanced.feeSpeedSubtitle')
		},
		feeSpeed: {
			title: k('settings.feeSpeed.title'),
			subtitle: k('settings.feeSpeed.subtitle'),
			// The send form's own tier words, so the two surfaces cannot drift.
			fast: k('send.gasTier.fast'),
			standard: k('send.gasTier.standard'),
			slow: k('send.gasTier.slow'),
			fastHint: k('send.gasTierHintFast'),
			standardHint: k('send.gasTierHintStandard'),
			slowHint: k('send.gasTierHintSlow')
		},
		signing: {
			title: k('settings.signing.title'),
			subtitle: k('settings.signing.subtitle'),
			pageOfficial: k('settings.signing.pageOfficial'),
			pageInvalid: k('settings.signing.pageInvalid'),
			pageInsecure: k('settings.signing.pageInsecure'),
			pageDuplicate: k('settings.signing.pageDuplicate'),
			pageAdd: k('settings.signing.pageAdd'),
			pageSave: k('settings.signing.pageSave'),
			keysOn: k('settings.signing.keysOn'),
			pageSelfHosted: k('settings.signing.pageSelfHosted'),
			pageRename: k('settings.signing.pageRename'),
			pageRemove: k('settings.signing.pageRemove'),
			pageTrust: k('settings.signing.pageTrust')
		},
		venue: {
			title: k('settings.venue.title'),
			subtitle: k('settings.venue.subtitle'),
			inVela: k('settings.venue.inVela'),
			inVelaBody: k('settings.venue.inVelaBody'),
			page: k('settings.venue.page'),
			pageBody: k('settings.venue.pageBody'),
			pageProtects: k('settings.venue.pageProtects'),
			pageLimit: k('settings.venue.pageLimit'),
			blocked: venueBlockWords(k)
		},
		integrity: {
			checking: k('componentsUi.signing.integrity.checking'),
			matches: k('componentsUi.signing.integrity.matches'),
			trusted: k('componentsUi.signing.integrity.trusted'),
			unchecked: k('componentsUi.signing.integrity.unchecked'),
			mismatch: k('componentsUi.signing.integrity.mismatch'),
			blocked: k('componentsUi.signing.integrity.blocked'),
			askTrust: k('componentsUi.signing.integrity.askTrust'),
			couldNotCheck: k('componentsUi.signing.integrity.couldNotCheck'),
			noVersion: k('componentsUi.signing.integrity.noVersion'),
			allBlocked: k('componentsUi.signing.integrity.allBlocked')
		},
		networks: {
			count: k('settings.networks.count'),
			custom: k('settings.networks.custom'),
			slow: k('settings.networks.slow'),
			builtinNote: k('settings.networks.builtinNote'),
			saveHint: k('settings.networks.saveHint'),
			online: k('settings.networks.online'),
			offline: k('settingsModals.health.offline'),
			httpsRequired: k('settingsModals.health.httpsRequired'),
			invalid: k('settingsModals.health.invalid'),
			chainId: k('settingsModals.network.chainId'),
			remove: k('settingsModals.network.removeTitle'),
			removeBody: k('settingsModals.network.removeBody'),
			removeConfirm: k('settingsModals.network.removeConfirm'),
			removeCancel: k('settingsModals.network.removeCancel'),
			rpcUrl: k('settingsModals.network.fieldRpcUrl'),
			explorer: k('settingsModals.network.fieldExplorer'),
			mismatch: k('settingsModals.network.rpcChainMismatch'),
			urlRefused: k('settingsModals.health.urlRefused'),
			relayNotice: k('settingsModals.network.relayNotice'),
			fallback: k('settingsModals.network.fallback'),
			ownNodeDown: k('assets.ownNodeUnreachable'),
			onlyMine: k('settingsModals.network.onlyMine'),
			onlyMineHint: k('settingsModals.network.onlyMineHint'),
			onlyMineTrust: k('settingsModals.network.onlyMineTrust'),
			hideBody: k('settingsModals.network.hideBody'),
			moreNetworks: k('settingsModals.network.moreNetworks')
		},
		addNetwork: {
			description: k('settingsModals.addNetwork.description'),
			searchPlaceholder: k('settingsModals.addNetwork.searchPlaceholder'),
			compatible: k('settingsModals.addNetwork.compatible'),
			incompatible: k('settingsModals.addNetwork.incompatible'),
			compatibilityCheck: k('settingsModals.addNetwork.compatibilityCheck'),
			customRpcTitle: k('settingsModals.addNetwork.customRpcTitle'),
			customRpcPlaceholder: k('settingsModals.addNetwork.customRpcPlaceholder'),
			addNetworkBtn: k('settingsModals.addNetwork.addNetworkBtn'),
			searching: k('settingsModals.addNetwork.searching'),
			checkingCompatibility: k('settingsModals.addNetwork.checkingCompatibility'),
			unableToVerify: k('settingsModals.addNetwork.unableToVerify'),
			retry: k('settingsModals.addNetwork.retry'),
			// By corpus key: the core names which line a refusal says.
			hints: Object.fromEntries(NET_HINT_KEYS.map((key) => [key, k(key)])) as Record<
				NetHintKey,
				string
			>,
			// …and which sentence a stopped wizard says (`error_key`).
			stops: Object.fromEntries(NET_STOP_KEYS.map((key) => [key, k(key)])) as Record<
				NetStopKey,
				string
			>,
			rpcFieldLabels: Object.fromEntries(NET_RPC_FIELD_KEYS.map((key) => [key, k(key)])) as Record<
				NetRpcFieldKey,
				string
			>,
			singleKeyOnly: k('settingsModals.addNetwork.singleKeyOnly'),
			velaNetwork: k('settingsModals.addNetwork.velaNetwork'),
			velaNetworkHint: k('settingsModals.addNetwork.velaNetworkHint'),
			openChainSetupTool: k('settingsModals.addNetwork.openChainSetupTool'),
			recheckWithRpc: k('settingsModals.addNetwork.recheckWithRpc'),
			testnet: k('settingsModals.addNetwork.testnet'),
			discard: {
				title: k('settingsModals.addNetwork.discardTitle'),
				body: k('settingsModals.addNetwork.discardBody'),
				confirm: k('settingsModals.addNetwork.discardConfirm'),
				cancel: k('settingsModals.addNetwork.discardCancel')
			},
			bestRpc: k('settingsModals.addNetwork.bestRpc'),
			// A product name, not prose — the corpus has no key for it and should
			// not: translating "EntryPoint v0.7" would make the checklist lie.
			checkEntryPoint: 'EntryPoint v0.7',
			checkSafe: k('settingsModals.addNetwork.checkSafe'),
			checkSigner: k('settingsModals.addNetwork.checkSigner'),
			checkRemaining: k('settingsModals.addNetwork.checkRemaining')
		},

		endpoints: {
			description: k('settingsModals.endpoints.description'),
			chainDataLabel: k('settingsModals.endpoints.chainDataLabel'),
			chainDataHint: k('settingsModals.endpoints.chainDataHint'),
			passkeyLabel: k('settingsModals.endpoints.passkeyLabel'),
			passkeyHint: k('settingsModals.endpoints.passkeyHint'),
			bundlerLabel: k('settingsModals.endpoints.bundlerLabel'),
			bundlerHint: k('settingsModals.endpoints.bundlerHint'),
			fiatLabel: k('settingsModals.endpoints.fiatLabel'),
			fiatHint: k('settingsModals.endpoints.fiatHint'),
			genesisLabel: k('settingsModals.endpoints.genesisLabel'),
			genesisHint: k('settingsModals.endpoints.genesisHint'),
			reset: k('settingsModals.endpoints.resetToDefaults'),
			resetTitle: k('settingsModals.endpoints.resetTitle'),
			resetBody: k('settingsModals.endpoints.resetBody'),
			resetConfirm: k('settingsModals.endpoints.resetConfirm'),
			resetCancel: k('settingsModals.endpoints.resetCancel'),
			guide: k('settingsModals.endpoints.selfHostGuide')
		},
		storage: {
			title: k('settings.storage.title'),
			subtitle: k('settings.storage.subtitle'),
			summary: k('settings.storage.summary'),
			userData: k('settings.storage.userData'),
			caches: k('settings.storage.caches'),
			connections: k('settings.storage.connections'),
			legendUserData: k('settings.storage.legendUserData'),
			legendCaches: k('settings.storage.legendCaches'),
			legendSessions: k('settings.storage.legendSessions'),
			itemTransactions: k('settings.storage.itemTransactions'),
			itemContacts: k('settings.storage.itemContacts'),
			itemCustom: k('settings.storage.itemCustom'),
			itemBrowsing: k('settings.storage.itemBrowsing'),
			itemBalances: k('settings.storage.itemBalances'),
			itemRates: k('settings.storage.itemRates'),
			itemScan: k('settings.storage.itemScan'),
			itemDapps: k('settings.storage.itemDapps'),
			records: k('settings.storage.records'),
			contactsCount: k('settings.storage.contactsCount'),
			itemsCount: k('settings.storage.itemsCount'),
			sitesCount: k('settings.storage.sitesCount'),
			clear: k('settings.storage.clear'),
			clearAllCaches: k('settings.storage.clearAllCaches'),
			disconnectAll: k('settings.storage.disconnectAll'),
			disconnectOne: k('connect.browser.disconnect'),
			clearTitle: k('settings.storage.clearTitle'),
			clearBody: k('settings.storage.clearBody'),
			clearConfirm: k('settings.storage.clearConfirm')
		},
		about: {
			title: k('settings.about.title'),
			subtitleTemplate: k('settings.about.subtitle'),
			version: k('about.version'),
			copy: k('componentsUi.signing.copyValue'),
			copied: k('receive.copied'),
			linkGitHub: k('about.linkGitHub'),
			linkPrivacy: k('about.linkPrivacy'),
			linkTerms: k('about.linkTerms'),
			linkSupport: k('about.linkSupport'),
			sectionLinks: k('about.sectionLinks'),
			footer: k('about.footer')
		},
		accounts: {
			title: k('settingsModals.account.modalTitle'),
			total: k('settingsModals.account.total'),
			totalPartial: k('settingsModals.account.totalPartial'),
			countPrefix: pluralCopy(locale, 'home.switcherAccountCount'),
			createNew: k('settingsModals.account.createNew'),
			signInExisting: k('settingsModals.account.signInExisting'),
			remove: k('settings.account.remove'),
			removeNamed: k('settings.account.removeNamed'),
			removeBody: k('settings.account.removeBody'),
			cancel: k('common.cancel'),
			syncing: k('home.syncingLine'),
			unreadable: k('onboarding.storage.unreadableTitle')
		},
		signOut: {
			button: k('settings.signOut.button'),
			title: k('settings.signOut.title'),
			desc: k('settings.signOut.desc'),
			descMany: k('settings.signOut.descMany'),
			keeps: k('settings.signOut.keeps'),
			warning: k('settings.signOut.warning'),
			anyway: k('settings.signOut.anyway'),
			cancel: k('settings.signOut.cancel')
		},
		erase: {
			title: k('settings.eraseDevice.title'),
			subtitle: k('settings.eraseDevice.subtitle'),
			desc: k('settings.eraseDevice.desc'),
			loses: k('settings.eraseDevice.loses'),
			keeps: k('settings.eraseDevice.keeps'),
			confirm: k('settings.eraseDevice.confirm'),
			cancel: k('settings.eraseDevice.cancel'),
			failed: k('settings.eraseDevice.failed'),
			failedHeld: k('settings.eraseDevice.failedHeld'),
			unsynced: k('settings.eraseDevice.unsynced'),
			unsyncedAck: k('settings.eraseDevice.unsyncedAck')
		},
		bugReport: {
			helpTitle: k('componentsUi.bugReport.helpTitle'),
			helpSubtitle: k('componentsUi.bugReport.helpSubtitle'),
			helpPlaceholder: k('componentsUi.bugReport.helpPlaceholder'),
			whatLabel: k('componentsUi.bugReport.whatLabel'),
			previewVersion: k('componentsUi.bugReport.previewVersion'),
			previewPlatform: k('componentsUi.bugReport.previewPlatform'),
			previewLanguage: k('componentsUi.bugReport.previewLanguage'),
			previewRpc: k('componentsUi.bugReport.previewRpc'),
			chainFallback: k('assets.chainFallback'),
			previewFailures: k('componentsUi.bugReport.previewFailures'),
			previewNone: k('componentsUi.bugReport.previewNone'),
			detailsIncluded: k('componentsUi.bugReport.detailsIncluded'),
			detailsView: k('componentsUi.bugReport.detailsView'),
			detailsHide: k('componentsUi.bugReport.detailsHide'),
			consent: k('componentsUi.bugReport.consent'),
			sendHelp: k('componentsUi.bugReport.sendHelp'),
			sending: k('componentsUi.bugReport.sending'),
			thanksTitle: k('componentsUi.bugReport.thanksTitle'),
			thanksBody: k('componentsUi.bugReport.thanksBody'),
			followUpGithub: k('componentsUi.bugReport.followUpGithub'),
			viewIssue: k('componentsUi.bugReport.viewIssue'),
			fallbackTitle: k('componentsUi.bugReport.fallbackTitle'),
			fallbackBody: k('componentsUi.bugReport.fallbackBody'),
			openGithub: k('componentsUi.bugReport.openGithub'),
			screenshotsLabel: k('componentsUi.bugReport.screenshotsLabel'),
			attachScreenshot: k('componentsUi.bugReport.attachScreenshot'),
			screenshotsPublic: k('componentsUi.bugReport.screenshotsPublic'),
			removeScreenshot: k('componentsUi.bugReport.removeScreenshot'),
			screenshotsLimit: k('componentsUi.bugReport.screenshotsLimit'),
			screenshotsFull: k('componentsUi.bugReport.screenshotsFull'),
			screenshotUnsupported: k('componentsUi.bugReport.screenshotUnsupported'),
			dropHint: k('componentsUi.bugReport.dropHint'),
			screenshotsDropped: k('componentsUi.bugReport.screenshotsDropped'),
			fallbackScreenshots: k('componentsUi.bugReport.fallbackScreenshots'),
			done: k('componentsUi.bugReport.done'),
			viewScreenshot: k('componentsUi.bugReport.viewScreenshot'),
			closeViewer: k('componentsUi.bugReport.closeViewer'),
			removeFromViewer: k('componentsUi.bugReport.removeFromViewer')
		},
		rescue: {
			unreachableOne: k('assets.unreachableOne'),
			unreachableMany: k('assets.unreachableMany'),
			ownNodeUnreachable: k('assets.ownNodeUnreachable'),
			tokenListUnreachable: k('assets.tokenListUnreachable'),
			unreachableBody: k('assets.unreachableBody'),
			unreachableNone: k('assets.unreachableNone'),
			internal: Object.fromEntries(BALANCE_INTERNAL_KEYS.map((key) => [key, k(key)])),
			lines: Object.fromEntries(UNREACHABLE_LINE_KEYS.map((key) => [key, k(key)])),
			rpcFix: k('assets.rpcFix'),
			rpcFixTitle: k('assets.rpcFixTitle'),
			rpcFixWarning: k('assets.rpcFixWarning'),
			rpcFixLabel: k('assets.rpcFixLabel'),
			rpcFixSaveBtn: k('assets.rpcFixSaveBtn'),
			rpcFixRestored: k('assets.rpcFixRestored'),
			rpcProvidersTitle: k('assets.rpcProvidersTitle'),
			rpcReport: k('assets.rpcReport')
		},
		balanceDetail: {
			title: k('home.balanceDetailTitle'),
			total: k('assets.switcherTotal'),
			networksLabel: k('home.balanceDetailNetworksLabel'),
			networksNote: k('home.balanceDetailNetworksNote'),
			statusRetrying: k('home.balanceDetailStatusRetrying'),
			statusFailed: k('home.balanceDetailStatusFailed'),
			statuses: Object.fromEntries(BALANCE_STATUS_KEYS.map((key) => [key, k(key)])),
			statusPublicNodes: k('home.balanceDetailStatusPublicNodes'),
			updatedLabel: k('home.balanceDetailUpdatedLabel'),
			retry: k('home.balanceDetailRetry'),
			allUnpriced: k('home.allUnpriced')
		},
		backup: {
			// By corpus key: the core names which one each state says.
			words: Object.fromEntries(BACKUP_ROW_KEYS.map((key) => [key, k(key)])) as Record<
				BackupRowKey,
				string
			>,
			explains: Object.fromEntries(BACKUP_EXPLAIN_KEYS.map((key) => [key, k(key)])) as Record<
				BackupExplainKey,
				string
			>
		},
		keys: {
			title: k('settingsModals.keys.title'),
			subtitle: k('settingsModals.keys.subtitle'),
			subtitleSingle: k('settingsModals.keys.subtitleSingle'),
			keyN: k('settingsModals.keys.keyN'),
			synced: k('onboarding.create.keySyncedBadge'),
			notSynced: k('settingsModals.keys.notSynced'),
			fromDevice: k('settingsModals.keys.fromDevice'),
			providerPlatform: k('onboarding.create.providerPlatform'),
			providerGeneric: k('onboarding.create.providerGeneric'),
			providerSecurityKey: k('onboarding.create.providerSecurityKey'),
			publicKey: k('settingsModals.keys.publicKey'),
			credential: k('settingsModals.keys.credential'),
			transport: k('settingsModals.keys.transport'),
			attestation: k('settingsModals.keys.attestation'),
			userVerified: k('settingsModals.keys.userVerified'),
			signsHere: k('settingsModals.keys.signsHere'),
			copy: k('componentsUi.signing.copyValue'),
			copied: k('receive.copied')
		},
		relayer: {
			title: k('componentsUi.treasuryBootstrap.title'),
			lead: k('componentsUi.treasuryBootstrap.lead'),
			amountHint: k('componentsUi.treasuryBootstrap.amountHint'),
			addressLabel: k('componentsUi.treasuryBootstrap.addressLabel'),
			disclaimer: k('componentsUi.treasuryBootstrap.disclaimer'),
			retryBtn: k('componentsUi.treasuryBootstrap.retryBtn'),
			copyBtn: k('componentsUi.treasuryBootstrap.copyBtn'),
			operatorLead: k('componentsUi.treasuryBootstrap.operatorLead'),
			reportBtn: k('componentsUi.treasuryBootstrap.reportBtn'),
			selfFundToggle: k('componentsUi.treasuryBootstrap.selfFundToggle'),
			customLead: k('componentsUi.treasuryBootstrap.customLead'),
			balanceLine: k('componentsUi.treasuryBootstrap.balanceLine'),
			watching: k('componentsUi.treasuryBootstrap.watching'),
			qrLabel: k('componentsUi.treasuryBootstrap.qrLabel')
		},
		relayUnreachable: {
			title: k('componentsUi.relayUnreachable.title'),
			operatorLead: k('componentsUi.relayUnreachable.operatorLead'),
			customLead: k('componentsUi.relayUnreachable.customLead'),
			settingsHint: k('componentsUi.relayUnreachable.settingsHint'),
			reportBtn: k('componentsUi.relayUnreachable.reportBtn'),
			retryBtn: k('componentsUi.relayUnreachable.retryBtn'),
			closeBtn: k('componentsUi.relayUnreachable.closeBtn')
		},
		indexDown: {
			title: k('settings.indexDown.title'),
			subtitle: k('settings.indexDown.subtitle'),
			warning: k('onboarding.settings.warningText'),
			endpointLabel: k('onboarding.settings.endpointUrlLabel'),
			editEndpoint: k('settings.indexDown.editEndpoint'),
			passkeyHint: k('onboarding.settings.passkeyHint')
		},
		common: {
			cancel: k('common.cancel'),
			system: k('common.system'),
			automatic: k('common.automatic'),
			done: k('common.done'),
			tryAgain: k('common.tryAgain'),
			close: k('componentsUi.identiconViewer.close'),
			copyAddress: k('componentsUi.identiconViewer.copyAddress')
		},
		shell: {
			networksTitle: k('settingsModals.network.modalTitle'),
			allNetworks: k('componentsUi.networkFilter.allNetworks')
		},
		walletTitle: k('componentsUi.mainNav.wallet'),
		sendTitle: k('componentsUi.dock.send')
	};
}

/**
 * The Receive / Send / Activity / Assets copy (spec 021). Flat, like the
 * onboarding flow's and the intro's: with ~120 strings across four journeys a
 * nested manifest would be more field declarations than copy, and each one
 * would have to be restated here by hand.
 */
export function resolveWalletFlowMessages(locale: Locale): WalletFlowMessages {
	activate(locale);
	return Object.fromEntries(
		WALLET_FLOW_KEYS.map((key) => [key, t(locale, key)])
	) as WalletFlowMessages;
}

/**
 * The send picker's "N members" under a group (set-16): the address book's
 * own plural key, picked by count where the picker draws.
 */
export function resolveMemberCountCopy(locale: Locale): PluralCopy {
	return pluralCopy(locale, 'contacts.memberCount');
}

/**
 * A plural key as the forms `locale` has — the core's categories for it, the
 * same set `resolveFlowMessages` ships — for a screen that knows the count
 * only when it draws (`pluralForm`).
 */
function pluralCopy(locale: Locale, key: string): PluralCopy {
	activate(locale);
	return {
		locale,
		forms: Object.fromEntries(
			i18nPluralSuffixes(locale).map((suffix) => [suffix, t(locale, `${key}${suffix}`)])
		)
	};
}

/** Direct engine access for the differential test only. */
export function rawResolve(locale: Locale, key: string): string {
	activate(locale);
	return engine.t(key);
}

/** The serializable strings the Explore screens render (spec 022 §5). */
/**
 * The request window's copy (spec 027). These keys were written for the in-app
 * browser's connect sheet and say the right thing wherever a request arrives
 * from — one corpus, not one per surface.
 */
export function resolveRequestMessages(locale: Locale): RequestMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		title: k('connect.browser.title'),
		body: k('connect.browser.body'),
		connect: k('connect.browser.connect'),
		cancel: k('connect.browser.cancel'),
		preparing: k('connect.browser.preparing'),
		accountLabel: k('explore.account'),
		networkLabel: k('explore.network'),
		noWallet: k('connect.browser.signedOut'),
		createWallet: k('onboarding.welcome.createWallet'),
		haveWallet: k('onboarding.welcome.alreadyHaveWallet'),
		signInTitle: k('connect.browser.signInTitle'),
		multisigNoMessagesTitle: k('connect.browser.multisigNoMessagesTitle'),
		multisigNoMessagesBody: k('connect.browser.multisigNoMessagesBody'),
		multisigConnectNote: k('connect.browser.multisigConnectNote'),
		parentSafeTitle: k('componentsUi.signing.selfCallBlockedTitle'),
		parentSafeBlocked: k('componentsUi.signing.parentSafeBlocked'),
		parentSafeUnchecked: k('componentsUi.signing.parentSafeUnchecked'),
		multisigSelfCallBlocked: k('componentsUi.signing.multisigSelfCallBlocked'),
		spaceMoved: k('connect.browser.spaceMoved'),
		gotIt: k('common.gotIt'),
		addNetwork: {
			title: k('settingsModals.addNetwork.modalTitle'),
			lead: k('connect.browser.addLead'),
			fromSite: k('connect.browser.addFromSite'),
			name: k('addToken.labelName'),
			chainIdLabel: k('addToken.labelChainId'),
			nativeToken: k('addToken.labelNativeToken'),
			rpcUrl: k('addToken.labelRpcUrl'),
			explorer: k('addToken.labelExplorer'),
			chainName: k('addToken.chainId'),
			checking: k('settingsModals.addNetwork.checkingCompatibility'),
			compatible: k('settingsModals.addNetwork.compatible'),
			incompatible: k('settingsModals.addNetwork.incompatible'),
			hints: Object.fromEntries(NET_HINT_KEYS.map((key) => [key, k(key)])) as Record<
				NetHintKey,
				string
			>,
			singleKeyOnly: k('settingsModals.addNetwork.singleKeyOnly'),
			velaNetwork: k('settingsModals.addNetwork.velaNetwork'),
			velaNetworkHint: k('settingsModals.addNetwork.velaNetworkHint'),
			unableToVerify: k('settingsModals.addNetwork.unableToVerify'),
			rpcFromSite: k('connect.browser.addRpcFromSite'),
			whyNoRpc: k('connect.browser.addWhyNoRpc'),
			whyUnreachable: k('connect.browser.addWhyUnreachable'),
			addUnverified: k('connect.browser.addUnverified'),
			unverifiedWarning: k('connect.browser.addUnverifiedWarning'),
			addUnverifiedConfirm: k('connect.browser.addUnverifiedConfirm'),
			wrongRpc: k('assets.rpcFixWrongChain'),
			noRpc: k('componentsUi.browserStatus.reason.badRpc'),
			checksTitle: k('settingsModals.addNetwork.compatibilityCheck'),
			checkSigner: k('settingsModals.addNetwork.checkSigner'),
			add: k('settingsModals.addNetwork.addNetworkBtn'),
			retry: k('settingsModals.addNetwork.retry'),
			setupTool: k('settingsModals.addNetwork.openChainSetupTool'),
			cancel: k('connect.browser.cancel'),
			done: k('common.done')
		}
	};
}

/** The spaces' words (spec 105), on every `[locale]` page. */
export function resolveNetworkSpaceMessages(locale: Locale): NetworkSpaceMessages {
	activate(locale);
	const k = (key: string) => t(locale, `networkSpace.${key}`);
	return {
		mainnet: k('mainnet'),
		testnet: k('testnet'),
		bar: k('bar'),
		title: k('title'),
		mainnetHint: k('mainnetHint'),
		testnetHint: k('testnetHint'),
		toTestnet: k('toTestnet'),
		toMainnet: k('toMainnet'),
		noValue: k('noValue'),
		moveToMainnet: k('moveToMainnet'),
		moveToTestnet: k('moveToTestnet'),
		wizardQuestion: k('wizardQuestion'),
		wizardTestnet: k('wizardTestnet'),
		wizardMainnet: k('wizardMainnet'),
		headingMainnet: k('headingMainnet'),
		headingTestnet: k('headingTestnet'),
		addedTestnet: k('addedTestnet'),
		addedMainnet: k('addedMainnet'),
		cancel: t(locale, 'common.cancel')
	};
}

/** The packaged extension's notices (spec 094), on every `[locale]` page. */
/** The agent's layer over every page (spec 103). Keys by action `type`. */
export function resolveAgentMessages(locale: Locale): AgentMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	const action = (name: string) => k(`settings.agent.action.${name}`);
	const detail = (name: string) => k(`settings.agent.detail.${name}`);
	return {
		title: k('settings.agent.title'),
		connected: k('settings.agent.statusOnline'),
		elsewhere: k('settings.agent.statusElsewhere'),
		useHere: k('settings.agent.useHere'),
		approveTitle: k('settings.agent.approveTitle'),
		approveHint: k('settings.agent.approveHint'),
		approveLine: k('settings.agent.approveLine'),
		approveAction: k('settings.agent.approveAction'),
		pair: k('settings.agent.detailPair'),
		allow: k('settings.agent.allow'),
		deny: k('settings.agent.deny'),
		stop: k('settings.agent.stop'),
		showBalance: k('home.a11yShowBalance'),
		hideBalance: k('home.a11yHideBalance'),
		on: k('settings.agent.statusOn'),
		off: k('settings.agent.statusOff'),
		chain: k('settings.agent.chain'),
		// As Settings names them, so the request and the page it changes agree.
		preferences: {
			theme: k('settings.appearance.themeTitle'),
			language: k('language.title'),
			text_scale: k('settings.appearance.textScale'),
			number_format: k('settings.localization.numberTitle'),
			date_format: k('settings.localization.dateTitle'),
			time_format: k('settings.localization.timeTitle'),
			currency: k('settings.localization.currencyTitle'),
			fee_speed: k('settings.feeSpeed.title'),
			analytics: k('settings.analytics.title')
		},
		values: {
			'theme.system': k('settings.appearance.themeAuto'),
			'theme.light': k('settings.appearance.themeLight'),
			'theme.dark': k('settings.appearance.themeDark'),
			'language.auto': k('language.followSystem'),
			'number_format.auto': k('settings.formatNote.system'),
			'number_format.indian': k('settings.formatNote.indian'),
			'date_format.auto': k('settings.formatNote.system'),
			'time_format.auto': k('settings.formatNote.system'),
			'time_format.h24': k('settings.formatNote.h24'),
			'time_format.h12': k('settings.formatNote.h12'),
			'fee_speed.fast': k('send.gasTier.fast'),
			'fee_speed.standard': k('send.gasTier.standard'),
			'fee_speed.slow': k('send.gasTier.slow')
		},
		storage: {
			transactions: k('settings.storage.itemTransactions'),
			contacts: k('settings.storage.itemContacts'),
			custom: k('settings.storage.itemCustom'),
			browsing: k('settings.storage.itemBrowsing'),
			balances: k('settings.storage.itemBalances'),
			rates: k('settings.storage.itemRates'),
			scan: k('settings.storage.itemScan'),
			dapps: k('settings.storage.itemDapps')
		},
		services: {
			ethereum_data: detail('ethereumData'),
			passkey_index: detail('passkeyIndex'),
			bundler_service: detail('bundlerService'),
			fiat_rates: detail('fiatRates')
		},
		fields: { rpc: detail('rpc'), explorer: detail('explorer') },
		// The switch's own name on the network's page, so the two agree.
		onlyMine: k('settingsModals.network.onlyMine'),
		actions: {
			show: action('show'),
			switch_account: action('switchAccount'),
			set_preference: action('setPreference'),
			save_contact: action('saveContact'),
			delete_contact: action('deleteContact'),
			save_group: action('saveGroup'),
			delete_group: action('deleteGroup'),
			set_group_members: action('setGroupMembers'),
			set_contact_groups: action('setContactGroups'),
			remove_wallet: action('removeWallet'),
			sign_out: action('signOut'),
			erase_device: action('eraseDevice'),
			clear_storage_item: action('clearStorageItem'),
			clear_caches: action('clearCaches'),
			prepare_transfer: action('prepareTransfer'),
			submit_transfer: action('submitTransfer'),
			cancel_transfer: action('cancelTransfer'),
			prepare_batch_transfer: action('prepareBatchTransfer'),
			add_token: action('addToken'),
			delete_activity: action('deleteActivity'),
			set_balance_privacy: action('setBalancePrivacy'),
			set_network_space: action('setNetworkSpace'),
			refresh_balances: action('refreshBalances'),
			set_contact_favorite: action('setContactFavorite'),
			add_network: action('addNetwork'),
			set_network_endpoint: action('setNetworkEndpoint'),
			set_only_my_node: action('setOnlyMyNode'),
			remove_network: action('removeNetwork'),

			set_service_endpoint: action('setServiceEndpoint'),
			reset_service_endpoints: action('resetServiceEndpoints'),
			open_site: action('openSite'),
			browse: action('browse'),
			close_tab: action('closeTab'),
			close_all_tabs: action('closeAllTabs'),
			set_site_favorite: action('setSiteFavorite'),
			clear_recent_sites: action('clearRecentSites'),
			forget_site: action('forgetSite'),
			disconnect_site: action('disconnectSite'),
			set_site_network: action('setSiteNetwork'),
			reject_dapp_request: action('rejectDappRequest'),
			ask_person_dapp_request: action('askPersonDappRequest')
		}
	};
}

export function resolveExtensionMessages(locale: Locale): ExtensionMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		installedNote: k('connect.ext.installedNote'),
		accessNote: k('connect.ext.accessNote'),
		accessAllow: k('connect.ext.accessAllow'),
		dismiss: k('common.gotIt'),
		openInTabTitle: k('connect.ext.openInTabTitle'),
		openInTabBody: k('connect.ext.openInTabBody'),
		goToTab: k('connect.ext.goToTab'),
		useHereInstead: k('connect.ext.useHereInstead'),
		openInPanelTitle: k('connect.ext.openInPanelTitle'),
		openInPanelBody: k('connect.ext.openInPanelBody'),
		goToPanel: k('connect.ext.goToPanel')
	};
}

export function resolveExploreMessages(locale: Locale): ExploreMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		title: k('explore.title'),
		searchPlaceholder: k('explore.searchPlaceholder'),
		scan: k('explore.scan'),
		startTitle: k('explore.startTitle'),
		startHint: k('explore.startHint'),
		startCta: k('explore.startCta'),
		favorites: k('explore.favorites'),
		recent: k('explore.recent'),
		edit: k('explore.edit'),
		done: k('explore.done'),
		add: k('explore.add'),
		clear: k('explore.clear'),
		manageGroups: k('explore.manageGroups'),
		rename: k('explore.rename'),
		hide: k('explore.hide'),
		show: k('explore.show'),
		delete: k('explore.delete'),
		openInNewTab: k('explore.openInNewTab'),
		removeFromFavorites: k('explore.removeFromFavorites'),
		siteCount: pluralCopy(locale, 'explore.siteCount'),
		tabs: k('explore.tabs'),
		newTab: k('explore.newTab'),
		startPage: k('explore.startPage'),
		closeAllTabs: k('explore.closeAllTabs'),
		closeTab: k('explore.closeTab'),
		openTabs: pluralCopy(locale, 'explore.openTabs'),
		addToFavorites: k('explore.addToFavorites'),
		addedToFavorites: k('explore.addedToFavorites'),
		share: k('explore.share'),
		copyLink: k('explore.copyLink'),
		refresh: k('explore.refresh'),
		openInSystemBrowser: k('explore.openInSystemBrowser'),
		disconnect: k('explore.disconnect'),
		closePage: k('explore.closePage'),
		secureSite: k('explore.secureSite'),
		localSite: k('explore.localSite'),
		a11yLocal: k('connect.browser.a11yLocal'),
		a11yInsecure: k('connect.browser.a11yInsecure'),
		connectedTag: k('explore.connectedTag'),
		connectionTitle: k('explore.connectionTitle'),
		switchAccount: k('explore.switchAccount'),
		network: k('explore.network'),
		connectionExplainer: k('explore.connectionExplainer'),
		autoRequestHint: k('explore.autoRequestHint'),
		back: k('explore.back'),
		forward: k('explore.forward'),
		reload: k('explore.reload'),
		siteMenu: k('explore.siteMenu'),
		account: k('explore.account'),
		addressBar: k('explore.addressBar'),
		close: k('explore.close'),
		nav: {
			wallet: k('componentsUi.mainNav.wallet'),
			contacts: k('componentsUi.mainNav.contacts'),
			explore: k('componentsUi.mainNav.explore'),
			settings: k('componentsUi.mainNav.settings')
		},
		closeLabel: k('componentsUi.signing.close')
	};
}

/** The serializable strings the signing sheet renders (spec 022 §5). */
/**
 * The corpus keys `fee_policy::failure_reason_key` answers with (spec 082
 * RJ13) — one list for the sheet and the send screens, kept beside the send
 * screens' keys (`$lib/flows/messages`, client-safe).
 */
export { FEE_REASON_KEYS };

/**
 * Every sentence a refusal is told by (`tx_tracker::RefusalReason::key`,
 * correctness batch item 3): the fee words only for a fee refusal, "another
 * transaction from this account went first" for a spent nonce, else the
 * plain refusal.
 */
export const REFUSAL_KEYS = [
	'send.txRejectedFees',
	'componentsUi.signing.wentFirst',
	// §35: it expired before it was submitted (`RefusalReason::Expired`, and
	// the sheet's `failure_refusal_key` for a submit the EntryPoint refused AA22).
	'componentsUi.signing.expired',
	'componentsUi.signing.refused',
	// PR 2 polish: at submit, the relay turned it back because the account's
	// previous operation on this network still holds the nonce
	// (`SignView.failure_refusal_key` under `failure_not_sent`).
	'componentsUi.signing.notSentBody'
] as const;

/** Spec 099 R7: every line `sign_confirm::ConfirmBlock::key` can name. */
export const CONFIRM_BLOCK_KEYS = [
	// The account's previous transaction on this network still holds the nonce
	// (correctness batch item 3): one plain line while the confirm is held.
	'componentsUi.signing.confirmBlock.previousPending',
	'componentsUi.signing.confirmBlock.accountSwitching',
	'componentsUi.signing.confirmBlock.answered',
	'componentsUi.signing.confirmBlock.answeredRetry',
	'componentsUi.signing.confirmBlock.reading',
	'componentsUi.signing.confirmBlock.approvalChoice',
	'componentsUi.signing.confirmBlock.batchUnsettled',
	'componentsUi.signing.confirmBlock.feeMeasuring',
	'componentsUi.signing.confirmBlock.feeFailed',
	// PR 2 note 1: the core asks a failed fee again by itself — the footer
	// says so (`FeeFailureView.footer_key`), never "tap it" under a row that
	// asks for no tap.
	'componentsUi.signing.confirmBlock.feeRetrying',
	// PR 2 polish: the relay answered that the operation would fail — a fact,
	// asking for no tap; the row says what a tap does.
	'componentsUi.signing.confirmBlock.feeWouldFail',
	// PR 3: everything else is ready and the sheet's own simulation has not
	// given its verdict yet (`ConfirmBlock::SimChecking`) — "Checking what
	// this transaction does…", for four seconds at most.
	'componentsUi.signing.confirmBlock.simChecking',
	// cs-10: a message taller than the sheet whose end has not been in sight
	// (`ConfirmBlock::MessageUnread`) — "The message continues — scroll to
	// read all of it".
	'componentsUi.signing.confirmBlock.messageUnread',
	// R22: the request signs only "… anyway" and the person turned on
	// "Refuse blind signing" (`ConfirmBlock::BlindRefused`).
	'componentsUi.signing.confirmBlock.blindRefused'
	// No `FeeShort` line: issue #438 — the fee section already says a short
	// coin, and the confirm's note said it again.
] as const;

/** Spec 099 R8: the signer's three failures, by `SignErrorKind`. */
export const SIGNER_REASON_KEYS = {
	signer_unavailable: 'componentsUi.browserStatus.reason.signerUnavailable',
	signer_not_discoverable: 'componentsUi.browserStatus.reason.signerNotDiscoverable',
	signer_failed: 'componentsUi.browserStatus.reason.signerFailed',
	// Round 7 (r7-2, r7-7): the account's trusted signing page did not sign —
	// said as what happened there, never "couldn't be submitted".
	page_unanswered: 'componentsUi.signing.trustedSignerUnanswered',
	page_mismatch: 'componentsUi.signing.trustedSignerMismatch',
	page_refused: 'componentsUi.signing.trustedSignerRefused',
	page_not_opened: 'componentsUi.signing.integrity.couldNotCheck'
} as const;

export function resolveSigningMessages(locale: Locale): SigningMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		panelTitle: k('componentsUi.signing.signatureRequest'),
		signingAccount: k('componentsUi.signing.signingAccount'),
		advancedToggle: k('componentsUi.signing.advancedToggle'),
		close: k('componentsUi.signing.close'),
		verdictBelow: k('componentsUi.signing.verdictBelow'),
		verdictAbove: k('componentsUi.signing.verdictAbove'),
		confirmSend: k('componentsUi.signing.confirmSend'),
		confirmSwap: k('componentsUi.signing.confirmSwap'),
		confirmDeposit: k('componentsUi.signing.confirmDeposit'),
		confirmWithdraw: k('componentsUi.signing.confirmWithdraw'),
		confirmPlain: k('componentsUi.signing.confirmLabel'),
		signLabel: k('componentsUi.signing.signLabel'),
		intentSend: k('componentsUi.signing.intentSend'),
		intentApprove: k('componentsUi.signing.intentApprove'),
		intentApproveAll: k('componentsUi.signingApprove.verbApproveAll'),
		intentRevoke: k('componentsUi.signing.intentRevoke'),
		intentSwap: k('componentsUi.signing.intentSwap'),
		intentDeposit: k('componentsUi.signing.intentDeposit'),
		intentWithdraw: k('componentsUi.signing.intentWithdraw'),
		intentTransferNft: k('componentsUi.signing.intentTransferNft'),
		intentContractCall: k('componentsUi.signing.intentContractCall'),
		intentBatch: k('componentsUi.signing.batchIntent'),
		intentBlind: k('componentsUi.signing.ethSignIntent'),
		intentSignIn: k('componentsUi.signing.signInIntent'),
		intentMessage: k('componentsUi.signing.messageIntent'),
		intentTypedData: k('componentsUi.signing.typedDataIntent'),
		intentPermit: k('componentsUi.signing.permitIntent'),
		intentDeploy: k('componentsUi.signing.deployIntent'),
		intentSafe: k('componentsUi.signing.safeIntent'),
		labelRecipient: k('componentsUi.signing.recipientLabel'),
		labelSpender: k('componentsUi.signing.spenderLabel'),
		labelOperator: k('componentsUi.signingApprove.operatorLabel'),
		labelCollection: k('componentsUi.signingApprove.collectionLabel'),
		labelInteracting: k('componentsUi.signing.interactingLabel'),
		labelFrom: k('componentsUi.signing.labelFrom'),
		labelAmount: k('componentsUi.signing.labelAmount'),
		labelDeadline: k('componentsUi.signing.labelDeadline'),
		labelMinReceived: k('componentsUi.signing.labelMinReceived'),
		labelPay: k('componentsUi.signing.labelPay'),
		labelSiweSite: k('componentsUi.signing.siweDomain'),
		labelSiweOrigin: k('componentsUi.signing.siweOrigin'),
		labelSiweStatement: k('componentsUi.signing.siweStatement'),
		labelTypedDomain: k('componentsUi.signing.typedDomain'),
		labelType: k('componentsUi.signing.typeLabel'),
		labelSigningFor: k('componentsUi.signing.signingFor'),
		labelSpendingCap: k('componentsUi.signingApprove.spendingCap'),
		labelExpires: k('componentsUi.signingApprove.expiresLabel'),
		labelResultingTotal: k('componentsUi.signingApprove.resultingTotal'),
		labelBytecode: k('componentsUi.signing.deployBytecode'),
		labelPredictedAddress: k('componentsUi.signing.deployPredictedAddress'),
		labelDepositAsset: k('componentsUi.signing.depositAsset'),
		labelSharesReceived: k('componentsUi.signing.sharesReceived'),
		tagContact: k('componentsUi.signing.contactTag'),
		tagWallet: k('componentsUi.signing.walletTag'),
		tagContract: k('componentsUi.signing.contractTag'),
		tagVerified: k('componentsUi.signing.verifiedTag'),
		tagUnverified: k('componentsUi.signing.unverifiedTag'),
		tagFirstTime: k('componentsUi.signing.firstTimeTag'),
		tagExpired: k('componentsUi.signing.expiredTag'),
		selfName: k('componentsUi.signing.selfName'),
		chipRequested: k('componentsUi.signingApprove.requested'),
		chipBalance: k('componentsUi.signingApprove.balanceCap'),
		chipCustom: k('componentsUi.signingApprove.custom'),
		chipRevoke: k('componentsUi.signingApprove.revoke'),
		chipRevokeAccess: k('componentsUi.signingApprove.revokeAccess'),
		chipGrantAll: k('componentsUi.signingApprove.grantAllAnyway'),
		valueRevoke: k('componentsUi.signingApprove.revokeValue'),
		valueUnlimited: k('componentsUi.signingApprove.unlimitedValue'),
		valueAllNfts: k('componentsUi.signingApprove.allNfts'),
		unlimitedDisabled: k('componentsUi.signingApprove.unlimitedDisabled'),
		selfCallBlockedTitle: k('componentsUi.signing.selfCallBlockedTitle'),
		selfCallBlockedBody: k('componentsUi.signing.selfCallBlockedBody'),
		selfCallBlockedLegBody: k('componentsUi.signing.selfCallBlockedLegBody'),
		selfCallBlockedSafeTx: k('componentsUi.signing.selfCallBlockedSafeTx'),
		parentSafeBlocked: k('componentsUi.signing.parentSafeBlocked'),
		parentSafeUnchecked: k('componentsUi.signing.parentSafeUnchecked'),
		invalidAmount: k('componentsUi.signingApprove.invalidAmount'),
		loading: k('componentsUi.signing.loading'),
		summarySend: k('componentsUi.signing.summarySend'),
		summarySendFrom: k('componentsUi.signing.summarySendFrom'),
		summarySwap: k('componentsUi.signing.summarySwap'),
		summaryReceive: k('componentsUi.signing.summaryReceive'),
		summaryApprove: k('componentsUi.signingApprove.capSummary'),
		summaryApproveUnlimited: k('componentsUi.signing.summaryApproveUnlimited'),
		summaryRevoke: k('componentsUi.signingApprove.revokeSummary'),
		summaryTransferNft: k('componentsUi.signing.summaryTransferNft'),
		summaryApproveNft: k('componentsUi.signing.summaryApproveNft'),
		summaryPermit: k('componentsUi.signing.summaryPermit'),
		summaryPermitUnlimited: k('componentsUi.signing.summaryPermitUnlimited'),
		summaryDeploy: k('componentsUi.signing.summaryDeploy'),
		summaryBatch: k('componentsUi.signing.batchSubtitle'),
		summarySafe: k('componentsUi.signing.safeSummary'),
		summaryBestEffort: k('componentsUi.signing.bestEffortSummary'),
		summaryVerifiedAbi: k('componentsUi.signing.verifiedAbiSummary'),
		summaryDrain: k('componentsUi.signing.drainSummary'),
		warnUnlimited: k('componentsUi.signing.unlimitedWarning'),
		warnBlindDecode: k('componentsUi.signing.blindDecodeWarning'),
		warnSelectorNotListed: k('componentsUi.signing.selectorNotListed'),
		warnExpired: k('componentsUi.signing.expiredWarning'),
		warnWillFail: k(SIM_WILL_FAIL_KEY),
		warnWillFailReason: k(SIM_WILL_FAIL_REASON_KEY),
		warnHexMessage: k('componentsUi.signing.hexMessageWarning'),
		warnBlindTyped: k('componentsUi.signing.blindTypedWarning'),
		warnEthSign: k('componentsUi.signing.ethSignWarning'),
		bodyEthSign: k('componentsUi.signing.ethSignBody'),
		warnSiweMismatch: k('componentsUi.signing.siweMismatch'),
		warnHiddenChars: k('componentsUi.signing.hiddenChars'),
		messageBlankLines: k('componentsUi.signing.blankLines'),
		okSiwe: k('componentsUi.signing.siweOk'),
		warnTokenToContract: k('componentsUi.signing.tokenToContractWarning'),
		warnUnverifiedAmount: k('componentsUi.signing.unverifiedWarning'),
		warnApproveAll: k('componentsUi.signingApprove.setApprovalAllWarn'),
		warnPermitCantCap: k('componentsUi.signingApprove.permitCantCap'),
		warnBestEffort: k('componentsUi.signing.bestEffortWarning'),
		warnPartial: k('componentsUi.signing.partialWarning'),
		warnVerifiedAbi: k('componentsUi.signing.verifiedAbiWarning'),
		warnDescriptorFetched: k('componentsUi.signing.descriptorFetchedWarning'),
		warnOrderTerms: k('componentsUi.signing.warnOrderTerms'),
		warnSimUnavailable: k(SIM_COULD_NOT_CHECK_KEY),
		spaceMoved: k('connect.browser.spaceMoved'),
		warnDrain: k('componentsUi.signing.drainWarning'),
		okSelfTransfer: k('componentsUi.signing.balanceSelfTransfer'),
		okNoNetworkFee: k('componentsUi.signing.noNetworkFee'),
		balancesTitle: k('componentsUi.signing.balanceChangesTitle'),
		simChecking: k('componentsUi.funding.checking'),
		simRecheck: k('componentsUi.signing.simRecheck'),
		simRechecking: k('componentsUi.signing.simRechecking'),
		simUnconfirmed: k('componentsUi.signing.simUnconfirmed'),
		simDisagreed: k('componentsUi.signing.simDisagreed'),
		simOwnNode: k('componentsUi.signing.simOwnNode'),
		ownNodeHint: k('componentsUi.signing.ownNodeHint'),
		ownNodeAction: k('home.nodeFallbackOpen'),
		holdToConfirm: k('componentsUi.signing.holdToConfirm'),
		confirmAgain: k('componentsUi.signing.confirmAgain'),
		balanceUnverifiedToken: k('componentsUi.signing.balanceUnverifiedToken'),
		// PR 3 device round: the verdict's quiet line, by the key the core names.
		simSaid: Object.fromEntries(SIM_SAID_KEYS.map((key) => [key, k(key)])),
		balancesMatchHero: k('componentsUi.signing.balanceMatchesHero'),
		balancesBlindSimulated: k('componentsUi.signing.blindButSimulated'),
		balancesBestEffort: k('componentsUi.signing.bestEffortSimulated'),
		feeLabel: k('componentsUi.gas.networkFee'),
		feeEstimating: k('componentsUi.gas.estimating'),
		feeRetry: k('componentsUi.gas.estimateFailed'),
		feePayWithAnotherCoin: k('componentsUi.gas.payWithAnotherCoin'),
		feeTokenTitle: k('componentsUi.signing.feeTokenTitle'),
		feeShort: k('send.warnInsufficientGas'),
		feeShortWithAmount: k('send.warnInsufficientForGas'),
		feeCoinSpent: k('componentsUi.gas.feeCoinSpent'),
		feeRowShort: k('componentsUi.gas.rowShort'),
		feeNoCoinPays: k('componentsUi.gas.noCoinPays'),
		feeRefresh: k('send.feeRefresh'),
		feeStale: k('send.feeStale'),
		// Spec 082 RJ13: every key `fee_policy::failure_reason_key` can name.
		feeReasons: Object.fromEntries(FEE_REASON_KEYS.map((key) => [key, k(key)])),
		confirmBlock: Object.fromEntries(CONFIRM_BLOCK_KEYS.map((key) => [key, k(key)])),
		passkeyNote: {
			cancelled: k('componentsUi.signing.passkeyClosed'),
			timed_out: k('componentsUi.signing.passkeyTimedOut')
		},
		signerReasons: Object.fromEntries(
			Object.entries(SIGNER_REASON_KEYS).map(([kind, key]) => [kind, k(key)])
		),
		venueBlock: venueBlockWords(k),
		speed: {
			label: k('send.feeSpeedLabel'),
			once: k('send.feeSpeedOnce'),
			free: k('send.feeSpeedFree'),
			single: k('send.feeSpeedSingle'),
			gasPriceLabel: k('send.gasPriceLabel'),
			names: {
				fast: k('send.gasTier.fast'),
				standard: k('send.gasTier.standard'),
				slow: k('send.gasTier.slow')
			}
		},
		feeEstimated: k('componentsUi.signing.feeEstimated'),
		feeBalance: k('componentsUi.gas.rowBalance'),
		techFunction: k('componentsUi.signing.techFunction'),
		techParam: k('componentsUi.signing.techParam'),
		techRawUnits: k('componentsUi.signing.techRawUnits'),
		techRawData: k('componentsUi.signing.techRawData'),
		techRawCopy: k('componentsUi.signing.techRawCopy'),
		techRawCopied: k('componentsUi.identiconViewer.copied'),
		techRawShowAll: k('componentsUi.signing.techRawShowAll'),
		techRawJsonText: k('componentsUi.signing.techRawJsonText'),
		techSimResult: k('componentsUi.signing.simResultLabel'),
		techIdentityToken: k('componentsUi.signing.techIdentityToken'),
		techIdentityRecipient: k('componentsUi.signing.techIdentityRecipient'),
		copyValue: k('componentsUi.signing.copyValue'),
		certain: Object.fromEntries(
			CERTAIN_KEYS.map((key) => [key, k(`componentsUi.signing.certain.${key}`)])
		) as SigningMessages['certain'],
		terms: Object.fromEntries(CLEAR_TERMS.map((term) => [term, k(`componentsUi.signing.${term}`)])),
		// Spec 103: the AI agent's opinion, in its own card.
		agentAdvice: {
			title: k('componentsUi.signing.agentAdvice.title'),
			looksFine: k('componentsUi.signing.agentAdvice.looksFine'),
			caution: k('componentsUi.signing.agentAdvice.caution'),
			danger: k('componentsUi.signing.agentAdvice.danger'),
			unsure: k('componentsUi.signing.agentAdvice.unsure'),
			sources: k('componentsUi.signing.agentAdvice.sources'),
			footer: k('componentsUi.signing.agentAdvice.footer'),
			stale: k('componentsUi.signing.agentAdvice.stale'),
			feeWhenGiven: k('componentsUi.signing.agentAdvice.feeWhenGiven'),
			feeChanged: k('componentsUi.signing.agentAdvice.feeChanged')
		},
		// Spec 077: the send receipt's own words, for the landing this sheet
		// draws once a transaction is submitted.
		receipt: {
			confirming: k('componentsTx.receipt.confirming'),
			confirmingHint: k('componentsTx.receipt.confirmingHint'),
			submitted: k('componentsTx.receipt.statusSubmitted'),
			confirmed: k('componentsTx.receipt.statusConfirmed'),
			failed: k('componentsTx.receipt.statusFailed'),
			failedHint: k('componentsTx.receipt.failedHint'),
			opHashLabel: k('componentsTx.receipt.userOpHash'),
			txHashLabel: k('componentsTx.receipt.txHash'),
			explorer: k('componentsTx.receipt.explorer'),
			done: k('componentsTx.receipt.done'),
			// Spec 079: what the landing says when the wait ran out, when the
			// tracker gave up at 24 h, and when a message was signed.
			stillConfirming: k('componentsUi.signing.stillConfirming'),
			unknownOutcome: k('componentsUi.signing.unknownOutcome'),
			relayFunding: k('send.txRelayFunding'),
			relaySending: k('send.txRelaySending'),
			signed: k('clearSigning.alertSignedTitle'),
			// Spec 082 RA10: a lost reply, its way out, and "not sent".
			maybeSent: k('componentsUi.signing.maybeSent'),
			closeBackground: k('send.txCloseBackground'),
			notSentHint: k('send.txErrorGeneric'),
			// Spec 082: "提交至网络…" while it may have been sent (G56), and the
			// relay's refusal, said without "try again" (RJ3).
			submitting: k('send.txSubmitting'),
			refused: k('componentsUi.signing.refused'),
			// Correctness batch item 3: every key a refusal can be told by
			// (`tx_tracker::RefusalReason::key`).
			refusals: Object.fromEntries(REFUSAL_KEYS.map((key) => [key, k(key)]))
		},
		// Spec 079: after the approval — the send receipt's words, as
		// Android's signing receipt uses them.
		status: {
			preparing: k('send.txPreparing'),
			signing: k('send.txSigning'),
			submitting: k('send.txSubmitting'),
			backgroundHint: k('send.txBackgroundHint'),
			messageSigning: k('componentsUi.signing.signing'),
			pageSigning: k('componentsUi.signing.trustedSignerWaiting'),
			failedHint: k('send.txErrorGeneric'),
			retry: k('send.txRetryBtn'),
			notSentTitle: k('componentsUi.signing.notSentTitle')
		},
		viewOnExplorer: k('componentsUi.signing.viewOnExplorer'),
		byteSize: k('componentsUi.signing.byteSize'),
		safeInnerCall: k('componentsUi.signing.safeInnerCall'),
		batchStep: k('componentsUi.signing.batchStep'),
		labelTotal: k('send.splitTotalLabel'),
		expiredValue: k('componentsUi.signing.expiredValue'),
		sentToTokenContract: k('componentsUi.signing.sendingToTokenContract'),
		// The page, everywhere (2026-10-30): an account that reviews and signs
		// on its trusted page is handed there from every signing sheet.
		handoff: resolveHandoffMessages(locale)
	};
}

/** The multisig screens' copy: every leaf of `home.multisig`, by the shape's keys. */
export function resolveMultisigMessages(locale: Locale): MultisigMessages {
	activate(locale);
	// A leaf shaped `{ locale, forms }` in the shape is a plural key: it ships
	// as this locale's forms, picked at render by count (`pluralForm`).
	const walk = (node: Record<string, unknown>, prefix: string): Record<string, unknown> =>
		Object.fromEntries(
			Object.entries(node).map(([key, value]) => [
				key,
				typeof value === 'string'
					? t(locale, `${prefix}.${key}`)
					: 'forms' in (value as Record<string, unknown>)
						? pluralCopy(locale, `${prefix}.${key}`)
						: walk(value as Record<string, unknown>, `${prefix}.${key}`)
			])
		);
	const words = walk(
		MULTISIG_SHAPE as unknown as Record<string, unknown>,
		'home.multisig'
	) as unknown as MultisigMessages;
	// A member who approves on a trusted signing page sees the apps' hand-off card.
	return {
		...words,
		handoff: resolveHandoffMessages(locale),
		// r18-8 (§28, §32): a test build's refusal, in its own words.
		testBuild: { productionWriteRefused: t(locale, 'common.testBuild.productionWriteRefused') },
		// R22 / r21-1: the held confirm's words and an expired execution's.
		signing: {
			holdToConfirm: t(locale, 'componentsUi.signing.holdToConfirm'),
			confirmAgain: t(locale, 'componentsUi.signing.confirmAgain'),
			notSent: t(locale, 'componentsTx.receipt.statusNotSent'),
			expired: t(locale, 'componentsUi.signing.expired')
		}
	};
}
