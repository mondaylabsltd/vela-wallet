//
//  ExploreWire.swift
//  VelaWallet
//
//  The `explore_sites` machine's view model, in Swift.
//
//  The browser's own memory: which sites are pinned, which tabs are open and
//  which one is in front. One JSON document under `vela.explore`, the same
//  document the other three clients read. (Issue #465: there are no custom
//  groups — a document written before it still loads, and the core drops its
//  `groups` on the next write.)
//
//  Views are `Decodable` through `CoreJSON.decoder`; **operations and results
//  stay dictionaries**. That is the house rule every Wire file in this app
//  repeats, and it exists because an operation is a message to *perform*, not
//  a value to model — giving it a Swift type invites a shell to branch on it.
//

import Foundation

/// One pinned site.
///
/// `origin` is the identity and `url` is where it opens: a favourite is a
/// SITE, but a person's shortcut may point deeper than the origin because
/// that is where they actually work.
struct ExploreSiteWire: Decodable, Equatable, Identifiable {
    let origin: String
    let url: String
    let host: String
    /// The site's last good title, else its host — never an engine's error
    /// page (issues #329, #425) — until somebody renames it. A rename is kept
    /// forever after — a person who named a tile meant it, and a page can
    /// change its title at will.
    let name: String
    let renamed: Bool
    let addedMs: Double

    var id: String { origin }
}

/// One open tab.
struct ExploreTabWire: Decodable, Equatable, Identifiable {
    let id: String
    /// `nil` is the start page — the tab every window has before a site is
    /// open, drawn with the wallet's own mark rather than a favicon.
    let url: String?
    let title: String
    let host: String

    /// The tab as the core reads it back (`ExploreTab`) — what
    /// `exploreTabsClosedBy` takes the strip as.
    var wire: [String: Any] {
        ["id": id, "url": url.map { $0 as Any } ?? NSNull(), "title": title, "host": host]
    }
}

/// What a batch close takes (spec 099) — the core's `TabCloseScope`,
/// Chrome's three. The shell names the one a person picked; WHICH tabs it
/// takes is the core's (`exploreTabsClosedBy`) and is never decided here.
enum ExploreTabCloseScope: Equatable {
    /// Every tab but this one ("close other tabs").
    case others(keep: String)
    /// Every tab to the right of this one in the strip ("close tabs to the
    /// right").
    case right(of: String)
    /// Every tab ("close all tabs").
    case all

    var wire: [String: Any] {
        switch self {
        case .others(let keep): ["type": "others", "keep": keep]
        case .right(let of): ["type": "right", "of": of]
        case .all: ["type": "all"]
        }
    }
}

struct ExploreViewWire: Decodable, Equatable {
    let favorites: [ExploreSiteWire]
    let tabs: [ExploreTabWire]
    /// Always a tab that exists, whenever there is one at all.
    let selectedTab: String?
    let favoritesHidden: Bool
    let recentHidden: Bool
    /// The grid is full. The shell then draws **no** add affordance rather
    /// than one that refuses: a control that cannot work is worse than an
    /// absent one.
    let favoritesFull: Bool
    /// The strip is full, for the same reason.
    let tabsFull: Bool
    /// The mirror is live. Before this a screen shows nothing rather than an
    /// empty start page it would have to correct a frame later — and, more
    /// sharply, **every mutation dispatched before this is dropped**, so an
    /// intent arriving from a deep link has to wait for it.
    let ready: Bool
    /// The tabs most recently used first (spec 099 R2) — what the core's
    /// `browserEnginePlan` keeps live, in that order.
    var recentTabs: [String] = []

    static let empty = ExploreViewWire(
        favorites: [], tabs: [], selectedTab: nil,
        favoritesHidden: false, recentHidden: false,
        favoritesFull: false, tabsFull: false, ready: false
    )

    /// The tab in front, if there is one.
    var selected: ExploreTabWire? {
        guard let selectedTab else { return nil }
        return tabs.first { $0.id == selectedTab }
    }
}
