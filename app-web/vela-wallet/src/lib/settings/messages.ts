/**
 * Settings message manifest (spec 023).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts` and
 * `contacts/messages.ts`.
 *
 * Most of this copy was already in the corpus before this feature: the
 * `settings.*` namespace has shipped since the React Native app, and
 * `settingsModals.*`, `about.*`, `assets.*`, `home.balanceDetail*`,
 * `componentsUi.bugReport.*` and `componentsUi.treasuryBootstrap.*` all
 * describe screens these mocks redraw. Spec 023 minted 45 new keys and reused
 * roughly two hundred.
 */

export interface SettingsMessages {
	title: string;
	nav: { wallet: string; contacts: string; explore: string; settings: string };
	sections: {
		account: string;
		appearance: string;
		localization: string;
		advanced: string;
	};
	account: { switch: string; contactsSubtitle: string };
	contacts: string;
	feedback: { title: string; subtitle: string };
	appearance: {
		themeTitle: string;
		themeLight: string;
		themeDark: string;
		themeAuto: string;
		avatarTitle: string;
		avatarInitials: string;
		avatarIdenticon: string;
		textScale: string;
	};
	language: {
		title: string;
		pickerTitle: string;
		pickerSubtitle: string;
		followSystem: string;
		contributeNote: string;
		contributeCta: string;
	};
	localization: {
		currencyTitle: string;
		autoExample: string;
		numberTitle: string;
		numberSubtitle: string;
		dateTitle: string;
		dateSubtitle: string;
		timeTitle: string;
		timeSubtitle: string;
	};
	formatNote: { system: string; indian: string; h24: string; h12: string };
	currency: { title: string; searchPlaceholder: string };
	advanced: {
		networksTitle: string;
		networksSubtitle: string;
		rpcProvidersTitle: string;
		rpcProvidersSubtitle: string;
		addNetworkTitle: string;
		addNetworkSubtitle: string;
		endpointsTitle: string;
		endpointsSubtitle: string;
	};
	networks: {
		/** Template with `{{count}}`. */
		count: string;
		custom: string;
		builtinNote: string;
		saveHint: string;
		online: string;
		/** The prefix a slow endpoint's pill wears: "Slower · 1.2s". */
		slow: string;
		offline: string;
		/** settingsModals.health.* — endpoint badge labels (live wiring, 024). */
		httpsRequired: string;
		invalid: string;
		/** Template with `{{chainId}}`. */
		chainId: string;
		/** The delete control on a custom row — "Remove Network" (spec 028 Phase 8). */
		remove: string;
		rpcUrl: string;
		explorer: string;
		mismatch: string;
	};
	addNetwork: {
		description: string;
		searchPlaceholder: string;
		compatible: string;
		incompatible: string;
		compatibilityCheck: string;
		customRpcTitle: string;
		customRpcPlaceholder: string;
		addNetworkBtn: string;
		/** Wizard phase copy (live wiring, spec 024). */
		searching: string;
		checkingCompatibility: string;
		/** Invariant ③: an unanswered probe is "unable to verify", never "incompatible". */
		unableToVerify: string;
		retry: string;
		incompatibleHint: string;
		openChainSetupTool: string;
		recheckWithRpc: string;
		testnet: string;
		/** Template with `{{latencyMs}}`. */
		bestRpc: string;
		checkEntryPoint: string;
		checkSafe: string;
		checkSigner: string;
		/** Template with `{{count}}`. */
		checkRemaining: string;
	};
	rpcProviders: {
		description: string;
		getKey: string;
		checkKey: string;
		notSet: string;
		connected: string;
		/** Template with `{{count}}` and `{{total}}`. */
		supportsCount: string;
		/** Template with `{{ms}}`. */
		avgLatency: string;
	};
	endpoints: {
		description: string;
		chainDataLabel: string;
		chainDataHint: string;
		passkeyLabel: string;
		passkeyHint: string;
		bundlerLabel: string;
		bundlerHint: string;
		fiatLabel: string;
		fiatHint: string;
		reset: string;
		guide: string;
	};
	storage: {
		title: string;
		subtitle: string;
		/** Template with `{{count}}`. */
		summary: string;
		userData: string;
		caches: string;
		connections: string;
		legendUserData: string;
		legendCaches: string;
		legendSessions: string;
		itemTransactions: string;
		itemContacts: string;
		itemCustom: string;
		itemBrowsing: string;
		itemBalances: string;
		itemRates: string;
		itemScan: string;
		itemDapps: string;
		/** Templates with `{{count}}`. */
		records: string;
		contactsCount: string;
		itemsCount: string;
		sitesCount: string;
		clear: string;
		clearAllCaches: string;
		disconnectAll: string;
		/** Singular — one site's own row (spec 027). Lives in the `connect`
		 *  namespace because it was written for the connection panel; one corpus,
		 *  not one per surface. */
		disconnectOne: string;
		clearTitle: string;
		clearBody: string;
		clearConfirm: string;
	};
	about: {
		title: string;
		subtitleTemplate: string;
		tagline: string;
		/** Templates with `{{version}}` / `{{commit}}` / `{{count}}`. */
		version: string;
		sectionTechnical: string;
		techWalletLabel: string;
		techWalletValue: string;
		techAuthLabel: string;
		techAuthValue: string;
		techAccountTypeLabel: string;
		techAccountTypeValue: string;
		techSignerLabel: string;
		techSignerValue: string;
		techNetworksLabel: string;
		techNetworksValue: string;
		linkWebsite: string;
		linkGitHub: string;
		linkSafeWallet: string;
		sectionLinks: string;
		footer: string;
	};
	accounts: {
		title: string;
		/** Template with `{{amount}}`. */
		total: string;
		/** Template with `{{count}}`. */
		countPrefix: string;
		createNew: string;
		signInExisting: string;
	};
	signOut: {
		button: string;
		title: string;
		desc: string;
		keeps: string;
		warning: string;
		anyway: string;
		cancel: string;
	};
	erase: {
		title: string;
		subtitle: string;
		desc: string;
		loses: string;
		keeps: string;
		confirm: string;
		cancel: string;
		/** The erase ran and something survived — said, never swallowed. */
		failed: string;
	};
	bugReport: {
		title: string;
		subtitle: string;
		whatPlaceholder: string;
		addSteps: string;
		previewToggle: string;
		previewVersion: string;
		previewPlatform: string;
		previewLanguage: string;
		previewRpc: string;
		previewFailures: string;
		previewNone: string;
		consent: string;
		send: string;
		openGithubForm: string;
	};
	rescue: {
		/** Templates with `{{name}}` / `{{count}}`. */
		rpcUnavailableSingle: string;
		rpcUnavailableMultiple: string;
		rpcFix: string;
		rpcFixTitle: string;
		rpcFixWarning: string;
		rpcFixLabel: string;
		rpcFixSaveBtn: string;
		rpcFixRestored: string;
		rpcProvidersTitle: string;
		rpcReport: string;
	};
	balanceDetail: {
		title: string;
		/** Template with `{{amount}}`. */
		total: string;
		networksLabel: string;
		networksNote: string;
		statusRetrying: string;
		statusFailed: string;
		updatedLabel: string;
		retry: string;
	};
	relayer: {
		title: string;
		lead: string;
		/** Templates with `{{amount}}` / `{{symbol}}`. */
		amountHint: string;
		addressLabel: string;
		disclaimer: string;
		retryBtn: string;
		copyBtn: string;
	};
	indexDown: {
		title: string;
		subtitle: string;
		warning: string;
		endpointLabel: string;
		editEndpoint: string;
		passkeyHint: string;
	};
	common: {
		cancel: string;
		system: string;
		automatic: string;
		done: string;
		tryAgain: string;
		close: string;
		copyAddress: string;
	};
	shell: {
		networksTitle: string;
		allNetworks: string;
	};
	walletTitle: string;
	sendTitle: string;
}

/**
 * Every corpus key the settings screens consume, in the order the manifest
 * declares them. The parity test iterates this, so a key that stops resolving
 * fails the build rather than shipping a dotted path onto a screen.
 */
export const SETTINGS_KEYS = [
	'settings.title',
	'componentsUi.mainNav.wallet',
	'componentsUi.mainNav.contacts',
	'componentsUi.mainNav.explore',
	'componentsUi.mainNav.settings',
	'settings.sections.account',
	'settings.sections.appearance',
	'settings.sections.localization',
	'settings.sections.advanced',
	'settings.account.switch',
	'settings.account.contactsSubtitle',
	'settings.feedback.title',
	'settings.feedback.subtitle',
	'settings.appearance.themeTitle',
	'settings.appearance.themeLight',
	'settings.appearance.themeDark',
	'settings.appearance.themeAuto',
	'settings.appearance.avatarTitle',
	'settings.appearance.avatarInitials',
	'settings.appearance.avatarIdenticon',
	'settings.appearance.textScale',
	'language.title',
	'language.pickerTitle',
	'language.pickerSubtitle',
	'language.followSystem',
	'language.contributeNote',
	'language.contributeCta',
	'settings.localization.currencyTitle',
	'settings.localization.autoExample',
	'settings.localization.numberTitle',
	'settings.localization.numberSubtitle',
	'settings.localization.dateTitle',
	'settings.localization.dateSubtitle',
	'settings.localization.timeTitle',
	'settings.localization.timeSubtitle',
	'settings.formatNote.system',
	'settings.formatNote.indian',
	'settings.formatNote.h24',
	'settings.formatNote.h12',
	'componentsUi.currency.title',
	'componentsUi.currency.searchPlaceholder',
	'settings.advanced.networksTitle',
	'settings.advanced.networksSubtitle',
	'settings.advanced.rpcProvidersTitle',
	'settings.advanced.rpcProvidersSubtitle',
	'settings.advanced.addNetworkTitle',
	'settings.advanced.addNetworkSubtitle',
	'settings.advanced.endpointsTitle',
	'settings.advanced.endpointsSubtitle',
	'settings.networks.count',
	'settings.networks.custom',
	'settings.networks.builtinNote',
	'settings.networks.saveHint',
	'settings.networks.online',
	'settings.networks.slow',
	'settingsModals.health.offline',
	'settingsModals.network.chainId',
	'settingsModals.network.removeTitle',
	'settingsModals.network.fieldRpcUrl',
	'settingsModals.network.fieldExplorer',
	'settingsModals.network.rpcChainMismatch',
	'settingsModals.addNetwork.description',
	'settingsModals.addNetwork.searchPlaceholder',
	'settingsModals.addNetwork.compatible',
	'settingsModals.addNetwork.incompatible',
	'settingsModals.addNetwork.compatibilityCheck',
	'settingsModals.addNetwork.customRpcTitle',
	'settingsModals.addNetwork.customRpcPlaceholder',
	'settingsModals.addNetwork.addNetworkBtn',
	'settingsModals.addNetwork.incompatibleHint',
	'settingsModals.addNetwork.openChainSetupTool',
	'settingsModals.addNetwork.recheckWithRpc',
	'settingsModals.addNetwork.testnet',
	'settingsModals.addNetwork.bestRpc',
	'settingsModals.addNetwork.checkSafe',
	'settingsModals.addNetwork.checkSigner',
	'settingsModals.addNetwork.checkRemaining',
	'settingsModals.rpcProviders.description',
	'settingsModals.rpcProviders.getKey',
	'settingsModals.rpcProviders.checkKey',
	'settingsModals.rpcProviders.notSet',
	'activity.connected',
	'settingsModals.rpcProviders.supportsCount',
	'settingsModals.rpcProviders.avgLatency',
	'settingsModals.endpoints.description',
	'settingsModals.endpoints.chainDataLabel',
	'settingsModals.endpoints.chainDataHint',
	'settingsModals.endpoints.passkeyLabel',
	'settingsModals.endpoints.passkeyHint',
	'settingsModals.endpoints.bundlerLabel',
	'settingsModals.endpoints.bundlerHint',
	'settingsModals.endpoints.fiatLabel',
	'settingsModals.endpoints.fiatHint',
	'settingsModals.endpoints.resetToDefaults',
	'settingsModals.endpoints.selfHostGuide',
	'settings.storage.title',
	'settings.storage.subtitle',
	'settings.storage.summary',
	'settings.storage.userData',
	'settings.storage.caches',
	'settings.storage.connections',
	'settings.storage.legendUserData',
	'settings.storage.legendCaches',
	'settings.storage.legendSessions',
	'settings.storage.itemTransactions',
	'settings.storage.itemContacts',
	'settings.storage.itemCustom',
	'settings.storage.itemBrowsing',
	'settings.storage.itemBalances',
	'settings.storage.itemRates',
	'settings.storage.itemScan',
	'settings.storage.itemDapps',
	'settings.storage.records',
	'settings.storage.contactsCount',
	'settings.storage.itemsCount',
	'settings.storage.sitesCount',
	'settings.storage.clear',
	'settings.storage.clearAllCaches',
	'settings.storage.disconnectAll',
	'connect.browser.disconnect',
	'settings.storage.clearTitle',
	'settings.storage.clearBody',
	'settings.storage.clearConfirm',
	'settings.about.title',
	'settings.about.subtitle',
	'about.tagline',
	'about.version',
	'about.sectionTechnical',
	'about.techWalletLabel',
	'about.techWalletValue',
	'about.techAuthLabel',
	'about.techAuthValue',
	'about.techAccountTypeLabel',
	'about.techAccountTypeValue',
	'about.techSignerLabel',
	'about.techSignerValue',
	'about.techNetworksLabel',
	'about.techNetworksValue',
	'about.linkWebsite',
	'about.linkGitHub',
	'about.linkSafeWallet',
	'about.sectionLinks',
	'about.footer',
	'settingsModals.account.modalTitle',
	'settingsModals.account.total',
	'home.switcherAccountCount',
	'settingsModals.account.createNew',
	'settingsModals.account.signInExisting',
	'settings.signOut.button',
	'settings.signOut.title',
	'settings.signOut.desc',
	'settings.signOut.keeps',
	'settings.signOut.warning',
	'settings.signOut.anyway',
	'settings.signOut.cancel',
	'settings.eraseDevice.title',
	'settings.eraseDevice.subtitle',
	'settings.eraseDevice.desc',
	'settings.eraseDevice.loses',
	'settings.eraseDevice.keeps',
	'settings.eraseDevice.confirm',
	'settings.eraseDevice.cancel',
	'settings.eraseDevice.failed',
	'componentsUi.bugReport.title',
	'componentsUi.bugReport.subtitle',
	'componentsUi.bugReport.whatPlaceholder',
	'componentsUi.bugReport.addSteps',
	'componentsUi.bugReport.previewToggle',
	'componentsUi.bugReport.previewVersion',
	'componentsUi.bugReport.previewPlatform',
	'componentsUi.bugReport.previewLanguage',
	'componentsUi.bugReport.previewRpc',
	'componentsUi.bugReport.previewFailures',
	'componentsUi.bugReport.previewNone',
	'componentsUi.bugReport.consent',
	'componentsUi.bugReport.send',
	'componentsUi.bugReport.openGithubForm',
	'assets.rpcUnavailableSingle',
	'assets.rpcUnavailableMultiple',
	'assets.rpcFix',
	'assets.rpcFixTitle',
	'assets.rpcFixWarning',
	'assets.rpcFixLabel',
	'assets.rpcFixSaveBtn',
	'assets.rpcFixRestored',
	'assets.rpcProvidersTitle',
	'assets.rpcReport',
	'home.balanceDetailTitle',
	'assets.switcherTotal',
	'home.balanceDetailNetworksLabel',
	'home.balanceDetailNetworksNote',
	'home.balanceDetailStatusRetrying',
	'home.balanceDetailStatusFailed',
	'home.balanceDetailUpdatedLabel',
	'home.balanceDetailRetry',
	'componentsUi.treasuryBootstrap.title',
	'componentsUi.treasuryBootstrap.lead',
	'componentsUi.treasuryBootstrap.amountHint',
	'componentsUi.treasuryBootstrap.addressLabel',
	'componentsUi.treasuryBootstrap.disclaimer',
	'componentsUi.treasuryBootstrap.retryBtn',
	'componentsUi.treasuryBootstrap.copyBtn',
	'settings.indexDown.title',
	'settings.indexDown.subtitle',
	'onboarding.settings.warningText',
	'onboarding.settings.endpointUrlLabel',
	'settings.indexDown.editEndpoint',
	'onboarding.settings.passkeyHint',
	'common.cancel',
	'common.system',
	'common.automatic',
	'common.done',
	'common.tryAgain',
	'componentsUi.identiconViewer.close',
	'componentsUi.identiconViewer.copyAddress',
	'settingsModals.network.modalTitle',
	'componentsUi.networkFilter.allNetworks',
	'componentsUi.dock.send'
] as const;
