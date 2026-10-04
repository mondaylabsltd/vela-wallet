//! Explore — the desktop browser surface (spec 022).
//!
//! `fixtures` is the canonical data (data-model.md §2, verbatim mock content),
//! `components` the reusable visuals (theme + resolved strings in, `Div` out);
//! the page entity in `wallet/page.rs` owns the state and the interaction, the
//! same division spec 015 set and spec 018 kept.

pub mod components;
pub mod engine;
pub mod fixtures;
pub mod live;
pub mod probe;

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
    /// Spec 099: the tab menu's batch closes, Chrome's three.
    pub close_other_tabs: SharedString,
    pub close_tabs_to_right: SharedString,
    pub close_all_tabs: SharedString,
    pub add_to_favorites: SharedString,
    /// What the corpus calls the place a URL is typed. The desktop's
    /// add-a-favourite dialog takes one, and nothing else, so its field says
    /// this rather than the search box's "搜索 dApp，或输入网址" — an offer to
    /// search that this dialog cannot honour.
    pub address_bar: SharedString,
    pub back: SharedString,
    pub forward: SharedString,
    pub reload: SharedString,
    /// The toolbar's ↗ (spec 099) — the one row of the old ⋯ menu with no
    /// other door; its accessible name.
    pub open_in_system_browser: SharedString,
    pub account: SharedString,
    pub connected_tag: SharedString,
    /// What the Connection panel says of a site the core holds no grant for
    /// (spec 097 E) — the corpus's "No active connection".
    pub not_connected: SharedString,
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
    /// The page's renderer died (spec 070): said, with a way back.
    pub page_crashed_title: SharedString,
    pub page_crashed_body: SharedString,
    /// Spec 079 US3: the load-failure panel — its title (the generic line;
    /// the reason under it is the core's `reason_key`), Retry, and what the
    /// button says while an attempt runs.
    pub load_failed: SharedString,
    pub load_retry: SharedString,
    pub load_retrying: SharedString,
    /// Spec 079 US4: the page's chain cannot be reached (`{{chain}}`).
    pub chain_down: String,
    /// Spec 082 RD9 (ruling 3): the proxy itself could not be used — the
    /// panel's reason for the `proxy` class ("Your proxy isn't responding.").
    pub load_proxy: SharedString,
    /// Spec 082 RD1 (ruling 10): a tab switch held while a request is open —
    /// "Finish or cancel the request first", in the bar for 2.5 s.
    pub request_open: SharedString,
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
            close_other_tabs: s("explore.closeOtherTabs"),
            close_tabs_to_right: s("explore.closeTabsToRight"),
            close_all_tabs: s("explore.closeAllTabs"),
            add_to_favorites: s("explore.addToFavorites"),
            address_bar: s("explore.addressBar"),
            back: s("explore.back"),
            forward: s("explore.forward"),
            reload: s("explore.reload"),
            open_in_system_browser: s("explore.openInSystemBrowser"),
            account: s("explore.account"),
            connected_tag: s("explore.connectedTag"),
            not_connected: s("home.connEmptyTitle"),
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
            page_crashed_title: s("explore.pageCrashedTitle"),
            page_crashed_body: s("explore.pageCrashedBody"),
            load_failed: s("connect.browser.loadFailed"),
            load_retry: s("connect.browser.retry"),
            load_retrying: s("explore.loadRetrying"),
            chain_down: raw("explore.chainDown"),
            load_proxy: s("explore.loadProxy"),
            request_open: s("explore.requestOpen"),
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
            (s.page_crashed_title.as_ref(), "explore.pageCrashedTitle"),
            (s.page_crashed_body.as_ref(), "explore.pageCrashedBody"),
            (s.consent_body.as_ref(), "connect.browser.body"),
            (s.consent_connect.as_ref(), "connect.browser.connect"),
            (s.consent_cancel.as_ref(), "connect.browser.cancel"),
            (s.load_failed.as_ref(), "connect.browser.loadFailed"),
            (s.load_retry.as_ref(), "connect.browser.retry"),
            (s.load_retrying.as_ref(), "explore.loadRetrying"),
            // Spec 082 T058, T060: the two new keys this client shows.
            (s.load_proxy.as_ref(), "explore.loadProxy"),
            (s.request_open.as_ref(), "explore.requestOpen"),
            // Spec 097 E: a site with no grant says so.
            (s.not_connected.as_ref(), "home.connEmptyTitle"),
            // Spec 099: the tab menu's batch closes, and the ⋯'s one row
            // that became a control of its own.
            (s.close_other_tabs.as_ref(), "explore.closeOtherTabs"),
            (s.close_tabs_to_right.as_ref(), "explore.closeTabsToRight"),
            (s.close_all_tabs.as_ref(), "explore.closeAllTabs"),
            (
                s.open_in_system_browser.as_ref(),
                "explore.openInSystemBrowser",
            ),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
        }
        // Spec 099: the tab menu is close, then the three batch closes, in
        // the order the page's actions are wired.
        let menu = fixtures::tab_menu(&s);
        let labels: Vec<_> = menu.items.iter().map(|item| item.label.clone()).collect();
        assert_eq!(
            labels,
            [
                s.close_tab.clone(),
                s.close_other_tabs.clone(),
                s.close_tabs_to_right.clone(),
                s.close_all_tabs.clone()
            ]
        );
        assert_eq!(menu.divider_after, Some(0));
        assert!(
            s.consent_title.contains("{{host}}"),
            "connect.browser.title must keep its host slot"
        );
        assert!(
            s.site_count.contains("{{n}}"),
            "siteCount must be a template"
        );
        assert!(
            s.chain_down.contains("{{chain}}"),
            "chainDown must name the chain"
        );
        // Every reason the core's classifier can name resolves here.
        for key in [
            "explore.loadOffline",
            "explore.loadNotFound",
            "explore.loadCertificate",
            "explore.loadProxy",
            "connect.browser.loadFailed",
        ] {
            assert_ne!(loc.t(key).as_ref(), key, "`{key}` echoed the key");
        }
    }
}
