//! Settings (spec 023): the desktop's second-level nav and its eight panels.
//!
//! Same split the contacts module uses — `fixtures` is the canonical data read
//! off `design/settings/*.png`, `components` the reusable visuals — and the
//! page state lives in `wallet::page`, which already owns the three-column
//! shell this section slots into as a `Section::Settings` switch.
//!
//! Nothing here reaches for a preference store. Every value is a fixture, so
//! the later "wire real settings" feature replaces `fixtures.rs` and leaves
//! the visuals alone.

pub mod components;
pub mod fixtures;
pub mod live;
pub mod model;

use gpui::SharedString;

use crate::loc::Loc;

/// Every settings string the desktop renders, resolved once per locale.
///
/// Most of these keys predate this feature: the `settings.*` namespace has
/// shipped since the React Native app, and `settingsModals.*`, `about.*` and
/// `assets.rpcFix*` all describe screens these mocks redraw. Spec 023 added
/// the storage namespace, the network-list chrome and a handful of labels.
pub struct SettingsStrings {
    pub title: SharedString,
    // second-level nav
    pub nav_account: SharedString,
    pub nav_appearance: SharedString,
    pub nav_localization: SharedString,
    pub nav_networks: SharedString,
    pub nav_rpc_providers: SharedString,
    pub nav_endpoints: SharedString,
    pub nav_storage: SharedString,
    /// The default transaction speed (spec 069): the nav row, and the page's
    /// title and the sentence under it.
    pub nav_fee_speed: SharedString,
    pub fee_speed_title: SharedString,
    pub fee_speed_subtitle: SharedString,
    /// Settings → Signing pages (spec 102): the nav row and the sentence
    /// under it — the pages a person trusts, the official one first. Which
    /// page an account signs on is the account's own setting ("Where you
    /// review and sign"), never this page's.
    pub nav_signing: SharedString,
    pub signing_subtitle: SharedString,
    pub signer_page_invalid: SharedString,
    pub signer_page_insecure: SharedString,
    pub signer_page_duplicate: SharedString,
    pub signer_page_add: SharedString,
    /// Spec 102 core round 10: Settings → Signing pages' own words — rename a
    /// page, remove it, its name field and its save (a row's "Trust this
    /// version" is the row's own, `signing::pages::PageRow::trust`).
    pub signer_page_rename: SharedString,
    pub signer_page_remove: SharedString,
    pub signer_page_name: SharedString,
    pub signer_page_save: SharedString,
    /// The account's "Where you review and sign" (spec 102 D1): its title,
    /// the sentence under it, and the two venue rows' words.
    pub venue_title: SharedString,
    pub venue_subtitle: SharedString,
    pub venue_in_vela: SharedString,
    pub venue_in_vela_body: SharedString,
    pub venue_page: SharedString,
    pub venue_page_body: SharedString,
    pub nav_about: SharedString,
    /// The Feedback page (078 S-03): its nav row, and the web's
    /// `componentsUi.bugReport.*` words for the report itself.
    pub nav_feedback: SharedString,
    /// The Community group's label (Settings → Community, 2026-09-27).
    pub nav_community: SharedString,
    pub bug_title: SharedString,
    pub bug_subtitle: SharedString,
    pub bug_what_placeholder: SharedString,
    pub bug_add_steps: SharedString,
    pub bug_steps_placeholder: SharedString,
    pub bug_preview_toggle: SharedString,
    pub bug_labels: crate::executor::bug_report::EnvironmentLabels,
    pub bug_consent: SharedString,
    pub bug_send: SharedString,
    pub bug_sending: SharedString,
    pub bug_success_title: SharedString,
    /// Both carry `{{number}}`.
    pub bug_success_new: String,
    pub bug_success_deduped: String,
    pub bug_view_issue: SharedString,
    pub bug_fallback_title: SharedString,
    pub bug_fallback_body: SharedString,
    pub bug_open_github: SharedString,
    pub bug_open_github_form: SharedString,
    /// The screenshots (078 round 3). `screenshotsHint` and
    /// `screenshotsLimit` carry `{{max}}`. (`removeScreenshot` is the
    /// badge's accessibility label on the other shells; gpui draws no
    /// accessibility tree at this pin, so there is nowhere to put it here.)
    pub bug_screenshots_label: SharedString,
    pub bug_add_screenshots: SharedString,
    pub bug_screenshots_hint: String,
    pub bug_screenshots_public: SharedString,
    pub bug_screenshots_limit: String,
    pub bug_screenshot_unsupported: SharedString,
    pub bug_drop_hint: SharedString,
    pub bug_screenshots_dropped: SharedString,
    pub bug_fallback_screenshots: SharedString,
    pub bug_done: SharedString,
    /// The send button in the fallback state (v2 A2): a retry, not "Send".
    pub bug_try_again: SharedString,
    /// The screenshot viewer's one button (078 round 3, C3). Its "View
    /// screenshot {{index}}" and "Close" are accessibility labels on the
    /// other shells, with nowhere to go on this gpui.
    pub bug_remove_from_viewer: SharedString,
    // account panel
    /// "Total {{amount}}" — the second half of the summary. The count template
    /// ends in "· ", and the live panel printed that dangling separator with
    /// nothing after it until spec 034, because this half needs per-account
    /// balances nobody had asked the core for.
    pub accounts_total: String,
    pub accounts_count: String,
    /// The account switcher's title (078 H-01), as the web's dialog names it.
    pub accounts_title: SharedString,
    pub account_create: SharedString,
    pub account_sign_in: SharedString,
    pub sign_out_button: SharedString,
    /// The keys block (spec 062): which passkeys control the wallet.
    pub keys_title: SharedString,
    pub keys_subtitle: SharedString,
    /// `Key {{n}}` — a key nobody named.
    pub keys_key_n: String,
    pub keys_synced: SharedString,
    pub keys_not_synced: SharedString,
    pub keys_from_device: SharedString,
    pub keys_provider_platform: SharedString,
    pub keys_provider_generic: SharedString,
    pub keys_provider_security_key: SharedString,
    pub keys_user_verified: SharedString,
    /// The key this device signs with — the one it signed in with.
    pub keys_signs_here: SharedString,
    pub keys_public_key: SharedString,
    pub keys_credential: SharedString,
    pub keys_transport: SharedString,
    pub keys_attestation: SharedString,
    pub keys_copy: SharedString,
    pub keys_copied: SharedString,
    /// What copying the record to Ethereum makes public and what it costs
    /// (`registry_backup::EXPLAIN_KEY`). The backup ROW's own words are the
    /// core's (`BackupState::row`), resolved where it is drawn.
    pub backup_explain: SharedString,
    pub sign_out_desc: SharedString,
    pub erase_title: SharedString,
    pub erase_subtitle: SharedString,
    pub erase_confirm: SharedString,
    /// The erase's question (spec 072): what goes, what does not, and the
    /// line it says instead of leaving when something survived.
    pub erase_desc: SharedString,
    pub erase_loses: SharedString,
    pub erase_keeps: SharedString,
    pub erase_cancel: SharedString,
    pub erase_failed: SharedString,
    /// Every other question's way out.
    pub cancel: SharedString,
    // appearance panel
    pub language: SharedString,
    /// The language menu's first row: `auto`.
    pub language_follow_system: SharedString,
    pub theme_title: SharedString,
    pub theme_light: SharedString,
    pub theme_dark: SharedString,
    pub theme_auto: SharedString,
    pub text_scale: SharedString,
    // localization panel
    pub currency: SharedString,
    pub number_format: SharedString,
    pub number_subtitle: SharedString,
    pub date_format: SharedString,
    pub time_format: SharedString,
    pub note_system: SharedString,
    pub note_automatic: SharedString,
    pub note_indian: SharedString,
    // networks panel
    pub networks_subtitle: SharedString,
    pub add_network: SharedString,
    pub add_network_desc: SharedString,
    pub search_placeholder: SharedString,
    pub chain_id: String,
    pub rpc_url: SharedString,
    pub explorer: SharedString,
    pub network_custom: SharedString,
    /// The custom-network delete, and the confirmation the core leaves to the
    /// shell in so many words.
    pub network_remove_title: SharedString,
    pub network_remove_body: SharedString,
    pub network_remove_cancel: SharedString,
    pub network_remove_confirm: SharedString,
    /// The prefix a slow endpoint's pill wears: "Slower · 1.2s".
    pub network_slow: SharedString,
    pub network_save_hint: SharedString,
    /// Spec 098 §5.1: the relay is sent this network's RPC, key and all — said
    /// under every field where an RPC is set.
    pub network_relay_notice: SharedString,
    /// While the blur's chain-id verdict is outstanding. The standing hint
    /// says "saved as soon as you leave the field", which for those seconds
    /// is not yet true — the override is written only once the RPC agrees
    /// about which chain it serves.
    pub network_save_checking: SharedString,
    pub compatible: SharedString,
    pub compatibility_check: SharedString,
    pub check_safe: SharedString,
    pub check_signer: SharedString,
    pub check_remaining: String,
    /// Spec 081 FR-009: the chain works, but only for a one-key wallet —
    /// Safe's passkey signer factory is not deployed here. Shown beside a
    /// "Compatible" badge, which without it reads as a contradiction of the
    /// two crossed rows above.
    pub single_key_only: SharedString,
    pub custom_rpc_title: SharedString,
    pub custom_rpc_placeholder: SharedString,
    pub best_rpc: String,
    // rpc providers panel
    pub providers_desc: SharedString,
    pub provider_connected: SharedString,
    pub provider_not_set: SharedString,
    pub provider_check_key: SharedString,
    /// The explicit re-run. A key blur already tests, so this is for the
    /// person who changed nothing and wants to know whether it works NOW.
    pub provider_test: SharedString,
    pub provider_get_key: SharedString,
    pub provider_supports: String,
    pub provider_avg_latency: String,
    // endpoints panel
    pub endpoints_desc: SharedString,
    pub endpoint_chain_data: SharedString,
    pub endpoint_chain_data_hint: SharedString,
    pub endpoint_passkey: SharedString,
    pub endpoint_passkey_hint: SharedString,
    pub endpoint_relay: SharedString,
    pub endpoint_relay_hint: SharedString,
    pub endpoint_fiat: SharedString,
    pub endpoint_fiat_hint: SharedString,
    /// The three ways a service endpoint can be wrong, in the same words the
    /// RN `ServiceHealthBadge` uses — one wording per state across clients.
    pub health_https_required: SharedString,
    pub health_offline: SharedString,
    pub health_invalid: SharedString,
    /// Spec 081: the two ways out of an INCOMPATIBLE verdict, which the web
    /// has always offered and desktop did not — a person could type a custom
    /// RPC under a red verdict and have nothing to press.
    pub recheck_with_rpc: SharedString,
    pub open_chain_setup_tool: SharedString,
    /// What the wizard is DOING between a click and a verdict. The dialog is
    /// otherwise inert while the index resolves and the probes run, which is
    /// the specific silence phase 6 found on the send screen: a screen that
    /// looks broken because nobody said it was working.
    pub wizard_searching: SharedString,
    pub wizard_checking: SharedString,
    /// The four ways the wizard STOPS (`NetWizardErrorKind`). The core decides
    /// which; these are only the words, and all four were already in the
    /// corpus — the scan path and the add-token screen say the same things.
    pub wizard_already_added: SharedString,
    pub wizard_not_found: SharedString,
    /// "Can't reach {{name}} right now" (spec 092's one-network line): the
    /// registry listed no endpoint for the resolved chain, and no custom RPC
    /// was typed. Carries the chain's name because at this point the wizard
    /// HAS resolved it.
    pub wizard_no_rpc: String,
    pub wizard_incompatible: SharedString,
    /// Spec 038 #E1: the probes failed — not a verdict.
    pub wizard_unable_to_verify: SharedString,
    /// The unverifiable verdict's CTA (the web's `retry`, 078 S-05). The
    /// refused verdict's sentence is the core's, by reason
    /// (`NetCompatibility.hint_key` — `live::net_refusal`).
    pub wizard_retry: SharedString,
    pub endpoints_reset: SharedString,
    /// Spec 072 (FR-010): the question the reset asks first.
    pub endpoints_reset_title: SharedString,
    pub endpoints_reset_body: SharedString,
    pub endpoints_reset_confirm: SharedString,
    pub endpoints_reset_cancel: SharedString,
    pub endpoints_guide: SharedString,
    // storage panel
    pub storage_subtitle: SharedString,
    pub storage_summary: String,
    pub storage_user_data: SharedString,
    pub storage_caches: SharedString,
    pub storage_connections: SharedString,
    pub item_transactions: SharedString,
    pub item_contacts: SharedString,
    pub item_custom: SharedString,
    pub item_browsing: SharedString,
    pub item_balances: SharedString,
    pub item_rates: SharedString,
    pub item_scan: SharedString,
    pub item_dapps: SharedString,
    pub count_records: String,
    pub count_contacts: String,
    pub count_items: String,
    pub count_sites: String,
    pub storage_clear: SharedString,
    pub storage_clear_all: SharedString,
    /// The bar's legend (078 S-11).
    pub storage_legend_user: SharedString,
    pub storage_legend_caches: SharedString,
    pub storage_legend_sessions: SharedString,
    pub storage_disconnect_all: SharedString,
    /// "Clear all caches?" — the question before it happens.
    pub storage_clear_title: SharedString,
    pub storage_clear_body: SharedString,
    pub storage_clear_confirm: SharedString,
    // about panel
    pub about_tagline: SharedString,
    pub about_version: String,
    pub about_section_technical: SharedString,
    pub about_section_links: SharedString,
    pub about_wallet_label: SharedString,
    pub about_wallet_value: SharedString,
    pub about_auth_label: SharedString,
    pub about_auth_value: SharedString,
    pub about_account_label: SharedString,
    pub about_account_value: SharedString,
    pub about_signer_label: SharedString,
    pub about_signer_value: SharedString,
    pub about_networks_label: SharedString,
    pub about_networks_value: String,
    pub about_link_website: SharedString,
    pub about_link_github: SharedString,
    pub about_link_safe: SharedString,
    pub about_link_privacy: SharedString,
    pub about_link_terms: SharedString,
    pub about_link_support: SharedString,
    pub about_footer: SharedString,
    /// Spec 091: the hidden developer switch, its one line, and the notice
    /// when seven taps on the version reveal it.
    pub about_debug_mode: SharedString,
    pub about_debug_mode_body: SharedString,
    pub about_debug_mode_revealed: SharedString,
    // rescue (DSR1)
    pub rpc_fix_title: SharedString,
    pub rpc_fix_warning: SharedString,
    pub rpc_fix_label: SharedString,
    pub rpc_fix_save: SharedString,
    pub rpc_providers_hint: SharedString,
    pub rpc_report: SharedString,
    /// The banner's line over several unreachable networks (`{{n}}`, spec 092).
    pub unreachable_many: String,
    pub rpc_fix_action: SharedString,
    /// The one refusal the override gate makes: this endpoint answered
    /// `eth_chainId` with ANOTHER chain's id, so nothing was written. Carries
    /// `{{expected}}` and `{{actual}}`.
    pub rpc_wrong_chain: String,
    pub offline: SharedString,
}

impl SettingsStrings {
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(key);
        let raw = |key: &str| loc.t(key).to_string();
        Self {
            title: s("settings.title"),
            nav_account: s("settings.sections.account"),
            nav_appearance: s("settings.sections.appearance"),
            nav_localization: s("settings.sections.localization"),
            nav_networks: s("settings.advanced.networksTitle"),
            nav_rpc_providers: s("settings.advanced.rpcProvidersTitle"),
            nav_endpoints: s("settings.advanced.endpointsTitle"),
            nav_storage: s("settings.storage.title"),
            nav_fee_speed: s("settings.advanced.feeSpeedTitle"),
            fee_speed_title: s("settings.feeSpeed.title"),
            fee_speed_subtitle: s("settings.feeSpeed.subtitle"),
            nav_signing: s("settings.signing.title"),
            signing_subtitle: s("settings.signing.subtitle"),
            signer_page_invalid: s("settings.signing.pageInvalid"),
            signer_page_insecure: s("settings.signing.pageInsecure"),
            signer_page_duplicate: s("settings.signing.pageDuplicate"),
            signer_page_add: s("settings.signing.pageAdd"),
            signer_page_rename: s("settings.signing.pageRename"),
            signer_page_remove: s("settings.signing.pageRemove"),
            signer_page_name: s("settings.signing.pageName"),
            signer_page_save: s("settings.signing.pageSave"),
            venue_title: s("settings.venue.title"),
            venue_subtitle: s("settings.venue.subtitle"),
            venue_in_vela: s("settings.venue.inVela"),
            venue_in_vela_body: s("settings.venue.inVelaBody"),
            venue_page: s("settings.venue.page"),
            venue_page_body: s("settings.venue.pageBody"),
            nav_about: s("settings.about.title"),
            nav_feedback: s("settings.feedback.title"),
            nav_community: s("settings.sections.community"),
            bug_title: s("componentsUi.bugReport.title"),
            bug_subtitle: s("componentsUi.bugReport.subtitle"),
            bug_what_placeholder: s("componentsUi.bugReport.whatPlaceholder"),
            bug_add_steps: s("componentsUi.bugReport.addSteps"),
            bug_steps_placeholder: s("componentsUi.bugReport.stepsPlaceholder"),
            bug_preview_toggle: s("componentsUi.bugReport.previewToggle"),
            bug_labels: crate::executor::bug_report::EnvironmentLabels {
                version: raw("componentsUi.bugReport.previewVersion"),
                platform: raw("componentsUi.bugReport.previewPlatform"),
                language: raw("componentsUi.bugReport.previewLanguage"),
                rpc: raw("componentsUi.bugReport.previewRpc"),
                failures: raw("componentsUi.bugReport.previewFailures"),
                none: raw("componentsUi.bugReport.previewNone"),
            },
            bug_consent: s("componentsUi.bugReport.consent"),
            bug_send: s("componentsUi.bugReport.send"),
            bug_sending: s("componentsUi.bugReport.sending"),
            bug_success_title: s("componentsUi.bugReport.successTitle"),
            bug_success_new: raw("componentsUi.bugReport.successBodyNew"),
            bug_success_deduped: raw("componentsUi.bugReport.successBodyDeduped"),
            bug_view_issue: s("componentsUi.bugReport.viewIssue"),
            bug_fallback_title: s("componentsUi.bugReport.fallbackTitle"),
            bug_fallback_body: s("componentsUi.bugReport.fallbackBody"),
            bug_open_github: s("componentsUi.bugReport.openGithub"),
            bug_open_github_form: s("componentsUi.bugReport.openGithubForm"),
            bug_screenshots_label: s("componentsUi.bugReport.screenshotsLabel"),
            bug_add_screenshots: s("componentsUi.bugReport.addScreenshots"),
            bug_screenshots_hint: raw("componentsUi.bugReport.screenshotsHint"),
            bug_screenshots_public: s("componentsUi.bugReport.screenshotsPublic"),
            bug_screenshots_limit: raw("componentsUi.bugReport.screenshotsLimit"),
            bug_screenshot_unsupported: s("componentsUi.bugReport.screenshotUnsupported"),
            bug_drop_hint: s("componentsUi.bugReport.dropHint"),
            bug_screenshots_dropped: s("componentsUi.bugReport.screenshotsDropped"),
            bug_fallback_screenshots: s("componentsUi.bugReport.fallbackScreenshots"),
            bug_done: s("componentsUi.bugReport.done"),
            bug_try_again: s("common.tryAgain"),
            bug_remove_from_viewer: s("componentsUi.bugReport.removeFromViewer"),
            accounts_total: raw("settingsModals.account.total"),
            accounts_count: raw("home.switcherAccountCount"),
            accounts_title: s("settingsModals.account.modalTitle"),
            account_create: s("settingsModals.account.createNew"),
            account_sign_in: s("settingsModals.account.signInExisting"),
            sign_out_button: s("settings.signOut.button"),
            keys_title: s("settingsModals.keys.title"),
            keys_subtitle: s("settingsModals.keys.subtitle"),
            keys_key_n: raw("settingsModals.keys.keyN"),
            keys_synced: s("onboarding.create.keySyncedBadge"),
            keys_not_synced: s("settingsModals.keys.notSynced"),
            keys_from_device: s("settingsModals.keys.fromDevice"),
            keys_provider_platform: s("onboarding.create.providerPlatform"),
            keys_provider_generic: s("onboarding.create.providerGeneric"),
            keys_provider_security_key: s("onboarding.create.providerSecurityKey"),
            keys_user_verified: s("settingsModals.keys.userVerified"),
            keys_signs_here: s("settingsModals.keys.signsHere"),
            keys_public_key: s("settingsModals.keys.publicKey"),
            keys_credential: s("settingsModals.keys.credential"),
            keys_transport: s("settingsModals.keys.transport"),
            keys_attestation: s("settingsModals.keys.attestation"),
            keys_copy: s("componentsUi.signing.copyValue"),
            keys_copied: s("receive.copied"),
            backup_explain: s("settingsModals.backup.explain"),
            sign_out_desc: s("settings.signOut.desc"),
            erase_title: s("settings.eraseDevice.title"),
            erase_subtitle: s("settings.eraseDevice.subtitle"),
            erase_confirm: s("settings.eraseDevice.confirm"),
            erase_desc: s("settings.eraseDevice.desc"),
            erase_loses: s("settings.eraseDevice.loses"),
            erase_keeps: s("settings.eraseDevice.keeps"),
            erase_cancel: s("settings.eraseDevice.cancel"),
            erase_failed: s("settings.eraseDevice.failed"),
            cancel: s("common.cancel"),
            language: s("language.title"),
            language_follow_system: s("language.followSystem"),
            theme_title: s("settings.appearance.themeTitle"),
            theme_light: s("settings.appearance.themeLight"),
            theme_dark: s("settings.appearance.themeDark"),
            theme_auto: s("settings.appearance.themeAuto"),
            text_scale: s("settings.appearance.textScale"),
            currency: s("settings.localization.currencyTitle"),
            number_format: s("settings.localization.numberTitle"),
            number_subtitle: s("settings.localization.numberSubtitle"),
            date_format: s("settings.localization.dateTitle"),
            time_format: s("settings.localization.timeTitle"),
            note_system: s("common.system"),
            note_automatic: s("common.automatic"),
            note_indian: s("settings.formatNote.indian"),
            networks_subtitle: s("settings.advanced.networksSubtitle"),
            add_network: s("settings.advanced.addNetworkTitle"),
            add_network_desc: s("settingsModals.addNetwork.description"),
            search_placeholder: s("settingsModals.addNetwork.searchPlaceholder"),
            chain_id: raw("settingsModals.network.chainId"),
            rpc_url: s("settingsModals.network.fieldRpcUrl"),
            explorer: s("settingsModals.network.fieldExplorer"),
            network_custom: s("settings.networks.custom"),
            network_remove_title: s("settingsModals.network.removeTitle"),
            network_remove_body: s("settingsModals.network.removeBody"),
            network_remove_cancel: s("settingsModals.network.removeCancel"),
            network_remove_confirm: s("settingsModals.network.removeConfirm"),
            network_slow: s("settings.networks.slow"),
            network_save_hint: s("settings.networks.saveHint"),
            network_relay_notice: s("settingsModals.network.relayNotice"),
            network_save_checking: s("componentsUi.funding.checking"),
            compatible: s("settingsModals.addNetwork.compatible"),
            compatibility_check: s("settingsModals.addNetwork.compatibilityCheck"),
            check_safe: s("settingsModals.addNetwork.checkSafe"),
            check_signer: s("settingsModals.addNetwork.checkSigner"),
            check_remaining: raw("settingsModals.addNetwork.checkRemaining"),
            single_key_only: s("settingsModals.addNetwork.singleKeyOnly"),
            custom_rpc_title: s("settingsModals.addNetwork.customRpcTitle"),
            custom_rpc_placeholder: s("settingsModals.addNetwork.customRpcPlaceholder"),
            best_rpc: raw("settingsModals.addNetwork.bestRpc"),
            // Spec 098 §5.1: a key set here rides in the RPC URL the relay is
            // sent, and that is said where the key is set.
            providers_desc: SharedString::from(format!(
                "{} {}",
                s("settingsModals.rpcProviders.description"),
                s("settingsModals.rpcProviders.relayNotice")
            )),
            provider_connected: s("activity.connected"),
            provider_not_set: s("settingsModals.rpcProviders.notSet"),
            provider_test: s("settingsModals.rpcProviders.test"),
            provider_check_key: s("settingsModals.rpcProviders.checkKey"),
            provider_get_key: s("settingsModals.rpcProviders.getKey"),
            provider_supports: raw("settingsModals.rpcProviders.supportsCount"),
            provider_avg_latency: raw("settingsModals.rpcProviders.avgLatency"),
            endpoints_desc: s("settingsModals.endpoints.description"),
            endpoint_chain_data: s("settingsModals.endpoints.chainDataLabel"),
            endpoint_chain_data_hint: s("settingsModals.endpoints.chainDataHint"),
            endpoint_passkey: s("settingsModals.endpoints.passkeyLabel"),
            endpoint_passkey_hint: s("settingsModals.endpoints.passkeyHint"),
            endpoint_relay: s("settingsModals.endpoints.bundlerLabel"),
            endpoint_relay_hint: s("settingsModals.endpoints.bundlerHint"),
            endpoint_fiat: s("settingsModals.endpoints.fiatLabel"),
            endpoint_fiat_hint: s("settingsModals.endpoints.fiatHint"),
            health_https_required: s("settingsModals.health.httpsRequired"),
            health_offline: s("settingsModals.health.offline"),
            health_invalid: s("settingsModals.health.invalid"),
            recheck_with_rpc: s("settingsModals.addNetwork.recheckWithRpc"),
            open_chain_setup_tool: s("settingsModals.addNetwork.openChainSetupTool"),
            wizard_searching: s("settingsModals.addNetwork.searching"),
            wizard_checking: s("settingsModals.addNetwork.checkingCompatibility"),
            wizard_already_added: s("addToken.errorAlreadyAdded"),
            wizard_not_found: s("addToken.errorChainNotFound"),
            wizard_no_rpc: raw("assets.unreachableOne"),
            wizard_incompatible: s("settingsModals.addNetwork.incompatible"),
            wizard_unable_to_verify: s("settingsModals.addNetwork.unableToVerify"),
            wizard_retry: s("settingsModals.addNetwork.retry"),
            endpoints_reset: s("settingsModals.endpoints.resetToDefaults"),
            endpoints_reset_title: s("settingsModals.endpoints.resetTitle"),
            endpoints_reset_body: s("settingsModals.endpoints.resetBody"),
            endpoints_reset_confirm: s("settingsModals.endpoints.resetConfirm"),
            endpoints_reset_cancel: s("settingsModals.endpoints.resetCancel"),
            endpoints_guide: s("settingsModals.endpoints.selfHostGuide"),
            storage_subtitle: s("settings.storage.subtitle"),
            storage_summary: raw("settings.storage.summary"),
            storage_user_data: s("settings.storage.userData"),
            storage_caches: s("settings.storage.caches"),
            storage_connections: s("settings.storage.connections"),
            item_transactions: s("settings.storage.itemTransactions"),
            item_contacts: s("settings.storage.itemContacts"),
            item_custom: s("settings.storage.itemCustom"),
            item_browsing: s("settings.storage.itemBrowsing"),
            item_balances: s("settings.storage.itemBalances"),
            item_rates: s("settings.storage.itemRates"),
            item_scan: s("settings.storage.itemScan"),
            item_dapps: s("settings.storage.itemDapps"),
            count_records: raw("settings.storage.records"),
            count_contacts: raw("settings.storage.contactsCount"),
            count_items: raw("settings.storage.itemsCount"),
            count_sites: raw("settings.storage.sitesCount"),
            storage_clear: s("settings.storage.clear"),
            storage_clear_all: s("settings.storage.clearAllCaches"),
            storage_legend_user: s("settings.storage.legendUserData"),
            storage_legend_caches: s("settings.storage.legendCaches"),
            storage_legend_sessions: s("settings.storage.legendSessions"),
            storage_disconnect_all: s("settings.storage.disconnectAll"),
            storage_clear_title: s("settings.storage.clearTitle"),
            storage_clear_body: s("settings.storage.clearBody"),
            storage_clear_confirm: s("settings.storage.clearConfirm"),
            about_tagline: s("about.tagline"),
            about_version: raw("about.version"),
            about_section_technical: s("about.sectionTechnical"),
            about_section_links: s("about.sectionLinks"),
            about_wallet_label: s("about.techWalletLabel"),
            about_wallet_value: s("about.techWalletValue"),
            about_auth_label: s("about.techAuthLabel"),
            about_auth_value: s("about.techAuthValue"),
            about_account_label: s("about.techAccountTypeLabel"),
            about_account_value: s("about.techAccountTypeValue"),
            about_signer_label: s("about.techSignerLabel"),
            about_signer_value: s("about.techSignerValue"),
            about_networks_label: s("about.techNetworksLabel"),
            about_networks_value: raw("about.techNetworksValue"),
            about_link_website: s("about.linkWebsite"),
            about_link_github: s("about.linkGitHub"),
            about_link_safe: s("about.linkSafeWallet"),
            about_link_privacy: s("about.linkPrivacy"),
            about_link_terms: s("about.linkTerms"),
            about_link_support: s("about.linkSupport"),
            about_footer: s("about.footer"),
            about_debug_mode: s("about.debugMode"),
            about_debug_mode_body: s("about.debugModeBody"),
            about_debug_mode_revealed: s("about.debugModeRevealed"),
            rpc_fix_title: s("assets.rpcFixTitle"),
            rpc_fix_warning: s("assets.rpcFixWarning"),
            rpc_fix_label: s("assets.rpcFixLabel"),
            rpc_fix_save: s("assets.rpcFixSaveBtn"),
            rpc_providers_hint: s("assets.rpcProvidersTitle"),
            rpc_report: s("assets.rpcReport"),
            unreachable_many: raw("assets.unreachableMany"),
            rpc_fix_action: s("assets.rpcFix"),
            rpc_wrong_chain: raw("assets.rpcFixWrongChain"),
            offline: s("settingsModals.health.offline"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 091: the debug-mode words resolve — never their own keys.
    #[test]
    fn the_debug_mode_words_resolve() {
        let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
        for (value, key) in [
            (&s.about_debug_mode, "about.debugMode"),
            (&s.about_debug_mode_body, "about.debugModeBody"),
            (&s.about_debug_mode_revealed, "about.debugModeRevealed"),
        ] {
            assert!(!value.is_empty() && value.as_ref() != key, "{key}");
        }
    }

    /// The words this cut newly reads.
    ///
    /// All five already existed — the phone drew this dialog and this button
    /// years ago, so the corpus carries them in fifteen languages and the
    /// desktop's half of the feature costs zero new keys. What a test can still
    /// catch is a key that does not resolve, which is how a confirm dialog ends
    /// up with `settingsModals.network.removeTitle` as its title.
    #[test]
    fn the_remove_and_test_words_resolve() {
        {
            // `Loc::from_env` honours VELA_LANG; the env-independent check is
            // that the resolved strings differ from their keys, which is the
            // same shape `wallet_strings_resolve_without_echo` uses.
            let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
            for (value, key) in [
                (
                    &s.network_remove_title,
                    "settingsModals.network.removeTitle",
                ),
                (&s.network_remove_body, "settingsModals.network.removeBody"),
                (
                    &s.network_remove_cancel,
                    "settingsModals.network.removeCancel",
                ),
                (
                    &s.network_remove_confirm,
                    "settingsModals.network.removeConfirm",
                ),
                (&s.provider_test, "settingsModals.rpcProviders.test"),
            ] {
                assert_ne!(value.as_ref(), key, "`{key}` echoed the key");
                assert!(!value.is_empty(), "`{key}` resolved empty");
            }
        }
    }

    /// The words spec 072's questions and menus read — every one already in
    /// the corpus in fifteen languages, so the desktop's half costs no key.
    #[test]
    fn the_settings_parity_words_resolve() {
        let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
        for (value, key) in [
            (&s.erase_desc, "settings.eraseDevice.desc"),
            (&s.erase_loses, "settings.eraseDevice.loses"),
            (&s.erase_keeps, "settings.eraseDevice.keeps"),
            (&s.erase_cancel, "settings.eraseDevice.cancel"),
            (&s.erase_failed, "settings.eraseDevice.failed"),
            (&s.cancel, "common.cancel"),
            (&s.language_follow_system, "language.followSystem"),
            (&s.storage_clear_title, "settings.storage.clearTitle"),
            (&s.storage_clear_body, "settings.storage.clearBody"),
            (&s.storage_clear_confirm, "settings.storage.clearConfirm"),
        ] {
            assert_ne!(value.as_ref(), key, "`{key}` echoed the key");
            assert!(!value.is_empty(), "`{key}` resolved empty");
        }
    }

    /// The report's screenshot words (078 round 3), every one of them in the
    /// corpus already — and the templates keep their placeholders.
    #[test]
    fn the_screenshot_words_resolve() {
        let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
        for (value, key) in [
            (
                &s.bug_screenshots_label,
                "componentsUi.bugReport.screenshotsLabel",
            ),
            (
                &s.bug_add_screenshots,
                "componentsUi.bugReport.addScreenshots",
            ),
            (
                &s.bug_screenshots_public,
                "componentsUi.bugReport.screenshotsPublic",
            ),
            (
                &s.bug_screenshot_unsupported,
                "componentsUi.bugReport.screenshotUnsupported",
            ),
            (&s.bug_drop_hint, "componentsUi.bugReport.dropHint"),
            (
                &s.bug_screenshots_dropped,
                "componentsUi.bugReport.screenshotsDropped",
            ),
            (
                &s.bug_fallback_screenshots,
                "componentsUi.bugReport.fallbackScreenshots",
            ),
            (&s.bug_done, "componentsUi.bugReport.done"),
            (&s.bug_try_again, "common.tryAgain"),
            (&s.nav_community, "settings.sections.community"),
            (
                &s.bug_remove_from_viewer,
                "componentsUi.bugReport.removeFromViewer",
            ),
        ] {
            assert_ne!(value.as_ref(), key, "`{key}` echoed the key");
            assert!(!value.is_empty(), "`{key}` resolved empty");
        }
        assert!(s.bug_screenshots_hint.contains("{{max}}"));
        assert!(s.bug_screenshots_limit.contains("{{max}}"));
    }

    /// The keys list's mark for the key this device signs with.
    #[test]
    fn the_signs_here_mark_resolves() {
        let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
        let mark = s.keys_signs_here.to_string();
        assert!(
            !mark.is_empty() && mark != "settingsModals.keys.signsHere",
            "{mark:?}"
        );
    }

    /// Settings → Signing pages and the account's "Where you review and
    /// sign" (spec 102), every word a key the corpus carries in fifteen
    /// languages — in every one of them.
    #[test]
    fn the_signing_page_and_venue_words_resolve() {
        for (tag, loc) in crate::loc::Loc::every_language() {
            let s = SettingsStrings::resolve(&loc);
            for (value, key) in [
                (&s.nav_signing, "settings.signing.title"),
                (&s.signing_subtitle, "settings.signing.subtitle"),
                (&s.signer_page_invalid, "settings.signing.pageInvalid"),
                (&s.signer_page_insecure, "settings.signing.pageInsecure"),
                (&s.signer_page_duplicate, "settings.signing.pageDuplicate"),
                (&s.signer_page_add, "settings.signing.pageAdd"),
                (&s.signer_page_rename, "settings.signing.pageRename"),
                (&s.signer_page_remove, "settings.signing.pageRemove"),
                (&s.signer_page_name, "settings.signing.pageName"),
                (&s.signer_page_save, "settings.signing.pageSave"),
                (&s.venue_title, "settings.venue.title"),
                (&s.venue_subtitle, "settings.venue.subtitle"),
                (&s.venue_in_vela, "settings.venue.inVela"),
                (&s.venue_in_vela_body, "settings.venue.inVelaBody"),
                (&s.venue_page, "settings.venue.page"),
                (&s.venue_page_body, "settings.venue.pageBody"),
            ] {
                assert_ne!(value.as_ref(), key, "{tag}: `{key}` echoed the key");
                assert!(!value.is_empty(), "{tag}: `{key}` resolved empty");
            }
            // No "Trusted Signer" left anywhere on these surfaces.
            assert!(
                !s.nav_signing.to_lowercase().contains("trusted signer"),
                "{tag}: {}",
                s.nav_signing
            );
        }
    }
}
