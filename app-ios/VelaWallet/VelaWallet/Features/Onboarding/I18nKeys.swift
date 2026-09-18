//
//  I18nKeys.swift
//  VelaWallet
//
//  Corpus key paths used by the onboarding surface (spec 019 T137).
//
//  iOS was the one client with no centralised key file: `FlowStates.swift` and
//  `FlowSheet.swift` carried inline `"onboarding.create.…"` literals, which is
//  tolerable while a screen has six of them and stops being tolerable at the
//  ~90 this feature needs. Two properties are what the file buys:
//
//  - **A key typo is a compile error rather than a screen that renders its own
//    key path at a person.** The engine's missing-key behaviour is to echo the
//    key, which is the right failure signal and a terrible thing to ship.
//  - **The i18n audit can find every key this app uses** by reading one file,
//    which is what `audit-literals.mjs` checks and what makes "does the corpus
//    still cover iOS?" a question with an answer.
//

enum I18nKeys {

    enum Welcome {
        static let tagline = "onboarding.welcome.desktopTagline"
        static let createWallet = "onboarding.welcome.createWallet"
        static let alreadyHaveWallet = "onboarding.welcome.alreadyHaveWallet"
    }

    enum Create {
        static let header = "onboarding.create.headerDefault"
        static let headerCreated = "onboarding.create.headerCreated"
        static let headerSyncFailed = "onboarding.create.headerSyncFailed"

        // Name screen. THREE acknowledgements, each a fact about where something
        // ends up: the public key and the name go on-chain, the private key
        // stays in the device or on a security key, and the legal assent. The
        // legal line's fragments are named for the row they render on — `ack2*`
        // — because a fragment key that disagrees with its index is how the
        // earlier ack3 -> ack1 confusion started.
        static let nameTitle = "onboarding.create.nameTitle"
        static let accountNamePlaceholder = "onboarding.create.accountNamePlaceholder"
        static let nameTooLong = "onboarding.create.nameTooLong"
        static let ack0 = "onboarding.create.ack0"
        static let ack1 = "onboarding.create.ack1"
        static let ack2 = "onboarding.create.ack2"
        static let ack2PrivacyPolicy = "onboarding.create.ack2PrivacyPolicy"
        static let ack2And = "onboarding.create.ack2And"
        static let ack2Terms = "onboarding.create.ack2Terms"
        static let ack2Period = "onboarding.create.ack2Period"
        static let nextBtn = "onboarding.create.nextBtn"
        static let createWalletBtn = "onboarding.create.createWalletBtn"
        static let finishVerifyBtn = "onboarding.create.finishVerifyBtn"
        static let startOverBtn = "onboarding.create.startOverBtn"

        // Statuses.
        static let statusSettingUpIdentity = "onboarding.create.statusSettingUpIdentity"
        static let statusVerifyingIdentity = "onboarding.create.statusVerifyingIdentity"
        static let statusExtractingKey = "onboarding.create.statusExtractingKey"
        static let statusComputingAddress = "onboarding.create.statusComputingAddress"
        static let statusSyncingKey = "onboarding.create.statusSyncingKey"
        static let statusSetupCancelled = "onboarding.create.statusSetupCancelled"
        static let statusVerifyCancelled = "onboarding.create.statusVerifyCancelled"

        // Keys screen.
        static let keysTitle = "onboarding.create.keysTitle"
        static let keysTitleBlocked = "onboarding.create.keysTitleBlocked"
        static let keysSubtitle = "onboarding.create.keysSubtitle"
        static let keysSubtitleBlocked = "onboarding.create.keysSubtitleBlocked"
        static let keysSubtitleFull = "onboarding.create.keysSubtitleFull"
        static let keysLabel = "onboarding.create.keysLabel"
        static let keysHint = "onboarding.create.keysHint"
        static let keyCount = "onboarding.create.keyCount"
        static let keySyncedBadge = "onboarding.create.keySyncedBadge"
        static let keyDeviceOnlyBadge = "onboarding.create.keyDeviceOnlyBadge"
        static let keyLimitReached = "onboarding.create.keyLimitReached"
        static let needSecondKeyHint = "onboarding.create.needSecondKeyHint"
        static let addKeyBtn = "onboarding.create.addKeyBtn"
        static let addSecondKeyBtn = "onboarding.create.addSecondKeyBtn"
        static let confirmKeyBtn = "onboarding.create.confirmKeyBtn"
        static let removeKeyBtn = "onboarding.create.removeKeyBtn"
        static let addMethodLabel = "onboarding.create.addMethodLabel"
        static let methodPlatformTitle = "onboarding.create.methodPlatformTitle"
        static let methodPlatformBody = "onboarding.create.methodPlatformBody"
        static let methodHybridTitle = "onboarding.create.methodHybridTitle"
        static let methodHybridBody = "onboarding.create.methodHybridBody"
        static let methodHybridUnavailable = "onboarding.create.methodHybridUnavailable"
        static let methodSecurityKeyTitle = "onboarding.create.methodSecurityKeyTitle"
        static let methodSecurityKeyBody = "onboarding.create.methodSecurityKeyBody"
        static let providerPlatform = "onboarding.create.providerPlatform"
        static let providerGeneric = "onboarding.create.providerGeneric"
        static let providerSecurityKey = "onboarding.create.providerSecurityKey"

        // Progress screen.
        static let progressTitle = "onboarding.create.progressTitle"
        static let progressSubtitle = "onboarding.create.progressSubtitle"
        static let progressMeterLabel = "onboarding.create.progressMeterLabel"
        static let taskVerifyKey = "onboarding.create.taskVerifyKey"
        static let taskDeriveAddress = "onboarding.create.taskDeriveAddress"
        static let taskWriteIndex = "onboarding.create.taskWriteIndex"

        // Retry screen.
        static let syncFailedTitle = "onboarding.create.syncFailedTitle"
        static let syncFailedMessage = "onboarding.create.syncFailedMessage"
        static let syncFailedHint = "onboarding.create.syncFailedHint"
        static let retryUploadBtn = "onboarding.create.retryUploadBtn"
        static let technicalDetails = "onboarding.create.technicalDetails"

        // Done screen.
        static let successTitle = "onboarding.create.successTitle"
        static let successMessage = "onboarding.create.successMessage"
        static let identiconHint = "onboarding.create.identiconHint"
        static let walletAddressLabel = "onboarding.create.walletAddressLabel"
        static let verifyHint = "onboarding.create.verifyHint"
        static let enterWalletBtn = "onboarding.create.enterWalletBtn"

        // Prompts (data-model §5).
        static let alertErrorTitle = "onboarding.create.alertErrorTitle"
        static let alertNotSupportedTitle = "onboarding.create.alertNotSupportedTitle"
        static let alertNotSupportedBody = "onboarding.create.alertNotSupportedBody"
        // The app-owned CTAP path's own dialogs (spec 019 §5), shared with the desktop.
        static let pinTitle = "onboarding.create.pinTitle"
        static let pinBody = "onboarding.create.pinBody"
        static let pinLabel = "onboarding.create.pinLabel"
        static let pinAttemptsLeft = "onboarding.create.pinAttemptsLeft"
        static let pinRejected = "onboarding.create.pinRejected"
        static let touchTitle = "onboarding.create.touchTitle"
        static let touchBody = "onboarding.create.touchBody"
        static let touchFingerprintBody = "onboarding.create.touchFingerprintBody"
        static let touchSelectBody = "onboarding.create.touchSelectBody"
    }

    enum Login {
        static let header = "onboarding.login.header"
        static let alertNotSupportedTitle = "onboarding.login.alertNotSupportedTitle"
        static let alertNotSupportedBody = "onboarding.login.alertNotSupportedBody"
        static let alertIncompatibleTitle = "onboarding.login.alertIncompatibleTitle"
        static let alertIncompatibleBody = "onboarding.login.alertIncompatibleBody"
        static let alertIncompatibleBodyCreate = "onboarding.login.alertIncompatibleBodyCreate"
        static let alertSignInFailedTitle = "onboarding.login.alertSignInFailedTitle"
        static let alertSignInFailedBody = "onboarding.login.alertSignInFailedBody"
        static let recoverOfferTitle = "onboarding.login.recoverOfferTitle"
        static let recoverOfferBody = "onboarding.login.recoverOfferBody"
        static let recoverConfirm = "onboarding.login.recoverConfirm"
        static let recoverCancel = "onboarding.login.recoverCancel"
        static let recoverFailedTitle = "onboarding.login.recoverFailedTitle"
        static let recoverFailedBody = "onboarding.login.recoverFailedBody"
        // The which-wallet picker for the app-owned CTAP path, shared with the desktop.
        static let pickTitle = "onboarding.login.pickTitle"
        static let pickBody = "onboarding.login.pickBody"
        static let pickUnnamed = "onboarding.login.pickUnnamed"
    }

    /// `onboarding.common.*` — the shared flow scaffolding.
    enum Flow {
        static let touchRemoteTitle = "onboarding.common.touchRemoteTitle"
        static let touchRemoteBody = "onboarding.common.touchRemoteBody"
        static let back = "onboarding.common.back"
        static let retry = "onboarding.common.retry"
        static let close = "onboarding.common.close"
        static let copyAddress = "onboarding.common.copyAddress"
        static let copied = "onboarding.common.copied"
        static let confirmInPrompt = "onboarding.common.confirmInPrompt"
        static let editIndexEndpoint = "onboarding.common.editIndexEndpoint"
        static let notDiscoverableTitle = "onboarding.common.notDiscoverableTitle"
        static let notDiscoverableBody = "onboarding.common.notDiscoverableBody"
    }

    enum Settings {
        static let sectionPasskeyIndex = "onboarding.settings.sectionPasskeyIndex"
        static let endpointUrlLabel = "onboarding.settings.endpointUrlLabel"
        static let passkeyHint = "onboarding.settings.passkeyHint"
        static let resetToDefault = "onboarding.settings.resetToDefault"
        static let warningText = "onboarding.settings.warningText"
    }

    /// Every key the onboarding surface renders, for the coverage test.
    ///
    /// Hand-maintained rather than derived by reflection: Swift has no way to
    /// enumerate an enum's static properties, and a list that silently missed a
    /// key would make the coverage test pass by covering less.
    static let all: [String] = [
        Welcome.tagline, Welcome.createWallet, Welcome.alreadyHaveWallet,
        Create.header, Create.headerCreated, Create.headerSyncFailed,
        Create.nameTitle, Create.accountNamePlaceholder, Create.nameTooLong,
        Create.ack0, Create.ack1,
        Create.ack2, Create.ack2PrivacyPolicy, Create.ack2And,
        Create.ack2Terms, Create.ack2Period,
        Create.nextBtn, Create.createWalletBtn, Create.finishVerifyBtn, Create.startOverBtn,
        Create.statusSettingUpIdentity, Create.statusVerifyingIdentity,
        Create.statusExtractingKey, Create.statusComputingAddress, Create.statusSyncingKey,
        Create.statusSetupCancelled, Create.statusVerifyCancelled,
        Create.keysTitle, Create.keysTitleBlocked, Create.keysSubtitle,
        Create.keysSubtitleBlocked, Create.keysSubtitleFull, Create.keysLabel,
        Create.keysHint, Create.keyCount, Create.keySyncedBadge,
        Create.keyDeviceOnlyBadge, Create.keyLimitReached, Create.needSecondKeyHint,
        Create.addKeyBtn, Create.addSecondKeyBtn, Create.confirmKeyBtn,
        Create.removeKeyBtn, Create.addMethodLabel,
        Create.methodPlatformTitle, Create.methodPlatformBody,
        Create.methodHybridTitle, Create.methodHybridBody, Create.methodHybridUnavailable,
        Create.methodSecurityKeyTitle, Create.methodSecurityKeyBody,
        Create.providerPlatform, Create.providerGeneric, Create.providerSecurityKey,
        Create.progressTitle, Create.progressSubtitle, Create.progressMeterLabel,
        Create.taskVerifyKey, Create.taskDeriveAddress, Create.taskWriteIndex,
        Create.syncFailedTitle, Create.syncFailedMessage, Create.syncFailedHint,
        Create.retryUploadBtn, Create.technicalDetails,
        Create.successTitle, Create.successMessage, Create.identiconHint,
        Create.walletAddressLabel, Create.verifyHint, Create.enterWalletBtn,
        Create.alertErrorTitle, Create.alertNotSupportedTitle, Create.alertNotSupportedBody,
        Login.header, Login.alertNotSupportedTitle, Login.alertNotSupportedBody,
        Login.alertIncompatibleTitle, Login.alertIncompatibleBody,
        Login.alertIncompatibleBodyCreate,
        Login.alertSignInFailedTitle, Login.alertSignInFailedBody,
        Login.recoverOfferTitle, Login.recoverOfferBody,
        Login.recoverConfirm, Login.recoverCancel,
        Login.recoverFailedTitle, Login.recoverFailedBody,
        Flow.back, Flow.retry, Flow.close, Flow.copyAddress, Flow.copied,
        Flow.confirmInPrompt, Flow.editIndexEndpoint,
        Flow.notDiscoverableTitle, Flow.notDiscoverableBody,
        Settings.sectionPasskeyIndex, Settings.endpointUrlLabel, Settings.passkeyHint,
        Settings.resetToDefault, Settings.warningText,
    ]

    /// The settings SCREEN (spec 023) — distinct from `Settings` below, which
    /// is the onboarding flow's own `onboarding.settings.*` endpoint surface.
    ///
    /// Most of these keys predate the feature: the `settings.*` namespace has
    /// shipped since the React Native app, and `settingsModals.*`, `about.*`,
    /// `assets.rpcFix*`, `home.balanceDetail*`, `componentsUi.bugReport.*` and
    /// `componentsUi.treasuryBootstrap.*` all describe screens these mocks
    /// redraw. Mirrors Android's `I18nKeys.SettingsUi` key for key.
    enum SettingsUi {
        static let title = "settings.title"

            // Sections.
        static let sectionAccount = "settings.sections.account"
        static let sectionAppearance = "settings.sections.appearance"
        static let sectionLocalization = "settings.sections.localization"
        static let sectionAdvanced = "settings.sections.advanced"

            // Home rows.
        static let accountSwitch = "settings.account.switch"
        static let contactsSubtitle = "settings.account.contactsSubtitle"
        static let feedbackTitle = "settings.feedback.title"
        static let feedbackSubtitle = "settings.feedback.subtitle"

            // Appearance.
        static let themeTitle = "settings.appearance.themeTitle"
        static let themeLight = "settings.appearance.themeLight"
        static let themeDark = "settings.appearance.themeDark"
        static let themeAuto = "settings.appearance.themeAuto"
        static let avatarTitle = "settings.appearance.avatarTitle"
        static let avatarInitials = "settings.appearance.avatarInitials"
        static let avatarIdenticon = "settings.appearance.avatarIdenticon"
        static let textScale = "settings.appearance.textScale"

            // Language picker.
        static let languageTitle = "language.title"
        static let languagePickerTitle = "language.pickerTitle"
        static let languagePickerSubtitle = "language.pickerSubtitle"
        static let languageFollowSystem = "language.followSystem"
        static let languageContributeNote = "language.contributeNote"
        static let languageContributeCta = "language.contributeCta"

            // Localization.
        static let currencyTitle = "settings.localization.currencyTitle"
        static let numberTitle = "settings.localization.numberTitle"
        static let numberSubtitle = "settings.localization.numberSubtitle"
        static let dateTitle = "settings.localization.dateTitle"
        static let dateSubtitle = "settings.localization.dateSubtitle"
        static let timeTitle = "settings.localization.timeTitle"
        static let timeSubtitle = "settings.localization.timeSubtitle"
        static let noteIndian = "settings.formatNote.indian"
        static let noteH24 = "settings.formatNote.h24"
        static let noteH12 = "settings.formatNote.h12"
        static let currencySheetTitle = "componentsUi.currency.title"
        static let currencySearch = "componentsUi.currency.searchPlaceholder"

            // Advanced.
        static let networksTitle = "settings.advanced.networksTitle"
        static let networksSubtitle = "settings.advanced.networksSubtitle"
        static let rpcProvidersTitle = "settings.advanced.rpcProvidersTitle"
        static let rpcProvidersSubtitle = "settings.advanced.rpcProvidersSubtitle"
        static let addNetworkTitle = "settings.advanced.addNetworkTitle"
        static let addNetworkSubtitle = "settings.advanced.addNetworkSubtitle"
        static let endpointsTitle = "settings.advanced.endpointsTitle"
        static let endpointsSubtitle = "settings.advanced.endpointsSubtitle"

            // Network list + detail.
        static let networkCount = "settings.networks.count"
        static let networkCustom = "settings.networks.custom"
        static let networkBuiltinNote = "settings.networks.builtinNote"
        static let networkSaveHint = "settings.networks.saveHint"
        static let networkOnline = "settings.networks.online"
        static let networkSlow = "settings.networks.slow"
            // Ethereum backup of the founding keys (spec 062).
        static let backupTitle = "settingsModals.backup.title"
        static let backupBackedUp = "settingsModals.backup.backedUp"
        static let backupNotBackedUp = "settingsModals.backup.notBackedUp"
        static let backupCouldNotCheck = "settingsModals.backup.couldNotCheck"
        static let backupChecking = "componentsUi.funding.checking"
        static let networkOffline = "settingsModals.health.offline"
        static let chainId = "settingsModals.network.chainId"
        static let fieldRpcUrl = "settingsModals.network.fieldRpcUrl"
        static let fieldExplorer = "settingsModals.network.fieldExplorer"
        static let rpcChainMismatch = "settingsModals.network.rpcChainMismatch"

            // Add network.
        static let addDescription = "settingsModals.addNetwork.description"
        static let addSearch = "settingsModals.addNetwork.searchPlaceholder"
        static let addCompatible = "settingsModals.addNetwork.compatible"
        static let addIncompatible = "settingsModals.addNetwork.incompatible"
        static let addCompatibilityCheck = "settingsModals.addNetwork.compatibilityCheck"
        static let addCheckSafe = "settingsModals.addNetwork.checkSafe"
        static let addCheckSigner = "settingsModals.addNetwork.checkSigner"
        static let addCheckRemaining = "settingsModals.addNetwork.checkRemaining"
        static let addCustomRpcTitle = "settingsModals.addNetwork.customRpcTitle"
        static let addCustomRpcPlaceholder = "settingsModals.addNetwork.customRpcPlaceholder"
        static let addBestRpc = "settingsModals.addNetwork.bestRpc"
        static let addButton = "settingsModals.addNetwork.addNetworkBtn"
        static let addIncompatibleHint = "settingsModals.addNetwork.incompatibleHint"
        static let addChainTool = "settingsModals.addNetwork.openChainSetupTool"
        static let addRecheckWithRpc = "settingsModals.addNetwork.recheckWithRpc"
        static let addTestnet = "settingsModals.addNetwork.testnet"
        static let addUnableToVerify = "settingsModals.addNetwork.unableToVerify"

            // The wizard's three refusals (spec 050).
            //
            // Filed under `addToken.*` for a historical reason — the Expo
            // client's add-token flow embedded an add-network step — and the
            // WORDING is exactly this screen's: "This network is already
            // added", "Chain info not found", "Not compatible with Vela
            // Wallet". Using the corpus entry whose text says the right thing
            // beats inventing a key, which this feature may not do.
        static let addAlreadyAdded = "addToken.errorAlreadyAdded"
        static let addChainNotFound = "addToken.errorChainNotFound"
        static let addNotCompatible = "addToken.errorNotCompatible"

            // RPC providers.
        static let providersDescription = "settingsModals.rpcProviders.description"
        static let providerGetKey = "settingsModals.rpcProviders.getKey"
        static let providerCheckKey = "settingsModals.rpcProviders.checkKey"
        static let providerNotSet = "settingsModals.rpcProviders.notSet"
        static let providerConnected = "activity.connected"
        static let providerSupports = "settingsModals.rpcProviders.supportsCount"

            // Service endpoints.
        static let endpointsDescription = "settingsModals.endpoints.description"
        static let endpointChainData = "settingsModals.endpoints.chainDataLabel"
        static let endpointChainDataHint = "settingsModals.endpoints.chainDataHint"
        static let endpointPasskey = "settingsModals.endpoints.passkeyLabel"
        static let endpointPasskeyHint = "settingsModals.endpoints.passkeyHint"
        static let endpointRelay = "settingsModals.endpoints.bundlerLabel"
        static let endpointRelayHint = "settingsModals.endpoints.bundlerHint"
        static let endpointFiat = "settingsModals.endpoints.fiatLabel"
        static let endpointFiatHint = "settingsModals.endpoints.fiatHint"
        static let endpointsReset = "settingsModals.endpoints.resetToDefaults"

            // Device storage.
        static let storageTitle = "settings.storage.title"
        static let storageSubtitle = "settings.storage.subtitle"
        static let storageSummary = "settings.storage.summary"
        static let storageUserData = "settings.storage.userData"
        static let storageCaches = "settings.storage.caches"
        static let storageConnections = "settings.storage.connections"
        static let legendUserData = "settings.storage.legendUserData"
        static let legendCaches = "settings.storage.legendCaches"
        static let legendSessions = "settings.storage.legendSessions"
        static let itemTransactions = "settings.storage.itemTransactions"
        static let itemContacts = "settings.storage.itemContacts"
        static let itemCustom = "settings.storage.itemCustom"
        static let itemBrowsing = "settings.storage.itemBrowsing"
        static let itemBalances = "settings.storage.itemBalances"
        static let itemRates = "settings.storage.itemRates"
        static let itemScan = "settings.storage.itemScan"
        static let itemDapps = "settings.storage.itemDapps"
        static let countRecords = "settings.storage.records"
        static let countContacts = "settings.storage.contactsCount"
        static let countItems = "settings.storage.itemsCount"
        static let countSites = "settings.storage.sitesCount"
        static let storageClear = "settings.storage.clear"
        static let storageClearAll = "settings.storage.clearAllCaches"
        static let storageDisconnectAll = "settings.storage.disconnectAll"
        static let storageClearTitle = "settings.storage.clearTitle"
        static let storageClearBody = "settings.storage.clearBody"
        static let storageClearConfirm = "settings.storage.clearConfirm"

            // About.
        static let aboutTitle = "settings.about.title"
        static let aboutSubtitle = "settings.about.subtitle"
        static let aboutTagline = "about.tagline"
        static let aboutVersion = "about.version"
        static let aboutSectionTechnical = "about.sectionTechnical"
        static let aboutSectionLinks = "about.sectionLinks"
        static let aboutWalletLabel = "about.techWalletLabel"
        static let aboutWalletValue = "about.techWalletValue"
        static let aboutAuthLabel = "about.techAuthLabel"
        static let aboutAuthValue = "about.techAuthValue"
        static let aboutAccountLabel = "about.techAccountTypeLabel"
        static let aboutAccountValue = "about.techAccountTypeValue"
        static let aboutSignerLabel = "about.techSignerLabel"
        static let aboutSignerValue = "about.techSignerValue"
        static let aboutNetworksLabel = "about.techNetworksLabel"
        static let aboutNetworksValue = "about.techNetworksValue"
        static let aboutLinkWebsite = "about.linkWebsite"
        static let aboutLinkGithub = "about.linkGitHub"
        static let aboutLinkSafe = "about.linkSafeWallet"
        static let aboutFooter = "about.footer"

            // Account switcher + sign out + erase.
        static let accountsTitle = "settingsModals.account.modalTitle"
        static let accountsTotal = "settingsModals.account.total"
        static let accountsCount = "home.switcherAccountCount"
        static let accountCreate = "settingsModals.account.createNew"
        static let accountSignIn = "settingsModals.account.signInExisting"
        static let signOutButton = "settings.signOut.button"
        static let signOutTitle = "settings.signOut.title"
        static let signOutDesc = "settings.signOut.desc"
        static let signOutKeeps = "settings.signOut.keeps"
        static let signOutWarning = "settings.signOut.warning"
        static let signOutAnyway = "settings.signOut.anyway"
        static let signOutCancel = "settings.signOut.cancel"
        static let eraseTitle = "settings.eraseDevice.title"
        static let eraseSubtitle = "settings.eraseDevice.subtitle"
        static let eraseDesc = "settings.eraseDevice.desc"
        static let eraseLoses = "settings.eraseDevice.loses"
        static let eraseKeeps = "settings.eraseDevice.keeps"
        static let eraseConfirm = "settings.eraseDevice.confirm"
        static let eraseCancel = "settings.eraseDevice.cancel"

            // Feedback.
        static let bugTitle = "componentsUi.bugReport.title"
        static let bugSubtitle = "componentsUi.bugReport.subtitle"
        static let bugPlaceholder = "componentsUi.bugReport.whatPlaceholder"
        static let bugAddSteps = "componentsUi.bugReport.addSteps"
        static let bugPreviewToggle = "componentsUi.bugReport.previewToggle"
        static let bugPreviewVersion = "componentsUi.bugReport.previewVersion"
        static let bugPreviewPlatform = "componentsUi.bugReport.previewPlatform"
        static let bugPreviewLanguage = "componentsUi.bugReport.previewLanguage"
        static let bugPreviewRpc = "componentsUi.bugReport.previewRpc"
        static let bugPreviewFailures = "componentsUi.bugReport.previewFailures"
        static let bugPreviewNone = "componentsUi.bugReport.previewNone"
        static let bugConsent = "componentsUi.bugReport.consent"
        static let bugSend = "componentsUi.bugReport.send"
        static let bugGithub = "componentsUi.bugReport.openGithubForm"

            // Rescue (SR1–SR5).
        static let rpcUnavailableMultiple = "assets.rpcUnavailableMultiple"
        static let rpcFix = "assets.rpcFix"
        static let rpcFixTitle = "assets.rpcFixTitle"
        static let rpcFixWarning = "assets.rpcFixWarning"
        static let rpcFixLabel = "assets.rpcFixLabel"
        static let rpcFixSave = "assets.rpcFixSaveBtn"
        static let rpcFixRestored = "assets.rpcFixRestored"
        static let rpcProvidersHint = "assets.rpcProvidersTitle"
        static let rpcReport = "assets.rpcReport"
        static let balanceDetailTitle = "home.balanceDetailTitle"
        static let balanceDetailTotal = "assets.switcherTotal"
        static let balanceDetailNetworks = "home.balanceDetailNetworksLabel"
        static let balanceDetailNote = "home.balanceDetailNetworksNote"
        static let balanceDetailRetrying = "home.balanceDetailStatusRetrying"
        static let balanceDetailFailed = "home.balanceDetailStatusFailed"
        static let balanceDetailUpdated = "home.balanceDetailUpdatedLabel"
        static let balanceDetailRetry = "home.balanceDetailRetry"
        static let relayerTitle = "componentsUi.treasuryBootstrap.title"
        static let relayerLead = "componentsUi.treasuryBootstrap.lead"
        static let relayerAmountHint = "componentsUi.treasuryBootstrap.amountHint"
        static let relayerAddressLabel = "componentsUi.treasuryBootstrap.addressLabel"
        static let relayerDisclaimer = "componentsUi.treasuryBootstrap.disclaimer"
        static let relayerCopy = "componentsUi.treasuryBootstrap.copyBtn"
        static let relayerRetry = "componentsUi.treasuryBootstrap.retryBtn"
        static let indexDownTitle = "settings.indexDown.title"
        static let indexDownSubtitle = "settings.indexDown.subtitle"
        static let indexDownWarning = "onboarding.settings.warningText"
        static let indexDownEndpointLabel = "onboarding.settings.endpointUrlLabel"
        static let indexDownEdit = "settings.indexDown.editEndpoint"
        static let indexDownFooter = "onboarding.settings.passkeyHint"

            // Shared.
        static let commonCancel = "common.cancel"
        static let commonSystem = "common.system"
        static let commonAutomatic = "common.automatic"
        static let commonDone = "common.done"
        static let commonTryAgain = "common.tryAgain"
        static let close = "componentsUi.identiconViewer.close"
        static let navWallet = "componentsUi.mainNav.wallet"
        static let navContacts = "componentsUi.mainNav.contacts"
        static let navExplore = "componentsUi.mainNav.explore"
        static let navSettings = "componentsUi.mainNav.settings"
        static let actionSend = "componentsUi.dock.send"
    }
}
