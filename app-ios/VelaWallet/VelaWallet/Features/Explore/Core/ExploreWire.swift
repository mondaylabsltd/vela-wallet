//
//  ExploreWire.swift
//  VelaWallet
//
//  The `explore_sites` machine's view model, in Swift.
//
//  The browser's own memory: which sites are pinned, how they are grouped,
//  which tabs are open and which one is in front. One JSON document under
//  `vela.explore`, the same document the other three clients read.
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
    /// The page's own `<title>` until somebody renames it. A rename is kept
    /// forever after — a person who named a tile meant it, and a page can
    /// change its title at will.
    let name: String
    let renamed: Bool
    let addedMs: Double

    var id: String { origin }
}

/// A named collection of favourites, resolved for drawing.
///
/// Membership is by origin and a site may be in several groups: a group is a
/// VIEW over the favourites, never a container that owns them. Deleting one
/// keeps its sites — the rule `contacts` settled for contact groups, so a
/// person meets one behaviour rather than two.
struct ExploreGroupWire: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let hidden: Bool
    /// Already resolved from the favourites, in membership order. A member
    /// whose site is gone is dropped here rather than drawn as a blank row.
    let sites: [ExploreSiteWire]
}

/// One open tab.
struct ExploreTabWire: Decodable, Equatable, Identifiable {
    let id: String
    /// `nil` is the start page — the tab every window has before a site is
    /// open, drawn with the wallet's own mark rather than a favicon.
    let url: String?
    let title: String
    let host: String
}

struct ExploreViewWire: Decodable, Equatable {
    let favorites: [ExploreSiteWire]
    let groups: [ExploreGroupWire]
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

    static let empty = ExploreViewWire(
        favorites: [], groups: [], tabs: [], selectedTab: nil,
        favoritesHidden: false, recentHidden: false,
        favoritesFull: false, tabsFull: false, ready: false
    )

    /// The tab in front, if there is one.
    var selected: ExploreTabWire? {
        guard let selectedTab else { return nil }
        return tabs.first { $0.id == selectedTab }
    }
}
