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
    /// "Sign with" (spec 071): the nav row is the page's title; the sentence
    /// under it; the choices in the core's order with the Trusted Signer's
    /// line; and the Trusted Signer page's section.
    pub nav_signing: SharedString,
    pub signing_subtitle: SharedString,
    pub sign_with_options: Vec<(&'static str, SharedString)>,
    pub trusted_signer_body: SharedString,
    pub signer_page_title: SharedString,
    pub signer_page_subtitle: SharedString,
    pub signer_page_official: SharedString,
    pub signer_page_invalid: SharedString,
    pub signer_page_insecure: SharedString,
    pub signer_page_foreign: SharedString,
    pub signer_page_reset: SharedString,
    pub signer_page_save: SharedString,
    pub nav_about: SharedString,
    // account panel
    /// "Total {{amount}}" — the second half of the summary. The count template
    /// ends in "· ", and the live panel printed that dangling separator with
    /// nothing after it until spec 034, because this half needs per-account
    /// balances nobody had asked the core for.
    pub accounts_total: String,
    pub accounts_count: String,
    pub account_create: SharedString,
    pub account_sign_in: SharedString,
    pub sign_out_button: SharedString,
    /// The Ethereum backup row (spec 062).
    pub backup_title: SharedString,
    pub backup_backed_up: SharedString,
    pub backup_not_backed_up: SharedString,
    pub backup_could_not_check: SharedString,
    pub backup_checking: SharedString,
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
    pub keys_public_key: SharedString,
    pub keys_credential: SharedString,
    pub keys_transport: SharedString,
    pub keys_attestation: SharedString,
    pub keys_copy: SharedString,
    pub keys_copied: SharedString,
    /// PUBLIC keys only; private keys never leave the device.
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
    /// The wizard's retry, for a chain the probe could not reach — never a
    /// condemnation (the core's invariant ③).
    pub recheck: SharedString,
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
    /// `{{name}} RPC unavailable`: the registry listed no endpoint for the
    /// resolved chain, and no custom RPC was typed. Carries the chain's name
    /// because at this point the wizard HAS resolved it.
    pub wizard_no_rpc: String,
    pub wizard_incompatible: SharedString,
    /// Spec 038 #E1: the probes failed — not a verdict.
    pub wizard_unable_to_verify: SharedString,
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
    pub about_footer: SharedString,
    // rescue (DSR1)
    pub rpc_fix_title: SharedString,
    pub rpc_fix_warning: SharedString,
    pub rpc_fix_label: SharedString,
    pub rpc_fix_save: SharedString,
    pub rpc_providers_hint: SharedString,
    pub rpc_report: SharedString,
    pub rpc_unavailable_multiple: String,
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
            sign_with_options: vela_core::wallet_keys::SIGN_METHODS
                .iter()
                .map(|method| (*method, s(crate::signing::sign_method_key(method))))
                .collect(),
            trusted_signer_body: s("componentsUi.signing.trustedSignerBody"),
            signer_page_title: s("settings.signing.pageTitle"),
            signer_page_subtitle: s("settings.signing.pageSubtitle"),
            signer_page_official: s("settings.signing.pageOfficial"),
            signer_page_invalid: s("settings.signing.pageInvalid"),
            signer_page_insecure: s("settings.signing.pageInsecure"),
            signer_page_foreign: s("settings.signing.pageForeign"),
            signer_page_reset: s("settings.signing.pageReset"),
            signer_page_save: s("settings.signing.pageSave"),
            nav_about: s("settings.about.title"),
            accounts_total: raw("settingsModals.account.total"),
            accounts_count: raw("home.switcherAccountCount"),
            account_create: s("settingsModals.account.createNew"),
            account_sign_in: s("settingsModals.account.signInExisting"),
            sign_out_button: s("settings.signOut.button"),
            backup_title: s("settingsModals.backup.title"),
            backup_backed_up: s("settingsModals.backup.backedUp"),
            backup_not_backed_up: s("settingsModals.backup.notBackedUp"),
            backup_could_not_check: s("settingsModals.backup.couldNotCheck"),
            backup_checking: s("componentsUi.funding.checking"),
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
            providers_desc: s("settingsModals.rpcProviders.description"),
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
            recheck: s("settingsModals.addNetwork.recheck"),
            recheck_with_rpc: s("settingsModals.addNetwork.recheckWithRpc"),
            open_chain_setup_tool: s("settingsModals.addNetwork.openChainSetupTool"),
            wizard_searching: s("settingsModals.addNetwork.searching"),
            wizard_checking: s("settingsModals.addNetwork.checkingCompatibility"),
            wizard_already_added: s("addToken.errorAlreadyAdded"),
            wizard_not_found: s("addToken.errorChainNotFound"),
            wizard_no_rpc: raw("assets.rpcUnavailableSingle"),
            wizard_incompatible: s("settingsModals.addNetwork.incompatible"),
            wizard_unable_to_verify: s("settingsModals.addNetwork.unableToVerify"),
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
            about_footer: s("about.footer"),
            rpc_fix_title: s("assets.rpcFixTitle"),
            rpc_fix_warning: s("assets.rpcFixWarning"),
            rpc_fix_label: s("assets.rpcFixLabel"),
            rpc_fix_save: s("assets.rpcFixSaveBtn"),
            rpc_providers_hint: s("assets.rpcProvidersTitle"),
            rpc_report: s("assets.rpcReport"),
            rpc_unavailable_multiple: raw("assets.rpcUnavailableMultiple"),
            rpc_fix_action: s("assets.rpcFix"),
            rpc_wrong_chain: raw("assets.rpcFixWrongChain"),
            offline: s("settingsModals.health.offline"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The "Sign with" page's words (spec 071), every one of them a key the
    /// corpus already carries in fifteen languages — and its choices are the
    /// core's five, in the core's order.
    #[test]
    fn the_sign_with_words_resolve() {
        let s = SettingsStrings::resolve(&crate::loc::Loc::from_env());
        for (value, key) in [
            (&s.nav_signing, "settings.signing.title"),
            (&s.signing_subtitle, "settings.signing.subtitle"),
            (
                &s.trusted_signer_body,
                "componentsUi.signing.trustedSignerBody",
            ),
            (&s.signer_page_title, "settings.signing.pageTitle"),
            (&s.signer_page_subtitle, "settings.signing.pageSubtitle"),
            (&s.signer_page_official, "settings.signing.pageOfficial"),
            (&s.signer_page_invalid, "settings.signing.pageInvalid"),
            (&s.signer_page_insecure, "settings.signing.pageInsecure"),
            (&s.signer_page_foreign, "settings.signing.pageForeign"),
            (&s.signer_page_reset, "settings.signing.pageReset"),
            (&s.signer_page_save, "settings.signing.pageSave"),
        ] {
            assert_ne!(value.as_ref(), key, "`{key}` echoed the key");
            assert!(!value.is_empty(), "`{key}` resolved empty");
        }
        let ids: Vec<&str> = s.sign_with_options.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vela_core::wallet_keys::SIGN_METHODS);
    }
}
