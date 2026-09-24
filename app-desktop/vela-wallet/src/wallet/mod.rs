//! Wallet home (spec 015): fixture-driven three-column UI + gallery.
//!
//! `fixtures` is the canonical data (data-model.md, verbatim mock content),
//! `components` the reusable visuals (theme + resolved strings in, `Div` out),
//! `page` the one entity that owns state and interaction.

pub mod browser_host;
pub mod components;
pub mod fixtures;
pub mod live;
pub mod money;
pub mod page;
/// The signing panel's four machines as one journey.
///
/// Unwired until the browser's ipc handler can reach the page to open one —
/// the last hop, and the one that needs a gpui handle inside a wry callback.
/// Marked rather than left to make the warning count meaningless, and **taken
/// off when that hop lands**: an allow marks callees live too (spec 032
/// lesson 1).
pub mod signing_host;
pub mod speed_control;

use gpui::SharedString;

use crate::loc::Loc;

/// `{{var}}` interpolation for the handful of templated wallet strings —
/// corpus-linted templates with known vars, same as the web's `fill`
/// (research.md D3), not a parallel i18n engine.
pub fn fill(template: &str, name: &str, value: &str) -> String {
    let needle = format!("{{{{{name}}}}}");
    template.replace(&needle, value)
}

/// Every wallet string, resolved once per locale (research.md D3 key map).
pub struct WalletStrings {
    pub nav_wallet: SharedString,
    pub nav_contacts: SharedString,
    pub nav_explore: SharedString,
    pub nav_settings: SharedString,
    pub total_balance: SharedString,
    pub live_indicator: SharedString,
    pub balance_stale: SharedString,
    /// Nothing could be read and nothing is known (spec 038): the network
    /// sentence, not a $0.
    pub balance_unreachable: SharedString,
    pub balance_unpriced: SharedString,
    pub no_price: SharedString,
    pub action_receive: SharedString,
    pub action_send: SharedString,
    pub action_scan: SharedString,
    pub section_activity: SharedString,
    pub section_assets: SharedString,
    pub action_all: SharedString,
    pub action_add: SharedString,
    pub label_sent: SharedString,
    pub label_received: SharedString,
    pub label_dapp: SharedString,
    pub today: SharedString,
    pub yesterday: SharedString,
    /// Templates carrying `{{name}}`.
    pub to_name: String,
    pub from_name: String,
    /// The money-in celebration, `{{amount}} {{token}}` — the toast the core
    /// arms and expires. A template, so the number and the coin are the
    /// core's and only the sentence around them is the corpus's.
    pub toast_received: String,
    pub empty_activity_title: SharedString,
    pub empty_activity_caption: SharedString,
    pub empty_assets_title: SharedString,
    pub empty_assets_caption: SharedString,
    pub networks_title: SharedString,
    /// The sign-out row and its confirmation. The copy is the shipping
    /// client's, already translated in all fifteen locales.
    pub sign_out_title: SharedString,
    pub sign_out_keeps: SharedString,
    pub sign_out_warning: SharedString,
    pub sign_out_anyway: SharedString,
    pub sign_out_cancel: SharedString,
    /// How many wallets a sign-out takes, when it is more than one
    /// (2026-09-23). `{{count}}` is filled where it is drawn.
    pub sign_out_desc_many: SharedString,
    /// Taking ONE wallet off this device, and what that means.
    pub account_remove: SharedString,
    pub account_remove_body: SharedString,
    pub all_networks: SharedString,
    pub receive_title: SharedString,
    pub address_label: SharedString,
    pub copy_address: SharedString,
    pub qr_caption: SharedString,
    /// 关闭 — for a dialog that is only ever LOOKED at. "取消" would be wrong:
    /// nothing is being cancelled by closing a picture.
    pub close_viewer: SharedString,
    /// The identicon viewer (078 H-02): the artwork, big, beside the address
    /// that drew it.
    pub viewer_title: SharedString,
    pub viewer_caption: SharedString,
    pub viewer_copied: SharedString,
    pub warning_title: SharedString,
    pub warning_reminder: SharedString,
    /// Template carrying `{{count}}`.
    pub networks_line: String,
    /// Template carrying `{{name}}` and `{{id}}`.
    pub network_detail: String,
    pub detail_send: SharedString,
    pub detail_receive: SharedString,
    pub label_name: SharedString,
    pub label_price: SharedString,
    /// Template carrying `{{symbol}}` and `{{value}}`.
    pub price_value: String,
    pub label_contract: SharedString,
    pub label_decimals: SharedString,
    pub label_transactions: SharedString,
    pub view_on_explorer: SharedString,
    pub native_token: SharedString,
}

impl WalletStrings {
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(key);
        let raw = |key: &str| loc.t(key).to_string();
        Self {
            nav_wallet: s("componentsUi.mainNav.wallet"),
            nav_contacts: s("componentsUi.mainNav.contacts"),
            nav_explore: s("componentsUi.mainNav.explore"),
            nav_settings: s("componentsUi.mainNav.settings"),
            total_balance: s("home.totalBalance"),
            live_indicator: s("home.liveIndicator"),
            balance_stale: s("home.balanceStale"),
            balance_unreachable: s("onboarding.common.networkBody"),
            balance_unpriced: s("home.balanceUnpriced"),
            no_price: s("home.balanceDetailNoPrice"),
            action_receive: s("componentsUi.dock.receive"),
            action_send: s("componentsUi.dock.send"),
            action_scan: s("componentsUi.dock.scan"),
            section_activity: s("home.tabActivity"),
            section_assets: s("assets.sectionTitle"),
            action_all: s("history.filterAll"),
            action_add: s("assets.addToken"),
            label_sent: s("history.labelSent"),
            label_received: s("history.labelReceived"),
            label_dapp: s("history.txLabelDappTx"),
            today: s("componentsUi.dayGroup.today"),
            yesterday: s("componentsUi.dayGroup.yesterday"),
            to_name: raw("history.toName"),
            from_name: raw("history.fromName"),
            toast_received: raw("home.toastReceived"),
            empty_activity_title: s("home.emptyNoActivity"),
            empty_activity_caption: s("home.emptySubtitle"),
            empty_assets_title: s("assets.emptyTitle"),
            empty_assets_caption: s("assets.emptySubtext"),
            networks_title: s("settingsModals.network.modalTitle"),
            sign_out_title: s("settings.signOut.title"),
            // `settings.signOut.desc` is deliberately NOT read here. It ends
            // "your passkey stays in Face ID / fingerprint", which is a fact
            // about a phone; on this platform the passkey is on the security
            // key in the person's hand, and telling them otherwise while they
            // decide whether to sign out is worse than saying less. `keeps`
            // carries the part the decision actually turns on — the address and
            // everything under it comes back — and is true everywhere.
            sign_out_keeps: s("settings.signOut.keeps"),
            sign_out_warning: s("settings.signOut.warning"),
            sign_out_anyway: s("settings.signOut.anyway"),
            sign_out_cancel: s("settings.signOut.cancel"),
            sign_out_desc_many: s("settings.signOut.descMany"),
            account_remove: s("settings.account.remove"),
            account_remove_body: s("settings.account.removeBody"),
            all_networks: s("componentsUi.networkFilter.allNetworks"),
            receive_title: s("receive.title"),
            address_label: s("receive.addressLabel"),
            copy_address: s("componentsUi.identiconViewer.copyAddress"),
            close_viewer: s("componentsUi.identiconViewer.close"),
            viewer_title: s("componentsUi.identiconViewer.title"),
            viewer_caption: s("componentsUi.identiconViewer.caption"),
            viewer_copied: s("componentsUi.identiconViewer.copied"),
            qr_caption: s("componentsUi.qrPlaceholder.caption"),
            warning_title: s("receive.warningTitle"),
            warning_reminder: s("receive.warningReminder"),
            networks_line: raw("receive.networksLine"),
            network_detail: raw("receive.networkDetail"),
            detail_send: s("tokenDetail.send"),
            detail_receive: s("tokenDetail.receive"),
            label_name: s("tokenDetail.labelName"),
            label_price: s("tokenDetail.labelPrice"),
            price_value: raw("tokenDetail.priceValue"),
            label_contract: s("tokenDetail.labelContract"),
            label_decimals: s("tokenDetail.labelDecimals"),
            label_transactions: s("tokenDetail.labelTransactions"),
            view_on_explorer: s("tokenDetail.viewOnExplorer"),
            native_token: s("addToken.labelNativeToken"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC-004 discipline from spec 007, applied to the wallet keys: none may
    /// echo in the locales the visual pass uses.
    #[test]
    fn wallet_strings_resolve_without_echo() {
        // Loc::from_env honours VELA_LANG; pin zh then en via the raw engine
        // path used by loc::tests — simplest here is the env-independent check
        // that the resolved strings differ from their keys.
        let loc = Loc::from_env();
        let s = WalletStrings::resolve(&loc);
        for (value, key) in [
            (s.nav_wallet.as_ref(), "componentsUi.mainNav.wallet"),
            (s.total_balance.as_ref(), "home.totalBalance"),
            (s.no_price.as_ref(), "home.balanceDetailNoPrice"),
            (s.qr_caption.as_ref(), "componentsUi.qrPlaceholder.caption"),
            (s.address_label.as_ref(), "receive.addressLabel"),
            (
                s.label_transactions.as_ref(),
                "tokenDetail.labelTransactions",
            ),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
        }
        assert!(s.to_name.contains("{{name}}"), "toName must be a template");
        assert!(
            s.toast_received.contains("{{amount}}") && s.toast_received.contains("{{token}}"),
            "toastReceived must carry both vars: a celebration that names one              of them is a sentence with a hole in it"
        );
        assert!(
            s.networks_line.contains("{{count}}"),
            "networksLine must be a template"
        );
    }

    #[test]
    fn fill_replaces_named_vars() {
        assert_eq!(fill("至 {{name}}", "name", "hold on"), "至 hold on");
        assert_eq!(fill("{{a}} & {{b}}", "a", "x"), "x & {{b}}");
    }
}
