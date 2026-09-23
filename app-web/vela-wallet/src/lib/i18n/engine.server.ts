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
import { I18n as WasmI18n } from '../../../../../rust/pkg-web/vela_core.js';
import './wasm-init.server';
import { FALLBACK_LOCALE, type Locale } from './locales';
import { FLOW_KEYS, type FlowMessages, type WelcomeMessages } from './messages';
import type { WalletMessages } from '$lib/wallet/messages';
import type { ContactsMessages } from '$lib/contacts/messages';
import { INTRO_KEYS } from '$lib/intro/slides';
import { WALLET_FLOW_KEYS, type WalletFlowMessages } from '$lib/flows/messages';
import type { ExploreMessages } from '$lib/explore/messages';
import type { RequestMessages } from '$lib/dapp/messages';
import type { SigningMessages } from '$lib/signing/messages';
import type { SettingsMessages } from '$lib/settings/messages';

/** Generated runtime catalogs (gen-i18n.mjs stage 4), one per locale. */
const CATALOGS = import.meta.glob('../../../../../assets/i18n/*.json', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

function catalogBytes(locale: string): Uint8Array {
	const entry = Object.entries(CATALOGS).find(([path]) => path.endsWith(`/${locale}.json`));
	if (!entry) throw new Error(`no generated catalog for locale "${locale}" in assets/i18n/`);
	return new TextEncoder().encode(entry[1]);
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
		alreadyHaveWallet: t(locale, 'onboarding.welcome.alreadyHaveWallet')
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
	return Object.fromEntries(FLOW_KEYS.map((key) => [key, t(locale, key)])) as FlowMessages;
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
			stale: k('home.balanceStale'),
			unpriced: k('home.balanceUnpriced'),
			unreachable: k('onboarding.common.networkBody'),
			noPrice: k('home.balanceDetailNoPrice'),
			a11yHide: k('home.a11yHideBalance'),
			a11yShow: k('home.a11yShowBalance')
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
			today: k('componentsUi.dayGroup.today'),
			yesterday: k('componentsUi.dayGroup.yesterday'),
			toName: k('history.toName'),
			fromName: k('history.fromName'),
			emptyTitle: k('home.emptyNoActivity'),
			emptyCaption: k('home.emptySubtitle')
		},
		assets: {
			emptyTitle: k('assets.emptyTitle'),
			emptyCaption: k('assets.emptySubtext'),
			rpcUnavailableSingle: k('assets.rpcUnavailableSingle'),
			rpcUnavailableMultiple: k('assets.rpcUnavailableMultiple')
		},
		networkFilter: {
			pillAll: k('componentsUi.networkFilter.pillAll'),
			sheetTitle: k('componentsUi.networkFilter.selectChain'),
			allNetworks: k('componentsUi.networkFilter.allNetworks')
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
		nameLabel: k('contacts.nameLabel'),
		namePlaceholder: k('contacts.namePlaceholder'),
		addressPlaceholder: k('contacts.addressPlaceholder'),
		save: k('contacts.save'),
		invalidAddress: k('contacts.invalidAddress'),
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
		receive: k('componentsUi.dock.receive'),
		actionQr: k('contacts.actionQr'),
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
		exportBody: k('contacts.exportBody'),
		groupDeleteBody: k('contacts.groupDeleteBody'),
		groupMembersLabel: k('contacts.groupMembersLabel'),
		groupNoContacts: k('contacts.groupNoContacts'),
		copied: k('componentsUi.identiconViewer.copied'),
		noActivity: k('history.emptyTitle'),
		done: k('common.done'),
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
			advanced: k('settings.sections.advanced')
		},
		account: {
			switch: k('settings.account.switch'),
			contactsSubtitle: k('settings.account.contactsSubtitle')
		},
		contacts: k('componentsUi.mainNav.contacts'),
		feedback: { title: k('settings.feedback.title'), subtitle: k('settings.feedback.subtitle') },
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
			searchPlaceholder: k('componentsUi.currency.searchPlaceholder')
		},
		advanced: {
			networksTitle: k('settings.advanced.networksTitle'),
			networksSubtitle: k('settings.advanced.networksSubtitle'),
			rpcProvidersTitle: k('settings.advanced.rpcProvidersTitle'),
			rpcProvidersSubtitle: k('settings.advanced.rpcProvidersSubtitle'),
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
			// The signing sheet's own names, so the two surfaces cannot drift.
			methods: {
				auto: k('common.automatic'),
				platform: k('onboarding.create.methodPlatformTitle'),
				hybrid: k('onboarding.create.methodHybridTitle'),
				security_key: k('onboarding.create.methodSecurityKeyTitle'),
				clear_signer: k('componentsUi.signing.clearSignerTitle')
			},
			clearSignerBody: k('componentsUi.signing.clearSignerBody'),
			pageTitle: k('settings.signing.pageTitle'),
			pageSubtitle: k('settings.signing.pageSubtitle'),
			pageOfficial: k('settings.signing.pageOfficial'),
			pageInvalid: k('settings.signing.pageInvalid'),
			pageInsecure: k('settings.signing.pageInsecure'),
			pageForeign: k('settings.signing.pageForeign'),
			pageReset: k('settings.signing.pageReset'),
			pageSave: k('settings.signing.pageSave')
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
			mismatch: k('settingsModals.network.rpcChainMismatch')
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
			incompatibleHint: k('settingsModals.addNetwork.incompatibleHint'),
			openChainSetupTool: k('settingsModals.addNetwork.openChainSetupTool'),
			recheckWithRpc: k('settingsModals.addNetwork.recheckWithRpc'),
			testnet: k('settingsModals.addNetwork.testnet'),
			bestRpc: k('settingsModals.addNetwork.bestRpc'),
			// A product name, not prose — the corpus has no key for it and should
			// not: translating "EntryPoint v0.7" would make the checklist lie.
			checkEntryPoint: 'EntryPoint v0.7',
			checkSafe: k('settingsModals.addNetwork.checkSafe'),
			checkSigner: k('settingsModals.addNetwork.checkSigner'),
			checkRemaining: k('settingsModals.addNetwork.checkRemaining')
		},
		rpcProviders: {
			description: k('settingsModals.rpcProviders.description'),
			getKey: k('settingsModals.rpcProviders.getKey'),
			checkKey: k('settingsModals.rpcProviders.checkKey'),
			notSet: k('settingsModals.rpcProviders.notSet'),
			connected: k('activity.connected'),
			supportsCount: k('settingsModals.rpcProviders.supportsCount'),
			avgLatency: k('settingsModals.rpcProviders.avgLatency')
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
			tagline: k('about.tagline'),
			version: k('about.version'),
			sectionTechnical: k('about.sectionTechnical'),
			techWalletLabel: k('about.techWalletLabel'),
			techWalletValue: k('about.techWalletValue'),
			techAuthLabel: k('about.techAuthLabel'),
			techAuthValue: k('about.techAuthValue'),
			techAccountTypeLabel: k('about.techAccountTypeLabel'),
			techAccountTypeValue: k('about.techAccountTypeValue'),
			techSignerLabel: k('about.techSignerLabel'),
			techSignerValue: k('about.techSignerValue'),
			techNetworksLabel: k('about.techNetworksLabel'),
			techNetworksValue: k('about.techNetworksValue'),
			linkWebsite: k('about.linkWebsite'),
			linkGitHub: k('about.linkGitHub'),
			linkSafeWallet: k('about.linkSafeWallet'),
			sectionLinks: k('about.sectionLinks'),
			footer: k('about.footer')
		},
		accounts: {
			title: k('settingsModals.account.modalTitle'),
			total: k('settingsModals.account.total'),
			countPrefix: k('home.switcherAccountCount'),
			createNew: k('settingsModals.account.createNew'),
			signInExisting: k('settingsModals.account.signInExisting'),
			remove: k('settings.account.remove'),
			removeBody: k('settings.account.removeBody')
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
			failed: k('settings.eraseDevice.failed')
		},
		bugReport: {
			title: k('componentsUi.bugReport.title'),
			subtitle: k('componentsUi.bugReport.subtitle'),
			whatPlaceholder: k('componentsUi.bugReport.whatPlaceholder'),
			addSteps: k('componentsUi.bugReport.addSteps'),
			previewToggle: k('componentsUi.bugReport.previewToggle'),
			previewVersion: k('componentsUi.bugReport.previewVersion'),
			previewPlatform: k('componentsUi.bugReport.previewPlatform'),
			previewLanguage: k('componentsUi.bugReport.previewLanguage'),
			previewRpc: k('componentsUi.bugReport.previewRpc'),
			previewFailures: k('componentsUi.bugReport.previewFailures'),
			previewNone: k('componentsUi.bugReport.previewNone'),
			consent: k('componentsUi.bugReport.consent'),
			send: k('componentsUi.bugReport.send'),
			openGithubForm: k('componentsUi.bugReport.openGithubForm')
		},
		rescue: {
			rpcUnavailableSingle: k('assets.rpcUnavailableSingle'),
			rpcUnavailableMultiple: k('assets.rpcUnavailableMultiple'),
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
			updatedLabel: k('home.balanceDetailUpdatedLabel'),
			retry: k('home.balanceDetailRetry')
		},
		backup: {
			title: k('settingsModals.backup.title'),
			backedUp: k('settingsModals.backup.backedUp'),
			notBackedUp: k('settingsModals.backup.notBackedUp'),
			couldNotCheck: k('settingsModals.backup.couldNotCheck'),
			checking: k('componentsUi.funding.checking'),
			explain: k('settingsModals.backup.explain')
		},
		keys: {
			title: k('settingsModals.keys.title'),
			subtitle: k('settingsModals.keys.subtitle'),
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
			customLead: k('componentsUi.treasuryBootstrap.customLead')
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
		preparing: k('connect.browser.preparing')
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
		groupOptions: k('explore.groupOptions'),
		manageGroups: k('explore.manageGroups'),
		newGroup: k('explore.newGroup'),
		rename: k('explore.rename'),
		hide: k('explore.hide'),
		show: k('explore.show'),
		delete: k('explore.delete'),
		moveToGroup: k('explore.moveToGroup'),
		openInNewTab: k('explore.openInNewTab'),
		removeFromFavorites: k('explore.removeFromFavorites'),
		systemGroup: k('explore.systemGroup'),
		hiddenTag: k('explore.hiddenTag'),
		siteCount: k('explore.siteCount'),
		hiddenCount: k('explore.hiddenCount'),
		tabs: k('explore.tabs'),
		newTab: k('explore.newTab'),
		startPage: k('explore.startPage'),
		closeAllTabs: k('explore.closeAllTabs'),
		closeTab: k('explore.closeTab'),
		openTabs: k('explore.openTabs'),
		addToFavorites: k('explore.addToFavorites'),
		addedToFavorites: k('explore.addedToFavorites'),
		share: k('explore.share'),
		copyLink: k('explore.copyLink'),
		refresh: k('explore.refresh'),
		openInSystemBrowser: k('explore.openInSystemBrowser'),
		disconnect: k('explore.disconnect'),
		closePage: k('explore.closePage'),
		secureSite: k('explore.secureSite'),
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
export function resolveSigningMessages(locale: Locale): SigningMessages {
	activate(locale);
	const k = (key: string) => t(locale, key);
	return {
		panelTitle: k('componentsUi.signing.signatureRequest'),
		signingAccount: k('componentsUi.signing.signingAccount'),
		advancedToggle: k('componentsUi.signing.advancedToggle'),
		close: k('componentsUi.signing.close'),
		slideToConfirm: k('componentsUi.signing.slideToConfirm'),
		slideConfirmAction: k('componentsUi.signing.slideConfirmAction'),
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
		invalidAmount: k('componentsUi.signingApprove.invalidAmount'),
		choosePrompt: k('componentsUi.signingApprove.choosePrompt'),
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
		warnWillFail: k('componentsUi.signing.simWillFail'),
		warnHexMessage: k('componentsUi.signing.hexMessageWarning'),
		warnBlindTyped: k('componentsUi.signing.blindTypedWarning'),
		warnEthSign: k('componentsUi.signing.ethSignWarning'),
		bodyEthSign: k('componentsUi.signing.ethSignBody'),
		warnSiweMismatch: k('componentsUi.signing.siweMismatch'),
		okSiwe: k('componentsUi.signing.siweOk'),
		warnTokenToContract: k('componentsUi.signing.tokenToContractWarning'),
		warnUnverifiedAmount: k('componentsUi.signing.unverifiedWarning'),
		warnApproveAll: k('componentsUi.signingApprove.setApprovalAllWarn'),
		warnPermitCantCap: k('componentsUi.signingApprove.permitCantCap'),
		warnBestEffort: k('componentsUi.signing.bestEffortWarning'),
		warnVerifiedAbi: k('componentsUi.signing.verifiedAbiWarning'),
		warnSimUnavailable: k('componentsUi.signing.simUnavailableWarning'),
		warnDrain: k('componentsUi.signing.drainWarning'),
		okSelfTransfer: k('componentsUi.signing.balanceSelfTransfer'),
		okNoNetworkFee: k('componentsUi.signing.noNetworkFee'),
		balancesTitle: k('componentsUi.signing.balanceChangesTitle'),
		balancesMatchHero: k('componentsUi.signing.balanceMatchesHero'),
		balancesBlindSimulated: k('componentsUi.signing.blindButSimulated'),
		balancesBestEffort: k('componentsUi.signing.bestEffortSimulated'),
		feeLabel: k('componentsUi.gas.networkFee'),
		feeEstimating: k('componentsUi.gas.estimating'),
		feeRetry: k('componentsUi.gas.estimateFailed'),
		feeTokenTitle: k('componentsUi.signing.feeTokenTitle'),
		feeShort: k('send.warnInsufficientGas'),
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
			},
			hints: {
				fast: k('send.gasTierHintFast'),
				standard: k('send.gasTierHintStandard'),
				slow: k('send.gasTierHintSlow')
			}
		},
		feeEstimated: k('componentsUi.signing.feeEstimated'),
		feeBalance: k('componentsUi.gas.rowBalance'),
		techFunction: k('componentsUi.signing.techFunction'),
		techParam: k('componentsUi.signing.techParam'),
		techRawUnits: k('componentsUi.signing.techRawUnits'),
		techRawData: k('componentsUi.signing.techRawData'),
		techSimResult: k('componentsUi.signing.simResultLabel'),
		techIdentityToken: k('componentsUi.signing.techIdentityToken'),
		techIdentityRecipient: k('componentsUi.signing.techIdentityRecipient'),
		copyValue: k('componentsUi.signing.copyValue'),
		backupIntent: k('settingsModals.backup.intent'),
		backupRegisteredAs: k('settingsModals.backup.registeredAs'),
		backupAddress: k('contacts.addressLabel'),
		backupPublicKeys: k('settingsModals.backup.publicKeys'),
		signWithLabel: k('componentsUi.signing.signWith'),
		signWithAuto: k('common.automatic'),
		// Titles only: the create flow's descriptions say "create it on…".
		signWithPlatform: k('onboarding.create.methodPlatformTitle'),
		signWithHybrid: k('onboarding.create.methodHybridTitle'),
		signWithSecurityKey: k('onboarding.create.methodSecurityKeyTitle'),
		viewOnExplorer: k('componentsUi.signing.viewOnExplorer'),
		byteSize: k('componentsUi.signing.byteSize'),
		safeInnerCall: k('componentsUi.signing.safeInnerCall'),
		batchStep: k('componentsUi.signing.batchStep'),
		expiredValue: k('componentsUi.signing.expiredValue'),
		sentToTokenContract: k('componentsUi.signing.sendingToTokenContract')
	};
}
