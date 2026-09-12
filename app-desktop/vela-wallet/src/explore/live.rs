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
    let name = if entry.title.trim().is_empty() {
        entry.host.clone()
    } else {
        entry.title.clone()
    };
    SiteModel {
        // Keyed by ORIGIN, which is what the core dedupes on, so a row's
        // element id is stable across visits to the same site.
        id: "recent",
        name: SharedString::from(name),
        host: SharedString::from(entry.host.clone()),
        letter: SharedString::from(letter_of(&entry.host)),
        tint: tint_of(&entry.host),
        subtitle: Some(SharedString::from(entry.host.clone())),
        // No "2 hours ago": there is no word for it in the corpus, and an
        // English one on a Chinese screen is worse than no line at all
        // (phase 22's rule about showing a key to somebody who reads Chinese).
        meta: None,
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
#[must_use]
pub fn tab_models(view: &ExploreView, strings: &ExploreStrings) -> Vec<TabModel> {
    view.tabs
        .iter()
        .map(|tab| TabModel {
            id: "tab",
            title: if tab.title.trim().is_empty() {
                strings.start_page.clone()
            } else {
                SharedString::from(tab.title.clone())
            },
            site: tab.url.as_ref().map(|_| SiteModel {
                id: "tab",
                name: SharedString::from(tab.title.clone()),
                host: SharedString::from(tab.host.clone()),
                letter: SharedString::from(letter_of(&tab.host)),
                tint: tint_of(&tab.host),
                subtitle: None,
                meta: None,
            }),
            selected: view.selected_tab.as_deref() == Some(tab.id.as_str()),
        })
        .collect()
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

/// The first letter a person would read off the host.
fn letter_of(host: &str) -> String {
    host.chars()
        .find(char::is_ascii_alphanumeric)
        .map_or_else(|| "?".to_owned(), |c| c.to_uppercase().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A page with no title is its host, not a blank row.
    #[test]
    fn an_untitled_page_falls_back_to_its_host() {
        let row = site_of(&entry("127.0.0.1:8137", "   "));
        assert_eq!(row.name, SharedString::from("127.0.0.1:8137"));
        assert_eq!(row.letter, SharedString::from("1"));
    }

    /// One colour per host, every launch.
    #[test]
    fn a_hosts_colour_is_stable_and_not_shared_with_its_neighbour() {
        let a = tint_of("app.uniswap.org");
        assert!((a.h - tint_of("app.uniswap.org").h).abs() < f32::EPSILON);
        assert!((a.h - tint_of("polymarket.com").h).abs() > f32::EPSILON);
    }

    /// Nothing visited draws no heading at all.
    #[test]
    fn an_empty_history_draws_no_group() {
        let strings = ExploreStrings::resolve(&crate::loc::Loc::from_env());
        assert!(recent_group(&[], &strings).is_none());
        assert!(recent_group(&[entry("a.example", "A")], &strings).is_some());
    }
}

#[cfg(test)]
mod menu_tests {
    use super::super::ExploreStrings;
    use super::super::fixtures::site_menu;

    /// The site menu's ORDER is a contract with the page.
    ///
    /// `page.rs` arms three of these six by position — refresh, disconnect,
    /// close — because that is how the menu component takes its actions. An
    /// item inserted into the drawing would slide every action below it onto
    /// the wrong row, and "Add to favourites" would start revoking a site's
    /// access. Nothing crashes; it would just quietly do the wrong thing, so
    /// the order is pinned here rather than trusted.
    #[test]
    fn the_site_menu_keeps_the_order_the_page_arms() {
        let s = ExploreStrings::resolve(&crate::loc::Loc::from_env());
        let labels: Vec<String> = site_menu(&s)
            .items
            .iter()
            .map(|item| item.label.to_string())
            .collect();
        assert_eq!(
            labels,
            vec![
                s.refresh.to_string(),
                s.site_menu.to_string(),
                s.add_to_favorites.to_string(),
                s.open_in_new_tab.to_string(),
                s.disconnect.to_string(),
                s.close.to_string(),
            ],
            "the site menu was reordered; page.rs arms items 0, 4 and 5 by position"
        );
    }
}
