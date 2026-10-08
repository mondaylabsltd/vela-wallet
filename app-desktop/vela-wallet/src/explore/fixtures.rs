//! Canonical explore fixtures — the desktop port of
//! `specs/022-explore-signing-ui/data-model.md` §2 (web reference:
//! `src/lib/explore/fixtures.ts`). Site names, hosts and the demo page are
//! verbatim mock content and are never translated; brand colours are fixture
//! data, not theme tokens — the same rule the wallet's chain dots follow.

use gpui::{Hsla, SharedString, rgb};

use crate::contacts::fixtures::{MenuItemModel, MenuModel};
use crate::icons::Icon;

use super::ExploreStrings;

pub fn brand_uniswap() -> Hsla {
    rgb(0xff007a).into()
}
pub fn brand_aave() -> Hsla {
    rgb(0x8b6dff).into()
}
pub fn brand_pancake() -> Hsla {
    rgb(0x1fc7d4).into()
}
pub fn brand_polymarket() -> Hsla {
    rgb(0x4267f4).into()
}
pub fn brand_opensea() -> Hsla {
    rgb(0x2081e2).into()
}
pub fn brand_lido() -> Hsla {
    rgb(0xf0616d).into()
}
pub fn brand_ens() -> Hsla {
    rgb(0x5284ff).into()
}
pub fn brand_hyperliquid() -> Hsla {
    rgb(0x50d2c1).into()
}

/// The stand-in web page's own palette (spec 022 §2): the SITE's colours.
pub mod demo_palette {
    use gpui::{Hsla, rgb};

    pub fn surface() -> Hsla {
        rgb(0xf0efec).into()
    }
    pub fn card() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn field() -> Hsla {
        rgb(0xf5f3ef).into()
    }
    pub fn ink() -> Hsla {
        rgb(0x1a1a18).into()
    }
    pub fn ink_muted() -> Hsla {
        rgb(0x8c887e).into()
    }
}

#[derive(Clone)]
pub struct SiteModel {
    /// Stable key for the page's element ids and for a future favourites store.
    #[allow(dead_code, reason = "identity of a site, keyed by the page's rows")]
    pub id: &'static str,
    pub name: SharedString,
    pub host: SharedString,
    pub letter: SharedString,
    pub tint: Hsla,
    pub subtitle: Option<SharedString>,
    pub meta: Option<SharedString>,
    /// What a click on the site opens: the history entry's URL, or the one a
    /// favourite was pinned at. `None` for a drawn site, which opens its host.
    pub url: Option<SharedString>,
    /// The site's own icon, tried in order OVER its letter (spec 079 F16;
    /// https only) — empty for a drawn site, which keeps its letter.
    pub icon_urls: Vec<SharedString>,
}

impl SiteModel {
    /// The address this site opens (078 E-02). Every row opens ITS site: a
    /// drawn one opens its host rather than nothing — "nothing" is what let a
    /// click fall through to whichever page was loaded last.
    #[must_use]
    pub fn open_url(&self) -> String {
        self.url
            .as_ref()
            .map_or_else(|| format!("https://{}", self.host), ToString::to_string)
    }
}

fn site(
    id: &'static str,
    name: &'static str,
    host: &'static str,
    letter: &'static str,
    tint: Hsla,
) -> SiteModel {
    SiteModel {
        id,
        name: name.into(),
        host: host.into(),
        letter: letter.into(),
        tint,
        subtitle: None,
        meta: None,
        url: None,
        icon_urls: Vec::new(),
    }
}

pub fn uniswap() -> SiteModel {
    site(
        "uniswap",
        "Uniswap",
        "app.uniswap.org",
        "U",
        brand_uniswap(),
    )
}

/// The favourites grid, in mock order (DE2).
pub fn favorites() -> Vec<SiteModel> {
    vec![
        uniswap(),
        site("aave", "Aave", "app.aave.com", "A", brand_aave()),
        site(
            "pancake",
            "PancakeSwap",
            "pancakeswap.finance",
            "P",
            brand_pancake(),
        ),
        site(
            "polymarket",
            "Polymarket",
            "polymarket.com",
            "P",
            brand_polymarket(),
        ),
        site("opensea", "OpenSea", "opensea.io", "O", brand_opensea()),
        site("lido", "Lido", "stake.lido.fi", "L", brand_lido()),
        site("ens", "ENS", "app.ens.domains", "E", brand_ens()),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GroupAction {
    /// 收藏's header action. The favourites section draws its own header, so
    /// no fixture carries this yet — it is part of the shared vocabulary.
    #[allow(dead_code, reason = "cross-platform group vocabulary")]
    Edit,
    Clear,
}

#[derive(Clone)]
pub struct GroupModel {
    pub id: &'static str,
    pub title: SharedString,
    pub action: GroupAction,
    pub sites: Vec<SiteModel>,
}

fn with_meta(mut s: SiteModel, meta: &'static str) -> SiteModel {
    s.meta = Some(meta.into());
    s.subtitle = Some(s.host.clone());
    s
}

/// The desktop's wider grid shows four recent rows where the phone shows one.
pub fn groups(strings: &ExploreStrings) -> Vec<GroupModel> {
    let hyperliquid = site(
        "hyperliquid",
        "Hyperliquid",
        "app.hyperliquid.xyz",
        "H",
        brand_hyperliquid(),
    );
    vec![GroupModel {
        id: "recent",
        title: strings.recent.clone(),
        action: GroupAction::Clear,
        sites: vec![
            with_meta(hyperliquid, "刚刚"),
            with_meta(
                site(
                    "polymarket",
                    "Polymarket",
                    "polymarket.com",
                    "P",
                    brand_polymarket(),
                ),
                "昨天",
            ),
            with_meta(uniswap(), ""),
            with_meta(
                site("opensea", "OpenSea", "opensea.io", "O", brand_opensea()),
                "昨天",
            ),
        ],
    }]
}

#[derive(Clone)]
pub struct TabModel {
    #[allow(dead_code, reason = "stable key for the strip's element ids")]
    pub id: &'static str,
    pub title: SharedString,
    pub site: Option<SiteModel>,
    pub selected: bool,
}

pub fn tabs(strings: &ExploreStrings, browsing: bool) -> Vec<TabModel> {
    if browsing {
        vec![
            TabModel {
                id: "uniswap",
                title: "Uniswap".into(),
                site: Some(uniswap()),
                selected: true,
            },
            TabModel {
                id: "polymarket",
                title: "Polymarket".into(),
                site: Some(site(
                    "polymarket",
                    "Polymarket",
                    "polymarket.com",
                    "P",
                    brand_polymarket(),
                )),
                selected: false,
            },
        ]
    } else {
        vec![TabModel {
            id: "start",
            title: strings.new_tab.clone(),
            site: None,
            selected: true,
        }]
    }
}

/// The page inside the browser. Fixture content: the site's words, not ours.
pub struct DemoPage {
    pub title: SharedString,
    pub fields: Vec<(SharedString, SharedString)>,
    pub cta: SharedString,
    pub cta_tint: Hsla,
}

pub fn demo_page() -> DemoPage {
    DemoPage {
        title: "兑换".into(),
        fields: vec![
            ("0.5".into(), "ETH".into()),
            ("1,280.42".into(), "USDC".into()),
        ],
        cta: "兑换".into(),
        cta_tint: brand_uniswap(),
    }
}

/// One row of the site's network picker (spec 070, spec 079 FR-017): the
/// wallet's own networks, each with its logo and what the account holds
/// there, the site's ticked. The tick IS the state, as in the group pickers.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkPick {
    pub chain_id: u32,
    pub name: SharedString,
    pub current: bool,
    /// The home screen's figure for this network — `None` when it is not
    /// known, nothing is held there, or balances are hidden. Never a zero.
    pub amount: Option<SharedString>,
}

/// Spec 099: a tab's right-click — close it, then Chrome's batch closes.
pub fn tab_menu(strings: &ExploreStrings) -> MenuModel {
    let item = |icon, label: &SharedString| MenuItemModel {
        icon,
        label: label.clone(),
        destructive: false,
    };
    MenuModel {
        items: vec![
            item(Icon::X, &strings.close_tab),
            item(Icon::X, &strings.close_other_tabs),
            item(Icon::ChevronRight, &strings.close_tabs_to_right),
            item(Icon::Trash2, &strings.close_all_tabs),
        ],
        divider_after: Some(0),
    }
}

/// The menu on a row in Recent (spec 032 phase 40).
///
/// History rows had no menu on any client, so the core's `DeleteOrigin` — one
/// site forgotten rather than the whole list cleared — could not be reached at
/// all. Three items in the order somebody wants them, with the destructive one
/// behind the divider, exactly as the tile menu arranges its own.
pub fn recent_menu(strings: &ExploreStrings) -> MenuModel {
    MenuModel {
        items: vec![
            MenuItemModel {
                icon: Icon::ExternalLink,
                label: strings.open_in_new_tab.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Star,
                label: strings.add_to_favorites.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Trash2,
                label: strings.delete.clone(),
                destructive: true,
            },
        ],
        divider_after: Some(1),
    }
}

/// DE2's right-click menu on a favourite tile: open it in a new tab, rename
/// it, and — behind the divider — unpin it. The page arms these rows by
/// their place (`menu_actions`), so an item added or taken out here is one
/// added or taken out there in the same change.
pub fn tile_menu(strings: &ExploreStrings) -> MenuModel {
    MenuModel {
        items: vec![
            MenuItemModel {
                icon: Icon::ExternalLink,
                label: strings.open_in_new_tab.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Pencil,
                label: strings.rename.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Trash2,
                label: strings.remove_from_favorites.clone(),
                destructive: true,
            },
        ],
        divider_after: Some(1),
    }
}
