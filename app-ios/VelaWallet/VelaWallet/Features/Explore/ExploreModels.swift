//
//  ExploreModels.swift
//  VelaWallet
//
//  Explore view models (spec 022, data-model.md §2 — the iOS port of the web
//  reference `src/lib/explore/model.ts`). Components consume ONLY these
//  display-ready shapes: no fetching, no URL parsing, no business state. A
//  real browser engine and a dApp registry replace the fixture layer that
//  builds them and nothing else.
//

import SwiftUI

enum ExploreStateId: String, CaseIterable, Identifiable {
    case e1, e2, e3, e4, e5, e6, e7
    var id: String { rawValue }
    /// Gallery chip label — mock naming, not translatable copy.
    var label: String { rawValue.uppercased() }
}

struct SiteModel: Identifiable, Hashable {
    let id: String
    let name: String
    let host: String
    /// Single grapheme drawn in the avatar — never a fetched favicon.
    let letter: String
    let tint: Color
    var subtitle: String?
    /// "刚刚" / "昨天" — fixture content.
    var meta: String?
}

enum TileModel: Identifiable {
    case site(SiteModel)
    case add(String)

    var id: String {
        switch self {
        case .site(let site): site.id
        case .add: "add"
        }
    }
}

/// `favorites` and `recent` are system groups: hideable, never deletable.
enum GroupKind { case favorites, recent, custom }

/// The trailing affordance on a group's header row.
enum GroupAction { case edit, clear, menu }

struct GroupModel: Identifiable {
    let id: String
    let title: String
    let kind: GroupKind
    let action: GroupAction?
    let sites: [SiteModel]
    var hidden: Bool
}

struct TabModel: Identifiable {
    let id: String
    let title: String
    let site: SiteModel?
    let selected: Bool
    /// The start page's own tab — drawn with the sail, not a favicon.
    let startPage: Bool
}

/// The page inside the browser. FIXTURE CONTENT, not chrome: it stands in for
/// whatever site is open, so its words are the mock's and are never
/// translated. A real WKWebView replaces this view wholesale.
struct DemoPageModel {
    struct Field { let value: String; let symbol: String }
    let title: String
    let fields: [Field]
    let cta: String
    let ctaTint: Color
}

struct BrowserModel {
    let url: String
    let host: String
    let secure: Bool
    let connected: Bool
    let canBack: Bool
    let canForward: Bool
    let bookmarked: Bool
    var account: (name: String, seed: String)
    let tabCount: Int
    let page: DemoPageModel
}

struct SiteMenuItem: Identifiable {
    let id: String
    /// A `LucideIcon` name; the model file stays free of icon imports.
    let icon: String
    let label: String
    var danger: Bool = false
}

struct GroupManageRow: Identifiable {
    let id: String
    let title: String
    /// "8 个网站" / "2 · 已隐藏" — resolved by the fixture layer.
    let meta: String?
    let system: Bool
    var hidden: Bool
}

struct ConnectionModel {
    let title: String
    let site: SiteModel
    let statusLine: String
    var account: (name: String, address: String, seed: String)
    let switchLabel: String
    let networkLabel: String
    let network: (name: String, dot: Color)
    let explainer: String
    let disconnect: String
    let footnote: String
    /// Whether the origin is on TLS. Drives the padlock, and nothing else may.
    var secure: Bool = true
    /// The panel's **not-yet-connected** form: a site is asking, and the two
    /// answers replace 断开连接. Same panel, because the facts a person needs
    /// in order to decide are exactly the facts they need in order to review
    /// — who is asking, which account, which network — and a second sheet
    /// would be a second chance to get one of them wrong.
    var consent: (approve: String, reject: String)?
}

enum ExploreSheet: Identifiable {
    case groupManage(title: String, rows: [GroupManageRow], newGroup: String)
    case siteMenu(site: SiteModel, statusLine: String, items: [SiteMenuItem])
    case connection(ConnectionModel)

    var id: String {
        switch self {
        case .groupManage: "group-manage"
        case .siteMenu: "site-menu"
        case .connection: "connection"
        }
    }
}

/// Which surface the screen is showing (SPEC 动效 · 探索 手机).
enum ExploreView { case start, browsing, tabs }

struct TabsScreenCopy {
    let title: String
    let done: String
    let newTab: String
    let closeAll: String
    let close: String
}

struct ExploreHomeModel {
    let state: ExploreStateId
    let view: ExploreView
    let title: String
    let tabCountLabel: String?
    let searchPlaceholder: String
    let scanLabel: String
    let empty: (title: String, caption: String, cta: String)?
    let favorites: (title: String, action: String, tiles: [TileModel])?
    var groups: [GroupModel]
    var browser: BrowserModel
    let tabs: [TabModel]
    let tabsScreen: TabsScreenCopy
    /// Which sheet the state opens with, if any (E3/E6/E7).
    let sheet: ExploreSheet?
    /// The sheets browsing can raise on demand — part of the model rather than
    /// built at the tap, so a screen never invents copy at interaction time.
    var menus: (groupManage: ExploreSheet, siteMenu: ExploreSheet, connection: ConnectionModel)
    let nav: TabsModel
}
