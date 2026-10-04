//! The explore screen, built from what `browser_history` remembers.
//!
//! The **sibling** of `fixtures.rs`, as `signing/live.rs` is of its own: both
//! produce the drawn models, and the page picks. What is live here is exactly
//! one group — Recent — because exactly one core owns it. The favourites grid
//! and the custom groups below it are still drawn, and they are drawn because
//! nothing in `vela-core` owns them yet, which is a gap rather than a choice.
//!
//! Nothing here decides what is recent. Dedupe by origin, recency order, the
//! cap and the "a report without a title must not clobber one" rule are all
//! `browser_history`'s; this maps its entries onto rows and picks a colour.

use gpui::{Hsla, SharedString, hsla};

use vela_core::app::browser_history::BhistEntry;
use vela_core::app::explore_sites::{ExploreSite, ExploreView};

use super::ExploreStrings;
use super::fixtures::{GroupAction, GroupModel, SiteModel, TabModel};

/// One remembered visit as a row.
///
/// The TITLE is what the page called itself and the HOST is what it actually
/// is, so the host is the subtitle and never the other way round: a page may
/// title itself anything at all, and a row that showed only that would let a
/// site name itself after another one.
#[must_use]
pub fn site_of(entry: &BhistEntry) -> SiteModel {
    // The core's one wording (spec 082 RE7): a title that is its host — or
    // no title — is said once; any other title stands over the host.
    let label = vela_core::app::browser_load::site_label(&entry.title, &entry.host);
    SiteModel {
        // Keyed by ORIGIN, which is what the core dedupes on, so a row's
        // element id is stable across visits to the same site.
        id: "recent",
        name: SharedString::from(label.name),
        host: SharedString::from(entry.host.clone()),
        letter: SharedString::from(letter_of(&entry.host)),
        tint: tint_of(&entry.host),
        subtitle: label.host_line.map(SharedString::from),
        // No "2 hours ago": there is no word for it in the corpus, and an
        // English one on a Chinese screen is worse than no line at all
        // (phase 22's rule about showing a key to somebody who reads Chinese).
        meta: None,
        url: Some(SharedString::from(entry.url.clone())),
        // The icon the page named when it was visited, then the usual places.
        icon_urls: icons_of(&entry.url, Some(&entry.favicon)),
    }
}

/// The Recent group, or `None` when nothing has been visited.
///
/// An empty group is not drawn: a heading with nothing under it reads as a
/// feature that is broken rather than as a browser nobody has used yet.
#[must_use]
pub fn recent_group(entries: &[BhistEntry], strings: &ExploreStrings) -> Option<GroupModel> {
    if entries.is_empty() {
        return None;
    }
    Some(GroupModel {
        id: "recent",
        title: strings.recent.clone(),
        action: GroupAction::Clear,
        sites: entries.iter().map(site_of).collect(),
    })
}

/// One pinned site as a tile.
#[must_use]
pub fn tile_of(site: &ExploreSite) -> SiteModel {
    SiteModel {
        id: "favorite",
        name: SharedString::from(site.name.clone()),
        host: SharedString::from(site.host.clone()),
        letter: SharedString::from(letter_of(&site.host)),
        tint: tint_of(&site.host),
        subtitle: Some(SharedString::from(site.host.clone())),
        meta: None,
        url: Some(SharedString::from(site.url.clone())),
        icon_urls: icons_of(&site.url, None),
    }
}

/// The person's own groups, in the order they made them.
///
/// A hidden group draws nothing at all — hiding is what this wallet offers
/// instead of deleting for the two system groups, and a custom group that a
/// person hid should behave the same way rather than reappear greyed.
#[must_use]
pub fn custom_groups(view: &ExploreView) -> Vec<GroupModel> {
    view.groups
        .iter()
        .filter(|group| !group.hidden)
        .map(|group| GroupModel {
            // The page keys rows by (group id, index); a stable literal here
            // would collide across groups, so the id travels as the name's
            // own leaked string only for element ids — see `page.rs`.
            id: "custom",
            title: SharedString::from(group.name.clone()),
            action: GroupAction::Menu,
            sites: group.sites.iter().map(tile_of).collect(),
        })
        .collect()
}

/// The open tabs, as the strip draws them.
///
/// A tab with no url is the START PAGE — every browser's first tab, drawn with
/// the wallet's own mark rather than a favicon, and the core keeps that
/// distinction as `url: None` rather than as an empty string.
///
/// `lit` is the tab the page lights (spec 082 RD6: the one on screen — a
/// restored tab waits unlit), and `failed_host` the host a failure panel is
/// up for, which that tab is called while it is (RD5).
#[must_use]
pub fn tab_models(
    view: &ExploreView,
    strings: &ExploreStrings,
    lit: Option<&str>,
    failed_host: Option<String>,
) -> Vec<TabModel> {
    view.tabs
        .iter()
        .map(|tab| {
            // The tab on screen under a failure panel is named as the bar
            // names it (082 RD5, 083 H8): the site that failed — its letter,
            // and none of its icons, since nothing of it loaded. After a
            // certificate error it kept the page before's title and icon.
            let failed = failed_host
                .as_deref()
                .filter(|host| !host.is_empty() && lit == Some(tab.id.as_str()));
            TabModel {
                id: "tab",
                title: match failed {
                    Some(host) => SharedString::from(host.to_owned()),
                    None if tab.title.trim().is_empty() => strings.start_page.clone(),
                    None => SharedString::from(tab.title.clone()),
                },
                site: match failed {
                    Some(host) => Some(SiteModel {
                        id: "tab",
                        name: SharedString::from(host.to_owned()),
                        host: SharedString::from(host.to_owned()),
                        letter: SharedString::from(letter_of(host)),
                        tint: tint_of(host),
                        subtitle: None,
                        meta: None,
                        url: None,
                        icon_urls: Vec::new(),
                    }),
                    None => tab.url.as_ref().map(|url| SiteModel {
                        id: "tab",
                        name: SharedString::from(tab.title.clone()),
                        host: SharedString::from(tab.host.clone()),
                        letter: SharedString::from(letter_of(&tab.host)),
                        tint: tint_of(&tab.host),
                        subtitle: None,
                        meta: None,
                        url: None,
                        icon_urls: icons_of(url, None),
                    }),
                },
                selected: lit == Some(tab.id.as_str()),
            }
        })
        .collect()
}

/// The one tab a signed-in person has before the core records any (spec 083
/// W9): the site being opened, named by its host, or the start page. Until
/// 083 this gap drew the gallery's demo tabs — "Uniswap · Polymarket" —
/// neither of which was open or did anything.
pub fn pending_tab(strings: &ExploreStrings, opening: Option<&str>) -> TabModel {
    let host = opening
        .map(vela_core::app::browser_load::host_of)
        .filter(|host| !host.is_empty());
    match (opening, host) {
        (Some(url), Some(host)) => TabModel {
            id: "tab",
            title: SharedString::from(host.clone()),
            site: Some(address_site(&host, icons_of(url, None))),
            selected: true,
        },
        _ => TabModel {
            id: "tab",
            title: strings.start_page.clone(),
            site: None,
            selected: true,
        },
    }
}

/// A tab's site known by its address alone.
fn address_site(host: &str, icon_urls: Vec<SharedString>) -> SiteModel {
    SiteModel {
        id: "tab",
        name: SharedString::from(host.to_owned()),
        letter: SharedString::from(letter_of(host)),
        tint: tint_of(host),
        host: SharedString::from(host.to_owned()),
        subtitle: None,
        meta: None,
        url: None,
        icon_urls,
    }
}

/// The origin of the history row a screen row was drawn from.
///
/// Rows are keyed on screen by HOST, and every rule in the core is written
/// about the ORIGIN, so this is the one place the two meet. `None` for a row
/// the history does not have — a drawn one — which is what keeps a menu off
/// the gallery's rows.
#[must_use]
pub fn live_origin(entries: &[BhistEntry], host: &str) -> Option<String> {
    entries
        .iter()
        .find(|entry| entry.host == host)
        .map(|entry| entry.origin.clone())
}

/// The letter a site's avatar shows until its icon loads — the core's rule
/// (spec 079 F16): `app.uniswap.org` is "U", not "A".
fn letter_of(host: &str) -> String {
    vela_core::app::browser_load::site_letter(host)
}

/// Where a site's icon may be found, in the order they are tried over its
/// letter: the icon the page itself named, then the two places sites keep
/// one — the signing header's own list. HTTPS only (founder's ruling on site
/// icons): an icon fetched over plain http is a request anybody on the path
/// can answer.
#[must_use]
pub fn icons_of(url: &str, favicon: Option<&str>) -> Vec<SharedString> {
    let mut icons: Vec<SharedString> = favicon
        .filter(|icon| icon.to_ascii_lowercase().starts_with("https://"))
        .map(SharedString::from)
        .into_iter()
        .collect();
    if let Some(origin) = vela_core::app::dapp_permissions::origin_of(url)
        .filter(|origin| origin.starts_with("https://"))
    {
        for path in ["/apple-touch-icon.png", "/favicon.ico"] {
            let icon = SharedString::from(format!("{origin}{path}"));
            if !icons.contains(&icon) {
                icons.push(icon);
            }
        }
    }
    icons
}

/// A stable colour per host.
///
/// The drawn rows carry each protocol's brand colour, which is a fact about
/// ten sites and not about the web. For everything else the hue comes from the
/// host itself, so the same site is the same colour on every launch and two
/// sites are unlikely to collide — display only, and it decides nothing.
fn tint_of(host: &str) -> Hsla {
    // FNV-1a, for a stable spread with no dependency.
    let mut hash: u32 = 2_166_136_261;
    for byte in host.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "a hue, and the low bits are the point"
    )]
    let hue = (hash % 360) as f32 / 360.0;
    // The saturation and lightness the drawn brand marks sit at, so a live row
    // does not stand out beside a fixture one.
    hsla(hue, 0.72, 0.55, 1.0)
}

/// What the toolbar's "open in the system browser" hands the system (spec
/// 099): the address of the page on screen as its engine has it — never the
/// address bar's text, which may be a half-typed edit or a failed address —
/// and nothing over the start page, where the control is drawn disabled, or
/// for an engine with no document yet.
#[must_use]
pub fn system_browser_url(page_on_screen: bool, engine_url: Option<String>) -> Option<String> {
    engine_url.filter(|url| page_on_screen && !url.is_empty() && url != "about:blank")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 099: the ⋯ menu's "open in the system browser" is a control of
    /// its own — the engine's address, and nothing with no page.
    #[test]
    fn the_system_browser_gets_the_page_or_nothing() {
        let page = Some("https://app.uniswap.org/swap".to_owned());
        assert_eq!(system_browser_url(true, page.clone()), page);
        assert_eq!(system_browser_url(false, page), None, "the start page");
        assert_eq!(system_browser_url(true, None), None, "no engine yet");
        assert_eq!(
            system_browser_url(true, Some("about:blank".to_owned())),
            None,
            "a view with no document"
        );
    }

    fn entry(host: &str, title: &str) -> BhistEntry {
        BhistEntry {
            origin: format!("https://{host}"),
            url: format!("https://{host}/app"),
            host: host.to_owned(),
            title: title.to_owned(),
            favicon: String::new(),
            last_visited_ms: 1.0,
        }
    }

    /// Spec 079 F16: the site's own icon over the core's letter — the icon
    /// the page named first, https only, and never for a plain-http site.
    #[test]
    fn a_site_wears_its_own_icon_over_the_cores_letter() {
        let mut visited = entry("app.uniswap.org", "Uniswap");
        visited.favicon = "https://app.uniswap.org/favicon.png".to_owned();
        let row = site_of(&visited);
        assert_eq!(row.letter, SharedString::from("U"), "not \"A\" for app.");
        assert_eq!(
            row.icon_urls,
            vec![
                SharedString::from("https://app.uniswap.org/favicon.png"),
                SharedString::from("https://app.uniswap.org/apple-touch-icon.png"),
                SharedString::from("https://app.uniswap.org/favicon.ico"),
            ]
        );
        assert!(
            icons_of(
                "http://127.0.0.1:8137/",
                Some("http://127.0.0.1:8137/f.ico")
            )
            .is_empty()
        );
        assert_eq!(
            icons_of("https://a.example/x", Some("https://a.example/favicon.ico")).len(),
            2,
            "one icon is tried once"
        );
    }

    /// The host is the subtitle, always — a page's own title is a claim.
    #[test]
    fn a_row_shows_the_title_over_the_host_it_actually_is() {
        let row = site_of(&entry("evil.example", "app.uniswap.org"));
        assert_eq!(row.name, SharedString::from("app.uniswap.org"));
        assert_eq!(
            row.subtitle,
            Some(SharedString::from("evil.example")),
            "the host a page really is must still be on the row"
        );
        assert_eq!(row.letter, SharedString::from("E"));
    }

    /// A page with no title is its host, not a blank row — said once.
    #[test]
    fn an_untitled_page_falls_back_to_its_host() {
        let row = site_of(&entry("127.0.0.1:8137", "   "));
        assert_eq!(row.name, SharedString::from("127.0.0.1:8137"));
        assert_eq!(row.subtitle, None, "the host is not said twice");
        assert_eq!(row.letter, SharedString::from("1"));
    }

    /// Spec 082 RE7 (G8 parity): a page titled with its own host is named
    /// once, whatever the case of the letters.
    #[test]
    fn a_title_that_is_its_host_is_said_once() {
        let row = site_of(&entry("127.0.0.1:8137", "127.0.0.1:8137"));
        assert_eq!(row.name, SharedString::from("127.0.0.1:8137"));
        assert_eq!(row.subtitle, None);
        let row = site_of(&entry("app.example", "APP.EXAMPLE"));
        assert_eq!(row.subtitle, None);
    }

    /// One colour per host, every launch.
    #[test]
    fn a_hosts_colour_is_stable_and_not_shared_with_its_neighbour() {
        let a = tint_of("app.uniswap.org");
        assert!((a.h - tint_of("app.uniswap.org").h).abs() < f32::EPSILON);
        assert!((a.h - tint_of("polymarket.com").h).abs() > f32::EPSILON);
    }

    /// Every row opens its own site (078 E-02): a Recent row where the person
    /// left off, a pinned site the url it was pinned at, a drawn one its host.
    #[test]
    fn a_row_opens_its_own_site() {
        assert_eq!(
            site_of(&entry("app.uniswap.org", "Uniswap")).open_url(),
            "https://app.uniswap.org/app"
        );
        let pinned = ExploreSite {
            origin: "https://polymarket.com".to_owned(),
            url: "https://polymarket.com/markets".to_owned(),
            host: "polymarket.com".to_owned(),
            name: "Polymarket".to_owned(),
            renamed: false,
            added_ms: 1.0,
        };
        assert_eq!(
            tile_of(&pinned).open_url(),
            "https://polymarket.com/markets"
        );
        assert_eq!(
            super::super::fixtures::uniswap().open_url(),
            format!("https://{}", super::super::fixtures::uniswap().host)
        );
    }

    /// Nothing visited draws no heading at all.
    #[test]
    fn an_empty_history_draws_no_group() {
        let strings = ExploreStrings::resolve(&crate::loc::Loc::from_env());
        assert!(recent_group(&[], &strings).is_none());
        assert!(recent_group(&[entry("a.example", "A")], &strings).is_some());
    }

    /// 083 H8: over a failure panel the tab on screen is the address that
    /// failed — not the title of the page before it — and the tab beside it
    /// keeps its own.
    #[test]
    fn a_failed_load_names_the_tab_on_screen_by_its_host() {
        let strings = ExploreStrings::resolve(&crate::loc::Loc::from_env());
        let tab = |id: &str, host: &str, title: &str| vela_core::app::explore_sites::ExploreTab {
            id: id.to_owned(),
            url: Some(format!("https://{host}/")),
            title: title.to_owned(),
            host: host.to_owned(),
        };
        let view = ExploreView {
            favorites: Vec::new(),
            groups: Vec::new(),
            tabs: vec![
                tab("a", "app.uniswap.org", "Uniswap Interface"),
                tab("b", "polymarket.com", "Polymarket"),
            ],
            selected_tab: Some("a".to_owned()),
            favorites_hidden: false,
            recent_hidden: false,
            favorites_full: false,
            tabs_full: false,
            recent_tabs: Vec::new(),
            ready: true,
        };
        let tabs = tab_models(
            &view,
            &strings,
            Some("a"),
            Some(crate::diag::host_of("https://expired.badssl.com/path?q=1")),
        );

        assert_eq!(tabs[0].title, SharedString::from("expired.badssl.com"));
        let site = tabs[0].site.as_ref();
        assert_eq!(
            site.map(|site| site.host.clone()),
            Some(SharedString::from("expired.badssl.com"))
        );
        assert_eq!(
            site.map(|site| site.letter.clone()),
            Some(SharedString::from(letter_of("expired.badssl.com")))
        );
        assert!(
            site.is_some_and(|site| site.icon_urls.is_empty()),
            "nothing of the failed site loaded, its icon included"
        );
        assert_eq!(tabs[1].title, SharedString::from("Polymarket"));

        // An address with no host names nothing.
        let tabs = tab_models(
            &view,
            &strings,
            Some("a"),
            Some(crate::diag::host_of("https://")),
        );
        assert_eq!(tabs[0].title, SharedString::from("Uniswap Interface"));
    }
}
