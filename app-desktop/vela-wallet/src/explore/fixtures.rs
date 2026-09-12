//! Canonical explore fixtures — the desktop port of
//! `specs/022-explore-signing-ui/data-model.md` §2 (web reference:
//! `src/lib/explore/fixtures.ts`). Site names, hosts, group titles and the
//! demo page are verbatim mock content and are never translated; brand colours
//! are fixture data, not theme tokens — the same rule the wallet's chain dots
//! follow.

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
pub fn brand_curve() -> Hsla {
    rgb(0x7b7be8).into()
}
pub fn brand_limitless() -> Hsla {
    rgb(0x8b6dff).into()
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
    Menu,
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

fn with_subtitle(mut s: SiteModel, subtitle: &'static str) -> SiteModel {
    s.subtitle = Some(subtitle.into());
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
    vec![
        GroupModel {
            id: "recent",
            title: strings.recent.clone(),
            action: GroupAction::Clear,
            sites: vec![
                with_meta(hyperliquid.clone(), "刚刚"),
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
        },
        // Custom group titles and blurbs are what the person typed — mock
        // content, verbatim, never translated (the spec-015 rule).
        GroupModel {
            id: "trading",
            title: "交易".into(),
            action: GroupAction::Menu,
            sites: vec![
                with_subtitle(
                    site("curve", "Curve", "curve.fi", "C", brand_curve()),
                    "稳定币兑换",
                ),
                with_subtitle(hyperliquid, "永续合约交易"),
            ],
        },
        GroupModel {
            id: "prediction",
            title: "预测市场".into(),
            action: GroupAction::Menu,
            sites: vec![
                with_subtitle(
                    site(
                        "polymarket",
                        "Polymarket",
                        "polymarket.com",
                        "P",
                        brand_polymarket(),
                    ),
                    "事件预测市场",
                ),
                with_subtitle(
                    site(
                        "limitless",
                        "Limitless",
                        "limitless.exchange",
                        "L",
                        brand_limitless(),
                    ),
                    "预测市场",
                ),
            ],
        },
    ]
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

/// DE2's right-click menu on a favourite tile, and the toolbar's ⋯ site menu
/// (M3). Both ride the spec-018 menu card rather than growing a second one.
/// "Move to a group": make one, or pick one that exists (spec 032 phase 41).
///
/// A menu rather than a new picker: the question is "which of these", which is
/// what a menu already is. `new group` is FIRST because a wallet with no
/// groups yet must still be able to start one — the empty list is the common
/// case on the first use, and a menu whose only item is unreachable is a dead
/// end.
pub fn group_pick_menu(strings: &ExploreStrings, groups: &[String]) -> MenuModel {
    let mut items = vec![MenuItemModel {
        icon: Icon::FolderPlus,
        label: strings.new_group.clone(),
        destructive: false,
    }];
    for name in groups {
        items.push(MenuItemModel {
            // The same glyph the "new group" item carries: these are the
            // same kind of thing, and this shell has one folder icon.
            icon: Icon::FolderPlus,
            label: SharedString::from(name.clone()),
            destructive: false,
        });
    }
    MenuModel {
        divider_after: (!groups.is_empty()).then_some(0),
        items,
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
                icon: Icon::FolderPlus,
                label: strings.move_to_group.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Trash2,
                label: strings.remove_from_favorites.clone(),
                destructive: true,
            },
        ],
        divider_after: Some(2),
    }
}

/// The browsing toolbar's ⋯ (M3): what you can do to the page you are on.
pub fn site_menu(strings: &ExploreStrings) -> MenuModel {
    MenuModel {
        items: vec![
            MenuItemModel {
                icon: Icon::RefreshCw,
                label: strings.refresh.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Share2,
                label: strings.site_menu.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Star,
                label: strings.add_to_favorites.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::ExternalLink,
                label: strings.open_in_new_tab.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::Power,
                label: strings.disconnect.clone(),
                destructive: false,
            },
            MenuItemModel {
                icon: Icon::X,
                label: strings.close.clone(),
                destructive: false,
            },
        ],
        divider_after: Some(3),
    }
}
