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
		/** The official X / Telegram / Discord links (founder, 2026-09-27). */
		community: string;
	};
	account: { switch: string; contactsSubtitle: string };
	contacts: string;
	feedback: { title: string; subtitle: string };
	/** "Share anonymous usage statistics" — the web wallet's and extension's switch. */
	analytics: { title: string; subtitle: string };
	appearance: {
		themeTitle: string;
		themeLight: string;
		themeDark: string;
		themeAuto: string;
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
		/** Spec 068: the stored default transaction speed. */
		feeSpeedTitle: string;
		feeSpeedSubtitle: string;
	};
	/**
	 * The speed sheet (spec 068). The option NAMES are the send flow's own
	 * (`send.gasTier.*`) — one set of words for a tier, so Settings and the
	 * send form can never call the same speed two different things. `rapid` is
	 * not among them: it is a dead variant the bundler refuses.
	 */
	feeSpeed: {
		title: string;
		subtitle: string;
		fast: string;
		standard: string;
		slow: string;
		/**
		 * One line under each name, saying what that speed buys. The heading
		 * over this sheet asks about SPEED, so a name that answered "cheap"
		 * instead would not be answering the question — the advantage belongs
		 * here (spec 068, the owner's ruling).
		 */
		fastHint: string;
		standardHint: string;
		slowHint: string;
	};
	/**
	 * Spec 102: Settings → Signing pages — the pages this device trusts to show
	 * and sign requests. (Spec 071's single "Trusted Signer page" field, and
	 * the Trusted Signer as a fourth place a key lives, are gone.)
	 */
	signing: {
		title: string;
		subtitle: string;
		pageOfficial: string;
		/** Why an address was not added: not an address / not https / already saved. */
		pageInvalid: string;
		pageInsecure: string;
		pageDuplicate: string;
		pageAdd: string;
		pageSave: string;
		/** `Keys on {{domain}}` — the domain whose keys a page (or an account) uses. */
		keysOn: string;
		/** `Self-hosted · {{domain}}` — a saved page the person has not named. */
		pageSelfHosted: string;
		/** A saved page's own actions, in the signing pages' own words. */
		pageRename: string;
		pageRemove: string;
		/** The answer to a self-hosted page's "trust this version?" (D-15). */
		pageTrust: string;
	};
	/** Spec 102: the account's "Where you review and sign". */
	venue: {
		title: string;
		subtitle: string;
		inVela: string;
		inVelaBody: string;
		page: string;
		pageBody: string;
		/**
		 * Why a venue cannot be used, by the corpus key the core's
		 * `VenueBlock::key()` names (`VENUE_BLOCK_KEYS`); the core supplies
		 * the values that fill it (`venueBlockLine`).
		 */
		blocked: Record<VenueBlockKey, string>;
	};
	/**
	 * Spec 102 R6: a signing page's integrity line — `{{version}}`, `{{time}}`.
	 * Keyed by the core's `IntegrityState`, so no shell maps a state itself.
	 */
	integrity: Record<
		| 'checking'
		| 'matches'
		| 'trusted'
		| 'unchecked'
		| 'mismatch'
		| 'blocked'
		| 'askTrust'
		| 'couldNotCheck'
		| 'noVersion'
		| 'allBlocked',
		string
	>;
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
		/** What removing one asks first (spec 072). */
		removeBody: string;
		removeConfirm: string;
		removeCancel: string;
		rpcUrl: string;
		explorer: string;
		mismatch: string;
		/** Spec 098 §5.1: the relay is sent this RPC, key and all. */
		relayNotice: string;
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
		/**
		 * The line under a refused network's check, BY CORPUS KEY. The core
		 * says which one (`NetCompatibility.hint_key`): no P-256 verifier —
		 * the network cannot run Vela wallets and money sent there would be
		 * stuck — or contracts that are missing and can be deployed. The two
		 * have opposite next steps, so the shell never picks between them.
		 */
		hints: Record<NetHintKey, string>;
		/**
		 * The sentence for a wizard that STOPPED, BY CORPUS KEY — the core
		 * names which (`NetWizardView.error_key`, PR 3 notes 5, 10 and 18):
		 * already added, chain not found, no RPC endpoint listed, a check that
		 * could not be made, a refusal with no reason kept. A refusal that kept
		 * its check names one of `hints` instead. No shell words a stop itself:
		 * three of these read "Incompatible" here until the core said them.
		 */
		stops: Record<NetStopKey, string>;
		/**
		 * Spec 081 FR-009: the chain works, and a wallet with more than one
		 * passkey still cannot be created on it. Said beside a green
		 * "Compatible", which without this reads as a contradiction of the two
		 * crossed rows in the checklist.
		 */
		singleKeyOnly: string;
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
		/** Spec 098 §5.1: the relay is sent the RPC the wallet uses, key and all. */
		relayNotice: string;
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
		/** Spec 072 (FR-010): resetting is destructive and asks first. */
		resetTitle: string;
		resetBody: string;
		resetConfirm: string;
		resetCancel: string;
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
		linkPrivacy: string;
		linkTerms: string;
		linkSupport: string;
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
		/** Taking ONE wallet off this device (2026-09-23). */
		remove: string;
		removeBody: string;
	};
	signOut: {
		button: string;
		title: string;
		desc: string;
		/** Template with `{{count}}` — what a sign-out takes when it is more
		 *  than one wallet. */
		descMany: string;
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
		stepsPlaceholder: string;
		previewToggle: string;
		previewVersion: string;
		previewPlatform: string;
		previewLanguage: string;
		previewRpc: string;
		previewFailures: string;
		previewNone: string;
		consent: string;
		send: string;
		/** Spec 081: the button while the endpoint is being asked. */
		sending: string;
		/** Filed — `successBody*` carry `{{number}}`. */
		successTitle: string;
		successBodyNew: string;
		successBodyDeduped: string;
		viewIssue: string;
		/** The endpoint could not file it; the prefilled form still can. */
		fallbackTitle: string;
		fallbackBody: string;
		openGithub: string;
		openGithubForm: string;
		/** Screenshots (078 round 3). `{{max}}` = 5, `{{index}}` 1-based. */
		screenshotsLabel: string;
		addScreenshots: string;
		screenshotsHint: string;
		screenshotsPublic: string;
		removeScreenshot: string;
		screenshotsLimit: string;
		screenshotUnsupported: string;
		/** Web only: the section is a drop target and paste adds images. */
		dropHint: string;
		/** Filed, but some images could not be stored. */
		screenshotsDropped: string;
		/** Fell back: the GitHub form cannot carry the images. */
		fallbackScreenshots: string;
		done: string;
		/**
		 * The screenshot viewer (078 §C): a tile's a11y label (`{{index}}`,
		 * 1-based), the viewer's ✕, and its visible remove button.
		 */
		viewScreenshot: string;
		closeViewer: string;
		removeFromViewer: string;
	};
	rescue: {
		/**
		 * Spec 092: the line over the networks the wallet cannot reach — the
		 * home's, reused as the list's title. `{{name}}` / `{{n}}`.
		 */
		unreachableOne: string;
		unreachableMany: string;
		/**
		 * PR 3 note 4: the one network whose token list could not be loaded
		 * (`{{name}}`) — the home's line and the list's title then, and the
		 * status of that network's row in the balance breakdown.
		 */
		tokenListUnreachable: string;
		/** Under the title: what is there is unaffected, only unread. */
		unreachableBody: string;
		/** The title once every network in the list has come back. */
		unreachableNone: string;
		/**
		 * PR 2 note 11: the home's line when the read failed inside Vela
		 * (`BalanceView.internal_key`) — the list's title then too.
		 */
		internal: Readonly<Record<string, string>>;
		/**
		 * Each row's line, by the corpus key the core names in `line_key`
		 * ({@link UNREACHABLE_LINE_KEYS}); `assets.lastSeen` fills `{{amount}}`.
		 */
		lines: Record<string, string>;
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
	/** The Ethereum backup row (spec 062): the founding record's standing there. */
	backup: {
		/**
		 * The row's words BY CORPUS KEY. The core names the key for each state
		 * (`registry_backup::BackupRow`: its title, its second line); the shell
		 * only looks the key up, so which words a state says is never decided
		 * here. Every key the core can name is in `BACKUP_ROW_KEYS`.
		 */
		words: Record<BackupRowKey, string>;
		/**
		 * What the copy makes public — the wallet's name, and each key's name,
		 * public key, credential ID and authenticator model — and that it costs
		 * a network fee.
		 */
		explain: string;
	};
	/** The keys that control this wallet (spec 062). */
	keys: {
		title: string;
		subtitle: string;
		/** `Key {{n}}` — a key nobody named. */
		keyN: string;
		synced: string;
		notSynced: string;
		/** The registry did not answer; the rows are the device's memory. */
		fromDevice: string;
		providerPlatform: string;
		providerGeneric: string;
		providerSecurityKey: string;
		/** The labels of what a key row opens onto — the registry explorer's facts. */
		publicKey: string;
		credential: string;
		transport: string;
		attestation: string;
		userVerified: string;
		/** The key this device signs with — the one it signed in with. */
		signsHere: string;
		copy: string;
		copied: string;
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
		/** Shown when the OPERATOR runs this network's relayer (spec 060). */
		operatorLead: string;
		reportBtn: string;
		selfFundToggle: string;
		/** Shown when the person added the network themselves. */
		customLead: string;
		/** `{{balance}}` / `{{floor}}` / `{{symbol}}` — what it has against what it needs (098 §4). */
		balanceLine: string;
		/** The sheet asks again on its own and closes once funded (098 §4). */
		watching: string;
		/** Under the code: send from another wallet. */
		qrLabel: string;
	};
	/** The relay cannot serve this chain at all (spec 098 §2). */
	relayUnreachable: {
		title: string;
		operatorLead: string;
		customLead: string;
		settingsHint: string;
		reportBtn: string;
		retryBtn: string;
		closeBtn: string;
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
 * The corpus keys an unreachable network's row can be told with — every
 * `line_key` `balance_dashboard` names (spec 092). A key the core adds later
 * and this list lacks draws no line.
 */
export const UNREACHABLE_LINE_KEYS = [
	'assets.lastSeen',
	'assets.lastSeenUnpriced',
	'assets.lastSeenEmpty',
	'assets.notReadYet'
] as const;

/**
 * The lines a venue refusal can say (`VenueBlock::key()`): R1's two —
 * `blockedApp {{domain}}`, `blockedPage {{pageDomain}} {{domain}}` — and the
 * web's `blockedWeb` (no vars: it opens no signing page, D-16). Only what a
 * build-time manifest must resolve; which line a refusal says, and with
 * which values, is the core's (`venueBlockLine`). `venue.test.ts` holds this
 * list to the core.
 */
export const VENUE_BLOCK_KEYS = [
	'settings.venue.blockedApp',
	'settings.venue.blockedPage',
	'settings.venue.blockedWeb'
] as const;

export type VenueBlockKey = (typeof VENUE_BLOCK_KEYS)[number];

/**
 * The lines a refused network's check can say — every `hint_key` the core's
 * `NetCompatibility` names (`network_admin`: `NO_P256_HINT`,
 * `MISSING_CONTRACTS_HINT`). Settings' wizard and a dApp's add-network sheet
 * both resolve these; `net-refusal.test.ts` holds the list to the core's
 * source.
 */
export const NET_HINT_KEYS = [
	'settingsModals.addNetwork.noP256Hint',
	'settingsModals.addNetwork.incompatibleHint'
] as const;

export type NetHintKey = (typeof NET_HINT_KEYS)[number];

/**
 * The sentences a stopped wizard can say that are not a check's own reason —
 * every other key `network_admin`'s `wizard_error_key` answers with
 * (`WIZARD_ALREADY_ADDED`, `WIZARD_NOT_FOUND`, `WIZARD_NO_RPC_ENDPOINT`,
 * `WIZARD_CHECK_FAILED`, `WIZARD_NOT_COMPATIBLE`). Together with
 * {@link NET_HINT_KEYS} this is everything `NetWizardView.error_key` can be;
 * `net-refusal.test.ts` holds both lists to the core's source.
 */
export const NET_STOP_KEYS = [
	'addToken.errorAlreadyAdded',
	'addToken.errorChainNotFound',
	'settingsModals.addNetwork.noRpcEndpoint',
	'settingsModals.addNetwork.unableToVerify',
	'addToken.errorNotCompatible'
] as const;

export type NetStopKey = (typeof NET_STOP_KEYS)[number];

/**
 * Every corpus key the core's backup row can name
 * (`vela_core::registry_backup`: `TITLE_KEY`, `CHECKING_KEY` and each
 * `BackupState::row()` second line). `ethereum-backup-row.test.ts` reads the
 * core's source and fails if it names one that is not here.
 */
export const BACKUP_ROW_KEYS = [
	'settingsModals.backup.title',
	'settingsModals.backup.backedUp',
	'settingsModals.backup.notBackedUp',
	'settingsModals.backup.couldNotCheck',
	'settingsModals.backup.cannotCopy',
	'componentsUi.funding.checking'
] as const;

export type BackupRowKey = (typeof BACKUP_ROW_KEYS)[number];

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
	'settings.sections.community',
	'settings.account.switch',
	'settings.account.contactsSubtitle',
	'settings.feedback.title',
	'settings.feedback.subtitle',
	'settings.analytics.title',
	'settings.analytics.subtitle',
	'settings.appearance.themeTitle',
	'settings.appearance.themeLight',
	'settings.appearance.themeDark',
	'settings.appearance.themeAuto',
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
	'settings.advanced.feeSpeedTitle',
	'settings.advanced.feeSpeedSubtitle',
	'settings.feeSpeed.title',
	'settings.feeSpeed.subtitle',
	'send.gasTier.fast',
	'send.gasTier.standard',
	'send.gasTier.slow',
	'send.gasTierHintFast',
	'send.gasTierHintStandard',
	'send.gasTierHintSlow',
	'settings.signing.title',
	'settings.signing.subtitle',
	'settings.signing.pageOfficial',
	'settings.signing.pageInvalid',
	'settings.signing.pageInsecure',
	'settings.signing.pageDuplicate',
	'settings.signing.pageAdd',
	'settings.signing.pageSave',
	'settings.signing.keysOn',
	'settings.signing.pageSelfHosted',
	'settings.signing.pageRename',
	'settings.signing.pageRemove',
	'settings.signing.pageTrust',
	'settings.venue.title',
	'settings.venue.subtitle',
	'settings.venue.inVela',
	'settings.venue.inVelaBody',
	'settings.venue.page',
	'settings.venue.pageBody',
	...VENUE_BLOCK_KEYS,
	'componentsUi.signing.integrity.checking',
	'componentsUi.signing.integrity.matches',
	'componentsUi.signing.integrity.trusted',
	'componentsUi.signing.integrity.unchecked',
	'componentsUi.signing.integrity.mismatch',
	'componentsUi.signing.integrity.blocked',
	'componentsUi.signing.integrity.askTrust',
	'componentsUi.signing.integrity.couldNotCheck',
	'componentsUi.signing.integrity.noVersion',
	'componentsUi.signing.integrity.allBlocked',
	'settings.networks.count',
	'settings.networks.custom',
	'settings.networks.builtinNote',
	'settings.networks.saveHint',
	'settings.networks.online',
	'settings.networks.slow',
	'settingsModals.health.offline',
	'settingsModals.backup.title',
	'settingsModals.backup.backedUp',
	'settingsModals.backup.notBackedUp',
	'settingsModals.backup.couldNotCheck',
	'settingsModals.backup.cannotCopy',
	'componentsUi.funding.checking',
	'settingsModals.keys.title',
	'settingsModals.keys.subtitle',
	'settingsModals.keys.keyN',
	'settingsModals.keys.notSynced',
	'settingsModals.keys.fromDevice',
	'settingsModals.keys.publicKey',
	'settingsModals.keys.credential',
	'settingsModals.keys.transport',
	'settingsModals.keys.attestation',
	'settingsModals.keys.userVerified',
	'settingsModals.keys.signsHere',
	'settingsModals.backup.explain',
	'componentsUi.signing.copyValue',
	'receive.copied',
	'onboarding.create.keySyncedBadge',
	'onboarding.create.providerPlatform',
	'onboarding.create.providerGeneric',
	'onboarding.create.providerSecurityKey',
	'settingsModals.network.chainId',
	'settingsModals.network.removeTitle',
	'settingsModals.network.removeBody',
	'settingsModals.network.removeConfirm',
	'settingsModals.network.removeCancel',
	'settingsModals.network.fieldRpcUrl',
	'settingsModals.network.relayNotice',
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
	'settingsModals.addNetwork.noP256Hint',
	// A stopped wizard's own sentences (`NetWizardView.error_key`).
	'addToken.errorAlreadyAdded',
	'addToken.errorChainNotFound',
	'addToken.errorNotCompatible',
	'settingsModals.addNetwork.noRpcEndpoint',
	'settingsModals.addNetwork.singleKeyOnly',
	'settingsModals.addNetwork.openChainSetupTool',
	'settingsModals.addNetwork.recheckWithRpc',
	'settingsModals.addNetwork.testnet',
	'settingsModals.addNetwork.bestRpc',
	'settingsModals.addNetwork.checkSafe',
	'settingsModals.addNetwork.checkSigner',
	'settingsModals.addNetwork.checkRemaining',
	'settingsModals.rpcProviders.description',
	'settingsModals.rpcProviders.relayNotice',
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
	'settingsModals.endpoints.resetTitle',
	'settingsModals.endpoints.resetBody',
	'settingsModals.endpoints.resetConfirm',
	'settingsModals.endpoints.resetCancel',
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
	'about.linkPrivacy',
	'about.linkTerms',
	'about.linkSupport',
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
	'componentsUi.bugReport.stepsPlaceholder',
	'componentsUi.bugReport.previewToggle',
	'componentsUi.bugReport.previewVersion',
	'componentsUi.bugReport.previewPlatform',
	'componentsUi.bugReport.previewLanguage',
	'componentsUi.bugReport.previewRpc',
	'componentsUi.bugReport.previewFailures',
	'componentsUi.bugReport.previewNone',
	'componentsUi.bugReport.consent',
	'componentsUi.bugReport.send',
	'componentsUi.bugReport.sending',
	'componentsUi.bugReport.successTitle',
	'componentsUi.bugReport.successBodyNew',
	'componentsUi.bugReport.successBodyDeduped',
	'componentsUi.bugReport.viewIssue',
	'componentsUi.bugReport.fallbackTitle',
	'componentsUi.bugReport.fallbackBody',
	'componentsUi.bugReport.openGithub',
	'componentsUi.bugReport.openGithubForm',
	'componentsUi.bugReport.screenshotsLabel',
	'componentsUi.bugReport.addScreenshots',
	'componentsUi.bugReport.screenshotsHint',
	'componentsUi.bugReport.screenshotsPublic',
	'componentsUi.bugReport.removeScreenshot',
	'componentsUi.bugReport.screenshotsLimit',
	'componentsUi.bugReport.screenshotUnsupported',
	'componentsUi.bugReport.dropHint',
	'componentsUi.bugReport.screenshotsDropped',
	'componentsUi.bugReport.fallbackScreenshots',
	'componentsUi.bugReport.done',
	'componentsUi.bugReport.viewScreenshot',
	'componentsUi.bugReport.closeViewer',
	'componentsUi.bugReport.removeFromViewer',
	'assets.unreachableOne',
	'assets.unreachableMany',
	'assets.tokenListUnreachable',
	'assets.unreachableBody',
	'assets.unreachableNone',
	// PR 2 note 11: the list's title when the read failed inside Vela.
	'componentsUi.gas.reasonInternal',
	'assets.lastSeen',
	'assets.lastSeenUnpriced',
	'assets.lastSeenEmpty',
	'assets.notReadYet',
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
	'componentsUi.treasuryBootstrap.operatorLead',
	'componentsUi.treasuryBootstrap.reportBtn',
	'componentsUi.treasuryBootstrap.selfFundToggle',
	'componentsUi.treasuryBootstrap.customLead',
	'componentsUi.treasuryBootstrap.copyBtn',
	'componentsUi.treasuryBootstrap.balanceLine',
	'componentsUi.treasuryBootstrap.watching',
	'componentsUi.treasuryBootstrap.qrLabel',
	'componentsUi.relayUnreachable.title',
	'componentsUi.relayUnreachable.operatorLead',
	'componentsUi.relayUnreachable.customLead',
	'componentsUi.relayUnreachable.settingsHint',
	'componentsUi.relayUnreachable.reportBtn',
	'componentsUi.relayUnreachable.retryBtn',
	'componentsUi.relayUnreachable.closeBtn',
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
