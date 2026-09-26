package app.getvela.wallet.core.i18n

/**
 * Corpus key paths used by this app (rust/crates/vela-core/i18n/locales, flat
 * `onboarding.welcome.*` lineage per spec — the nested welcomeWeb set is web-only).
 * Centralised so feature code never scatters key literals.
 */
object I18nKeys {
    object Welcome {
        const val TAGLINE = "onboarding.welcome.desktopTagline"
        const val HERO_TITLE = "onboarding.welcome.heroTitle"
        /** The headline's type tier: `regular` or `long` (an enum, not prose). */
        const val HERO_TITLE_FIT = "onboarding.welcome.heroTitleFit"
        const val HERO_SUBTITLE = "onboarding.welcome.heroSubtitle"
        const val CREATE_WALLET = "onboarding.welcome.createWallet"
        const val ALREADY_HAVE_WALLET = "onboarding.welcome.alreadyHaveWallet"

        const val FEATURE_NO_MNEMONIC_TITLE = "onboarding.welcome.featureNoMnemonicTitle"
        const val FEATURE_NO_MNEMONIC_BODY = "onboarding.welcome.featureNoMnemonicBody"
        const val FEATURE_ONE_ADDRESS_TITLE = "onboarding.welcome.featureOneAddressTitle"
        const val FEATURE_ONE_ADDRESS_BODY = "onboarding.welcome.featureOneAddressBody"
        const val FEATURE_OPEN_SOURCE_TITLE = "onboarding.welcome.featureOpenSourceTitle"
        const val FEATURE_OPEN_SOURCE_BODY = "onboarding.welcome.featureOpenSourceBody"
        const val FEATURE_KEY_CUSTODY_TITLE = "onboarding.welcome.featureKeyCustodyTitle"
        const val FEATURE_KEY_CUSTODY_BODY = "onboarding.welcome.featureKeyCustodyBody"
        const val FEATURE_SAFE_CONTRACT_TITLE = "onboarding.welcome.featureSafeContractTitle"
        const val FEATURE_SAFE_CONTRACT_BODY = "onboarding.welcome.featureSafeContractBody"
        const val FEATURE_STABLECOIN_GAS_TITLE = "onboarding.welcome.featureStablecoinGasTitle"
        const val FEATURE_STABLECOIN_GAS_BODY = "onboarding.welcome.featureStablecoinGasBody"
    }

    object Settings {
        const val TITLE = "onboarding.settings.title"
        const val SECTION_APPEARANCE = "onboarding.settings.sectionAppearance"
        const val THEME_LIGHT = "onboarding.settings.themeLabelLight"
        const val THEME_DARK = "onboarding.settings.themeLabelDark"
        const val THEME_AUTO = "onboarding.settings.themeLabelAuto"

        // --- spec 019: the endpoint surface an unreachable index opens -----
        const val SECTION_PASSKEY_INDEX = "onboarding.settings.sectionPasskeyIndex"
        const val ENDPOINT_URL_LABEL = "onboarding.settings.endpointUrlLabel"
        const val PASSKEY_HINT = "onboarding.settings.passkeyHint"
        const val RESET_TO_DEFAULT = "onboarding.settings.resetToDefault"
        const val WARNING_TEXT = "onboarding.settings.warningText"

        // --- spec 019: the way back out of a signed-in wallet --------------
        //
        // `settings.*`, not `onboarding.settings.*`: sign-out belongs to the
        // wallet, and the copy already existed in all fifteen locales because
        // desktop needed it first. `settings.signOut.desc` is deliberately
        // absent — it ends "your passkey stays in Face ID / fingerprint", which
        // names the wrong platform's biometric on Android.
        const val SIGN_OUT_BUTTON = "settings.signOut.button"
        const val SIGN_OUT_TITLE = "settings.signOut.title"
        const val SIGN_OUT_KEEPS = "settings.signOut.keeps"
        const val SIGN_OUT_DESC_MANY = "settings.signOut.descMany"
        const val SIGN_OUT_WARNING = "settings.signOut.warning"
        const val SIGN_OUT_ANYWAY = "settings.signOut.anyway"
        const val SIGN_OUT_CANCEL = "settings.signOut.cancel"
    }

    object Create {
        const val HEADER = "onboarding.create.headerDefault"

        // Spec 014 create-flow keys (contracts/i18n-keys.md). All EXISTS in the
        // corpus except RETRY_VERIFY_BTN, which lands with the spec-014 corpus batch.
        const val HEADER_SYNC_FAILED = "onboarding.create.headerSyncFailed"
        const val ACCOUNT_NAME_PLACEHOLDER = "onboarding.create.accountNamePlaceholder"
        const val NAME_TOO_LONG = "onboarding.create.nameTooLong"
        const val TECHNICAL_DETAILS = "onboarding.create.technicalDetails"
        // THREE acknowledgements, each a fact about where something ends up:
        // the public key and the name go on-chain, the private key stays in the
        // device or on a security key, and the legal assent. The legal line's
        // link fragments are named for the row they render on — `ack2*` — because
        // a fragment key that disagrees with its index is how the earlier
        // `ack3` → `ack1` confusion started.
        const val ACK0 = "onboarding.create.ack0"
        const val ACK1 = "onboarding.create.ack1"
        const val ACK2 = "onboarding.create.ack2"
        const val CREATE_WALLET_BTN = "onboarding.create.createWalletBtn"
        const val STATUS_SETTING_UP_IDENTITY = "onboarding.create.statusSettingUpIdentity"
        const val STATUS_VERIFYING_IDENTITY = "onboarding.create.statusVerifyingIdentity"
        const val STATUS_EXTRACTING_KEY = "onboarding.create.statusExtractingKey"
        const val STATUS_COMPUTING_ADDRESS = "onboarding.create.statusComputingAddress"
        const val STATUS_SYNCING_KEY = "onboarding.create.statusSyncingKey"
        const val SUCCESS_TITLE = "onboarding.create.successTitle"
        const val SUCCESS_MESSAGE = "onboarding.create.successMessage"
        const val VERIFY_HINT = "onboarding.create.verifyHint"
        const val ENTER_WALLET_BTN = "onboarding.create.enterWalletBtn"
        const val FINISH_VERIFY_BTN = "onboarding.create.finishVerifyBtn"
        const val START_OVER_BTN = "onboarding.create.startOverBtn"
        const val SYNC_FAILED_TITLE = "onboarding.create.syncFailedTitle"
        const val RETRY_UPLOAD_BTN = "onboarding.create.retryUploadBtn"
        const val RETRY_VERIFY_BTN = "onboarding.create.retryVerifyBtn"

        // --- spec 019, the v2 create journey -------------------------------
        //
        // Name screen.
        const val NAME_TITLE = "onboarding.create.nameTitle"
        const val ACK2_PRIVACY_POLICY = "onboarding.create.ack2PrivacyPolicy"
        const val ACK2_AND = "onboarding.create.ack2And"
        const val ACK2_TERMS = "onboarding.create.ack2Terms"
        const val ACK2_PERIOD = "onboarding.create.ack2Period"
        const val NEXT_BTN = "onboarding.create.nextBtn"
        const val STATUS_SETUP_CANCELLED = "onboarding.create.statusSetupCancelled"
        const val STATUS_VERIFY_CANCELLED = "onboarding.create.statusVerifyCancelled"

        // Keys screen.
        const val KEYS_TITLE = "onboarding.create.keysTitle"
        const val KEYS_TITLE_BLOCKED = "onboarding.create.keysTitleBlocked"
        const val KEYS_SUBTITLE = "onboarding.create.keysSubtitle"
        const val KEYS_SUBTITLE_BLOCKED = "onboarding.create.keysSubtitleBlocked"
        const val KEYS_SUBTITLE_FULL = "onboarding.create.keysSubtitleFull"
        const val KEYS_LABEL = "onboarding.create.keysLabel"
        const val KEYS_HINT = "onboarding.create.keysHint"
        const val KEY_COUNT = "onboarding.create.keyCount"
        const val KEY_SYNCED_BADGE = "onboarding.create.keySyncedBadge"
        const val KEY_DEVICE_ONLY_BADGE = "onboarding.create.keyDeviceOnlyBadge"
        const val KEY_LIMIT_REACHED = "onboarding.create.keyLimitReached"
        const val NEED_SECOND_KEY_HINT = "onboarding.create.needSecondKeyHint"
        const val ADD_KEY_BTN = "onboarding.create.addKeyBtn"
        const val ADD_SECOND_KEY_BTN = "onboarding.create.addSecondKeyBtn"
        const val CONFIRM_KEY_BTN = "onboarding.create.confirmKeyBtn"
        const val REMOVE_KEY_BTN = "onboarding.create.removeKeyBtn"
        const val FINISH_KEYS_BTN = "onboarding.create.finishKeysBtn"
        const val ADD_METHOD_LABEL = "onboarding.create.addMethodLabel"
        const val METHOD_PLATFORM_TITLE = "onboarding.create.methodPlatformTitle"
        const val METHOD_PLATFORM_BODY = "onboarding.create.methodPlatformBody"
        const val METHOD_HYBRID_TITLE = "onboarding.create.methodHybridTitle"
        const val METHOD_HYBRID_BODY = "onboarding.create.methodHybridBody"
        const val METHOD_HYBRID_UNAVAILABLE = "onboarding.create.methodHybridUnavailable"
        const val METHOD_BLOCKED_HINT = "onboarding.create.methodBlockedHint"
        const val METHOD_BLOCKED_SIGNER = "onboarding.create.methodBlockedSigner"
        const val METHOD_SECURITY_KEY_TITLE = "onboarding.create.methodSecurityKeyTitle"
        const val METHOD_SECURITY_KEY_BODY = "onboarding.create.methodSecurityKeyBody"
        const val PROVIDER_PLATFORM = "onboarding.create.providerPlatform"
        const val PROVIDER_GENERIC = "onboarding.create.providerGeneric"
        const val PROVIDER_SECURITY_KEY = "onboarding.create.providerSecurityKey"

        // Progress screen.
        const val PROGRESS_TITLE = "onboarding.create.progressTitle"
        const val PROGRESS_SUBTITLE = "onboarding.create.progressSubtitle"
        const val PROGRESS_METER_LABEL = "onboarding.create.progressMeterLabel"
        const val TASK_VERIFY_KEY = "onboarding.create.taskVerifyKey"
        const val TASK_DERIVE_ADDRESS = "onboarding.create.taskDeriveAddress"
        const val TASK_WRITE_INDEX = "onboarding.create.taskWriteIndex"

        // Retry screen.
        const val SYNC_FAILED_MESSAGE = "onboarding.create.syncFailedMessage"
        const val SYNC_FAILED_HINT = "onboarding.create.syncFailedHint"

        // Done screen.
        const val HEADER_CREATED = "onboarding.create.headerCreated"
        const val IDENTICON_HINT = "onboarding.create.identiconHint"
        const val WALLET_ADDRESS_LABEL = "onboarding.create.walletAddressLabel"

        // Prompts (data-model 5).
        const val ALERT_ERROR_TITLE = "onboarding.create.alertErrorTitle"
        const val ALERT_NOT_SUPPORTED_TITLE = "onboarding.create.alertNotSupportedTitle"
        const val ALERT_NOT_SUPPORTED_BODY = "onboarding.create.alertNotSupportedBody"

        // The app-owned CTAP path's own dialogs — the ceremony a system passkey
        // sheet would otherwise draw. Shared with the desktop (spec 019 §5).
        const val PIN_TITLE = "onboarding.create.pinTitle"
        const val PIN_BODY = "onboarding.create.pinBody"
        const val PIN_LABEL = "onboarding.create.pinLabel"
        const val PIN_ATTEMPTS_LEFT = "onboarding.create.pinAttemptsLeft"
        const val PIN_REJECTED = "onboarding.create.pinRejected"
        const val TOUCH_TITLE = "onboarding.create.touchTitle"
        const val TOUCH_BODY = "onboarding.create.touchBody"
        const val TOUCH_FINGERPRINT_BODY = "onboarding.create.touchFingerprintBody"
        const val TOUCH_SELECT_BODY = "onboarding.create.touchSelectBody"
    }

    /** onboarding.login.* — spec 014 login flow (mix of EXISTS + NEW corpus keys). */
    object Login {
        const val HEADER = "onboarding.login.header"
        const val STATUS_AWAITING_PASSKEY = "onboarding.login.statusAwaitingPasskey"
        const val STATUS_AWAITING_PASSKEY_HINT = "onboarding.login.statusAwaitingPasskeyHint"
        const val STATUS_CANCELLED_TITLE = "onboarding.login.statusCancelledTitle"
        const val STATUS_CANCELLED_BODY = "onboarding.login.statusCancelledBody"
        const val SUCCESS_TITLE = "onboarding.login.successTitle"
        const val SUCCESS_MESSAGE = "onboarding.login.successMessage"
        const val SIGN_IN_FAILED_TITLE = "onboarding.login.alertSignInFailedTitle"
        const val SIGN_IN_FAILED_BODY = "onboarding.login.signInFailedBody"
        const val RETRY_LOGIN_BTN = "onboarding.login.retryLoginBtn"
        const val CREATE_NEW_WALLET_BTN = "onboarding.login.createNewWalletBtn"
        const val RECOVER_OFFER_TITLE = "onboarding.login.recoverOfferTitle"
        const val RECOVER_OFFER_BODY = "onboarding.login.recoverOfferBody"
        const val RECOVER_CONFIRM = "onboarding.login.recoverConfirm"
        const val RECOVER_CANCEL = "onboarding.login.recoverCancel"
        const val RECOVER_FAILED_TITLE = "onboarding.login.recoverFailedTitle"
        const val RECOVER_FAILED_BODY = "onboarding.login.recoverFailedBody"

        // --- spec 019 ------------------------------------------------------
        const val ALERT_NOT_SUPPORTED_TITLE = "onboarding.login.alertNotSupportedTitle"
        const val ALERT_NOT_SUPPORTED_BODY = "onboarding.login.alertNotSupportedBody"
        const val ALERT_INCOMPATIBLE_TITLE = "onboarding.login.alertIncompatibleTitle"
        const val ALERT_INCOMPATIBLE_BODY = "onboarding.login.alertIncompatibleBody"
        const val ALERT_INCOMPATIBLE_BODY_CREATE = "onboarding.login.alertIncompatibleBodyCreate"
        const val ALERT_SIGN_IN_FAILED_BODY = "onboarding.login.alertSignInFailedBody"
        const val ALERT_SELECTOR_UNRESPONSIVE = "onboarding.login.alertSelectorUnresponsive"
        const val SWITCH_DEVICE_BTN = "onboarding.login.switchDeviceBtn"

        // The which-wallet picker for the app-owned CTAP path — a key that
        // holds more than one Vela wallet. Shared with the desktop.
        const val PICK_TITLE = "onboarding.login.pickTitle"
        const val PICK_BODY = "onboarding.login.pickBody"
        const val PICK_UNNAMED = "onboarding.login.pickUnnamed"
    }

    /**
     * onboarding.common.* — the NEW shared flow-scaffolding branch (spec 014,
     * contracts/i18n-keys.md). Keys are added to the corpus by the spec-014
     * batch; the engine echoes missing keys until that lands.
     */
    object Flow {
        const val HEADER_SHARED = "onboarding.common.headerShared"
        const val STEP_COUNTER = "onboarding.common.stepCounter"
        const val CONFIRM_IN_PROMPT = "onboarding.common.confirmInPrompt"
        const val WAITED_SECONDS = "onboarding.common.waitedSeconds"
        const val NETWORK_TITLE = "onboarding.common.networkTitle"
        const val NETWORK_BODY = "onboarding.common.networkBody"
        const val SERVER_TITLE = "onboarding.common.serverTitle"
        const val SERVER_BODY = "onboarding.common.serverBody"
        const val TIMEOUT_TITLE = "onboarding.common.timeoutTitle"
        const val TIMEOUT_BODY = "onboarding.common.timeoutBody"
        const val UNKNOWN_TITLE = "onboarding.common.unknownTitle"
        const val UNKNOWN_BODY = "onboarding.common.unknownBody"
        const val CANCELLED_SETUP_TITLE = "onboarding.common.cancelledSetupTitle"
        const val CANCELLED_SETUP_BODY = "onboarding.common.cancelledSetupBody"
        const val CANCELLED_VERIFY_TITLE = "onboarding.common.cancelledVerifyTitle"
        const val CANCELLED_VERIFY_BODY = "onboarding.common.cancelledVerifyBody"
        const val UNSUPPORTED_TITLE = "onboarding.common.unsupportedTitle"
        const val UNSUPPORTED_BODY = "onboarding.common.unsupportedBody"
        const val INCOMPATIBLE_TITLE = "onboarding.common.incompatibleTitle"
        const val INCOMPATIBLE_BODY = "onboarding.common.incompatibleBody"
        const val NOT_DISCOVERABLE_TITLE = "onboarding.common.notDiscoverableTitle"
        const val NOT_DISCOVERABLE_BODY = "onboarding.common.notDiscoverableBody"
        const val NOT_FOUND_TITLE = "onboarding.common.notFoundTitle"
        const val NOT_FOUND_BODY = "onboarding.common.notFoundBody"
        const val VERIFY_STUCK_TITLE = "onboarding.common.verifyStuckTitle"
        const val VERIFY_STUCK_BODY = "onboarding.common.verifyStuckBody"
        const val SYNC_FAILED_BODY = "onboarding.common.syncFailedBody"
        const val BACK = "onboarding.common.back"
        const val RETRY = "onboarding.common.retry"
        const val RECREATE_WALLET = "onboarding.common.recreateWallet"
        const val EDIT_INDEX_ENDPOINT = "onboarding.common.editIndexEndpoint"
        const val REPORT_ERROR = "onboarding.common.reportError"
        const val OPEN_BIOMETRIC_SETTINGS = "onboarding.common.openBiometricSettings"
        const val LOCATION_NEEDED_TITLE = "onboarding.common.locationNeededTitle"
        const val LOCATION_NEEDED_BODY = "onboarding.common.locationNeededBody"
        const val OPEN_LOCATION_SETTINGS = "onboarding.common.openLocationSettings"
        const val INSERT_KEY_TITLE = "onboarding.common.insertKeyTitle"
        const val INSERT_KEY_BODY = "onboarding.common.insertKeyBody"
        const val OTG_OFF_HINT = "onboarding.common.otgOffHint"
        const val TOUCH_REMOTE_TITLE = "onboarding.common.touchRemoteTitle"
        const val TOUCH_REMOTE_BODY = "onboarding.common.touchRemoteBody"
        const val OPEN_CREDENTIAL_MANAGER_SETTINGS = "onboarding.common.openCredentialManagerSettings"
        const val COPY_ADDRESS = "onboarding.common.copyAddress"
        const val COPIED = "onboarding.common.copied"
        const val CLOSE = "onboarding.common.close"
    }

    object Common {
        const val CANCEL = "common.cancel"

        /**
         * An alert's acknowledgement (spec 078 round 2): "知道了" / "Got it".
         * The send alert's button said "完成" (the receipt's Done), which
         * reads as finishing the send the alert just refused.
         */
        const val GOT_IT = "common.gotIt"
    }

    /** Wallet home vocabulary (spec 015, research.md D3 key map — all pre-existing corpus keys). */
    object Wallet {
        // Main navigation (tab bar).
        const val NAV_WALLET = "componentsUi.mainNav.wallet"
        const val NAV_CONTACTS = "componentsUi.mainNav.contacts"
        const val NAV_EXPLORE = "componentsUi.mainNav.explore"
        const val NAV_SETTINGS = "componentsUi.mainNav.settings"

        // Balance display.
        const val TOTAL_BALANCE = "home.totalBalance"
        const val LIVE_INDICATOR = "home.liveIndicator"
        const val BALANCE_STALE = "home.balanceStale"
        const val BALANCE_UNPRICED = "home.balanceUnpriced"
        /** "{{name}} RPC unavailable" — the hero's line for one unreachable chain (web `liveBalance`). */
        const val RPC_UNAVAILABLE_SINGLE = "assets.rpcUnavailableSingle"
        /**
         * The hero's reason when a first load could read nothing and nothing is
         * cached (#188, spec 038 finding 15). Borrowed from the onboarding
         * flow's network line — the same key the web and desktop shells bind
         * for this state, so one corpus string serves three heroes.
         */
        const val BALANCE_UNREACHABLE = "onboarding.common.networkBody"
        const val NO_PRICE = "home.balanceDetailNoPrice"
        const val A11Y_HIDE_BALANCE = "home.a11yHideBalance"
        const val A11Y_SHOW_BALANCE = "home.a11yShowBalance"

        // Sections & empty states.
        const val SECTION_ACTIVITY = "home.tabActivity"
        const val EMPTY_NO_ACTIVITY = "home.emptyNoActivity"
        const val EMPTY_ACTIVITY_SUBTITLE = "home.emptySubtitle"
        const val SECTION_ASSETS = "assets.sectionTitle"
        const val ASSETS_ADD = "assets.addToken"
        const val ASSETS_EMPTY_TITLE = "assets.emptyTitle"
        const val ASSETS_EMPTY_SUBTEXT = "assets.emptySubtext"

        // Action dock.
        const val ACTION_RECEIVE = "componentsUi.dock.receive"
        const val ACTION_SEND = "componentsUi.dock.send"
        const val ACTION_SCAN = "componentsUi.dock.scan"

        // Activity rows (templates use {{name}} via t(key, vars)).
        const val FILTER_ALL = "history.filterAll"
        const val LABEL_SENT = "history.labelSent"
        const val LABEL_RECEIVED = "history.labelReceived"
        const val LABEL_DAPP_TX = "history.txLabelDappTx"
        const val TO_NAME = "history.toName"
        const val FROM_NAME = "history.fromName"
        const val DAY_TODAY = "componentsUi.dayGroup.today"
        const val DAY_YESTERDAY = "componentsUi.dayGroup.yesterday"

        // Network filter (pill + chain-select sheet).
        const val PILL_ALL = "componentsUi.networkFilter.pillAll"
        const val SELECT_CHAIN = "componentsUi.networkFilter.selectChain"
        const val ALL_NETWORKS = "componentsUi.networkFilter.allNetworks"

        // Shared component captions.
        const val QR_CAPTION = "componentsUi.qrPlaceholder.caption"
        const val COPY_ADDRESS = "componentsUi.identiconViewer.copyAddress"

        // The identicon viewer (spec 019 founder call): the artwork, big,
        // beside the address that drew it.
        const val IDENTICON_TITLE = "componentsUi.identiconViewer.title"
        const val IDENTICON_CAPTION = "componentsUi.identiconViewer.caption"
        const val IDENTICON_COPIED = "componentsUi.identiconViewer.copied"
        const val IDENTICON_CLOSE = "componentsUi.identiconViewer.close"
        const val IDENTICON_A11Y_OPEN = "componentsUi.identiconViewer.a11yOpen"
    }

    /**
     * Contacts vocabulary (spec 018, contracts/i18n-keys.md — the normative key
     * map). The 21 additions plus the two updated values already live in the
     * corpus; keys shared with the wallet map are re-exported here so contacts
     * code never reaches into [Wallet] for its own copy.
     */
    object Contacts {
        // Page / section chrome.
        const val TITLE = "contacts.title"
        const val SECTION_GROUPS = "contacts.sectionGroups"
        const val SECTION_CONTACTS = "contacts.sectionContacts"
        const val MANAGE = "contacts.manage"
        const val COUNT_PEOPLE = "contacts.countPeople"
        const val GROUP_MEMBERS = "contacts.groupMembers"
        const val MEMBERS_COUNT = "contacts.membersCount"
        const val ALL_CONTACTS = "contacts.allContacts"
        const val SEARCH_PLACEHOLDER = "contacts.searchPlaceholder"
        const val NO_RESULTS = "contacts.noResults"

        // Empty state.
        const val EMPTY = "contacts.empty"
        const val EMPTY_HINT = "contacts.emptyHint"
        const val ADD_CONTACT = "contacts.addContact"

        // Detail.
        const val ADDRESS_LABEL = "contacts.addressLabel"
        const val RECENT_ACTIVITY = "contacts.recentActivity"
        const val VIEW_ALL_ACTIVITY = "contacts.viewAllActivity"
        const val DELETE_CONTACT = "contacts.deleteContact"
        const val ACTION_QR = "contacts.actionQr"
        const val EDIT = "contacts.edit"
        const val MOVE_GROUP = "contacts.moveGroup"

        // Group detail.
        const val ADD_MEMBER = "contacts.addMember"
        const val BATCH_SEND = "contacts.batchSend"
        const val BATCH_SEND_HINT = "contacts.batchSendHint"
        const val BATCH_SEND_HINT_TITLED = "contacts.batchSendHintTitled"
        const val GROUP_NEW = "contacts.groupNew"
        const val GROUP_EDIT = "contacts.groupEdit"
        const val GROUP_RENAME = "contacts.groupRename"
        const val GROUP_DELETE = "contacts.groupDelete"

        // Menus (add / import / export).
        const val ADD_TITLE = "contacts.addTitle"
        const val IMPORT_FILE = "contacts.importFile"
        const val IMPORT_ALL = "contacts.importAll"
        const val EXPORT_TITLE = "contacts.exportTitle"
        const val EXPORT_ALL = "contacts.exportAll"
        const val IMPORT_GROUP = "contacts.importGroup"
        const val EXPORT_GROUP = "contacts.exportGroup"

        // Destructive confirmation.
        const val DELETE_TITLE = "contacts.deleteTitle"
        const val DELETE_BODY = "contacts.deleteBody"
        const val DELETE = "contacts.delete"
        const val CANCEL = "contacts.cancel"
        // Spec 045: the add/edit form (018's vocabulary), the favourite control, inspection, the book's travel.
        const val EDIT_TITLE = "contacts.editTitle"
        const val NAME_LABEL = "contacts.nameLabel"
        const val NAME_PLACEHOLDER = "contacts.namePlaceholder"
        const val ADDRESS_PLACEHOLDER = "contacts.addressPlaceholder"
        const val SAVE = "contacts.save"
        const val INVALID_ADDRESS = "contacts.invalidAddress"
        const val SECTION_FAVORITES = "contacts.sectionFavorites"
        const val CONTRACT_TAG = "componentsUi.signing.contractTag"
        const val WALLET_TAG = "componentsUi.signing.walletTag"
        const val FIRST_TIME_TAG = "componentsUi.signing.firstTimeTagNeutral"
        const val IMPORT_BTN = "contacts.importBtn"
        const val EXPORT_BTN = "contacts.exportBtn"
        const val IMPORT_DONE_TITLE = "contacts.importDoneTitle"
        const val IMPORT_DONE_BODY = "contacts.importDoneBody"
        const val IMPORT_DONE_INVALID = "contacts.importDoneInvalid"
        const val IMPORT_FAIL_TITLE = "contacts.importFailTitle"
        const val IMPORT_FAIL_BODY = "contacts.importFailBody"

        // Reused from the spec-015 map (same keys, no corpus change).
        const val ACTION_SEND = "componentsUi.dock.send"
        const val ACTION_RECEIVE = "componentsUi.dock.receive"
        const val COPY_ADDRESS = "componentsUi.identiconViewer.copyAddress"
        const val LABEL_SENT = "history.labelSent"
        const val LABEL_RECEIVED = "history.labelReceived"
        const val FILTER_ALL = "history.filterAll"
        const val NAV_WALLET = "componentsUi.mainNav.wallet"
        const val NAV_CONTACTS = "componentsUi.mainNav.contacts"
        const val NAV_EXPLORE = "componentsUi.mainNav.explore"
        const val NAV_SETTINGS = "componentsUi.mainNav.settings"
    }

    /**
     * Receive / Send / Activity / Assets (spec 021, docs/design/wallet-2).
     *
     * Most of this vocabulary already existed: the legacy React Native app left
     * `receive.*`, `send.*`, `history.*`, `assets.*`, `addToken.*`,
     * `tokenDetail.*`, `componentsTx.*` and `componentsUi.scanner.*` in the
     * corpus, and about nine strings in ten resolve against a key that was
     * already there. Only the thirty-three the mocks genuinely added are new.
     */
    object Flows {
        /** Spec 058: 删除记录 on the transaction detail — the local record only. */
        const val DELETE_RECORD = "history.deleteRecord"
        /** Spec 048: the toast after 保存图片 put the card in the gallery. */
        const val SAVED_BODY = "receive.request.savedBody"
        // Chrome shared by every flow screen.
        const val BACK = "receive.a11yBack"
        const val CLOSE = "componentsUi.identiconViewer.close"
        const val COPY_ADDRESS = "componentsUi.identiconViewer.copyAddress"
        const val PILL_ALL = "componentsUi.networkFilter.pillAll"
        const val DAY_TODAY = "componentsUi.dayGroup.today"
        const val DAY_YESTERDAY = "componentsUi.dayGroup.yesterday"
        const val DOCK_SEND = "componentsUi.dock.send"

        // Receive.
        const val RECEIVE_TITLE = "receive.title"
        const val RECEIVE_NETWORKS_LINE = "receive.networksLine"
        const val RECEIVE_SEARCH = "receive.searchNetworkPlaceholder"
        const val RECEIVE_SEARCH_EMPTY = "receive.searchNetworkEmpty"
        const val RECEIVE_QR_NETWORK = "receive.qrTitleNetwork"
        const val RECEIVE_QR_ASSET = "receive.qrTitleAsset"
        const val RECEIVE_TOKEN_CONTRACT = "receive.tokenContract"
        const val RECEIVE_WARNING = "receive.warningReminder"
        const val RECEIVE_COPIED = "receive.copied"
        const val RECEIVE_SAVE_IMAGE = "receive.request.saveImage"
        const val SHARE_CARD_HEADLINE = "receive.shareCardHeadline"
        const val SHARE_CARD_NETWORK_NOTE = "receive.shareCardNetworkNote"

        // Scan.
        const val SCAN_TITLE = "componentsUi.scanner.title"
        const val SCAN_HINT = "componentsUi.scanner.hint"
        const val SCAN_GALLERY = "componentsUi.scanner.gallery"
        const val SCAN_FROM_GALLERY = "componentsUi.scanner.fromGallery"
        const val SCAN_TORCH = "componentsUi.scanner.torch"
        const val SCAN_FLIP = "componentsUi.scanner.flipCamera"
        const val SCAN_PERMISSION_TEXT = "componentsUi.scanner.permissionText"
        const val SCAN_GRANT = "componentsUi.scanner.grantPermission"
        const val SCAN_NO_QR = "componentsUi.scanner.noQrFoundMsg"
        const val SCAN_CAMERA_UNAVAILABLE = "componentsUi.scanner.cameraUnavailable"
        const val SCAN_ERROR_IMAGE = "componentsUi.scanner.errorImage"
        /** A code read, but not one this surface can use (the web scanner's `unusable`). */
        const val SCAN_INVALID_QR = "home.invalidQrTitle"

        // Activity.
        const val HISTORY_TITLE = "history.navTitle"
        const val HISTORY_LOADING = "history.loadingText"
        const val HISTORY_EMPTY_FILTER = "history.emptyFilter"
        const val LABEL_SENT = "history.labelSent"
        const val LABEL_RECEIVED = "history.labelReceived"
        const val TX_LABEL_SENT = "history.txLabelSent"
        const val TX_LABEL_RECEIVED = "history.txLabelReceived"
        const val TO_NAME = "history.toName"
        const val FROM_NAME = "history.fromName"
        const val VIEW_ON_EXPLORER = "history.viewOnExplorer"
        const val STATUS_CONFIRMED = "componentsTx.receipt.statusConfirmed"
        const val DETAIL_FROM = "componentsTx.detail.from"
        const val DETAIL_TO = "componentsTx.detail.to"
        const val DETAIL_CHAIN = "componentsTx.detail.labelChain"
        const val DETAIL_DATE = "componentsTx.detail.labelDate"
        const val DETAIL_HASH = "componentsTx.detail.labelHash"
        const val DETAIL_SECTION_TITLE = "componentsTx.detail.sectionTitle"

        // Assets.
        const val ASSETS_TITLE = "assets.sectionTitle"
        const val ASSETS_ADD = "assets.addToken"
        const val ASSETS_SEARCH = "assets.searchPlaceholder"
        const val ASSETS_ADD_BY_ADDRESS = "assets.addByAddress"
        const val ASSETS_EMPTY_TITLE = "assets.emptyTitle"
        const val ASSETS_EMPTY_SUBTEXT = "assets.emptySubtext"
        const val ASSETS_NOT_SHOWING_TITLE = "assets.notShowingTitle"
        const val ASSETS_NOT_SHOWING_BODY = "assets.notShowingBody"
        const val TOKEN_SEND = "tokenDetail.send"
        const val TOKEN_RECEIVE = "tokenDetail.receive"
        const val TOKEN_PRICE = "tokenDetail.labelPrice"
        const val TOKEN_PRICE_VALUE = "tokenDetail.priceValue"
        const val TOKEN_CONTRACT = "tokenDetail.labelContract"
        const val TOKEN_DECIMALS = "tokenDetail.labelDecimals"
        const val TOKEN_TRANSACTIONS = "tokenDetail.labelTransactions"
        const val TOKEN_EXPLORER = "tokenDetail.viewOnExplorer"

        // Add token / add network.
        const val ADD_TOKEN_TITLE = "addToken.navTitle"
        const val ADD_TAB_ERC20 = "addToken.tabErc20"
        const val ADD_TAB_NATIVE = "addToken.tabNative"
        const val ADD_LABEL_NETWORK = "addToken.labelNetwork"
        const val ADD_TOKEN_ADDRESS = "addToken.tokenAddressLabel"
        const val ADD_TO_WALLET = "addToken.addToWalletBtn"
        const val ADD_TOKEN_ADDED = "addToken.tokenAdded"
        const val ADD_INVALID_ADDRESS = "addToken.invalidAddress"
        const val ADD_NOT_FOUND_TITLE = "addToken.notFoundTitle"
        const val ADD_NOT_FOUND_MESSAGE = "addToken.notFoundMessage"
        const val TREASURY_OPERATOR_LEAD = "componentsUi.treasuryBootstrap.operatorLead"
        const val TREASURY_CUSTOM_LEAD = "componentsUi.treasuryBootstrap.customLead"
        const val ADD_NATIVE_ALIAS_TITLE = "addToken.nativeAliasTitle"
        const val ADD_NATIVE_ALIAS_MESSAGE = "addToken.nativeAliasMessage"
        const val ADD_NET_SEARCH_LABEL = "addToken.netSearchLabel"
        const val ADD_NET_SEARCH_PLACEHOLDER = "addToken.netSearchPlaceholder"
        const val ADD_NET_PICKER_EMPTY = "addToken.netPickerEmpty"
        const val ADD_NET_PICKER_SEARCH = "addToken.netPickerSearchPlaceholder"
        const val ADD_CHAIN_ID = "addToken.labelChainId"
        const val ADD_NATIVE_TOKEN = "addToken.labelNativeToken"
        const val ADD_COMPATIBLE = "addToken.compatible"
        const val ADD_NOT_COMPATIBLE = "addToken.notCompatible"
        const val ADD_NETWORK_ADDED = "addToken.networkAdded"
        const val ADD_NETWORK_BTN = "addToken.addNetworkBtn"
        const val ADD_SEARCHING = "addToken.searchingNetworks"
        const val ADD_ERROR_SAVE = "addToken.errorSaveToken"
        const val ADD_LABEL_DECIMALS = "addToken.labelDecimals"
        const val ADD_DEPLOY_CONTRACTS = "addToken.deployContracts"
        const val ADD_ERROR_NOT_COMPATIBLE = "addToken.errorNotCompatible"

        // Send.
        const val SELECT_TOKEN_TITLE = "send.selectTokenTitle"
        const val SEND_SEARCH = "send.searchPlaceholder"
        const val FILTER_ALL = "history.filterAll"
        const val FILTER_STABLE = "send.filterStable"
        const val FILTER_GAS = "send.filterGas"
        const val FILTER_OTHER = "send.filterOther"
        const val MULTI_SEND_TITLE = "send.multiSendTitle"
        const val MULTI_SEND_SUMMARY = "send.multiSendSummary"
        const val MULTI_SEND_NOTICE = "send.multiSendChainNotice"
        const val MULTI_SEND_SAME_RECIPIENT = "send.multiSendSameRecipient"
        const val MULTI_SEND_CONTINUE = "send.multiSendContinue"
        const val SELECT_ALL_VALUABLE = "send.selectAllValuable"
        const val SEND_TITLE = "send.sendTitle"
        const val MAX = "send.maxBtn"
        const val BALANCE_LABEL = "send.balanceLabel"
        const val RECIPIENT_LABEL = "send.recipientLabel"
        const val RECIPIENT_N = "send.recipientN"
        const val RECIPIENT_COUNT = "send.recipientCount_other"
        const val RECIPIENT_COUNT_ONE = "send.recipientCount_one"
        const val ADD_RECIPIENT = "send.addRecipient"
        const val FROM_CONTACTS = "send.fromContacts"
        const val BATCH_IMPORT = "send.batchImport"
        const val REMOVE_RECIPIENT = "send.removeRecipient"
        const val RECIPIENT_PICK_ARIA = "send.recipientPickAria"
        const val SCAN_ARIA = "send.scanAria"
        const val SPLIT_TOTAL = "send.splitTotalLabel"
        const val SPLIT_NEEDS_AMOUNT = "send.splitNeedsAmount"
        const val SPLIT_NEEDS_ADDRESS = "send.splitNeedsAddress"
        const val SPLIT_REMAINING = "send.splitRemaining"
        const val SPLIT_FILL_EMPTY = "send.splitFillEmpty"
        const val RECIPIENT_DUPLICATE = "send.recipientDuplicate"
        const val BAD_AMOUNT = "send.badAmount"
        const val CONTINUE = "send.continueBtn"
        const val NETWORK_FEE = "componentsUi.gas.networkFee"

        // Send · fee token.
        const val FEE_TOKEN_LABEL = "send.feeTokenLabel"
        const val FEE_TOKEN_HINT = "send.feeTokenHint"
        const val FEE_TOKEN_ESTIMATE = "send.feeTokenEstimate"

        // Send · contact picker.
        const val PICK_CONTACT_TITLE = "send.pickContactTitle"
        const val PICK_CONTACT_SEARCH = "send.pickContactSearch"
        const val SCAN_TO_FILL = "send.scanToFill"
        const val CONTACTS_GROUPS = "contacts.sectionGroups"
        const val CONTACTS_TITLE = "contacts.title"
        const val GROUP_MEMBERS = "contacts.groupMembers"

        // Send · batch import.
        const val BATCH_TITLE = "send.batchTitle"
        const val BATCH_UNIT_FIAT = "send.batchUnitFiat"
        const val BATCH_UNIT_TOKEN = "send.batchUnitToken"
        const val BATCH_PASTE_PLACEHOLDER = "send.batchPastePlaceholder"
        const val BATCH_IMPORT_FILE = "send.batchImportFile"
        const val BATCH_TEMPLATE = "send.batchTemplate"
        const val BATCH_RATE_SECTION = "send.batchRateSection"
        const val BATCH_RATE_LABEL = "send.batchRateLabel"
        const val BATCH_RATE_HINT = "send.batchRateHint"
        const val BATCH_PARSED_COUNT = "send.batchParsedCount"
        const val BATCH_BAD_ADDRESS = "send.batchBadAddress"
        const val BATCH_DUP = "send.batchDup"
        const val BATCH_IMPORT_FAILED_TITLE = "send.batchImportFailedTitle"
        const val BATCH_IMPORT_FAILED_BODY = "send.batchImportFailedBody"
        const val BATCH_REJECTED_ONE = "send.batchRejected_one"
        const val BATCH_APPLY_OTHER = "send.batchApply_other"
        const val BATCH_REJECTED_OTHER = "send.batchRejected_other"
        const val BATCH_APPLY_ONE = "send.batchApply_one"
        const val BATCH_APPLY_EMPTY = "send.batchApplyEmpty"
        const val BATCH_RATE_LOADING = "send.batchRateLoading"
        const val BATCH_RATE_FAILED = "send.batchRateFailed"
        const val BATCH_RATE_RESET = "send.batchRateReset"
        const val BATCH_OVER_CAP = "send.batchOverCap"
        const val BATCH_OVER_BALANCE = "send.batchOverBalance"
        const val BATCH_TOKEN_HINT = "send.batchTokenHint"
        const val BATCH_ADDS_TO_ROWS = "send.batchAddsToRows"
        const val BATCH_REPLACES_ROWS = "send.batchReplacesRows"
        const val BATCH_REPLACE_INSTEAD = "send.batchReplaceInstead"
        const val BATCH_ADD_INSTEAD = "send.batchAddInstead"
        const val BATCH_TEMPLATE_SAVED = "send.batchTemplateSaved"
        const val FUNDING_CANCEL = "componentsUi.funding.cancel"

        // Send · confirm.
        const val CONFIRM_TITLE = "send.confirmTitle"
        const val FROM_LABEL = "send.fromLabel"
        const val TO_LABEL = "send.toLabel"
        const val EST_FEE = "send.estFeeLabel"
        const val CONFIRM_SEND = "send.confirmSendBtn"
        const val CONFIRM_TOTAL_LINE = "send.confirmTotalLine"
        const val ASSETS_COUNT = "componentsTx.receipt.assetsCount"

        // Send · receipt.
        const val TX_SUBMITTING = "send.txSubmitting"
        const val TX_PREPARING_BIOMETRIC = "send.txPreparingBiometric"
        const val TX_BACKGROUND_HINT = "send.txBackgroundHint"
        const val TX_CLOSE_BACKGROUND = "send.txCloseBackground"
        const val TX_SUBMITTED_TITLE = "send.txSubmittedTitle"
        const val TX_CONFIRMED_TITLE = "send.txConfirmedTitle"
        const val TX_WAITING_CONFIRM = "send.txWaitingConfirm"
        const val TX_TYPICAL_TIME = "send.txTypicalTime"
        const val TX_REMAINING = "send.txRemaining"
        const val TX_ELAPSED = "send.txElapsed"
        const val TX_SLOW_CONFIRM = "send.txSlowConfirm"
        const val TX_HASH = "componentsTx.receipt.txHash"
        const val DONE = "componentsTx.receipt.done"

        // Spec 043: the live send's remaining words — every one already in
        // the corpus; the fixture never needed them because it never failed.
        const val FEE_ESTIMATING = "componentsUi.gas.estimating"

        // Spec 068 (Android's since 069): the fee you can refresh, at a speed
        // you can choose. `send.gasTier.rapid` is deliberately absent — the
        // tier is dead, the relay refuses it, and nothing here may name it.
        const val FEE_REFRESH = "send.feeRefresh"
        const val FEE_STALE = "send.feeStale"
        const val FEE_SPEED_LABEL = "send.feeSpeedLabel"
        const val FEE_SPEED_ONCE = "send.feeSpeedOnce"
        const val FEE_SPEED_FREE = "send.feeSpeedFree"
        const val FEE_SPEED_SINGLE = "send.feeSpeedSingle"
        const val GAS_PRICE_LABEL = "send.gasPriceLabel"
        const val GAS_TIER_FAST = "send.gasTier.fast"
        const val GAS_TIER_STANDARD = "send.gasTier.standard"
        const val GAS_TIER_SLOW = "send.gasTier.slow"
        const val GAS_TIER_HINT_FAST = "send.gasTierHintFast"
        const val GAS_TIER_HINT_STANDARD = "send.gasTierHintStandard"
        const val GAS_TIER_HINT_SLOW = "send.gasTierHintSlow"
        const val CANNOT_CONVERT = "send.warnCannotConvert"
        const val TX_PREPARING = "send.txPreparing"
        const val TX_SIGNING = "send.txSigning"
        const val STATUS_FAILED = "componentsTx.receipt.statusFailed"
        const val TX_FAILED_HINT = "componentsTx.receipt.failedHint"
        const val TX_HELD_FEES = "send.txHeldFees"
        const val TX_REJECTED_FEES = "send.txRejectedFees"
        const val ALERT_ESTIMATE_TITLE = "send.alertEstimateFailedTitle"
        const val ALERT_ESTIMATE_BODY = "send.alertEstimateFailedBody"
        const val ALERT_LOAD_TOKENS = "send.alertLoadTokensError"
        const val TX_ERROR_GENERIC = "send.txErrorGeneric"
        const val WARN_INSUFFICIENT_FOR_GAS = "send.warnInsufficientForGas"
        const val WARN_INSUFFICIENT_GAS = "send.warnInsufficientGas"
        const val WARN_NEED_GAS = "send.warnNeedGas"
        // Parity with the web's send form: ⇄'s refusal (issue 197), the empty
        // picker's two sentences (issue 209), the first-time recipient tell.
        const val DENOM_TOGGLE_NO_RATE = "send.denomToggleNoRate"
        const val NO_TOKENS_WITH_BALANCE = "send.noTokensWithBalance"
        const val NO_MATCHING_TOKENS = "send.noMatchingTokens"
        const val FIRST_TIME_SEND = "componentsUi.signing.firstTimeTag"
        /** The notification when a verdict lands while the app is away (phase 4). */
        const val TX_CONFIRMED_NOTICE = "componentsTx.receipt.statusConfirmed"
        const val TX_CONFIRMED_NOTICE_BODY = "send.txSubmittedTitle"
        // Phase 5: every refusal in the core's words.
        const val ALERT_INVALID_ADDRESS_TITLE = "send.alertInvalidAddressTitle"
        const val ALERT_INVALID_ADDRESS_BODY = "send.alertInvalidAddressBody"
        const val ALERT_INVALID_AMOUNT_TITLE = "send.alertInvalidAmountTitle"
        const val ALERT_INVALID_AMOUNT_BODY = "send.alertInvalidAmountBody"
        const val ALERT_INSUFFICIENT_TITLE = "send.alertInsufficientBalanceTitle"
        const val ALERT_INSUFFICIENT_BODY = "send.alertInsufficientBalanceBody"
        const val ALERT_ACCOUNT_UNAVAILABLE_BODY = "send.alertAccountUnavailableBody"
        const val SAME_FEE_TITLE = "send.sameFeeTokenTitle"
        const val SAME_FEE_BODY = "send.sameFeeTokenBody"
        const val SAME_FEE_MAX = "send.sameFeeTokenMax"
        const val TREASURY_TITLE = "componentsUi.treasuryBootstrap.title"
        const val TREASURY_LEAD = "componentsUi.treasuryBootstrap.lead"
        const val TREASURY_AMOUNT_HINT = "componentsUi.treasuryBootstrap.amountHint"
        const val TREASURY_RETRY = "componentsUi.treasuryBootstrap.retryBtn"
        const val TX_ERROR_BUNDLER_FUND = "send.txErrorBundlerFund"
        const val TX_RETRY = "send.txRetryBtn"
        const val CANCEL = "home.cancel"
        const val STATUS_PENDING = "componentsTx.detail.statusPending"
        const val STATUS_SUCCEEDED = "componentsTx.detail.statusSucceeded"
    }

    /**
     * The settings SCREEN (spec 023) — distinct from [Settings] above, which is
     * the onboarding flow's own `onboarding.settings.*` endpoint surface.
     *
     * Most of these keys predate the feature: the
     * `settings.*` namespace has shipped since the React Native app, and
     * `settingsModals.*`, `about.*`, `assets.rpcFix*`, `home.balanceDetail*`,
     * `componentsUi.bugReport.*` and `componentsUi.treasuryBootstrap.*` all
     * describe screens these mocks redraw.
     */
    object SettingsUi {
        const val TITLE = "settings.title"

        // Sections.
        const val SECTION_ACCOUNT = "settings.sections.account"
        const val SECTION_APPEARANCE = "settings.sections.appearance"
        const val SECTION_LOCALIZATION = "settings.sections.localization"
        const val SECTION_ADVANCED = "settings.sections.advanced"

        // Home rows.
        const val ACCOUNT_SWITCH = "settings.account.switch"
        const val ACCOUNT_REMOVE = "settings.account.remove"
        const val ACCOUNT_REMOVE_BODY = "settings.account.removeBody"
        const val CONTACTS_SUBTITLE = "settings.account.contactsSubtitle"
        const val FEEDBACK_TITLE = "settings.feedback.title"
        const val FEEDBACK_SUBTITLE = "settings.feedback.subtitle"

        // Appearance.
        const val THEME_TITLE = "settings.appearance.themeTitle"
        const val THEME_LIGHT = "settings.appearance.themeLight"
        const val THEME_DARK = "settings.appearance.themeDark"
        const val THEME_AUTO = "settings.appearance.themeAuto"
        const val TEXT_SCALE = "settings.appearance.textScale"

        // Language picker.
        const val LANGUAGE_TITLE = "language.title"
        const val LANGUAGE_PICKER_TITLE = "language.pickerTitle"
        const val LANGUAGE_PICKER_SUBTITLE = "language.pickerSubtitle"
        const val LANGUAGE_FOLLOW_SYSTEM = "language.followSystem"
        const val LANGUAGE_CONTRIBUTE_NOTE = "language.contributeNote"
        const val LANGUAGE_CONTRIBUTE_CTA = "language.contributeCta"

        // Localization.
        const val CURRENCY_TITLE = "settings.localization.currencyTitle"
        const val NUMBER_TITLE = "settings.localization.numberTitle"
        const val NUMBER_SUBTITLE = "settings.localization.numberSubtitle"
        const val DATE_TITLE = "settings.localization.dateTitle"
        const val DATE_SUBTITLE = "settings.localization.dateSubtitle"
        const val TIME_TITLE = "settings.localization.timeTitle"
        const val TIME_SUBTITLE = "settings.localization.timeSubtitle"
        const val NOTE_INDIAN = "settings.formatNote.indian"
        const val NOTE_H24 = "settings.formatNote.h24"
        const val NOTE_H12 = "settings.formatNote.h12"
        const val CURRENCY_SHEET_TITLE = "componentsUi.currency.title"
        const val CURRENCY_SEARCH = "componentsUi.currency.searchPlaceholder"

        // Advanced.
        const val NETWORKS_TITLE = "settings.advanced.networksTitle"
        const val NETWORKS_SUBTITLE = "settings.advanced.networksSubtitle"
        const val RPC_PROVIDERS_TITLE = "settings.advanced.rpcProvidersTitle"
        const val RPC_PROVIDERS_SUBTITLE = "settings.advanced.rpcProvidersSubtitle"
        const val ADD_NETWORK_TITLE = "settings.advanced.addNetworkTitle"
        const val ADD_NETWORK_SUBTITLE = "settings.advanced.addNetworkSubtitle"
        const val ENDPOINTS_TITLE = "settings.advanced.endpointsTitle"
        const val ENDPOINTS_SUBTITLE = "settings.advanced.endpointsSubtitle"
        // Spec 069: the default transaction speed — the row, and its sheet.
        const val FEE_SPEED_TITLE = "settings.advanced.feeSpeedTitle"
        const val FEE_SPEED_SUBTITLE = "settings.advanced.feeSpeedSubtitle"
        const val FEE_SPEED_SHEET_TITLE = "settings.feeSpeed.title"
        const val FEE_SPEED_SHEET_SUBTITLE = "settings.feeSpeed.subtitle"

        // Network list + detail.
        const val NETWORK_COUNT = "settings.networks.count"
        const val NETWORK_CUSTOM = "settings.networks.custom"
        const val NETWORK_BUILTIN_NOTE = "settings.networks.builtinNote"
        const val NETWORK_SAVE_HINT = "settings.networks.saveHint"
        const val NETWORK_ONLINE = "settings.networks.online"
        const val NETWORK_SLOW = "settings.networks.slow"
        const val NETWORK_OFFLINE = "settingsModals.health.offline"
        const val HEALTH_HTTPS_REQUIRED = "settingsModals.health.httpsRequired"
        const val HEALTH_INVALID = "settingsModals.health.invalid"

        /** The Ethereum backup row (spec 062). */
        const val BACKUP_TITLE = "settingsModals.backup.title"
        const val BACKUP_BACKED_UP = "settingsModals.backup.backedUp"
        const val BACKUP_NOT_BACKED_UP = "settingsModals.backup.notBackedUp"
        const val BACKUP_COULD_NOT_CHECK = "settingsModals.backup.couldNotCheck"
        const val BACKUP_CHECKING = "componentsUi.funding.checking"
        const val KEYS_TITLE = "settingsModals.keys.title"
        const val KEYS_SUBTITLE = "settingsModals.keys.subtitle"
        const val KEYS_KEY_N = "settingsModals.keys.keyN"
        const val KEYS_NOT_SYNCED = "settingsModals.keys.notSynced"
        const val KEYS_FROM_DEVICE = "settingsModals.keys.fromDevice"
        const val KEYS_SYNCED = "onboarding.create.keySyncedBadge"
        const val KEYS_PUBLIC_KEY = "settingsModals.keys.publicKey"
        const val KEYS_CREDENTIAL = "settingsModals.keys.credential"
        const val KEYS_TRANSPORT = "settingsModals.keys.transport"
        const val KEYS_ATTESTATION = "settingsModals.keys.attestation"
        const val KEYS_USER_VERIFIED = "settingsModals.keys.userVerified"
        const val KEYS_COPY = "componentsUi.signing.copyValue"
        const val KEYS_COPIED = "receive.copied"
        const val BACKUP_EXPLAIN = "settingsModals.backup.explain"
        const val KEYS_PROVIDER_PLATFORM = "onboarding.create.providerPlatform"
        const val KEYS_PROVIDER_GENERIC = "onboarding.create.providerGeneric"
        const val KEYS_PROVIDER_SECURITY_KEY = "onboarding.create.providerSecurityKey"
        const val CHAIN_ID = "settingsModals.network.chainId"
        const val FIELD_RPC_URL = "settingsModals.network.fieldRpcUrl"
        const val FIELD_EXPLORER = "settingsModals.network.fieldExplorer"
        const val RPC_CHAIN_MISMATCH = "settingsModals.network.rpcChainMismatch"

        // Add network.
        const val ADD_DESCRIPTION = "settingsModals.addNetwork.description"
        const val ADD_SEARCH = "settingsModals.addNetwork.searchPlaceholder"
        const val ADD_COMPATIBLE = "settingsModals.addNetwork.compatible"
        const val ADD_INCOMPATIBLE = "settingsModals.addNetwork.incompatible"
        const val ADD_COMPATIBILITY_CHECK = "settingsModals.addNetwork.compatibilityCheck"
        const val ADD_CHECKING_COMPATIBILITY = "settingsModals.addNetwork.checkingCompatibility"
        const val ADD_CHECK_SAFE = "settingsModals.addNetwork.checkSafe"
        const val ADD_CHECK_SIGNER = "settingsModals.addNetwork.checkSigner"
        const val ADD_CHECK_REMAINING = "settingsModals.addNetwork.checkRemaining"
        const val ADD_CUSTOM_RPC_TITLE = "settingsModals.addNetwork.customRpcTitle"
        const val ADD_CUSTOM_RPC_PLACEHOLDER = "settingsModals.addNetwork.customRpcPlaceholder"
        const val ADD_BEST_RPC = "settingsModals.addNetwork.bestRpc"
        const val ADD_BUTTON = "settingsModals.addNetwork.addNetworkBtn"
        const val ADD_INCOMPATIBLE_HINT = "settingsModals.addNetwork.incompatibleHint"

        /** Spec 081 FR-009: compatible for one key, not for two to seven. */
        const val ADD_SINGLE_KEY_ONLY = "settingsModals.addNetwork.singleKeyOnly"
        const val ADD_CHAIN_TOOL = "settingsModals.addNetwork.openChainSetupTool"
        const val ADD_RECHECK_WITH_RPC = "settingsModals.addNetwork.recheckWithRpc"
        const val ADD_TESTNET = "settingsModals.addNetwork.testnet"

        // RPC providers.
        const val PROVIDERS_DESCRIPTION = "settingsModals.rpcProviders.description"
        const val PROVIDER_GET_KEY = "settingsModals.rpcProviders.getKey"
        const val PROVIDER_CHECK_KEY = "settingsModals.rpcProviders.checkKey"
        const val PROVIDER_NOT_SET = "settingsModals.rpcProviders.notSet"
        const val PROVIDER_CONNECTED = "activity.connected"
        const val PROVIDER_SUPPORTS = "settingsModals.rpcProviders.supportsCount"

        // Service endpoints.
        const val ENDPOINTS_DESCRIPTION = "settingsModals.endpoints.description"
        const val ENDPOINT_CHAIN_DATA = "settingsModals.endpoints.chainDataLabel"
        const val ENDPOINT_CHAIN_DATA_HINT = "settingsModals.endpoints.chainDataHint"
        const val ENDPOINT_PASSKEY = "settingsModals.endpoints.passkeyLabel"
        const val ENDPOINT_PASSKEY_HINT = "settingsModals.endpoints.passkeyHint"
        const val ENDPOINT_RELAY = "settingsModals.endpoints.bundlerLabel"
        const val ENDPOINT_RELAY_HINT = "settingsModals.endpoints.bundlerHint"
        const val ENDPOINT_FIAT = "settingsModals.endpoints.fiatLabel"
        const val ENDPOINT_FIAT_HINT = "settingsModals.endpoints.fiatHint"
        const val ENDPOINTS_RESET = "settingsModals.endpoints.resetToDefaults"
        const val ENDPOINTS_RESET_TITLE = "settingsModals.endpoints.resetTitle"
        const val ENDPOINTS_RESET_BODY = "settingsModals.endpoints.resetBody"
        const val ENDPOINTS_RESET_CONFIRM = "settingsModals.endpoints.resetConfirm"
        const val ENDPOINTS_RESET_CANCEL = "settingsModals.endpoints.resetCancel"

        // Device storage.
        const val STORAGE_TITLE = "settings.storage.title"
        const val STORAGE_SUBTITLE = "settings.storage.subtitle"
        const val STORAGE_SUMMARY = "settings.storage.summary"
        const val STORAGE_USER_DATA = "settings.storage.userData"
        const val STORAGE_CACHES = "settings.storage.caches"
        const val STORAGE_CONNECTIONS = "settings.storage.connections"
        const val LEGEND_USER_DATA = "settings.storage.legendUserData"
        const val LEGEND_CACHES = "settings.storage.legendCaches"
        const val LEGEND_SESSIONS = "settings.storage.legendSessions"
        const val ITEM_TRANSACTIONS = "settings.storage.itemTransactions"
        const val ITEM_CONTACTS = "settings.storage.itemContacts"
        const val ITEM_CUSTOM = "settings.storage.itemCustom"
        const val ITEM_BROWSING = "settings.storage.itemBrowsing"
        const val ITEM_BALANCES = "settings.storage.itemBalances"
        const val ITEM_RATES = "settings.storage.itemRates"
        const val ITEM_SCAN = "settings.storage.itemScan"
        const val ITEM_DAPPS = "settings.storage.itemDapps"
        const val COUNT_RECORDS = "settings.storage.records"
        const val COUNT_CONTACTS = "settings.storage.contactsCount"
        const val COUNT_ITEMS = "settings.storage.itemsCount"
        const val COUNT_SITES = "settings.storage.sitesCount"
        const val STORAGE_CLEAR = "settings.storage.clear"
        const val STORAGE_CLEAR_ALL = "settings.storage.clearAllCaches"
        const val STORAGE_DISCONNECT_ALL = "settings.storage.disconnectAll"
        const val STORAGE_CLEAR_TITLE = "settings.storage.clearTitle"
        const val STORAGE_CLEAR_BODY = "settings.storage.clearBody"
        const val STORAGE_CLEAR_CONFIRM = "settings.storage.clearConfirm"

        // About.
        const val ABOUT_TITLE = "settings.about.title"
        const val ABOUT_SUBTITLE = "settings.about.subtitle"
        const val ABOUT_TAGLINE = "about.tagline"
        const val ABOUT_VERSION = "about.version"
        const val ABOUT_SECTION_TECHNICAL = "about.sectionTechnical"
        const val ABOUT_SECTION_LINKS = "about.sectionLinks"
        const val ABOUT_WALLET_LABEL = "about.techWalletLabel"
        const val ABOUT_WALLET_VALUE = "about.techWalletValue"
        const val ABOUT_AUTH_LABEL = "about.techAuthLabel"
        const val ABOUT_AUTH_VALUE = "about.techAuthValue"
        const val ABOUT_ACCOUNT_LABEL = "about.techAccountTypeLabel"
        const val ABOUT_ACCOUNT_VALUE = "about.techAccountTypeValue"
        const val ABOUT_SIGNER_LABEL = "about.techSignerLabel"
        const val ABOUT_SIGNER_VALUE = "about.techSignerValue"
        const val ABOUT_NETWORKS_LABEL = "about.techNetworksLabel"
        const val ABOUT_NETWORKS_VALUE = "about.techNetworksValue"
        const val ABOUT_LINK_WEBSITE = "about.linkWebsite"
        const val ABOUT_LINK_GITHUB = "about.linkGitHub"
        const val ABOUT_LINK_SAFE = "about.linkSafeWallet"
        const val ABOUT_FOOTER = "about.footer"

        // Account switcher + sign out + erase.
        const val ACCOUNTS_TITLE = "settingsModals.account.modalTitle"
        const val ACCOUNTS_TOTAL = "settingsModals.account.total"
        const val ACCOUNTS_COUNT = "home.switcherAccountCount"
        const val ACCOUNT_CREATE = "settingsModals.account.createNew"
        const val ACCOUNT_SIGN_IN = "settingsModals.account.signInExisting"
        const val SIGN_OUT_BUTTON = "settings.signOut.button"
        const val SIGN_OUT_TITLE = "settings.signOut.title"
        const val SIGN_OUT_DESC = "settings.signOut.desc"
        const val SIGN_OUT_KEEPS = "settings.signOut.keeps"
        const val SIGN_OUT_WARNING = "settings.signOut.warning"
        const val SIGN_OUT_ANYWAY = "settings.signOut.anyway"
        const val SIGN_OUT_CANCEL = "settings.signOut.cancel"
        const val ERASE_TITLE = "settings.eraseDevice.title"
        const val ERASE_SUBTITLE = "settings.eraseDevice.subtitle"
        const val ERASE_DESC = "settings.eraseDevice.desc"
        const val ERASE_LOSES = "settings.eraseDevice.loses"
        const val ERASE_KEEPS = "settings.eraseDevice.keeps"
        const val ERASE_CONFIRM = "settings.eraseDevice.confirm"
        const val ERASE_CANCEL = "settings.eraseDevice.cancel"

        // Feedback.
        const val BUG_TITLE = "componentsUi.bugReport.title"
        const val BUG_SUBTITLE = "componentsUi.bugReport.subtitle"
        const val BUG_PLACEHOLDER = "componentsUi.bugReport.whatPlaceholder"
        const val BUG_ADD_STEPS = "componentsUi.bugReport.addSteps"
        const val BUG_PREVIEW_TOGGLE = "componentsUi.bugReport.previewToggle"
        const val BUG_PREVIEW_VERSION = "componentsUi.bugReport.previewVersion"
        const val BUG_PREVIEW_PLATFORM = "componentsUi.bugReport.previewPlatform"
        const val BUG_PREVIEW_LANGUAGE = "componentsUi.bugReport.previewLanguage"
        const val BUG_PREVIEW_RPC = "componentsUi.bugReport.previewRpc"
        const val BUG_PREVIEW_FAILURES = "componentsUi.bugReport.previewFailures"
        const val BUG_PREVIEW_NONE = "componentsUi.bugReport.previewNone"
        const val BUG_CONSENT = "componentsUi.bugReport.consent"
        const val BUG_SEND = "componentsUi.bugReport.send"
        const val BUG_GITHUB = "componentsUi.bugReport.openGithubForm"

        // Rescue (SR1–SR5).
        const val RPC_UNAVAILABLE_MULTIPLE = "assets.rpcUnavailableMultiple"
        const val RPC_FIX = "assets.rpcFix"
        const val RPC_FIX_TITLE = "assets.rpcFixTitle"
        const val RPC_FIX_WARNING = "assets.rpcFixWarning"
        const val RPC_FIX_LABEL = "assets.rpcFixLabel"
        const val RPC_FIX_SAVE = "assets.rpcFixSaveBtn"
        const val RPC_FIX_RESTORED = "assets.rpcFixRestored"
        const val RPC_PROVIDERS_HINT = "assets.rpcProvidersTitle"
        const val RPC_REPORT = "assets.rpcReport"
        const val BALANCE_DETAIL_TITLE = "home.balanceDetailTitle"
        const val BALANCE_DETAIL_TOTAL = "assets.switcherTotal"
        const val BALANCE_DETAIL_NETWORKS = "home.balanceDetailNetworksLabel"
        const val BALANCE_DETAIL_NOTE = "home.balanceDetailNetworksNote"
        const val BALANCE_DETAIL_RETRYING = "home.balanceDetailStatusRetrying"
        const val BALANCE_DETAIL_FAILED = "home.balanceDetailStatusFailed"
        const val BALANCE_DETAIL_UPDATED = "home.balanceDetailUpdatedLabel"
        const val BALANCE_DETAIL_RETRY = "home.balanceDetailRetry"
        const val RELAYER_TITLE = "componentsUi.treasuryBootstrap.title"
        const val RELAYER_LEAD = "componentsUi.treasuryBootstrap.lead"
        const val RELAYER_AMOUNT_HINT = "componentsUi.treasuryBootstrap.amountHint"
        const val RELAYER_ADDRESS_LABEL = "componentsUi.treasuryBootstrap.addressLabel"
        const val RELAYER_DISCLAIMER = "componentsUi.treasuryBootstrap.disclaimer"
        const val RELAYER_COPY = "componentsUi.treasuryBootstrap.copyBtn"
        const val RELAYER_RETRY = "componentsUi.treasuryBootstrap.retryBtn"
        const val INDEX_DOWN_TITLE = "settings.indexDown.title"
        const val INDEX_DOWN_SUBTITLE = "settings.indexDown.subtitle"
        const val INDEX_DOWN_WARNING = "onboarding.settings.warningText"
        const val INDEX_DOWN_ENDPOINT_LABEL = "onboarding.settings.endpointUrlLabel"
        const val INDEX_DOWN_EDIT = "settings.indexDown.editEndpoint"
        const val INDEX_DOWN_FOOTER = "onboarding.settings.passkeyHint"

        // Shared.
        const val COMMON_CANCEL = "common.cancel"
        const val COMMON_SYSTEM = "common.system"
        const val COMMON_AUTOMATIC = "common.automatic"
        const val COMMON_DONE = "common.done"
        const val COMMON_TRY_AGAIN = "common.tryAgain"
        const val CLOSE = "componentsUi.identiconViewer.close"
        const val NAV_WALLET = "componentsUi.mainNav.wallet"
        const val NAV_CONTACTS = "componentsUi.mainNav.contacts"
        const val NAV_EXPLORE = "componentsUi.mainNav.explore"
        const val NAV_SETTINGS = "componentsUi.mainNav.settings"
        const val ACTION_SEND = "componentsUi.dock.send"
    }
}
