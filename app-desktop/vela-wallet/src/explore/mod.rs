//! Explore — the desktop browser surface (spec 022).
//!
//! `fixtures` is the canonical data (data-model.md §2, verbatim mock content),
//! `components` the reusable visuals (theme + resolved strings in, `Div` out);
//! the page entity in `wallet/page.rs` owns the state and the interaction, the
//! same division spec 015 set and spec 018 kept.

pub mod components;
pub mod fixtures;
pub mod live;

use gpui::SharedString;

use crate::loc::Loc;

/// Every explore string, resolved once per locale (spec 022 §5 key map).
#[allow(
    dead_code,
    reason = "the phone shells resolve the same struct; the desktop mocks (DE1–DE4) draw a subset"
)]
pub struct ExploreStrings {
    pub title: SharedString,
    pub search_placeholder: SharedString,
    pub start_title: SharedString,
    pub start_hint: SharedString,
    pub start_cta: SharedString,
    pub favorites: SharedString,
    pub recent: SharedString,
    pub edit: SharedString,
    pub add: SharedString,
    pub clear: SharedString,
    pub manage_groups: SharedString,
    pub new_group: SharedString,
    pub rename: SharedString,
    pub hide: SharedString,
    pub delete: SharedString,
    pub move_to_group: SharedString,
    pub open_in_new_tab: SharedString,
    pub remove_from_favorites: SharedString,
    /// Template carrying `{{n}}`.
    pub site_count: String,
    pub system_group: SharedString,
    pub tabs: SharedString,
    pub new_tab: SharedString,
    pub start_page: SharedString,
    pub close_tab: SharedString,
    pub add_to_favorites: SharedString,
    pub refresh: SharedString,
    pub back: SharedString,
    pub forward: SharedString,
    pub reload: SharedString,
    pub site_menu: SharedString,
    pub account: SharedString,
    pub secure_site: SharedString,
    pub connected_tag: SharedString,
    pub connection_title: SharedString,
    pub switch_account: SharedString,
    pub network: SharedString,
    pub connection_explainer: SharedString,
    /// The consent the in-app browser asks for. `connect.browser.*` — the
    /// corpus the phone shells already ask this question with; a desktop
    /// wording of its own would be a second sentence about the same grant.
    pub consent_title: String,
    pub consent_body: SharedString,
    pub consent_connect: SharedString,
    pub consent_cancel: SharedString,
    pub auto_request_hint: SharedString,
    pub disconnect: SharedString,
    pub close: SharedString,
}

impl ExploreStrings {
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(key);
        let raw = |key: &str| loc.t(key).to_string();
        Self {
            title: s("explore.title"),
            search_placeholder: s("explore.searchPlaceholder"),
            start_title: s("explore.startTitle"),
            start_hint: s("explore.startHint"),
            start_cta: s("explore.startCta"),
            favorites: s("explore.favorites"),
            recent: s("explore.recent"),
            edit: s("explore.edit"),
            add: s("explore.add"),
            clear: s("explore.clear"),
            manage_groups: s("explore.manageGroups"),
            new_group: s("explore.newGroup"),
            rename: s("explore.rename"),
            hide: s("explore.hide"),
            delete: s("explore.delete"),
            move_to_group: s("explore.moveToGroup"),
            open_in_new_tab: s("explore.openInNewTab"),
            remove_from_favorites: s("explore.removeFromFavorites"),
            site_count: raw("explore.siteCount"),
            system_group: s("explore.systemGroup"),
            tabs: s("explore.tabs"),
            new_tab: s("explore.newTab"),
            start_page: s("explore.startPage"),
            close_tab: s("explore.closeTab"),
            add_to_favorites: s("explore.addToFavorites"),
            refresh: s("explore.refresh"),
            back: s("explore.back"),
            forward: s("explore.forward"),
            reload: s("explore.reload"),
            site_menu: s("explore.siteMenu"),
            account: s("explore.account"),
            secure_site: s("explore.secureSite"),
            connected_tag: s("explore.connectedTag"),
            connection_title: s("explore.connectionTitle"),
            switch_account: s("explore.switchAccount"),
            network: s("explore.network"),
            connection_explainer: s("explore.connectionExplainer"),
            consent_title: loc.t("connect.browser.title").to_string(),
            consent_body: loc.t("connect.browser.body"),
            consent_connect: loc.t("connect.browser.connect"),
            consent_cancel: loc.t("connect.browser.cancel"),
            auto_request_hint: s("explore.autoRequestHint"),
            disconnect: s("explore.disconnect"),
            close: s("explore.close"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC-004 discipline: none of the new keys may echo.
    #[test]
    fn explore_strings_resolve_without_echo() {
        let loc = Loc::from_env();
        let s = ExploreStrings::resolve(&loc);
        for (value, key) in [
            (s.title.as_ref(), "explore.title"),
            (s.start_cta.as_ref(), "explore.startCta"),
            (
                s.connection_explainer.as_ref(),
                "explore.connectionExplainer",
            ),
            (s.close.as_ref(), "explore.close"),
            (s.consent_body.as_ref(), "connect.browser.body"),
            (s.consent_connect.as_ref(), "connect.browser.connect"),
            (s.consent_cancel.as_ref(), "connect.browser.cancel"),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
        }
        assert!(
            s.consent_title.contains("{{host}}"),
            "connect.browser.title must keep its host slot"
        );
        assert!(
            s.site_count.contains("{{n}}"),
            "siteCount must be a template"
        );
    }
}
