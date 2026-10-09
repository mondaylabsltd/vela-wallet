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
        /// Spec 102 (D6): "Use a trusted signing page" — the choosers'
        /// advanced entry, whose list is Vela's official page and the
        /// self-hosted ones.
        static let signingPageTitle = "onboarding.create.signingPageTitle"
        static let signingPageBody = "onboarding.create.signingPageBody"
        static let methodSecurityKeyTitle = "onboarding.create.methodSecurityKeyTitle"
        static let methodSecurityKeyBody = "onboarding.create.methodSecurityKeyBody"
        static let providerPlatform = "onboarding.create.providerPlatform"
        static let providerGeneric = "onboarding.create.providerGeneric"
        static let providerSecurityKey = "onboarding.create.providerSecurityKey"

        // Progress screen.
        static let progressTitle = "onboarding.create.progressTitle"
        static let progressSubtitle = "onboarding.create.progressSubtitle"
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
        /// Plural (`_one`/`_other`/…): resolve with `loc.t(_:count:)`.
        static let successMessage = "onboarding.create.successMessage"
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
        /// PR 2: the registry could not be asked which wallet this passkey
        /// opens — a free retry from the signature already made, no new
        /// passkey (`registry_unreachable { local: false }`).
        static let registryUnreachableTitle = "onboarding.login.registryUnreachableTitle"
        static let registryUnreachableBody = "onboarding.login.registryUnreachableBody"
        /// The same prompt when no lookup left the device (`local: true`).
        static let networkTitle = "onboarding.common.networkTitle"
        static let networkBody = "onboarding.common.networkBody"
        static let tryAgain = "common.tryAgain"
        static let cancel = "common.cancel"
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
        /// Issue #446: the link to the other device failed (the core's `phone_link`).
        static let phoneLinkFailed = "onboarding.common.phoneLinkFailed"
        static let notDiscoverableBody = "onboarding.common.notDiscoverableBody"
        /// Issue #450: no key plugged in yet — the ceremony waits for one.
        static let insertKeyTitle = "onboarding.common.insertKeyTitle"
        static let insertKeyBody = "onboarding.common.insertKeyBody"
        /// Issue #450: a security-key route this device could not use (the
        /// core's `security_key` on a not-supported prompt).
        static let keyUnavailableTitle = "onboarding.common.keyUnavailableTitle"
        static let keyUnavailableBody = "onboarding.common.keyUnavailableBody"
        /// Issue #459: leaving the phone's code, or "look at your phone" — the
        /// shared word, as the desktop's card says it.
        static let cancel = "common.cancel"
    }

    /// PR 2 (the integration's core round): sentences the cores hand over by
    /// key — the fee's failure (`FeeView.failure`: its reason, figure and the
    /// line under the held confirm), Continue's estimate by its cause
    /// (`sendEstimateFailureBodyKey`), the home's internal fault
    /// (`BalanceView.internal_key`) and a refusal's reason on the signing
    /// sheet (`SignView.failure_refusal_key`). No surface spells them; they
    /// are listed so a key the corpus loses is a failed coverage test, not a
    /// key path drawn at a person.
    enum CoreRound {
        static let feeRetrying = "componentsUi.signing.confirmBlock.feeRetrying"
        static let feeFailed = "componentsUi.signing.confirmBlock.feeFailed"
        static let estimateFailed = "componentsUi.gas.estimateFailed"
        static let reasonChainDown = "componentsUi.gas.reasonChainDown"
        static let reasonInternal = "componentsUi.gas.reasonInternal"
        static let alertEstimateFailedTitle = "send.alertEstimateFailedTitle"
        static let alertEstimateFailedBody = "send.alertEstimateFailedBody"
        static let alertEstimateChainDownBody = "send.alertEstimateChainDownBody"
        static let previousPending = "componentsUi.signing.confirmBlock.previousPending"
        static let refused = "componentsUi.signing.refused"
        static let wentFirst = "componentsUi.signing.wentFirst"
        static let rejectedFees = "send.txRejectedFees"
        /// The home's line over a balance nothing could be read for
        /// (`BalanceView.unreachable`, no `internal_key`) — Android's and the
        /// web's own words for it.
        static let balanceUnreachable = "onboarding.common.networkBody"

        static let all = [
            feeRetrying, feeFailed, estimateFailed, reasonChainDown, reasonInternal,
            alertEstimateFailedTitle, alertEstimateFailedBody, alertEstimateChainDownBody,
            previousPending, refused, wentFirst, rejectedFees, balanceUnreachable,
        ]
    }

    /// Spec 088 FR-004: a page another app asked to open — the host is data,
    /// the button is the /pay page's own "Open in Vela Wallet" (reused: the
    /// ja + en budget is spent), the other is Cancel.
    enum ExternalPage {
        static let open = "receive.pay.open"
        static let cancel = "common.cancel"
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
        Create.signingPageTitle, Create.signingPageBody,
        Create.providerPlatform, Create.providerGeneric, Create.providerSecurityKey,
        Create.progressTitle, Create.progressSubtitle,
        Create.taskVerifyKey, Create.taskDeriveAddress, Create.taskWriteIndex,
        Create.syncFailedTitle, Create.syncFailedMessage, Create.syncFailedHint,
        Create.retryUploadBtn, Create.technicalDetails,
        Create.successTitle, Create.verifyHint, Create.enterWalletBtn,
        Create.alertErrorTitle, Create.alertNotSupportedTitle, Create.alertNotSupportedBody,
        Login.header, Login.alertNotSupportedTitle, Login.alertNotSupportedBody,
        Login.alertIncompatibleTitle, Login.alertIncompatibleBody,
        Login.alertIncompatibleBodyCreate,
        Login.alertSignInFailedTitle, Login.alertSignInFailedBody,
        Login.recoverOfferTitle, Login.recoverOfferBody,
        Login.recoverConfirm, Login.recoverCancel,
        Login.recoverFailedTitle, Login.recoverFailedBody,
        Login.registryUnreachableTitle, Login.registryUnreachableBody,
        Login.networkTitle, Login.networkBody, Login.tryAgain, Login.cancel,
        Flow.back, Flow.retry, Flow.close, Flow.copyAddress, Flow.copied,
        Flow.confirmInPrompt, Flow.editIndexEndpoint,
        Flow.notDiscoverableTitle, Flow.notDiscoverableBody, Flow.phoneLinkFailed,
        Flow.insertKeyTitle, Flow.insertKeyBody, Flow.keyUnavailableTitle, Flow.keyUnavailableBody,
        Settings.sectionPasskeyIndex, Settings.endpointUrlLabel, Settings.passkeyHint,
        Settings.resetToDefault, Settings.warningText,
    ] + CoreRound.all

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
        static let sectionCommunity = "settings.sections.community"

            // Home rows.
        static let accountSwitch = "settings.account.switch"
        static let accountRemove = "settings.account.remove"
        static let accountRemoveBody = "settings.account.removeBody"
        static let contactsSubtitle = "settings.account.contactsSubtitle"
        static let feedbackTitle = "settings.feedback.title"
        static let feedbackSubtitle = "settings.feedback.subtitle"

            // Appearance.
        static let themeTitle = "settings.appearance.themeTitle"
        static let themeLight = "settings.appearance.themeLight"
        static let themeDark = "settings.appearance.themeDark"
        static let themeAuto = "settings.appearance.themeAuto"
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
        /// Spec 098 §5.1: the relay is sent this network's RPC, key and all.
        static let networkRelayNotice = "settingsModals.network.relayNotice"
        /// Spec 098 §5.1: …and the RPC built from a provider key.
        static let providersRelayNotice = "settingsModals.rpcProviders.relayNotice"
        static let networkOnline = "settings.networks.online"
        static let networkSlow = "settings.networks.slow"
            // Ethereum backup of the founding keys (spec 062).
        static let backupTitle = "settingsModals.backup.title"
        static let backupBackedUp = "settingsModals.backup.backedUp"
        static let backupNotBackedUp = "settingsModals.backup.notBackedUp"
        static let backupCouldNotCheck = "settingsModals.backup.couldNotCheck"
        static let backupChecking = "componentsUi.funding.checking"
            // The keys block (spec 062).
        static let keysTitle = "settingsModals.keys.title"
        static let keysSubtitle = "settingsModals.keys.subtitle"
        static let keysKeyN = "settingsModals.keys.keyN"
        static let keysNotSynced = "settingsModals.keys.notSynced"
        static let keysFromDevice = "settingsModals.keys.fromDevice"
        static let keysSynced = "onboarding.create.keySyncedBadge"
        static let keysPublicKey = "settingsModals.keys.publicKey"
        static let keysCredential = "settingsModals.keys.credential"
        static let keysTransport = "settingsModals.keys.transport"
        static let keysAttestation = "settingsModals.keys.attestation"
        static let keysUserVerified = "settingsModals.keys.userVerified"
        static let keysSignsHere = "settingsModals.keys.signsHere"
        static let keysCopy = "componentsUi.signing.copyValue"
        static let keysCopied = "receive.copied"
        static let backupExplain = "settingsModals.backup.explain"
        static let keysProviderPlatform = "onboarding.create.providerPlatform"
        static let keysProviderGeneric = "onboarding.create.providerGeneric"
        static let keysProviderSecurityKey = "onboarding.create.providerSecurityKey"
        static let networkOffline = "settingsModals.health.offline"
        // The other two service-health verdicts (spec 081 FR-001). A refused
        // endpoint and one that answered with somebody else's service are
        // different failures, and Android has said so since 047 — iOS had no
        // key for either because the page was a drawing.
        static let healthHttpsRequired = "settingsModals.health.httpsRequired"
        static let healthInvalid = "settingsModals.health.invalid"
        static let chainId = "settingsModals.network.chainId"
        static let fieldRpcUrl = "settingsModals.network.fieldRpcUrl"
        static let fieldExplorer = "settingsModals.network.fieldExplorer"
        static let rpcChainMismatch = "settingsModals.network.rpcChainMismatch"
            // Removing a custom network, asked first (spec 072).
        static let networkRemoveTitle = "settingsModals.network.removeTitle"
        static let networkRemoveBody = "settingsModals.network.removeBody"
        static let networkRemoveConfirm = "settingsModals.network.removeConfirm"
        static let networkRemoveCancel = "settingsModals.network.removeCancel"

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
        /// Spec 081 FR-009: compatible for one key, not for two to seven.
        static let addSingleKeyOnly = "settingsModals.addNetwork.singleKeyOnly"
        static let addChainTool = "settingsModals.addNetwork.openChainSetupTool"
        static let addRecheckWithRpc = "settingsModals.addNetwork.recheckWithRpc"
        static let addTestnet = "settingsModals.addNetwork.testnet"
        static let addUnableToVerify = "settingsModals.addNetwork.unableToVerify"
        static let addChecking = "settingsModals.addNetwork.checkingCompatibility"
        static let addRetry = "settingsModals.addNetwork.retry"

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
        static let endpointsResetTitle = "settingsModals.endpoints.resetTitle"
        static let endpointsResetBody = "settingsModals.endpoints.resetBody"
        static let endpointsResetConfirm = "settingsModals.endpoints.resetConfirm"
        static let endpointsResetCancel = "settingsModals.endpoints.resetCancel"

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
        static let aboutLinkPrivacy = "about.linkPrivacy"
        static let aboutLinkTerms = "about.linkTerms"
        static let aboutLinkSupport = "about.linkSupport"
        static let aboutFooter = "about.footer"
        // Settings' hidden debug mode (spec 091).
        static let aboutDebugMode = "about.debugMode"
        static let aboutDebugModeBody = "about.debugModeBody"
        static let aboutDebugModeRevealed = "about.debugModeRevealed"

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
        /// The erase ran and something survived — said, never swallowed
        /// (spec 081 FR-017).
        static let eraseFailed = "settings.eraseDevice.failed"

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
        /// The one-click report's own states (round 3): typing, sending,
        /// filed, and the GitHub road when the endpoint could not file it.
        static let bugStepsPlaceholder = "componentsUi.bugReport.stepsPlaceholder"
        static let bugSending = "componentsUi.bugReport.sending"
        static let bugSuccessTitle = "componentsUi.bugReport.successTitle"
        static let bugSuccessBodyNew = "componentsUi.bugReport.successBodyNew"
        static let bugSuccessBodyDeduped = "componentsUi.bugReport.successBodyDeduped"
        static let bugViewIssue = "componentsUi.bugReport.viewIssue"
        static let bugFallbackTitle = "componentsUi.bugReport.fallbackTitle"
        static let bugFallbackBody = "componentsUi.bugReport.fallbackBody"
        static let bugOpenGithub = "componentsUi.bugReport.openGithub"
        static let bugDone = "componentsUi.bugReport.done"
        /// Screenshots (2026-09-26): attached, public on GitHub, refused.
        static let bugScreenshotsLabel = "componentsUi.bugReport.screenshotsLabel"
        static let bugAddScreenshots = "componentsUi.bugReport.addScreenshots"
        static let bugScreenshotsHint = "componentsUi.bugReport.screenshotsHint"
        static let bugScreenshotsPublic = "componentsUi.bugReport.screenshotsPublic"
        static let bugRemoveScreenshot = "componentsUi.bugReport.removeScreenshot"
        static let bugScreenshotsLimit = "componentsUi.bugReport.screenshotsLimit"
        static let bugScreenshotUnsupported = "componentsUi.bugReport.screenshotUnsupported"
        static let bugScreenshotsDropped = "componentsUi.bugReport.screenshotsDropped"
        static let bugFallbackScreenshots = "componentsUi.bugReport.fallbackScreenshots"
        /// Tap a screenshot to preview it (spec C, 2026-09-27): the tile's
        /// accessible name, the viewer's ✕ and its Remove.
        static let bugViewScreenshot = "componentsUi.bugReport.viewScreenshot"
        static let bugCloseViewer = "componentsUi.bugReport.closeViewer"
        static let bugRemoveFromViewer = "componentsUi.bugReport.removeFromViewer"

            // Rescue (SR1–SR6).
            // SR6 (spec 092): the networks the wallet cannot reach — the
            // home's line (`{{name}}` / `{{n}}`), the list under it, and the
            // four row lines the core names in `line_key`.
        static let unreachableOne = "assets.unreachableOne"
        static let unreachableMany = "assets.unreachableMany"
        static let unreachableBody = "assets.unreachableBody"
        static let unreachableNone = "assets.unreachableNone"
        static let lastSeen = "assets.lastSeen"
        static let lastSeenUnpriced = "assets.lastSeenUnpriced"
        static let lastSeenEmpty = "assets.lastSeenEmpty"
        static let notReadYet = "assets.notReadYet"
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
