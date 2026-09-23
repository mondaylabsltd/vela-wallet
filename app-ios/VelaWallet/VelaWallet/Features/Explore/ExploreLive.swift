//
//  ExploreLive.swift
//  VelaWallet
//
//  The browser's screens, built from what the machines say instead of from
//  `ExploreFixtures`.
//
//  Three views go in — `explore_sites` (favourites, groups, tabs),
//  `browser_history` (recents) and `dapp_permissions` (the connected chip) —
//  and the drawn `ExploreHomeModel` comes out. The fixtures stay exactly where
//  they are: E1–E7 are gallery states, and a gallery that ran somebody else's
//  JavaScript would not be a gallery.
//
//  ## A site's mark is drawn, never fetched
//
//  `LetterAvatarView` takes a grapheme and a tint, and spec 022 chose that
//  deliberately: a favicon is a request to a third party for every tile on the
//  start page, which tells that party which dApps a person keeps. So the
//  letter is the host's first character and the tint is a function of the
//  host — the same host always the same colour, without asking anybody.
//

import SwiftUI

enum ExploreLive {

    /// The whole start page, the tab strip and the browser chrome.
    static func home(
        explore: ExploreViewWire,
        history: BhistViewWire,
        permissions: DpermViewWire,
        engine: BrowserEngine?,
        identity: (name: String, address: String),
        chainId: Int = 100,
        loc: Loc
    ) -> ExploreHomeModel {
        let populated = !explore.favorites.isEmpty || !history.entries.isEmpty
            || !explore.tabs.isEmpty

        var tiles: [TileModel] = explore.favorites.map { .site(site(from: $0)) }
        // The grid is full: draw NO add affordance rather than one that
        // refuses. The core publishes the cap so the shell never counts.
        if !explore.favoritesFull { tiles.append(.add(loc.t("explore.add"))) }

        let connection = connectionModel(
            permissions: permissions, engine: engine, identity: identity,
            chainId: chainId, loc: loc
        )
        let siteMenu = ExploreSheet.siteMenu(
            site: engine.map(currentSite) ?? ExploreFixtures.uniswap,
            // The site MENU names the page; the connection sheet judges it.
            // Android splits them the same way: a menu that shouted "insecure"
            // at every http page would be a warning nobody reads, and the
            // warning belongs where a person is about to grant something.
            statusLine: (engine?.secure ?? false)
                ? loc.t("explore.secureSite")
                : (engine?.host ?? ""),
            items: ExploreFixtures.siteMenuItems(loc)
        )

        return ExploreHomeModel(
            state: .e2,
            // The screen decides which view is on; the controller's tabs are
            // what it decides from. This field is the fixture's story and the
            // live screen overrides it.
            view: engine == nil ? .start : .browsing,
            title: loc.t("explore.title"),
            tabCountLabel: explore.tabs.isEmpty ? nil : String(explore.tabs.count),
            searchPlaceholder: loc.t("explore.searchPlaceholder"),
            scanLabel: loc.t("explore.scan"),
            empty: populated ? nil : (
                title: loc.t("explore.startTitle"),
                caption: loc.t("explore.startHint"),
                cta: loc.t("explore.startCta")
            ),
            favorites: explore.favoritesHidden || explore.favorites.isEmpty
                ? nil
                : (title: loc.t("explore.favorites"), action: loc.t("explore.edit"), tiles: tiles),
            groups: groups(explore: explore, history: history, loc: loc),
            browser: browserModel(
                explore: explore, permissions: permissions, engine: engine,
                identity: identity
            ),
            tabs: tabs(explore: explore, loc: loc),
            tabsScreen: TabsScreenCopy(
                title: loc.t("explore.tabs"), done: loc.t("explore.done"),
                newTab: loc.t("explore.newTab"), closeAll: loc.t("explore.closeAllTabs"),
                close: loc.t("explore.closeTab")
            ),
            sheet: nil,
            menus: (
                groupManage: groupManage(explore: explore, loc: loc),
                siteMenu: siteMenu,
                connection: connection
            ),
            nav: TabsModel(
                wallet: loc.t("componentsUi.mainNav.wallet"),
                contacts: loc.t("componentsUi.mainNav.contacts"),
                explore: loc.t("componentsUi.mainNav.explore"),
                settings: loc.t("componentsUi.mainNav.settings")
            )
        )
    }

    // MARK: - Groups

    /// 最近 first, then the person's own, in their own order.
    ///
    /// Recents is a system group and comes from a different machine, which is
    /// why it is assembled here rather than being one of `explore.groups`. A
    /// hidden system group keeps everything and draws nothing.
    static func groups(
        explore: ExploreViewWire, history: BhistViewWire, loc: Loc
    ) -> [GroupModel] {
        var rows: [GroupModel] = []

        if !explore.recentHidden, !history.entries.isEmpty {
            rows.append(GroupModel(
                id: "recent",
                title: loc.t("explore.recent"),
                kind: .recent,
                action: .clear,
                sites: history.entries.map { entry in
                    var site = self.site(host: entry.host, name: entry.title, origin: entry.origin)
                    site.subtitle = entry.host
                    return site
                },
                hidden: false
            ))
        }

        rows.append(contentsOf: explore.groups.filter { !$0.hidden }.map { group in
            GroupModel(
                id: group.id,
                title: group.name,
                kind: .custom,
                action: .menu,
                sites: group.sites.map { pinned in
                    var site = self.site(from: pinned)
                    site.subtitle = pinned.host
                    return site
                },
                hidden: false
            )
        })
        return rows
    }

    static func groupManage(explore: ExploreViewWire, loc: Loc) -> ExploreSheet {
        var rows = [
            GroupManageRow(
                id: "favorites", title: loc.t("explore.favorites"),
                meta: loc.t("explore.siteCount", vars: ["n": String(explore.favorites.count)]),
                system: true, hidden: explore.favoritesHidden
            ),
            GroupManageRow(
                id: "recent", title: loc.t("explore.recent"),
                meta: loc.t("explore.systemGroup"),
                system: true, hidden: explore.recentHidden
            ),
        ]
        rows.append(contentsOf: explore.groups.map { group in
            GroupManageRow(
                id: group.id, title: group.name,
                meta: loc.t("explore.siteCount", vars: ["n": String(group.sites.count)]),
                system: false, hidden: group.hidden
            )
        })
        return .groupManage(
            title: loc.t("explore.manageGroups"), rows: rows,
            newGroup: loc.t("explore.newGroup")
        )
    }

    // MARK: - Tabs

    static func tabs(explore: ExploreViewWire, loc: Loc) -> [TabModel] {
        explore.tabs.map { tab in
            let isStart = (tab.url ?? "").isEmpty
            return TabModel(
                id: tab.id,
                title: isStart ? loc.t("explore.startPage") : displayTitle(tab),
                site: isStart ? nil : site(host: tab.host, name: displayTitle(tab), origin: tab.host),
                selected: explore.selectedTab == tab.id,
                startPage: isStart
            )
        }
    }

    /// The page's own `<title>` when it has one, the host when it does not.
    /// A blank strip entry is worse than a host.
    private static func displayTitle(_ tab: ExploreTabWire) -> String {
        tab.title.isEmpty ? tab.host : tab.title
    }

    // MARK: - The browser chrome

    static func browserModel(
        explore: ExploreViewWire,
        permissions: DpermViewWire,
        engine: BrowserEngine?,
        identity: (name: String, address: String)
    ) -> BrowserModel {
        BrowserModel(
            url: engine?.url ?? "",
            host: engine?.host ?? "",
            secure: engine?.secure ?? false,
            connected: permissions.isConnected,
            canBack: engine?.canGoBack ?? false,
            canForward: engine?.canGoForward ?? false,
            bookmarked: engine.map { current in
                explore.favorites.contains { $0.origin == current.origin }
            } ?? false,
            account: (name: identity.name, seed: identity.address),
            tabCount: explore.tabs.count,
            // Never drawn while an engine exists — the screen draws the web
            // view instead — and kept so the gallery's E4 still has a page.
            page: ExploreFixtures.demoPage
        )
    }

    static func connectionModel(
        permissions: DpermViewWire,
        engine: BrowserEngine?,
        identity: (name: String, address: String),
        chainId: Int = 100,
        loc: Loc
    ) -> ConnectionModel {
        // The origin that is ASKING outranks the one in the address bar. They
        // are normally the same; when a page navigates with a request still
        // open they are not, and the sheet must name the asker.
        let origin = permissions.consent?.origin
            ?? permissions.currentOrigin
            ?? engine?.origin ?? ""
        let host = BrowserEngine.hostOf(origin: origin)
        // The **origin** is the fact. A site's name and its icon are claims it
        // makes about itself, and a consent sheet that led with the claim
        // would be a sheet somebody can dress up.
        let asked = site(host: host, name: host, origin: origin)

        let status = statusLine(
            secure: origin.hasPrefix("https://"),
            connected: permissions.isConnected,
            host: host,
            loc: loc
        )

        return ConnectionModel(
            // A site that is ASKING is named in the title — "连接到 {host}".
            // The anti-phishing line: the sheet's first sentence is the origin
            // the request came from, not a generic heading a person skims.
            // Android has titled it this way since 044.
            title: permissions.consent == nil
                ? loc.t("explore.connectionTitle")
                : loc.t("connect.browser.title", vars: ["host": host]),
            site: asked,
            statusLine: status,
            // WHICH account this site holds, not which one the wallet is on.
            //
            // A grant is pinned to the address it was given to, so the two can
            // differ — and they differ exactly when it matters: after an
            // account switch, or on a grant made before one. The panel named
            // `identity` regardless, so a device found it saying
            // "Parallel One · 0x88cC…6894" over a page that had been handed
            // `0xA9aE…2B`. The identicon goes with it: that artwork is the
            // anti-forgery mark, and a mark for the wrong account is worse
            // than none.
            //
            // A consent card has no grant yet, so it names the account that is
            // about to get one — which IS the active one.
            account: grantedAccount(permissions: permissions, identity: identity),
            switchLabel: loc.t("explore.switchAccount"),
            networkLabel: loc.t("explore.network"),
            network: (
                name: ChainCatalog.meta(chainId)?.displayName ?? String(chainId),
                dot: SettingsLive.chainColor(chainId)
            ),
            explainer: loc.t("explore.connectionExplainer"),
            disconnect: loc.t("explore.disconnect"),
            // When a site is ASKING, the explainer is the one written for
            // exactly that moment: what a connection is, and what it is not.
            footnote: permissions.consent == nil
                ? loc.t("explore.autoRequestHint")
                : loc.t("connect.browser.body"),
            secure: origin.hasPrefix("https://"),
            consent: permissions.consent == nil ? nil : (
                approve: loc.t("connect.dapp.approve"),
                reject: loc.t("connect.dapp.reject")
            )
        )
    }

    /// The line under a site's name.
    ///
    /// 056 recorded that **no corpus sentence existed** for an insecure site
    /// and described an http page by its bare host. That was wrong:
    /// `connect.browser.a11yInsecure` — "Insecure site — not encrypted" — is
    /// in the corpus, translated into all fifteen languages, and Android has
    /// used it since 044. The copy ruler in 058 is what found the difference.
    ///
    /// The host is still shown when there is no sentence to show (an empty
    /// origin), because naming what you are looking at beats saying nothing.
    /// The account a connection panel is ABOUT.
    ///
    /// The granted address when the site holds one and it is not the active
    /// account; the active account otherwise. Where they differ the name is
    /// the address itself — this shell does not have the other account's name
    /// here, and a name that belongs to somebody else is the one thing this
    /// row must never print.
    private static func grantedAccount(
        permissions: DpermViewWire,
        identity: (name: String, address: String)
    ) -> (name: String, address: String, seed: String) {
        guard let granted = permissions.connectedAddress,
              !granted.isEmpty,
              granted.caseInsensitiveCompare(identity.address) != .orderedSame
        else {
            return (identity.name, AddressText.short(identity.address), identity.address)
        }
        return (AddressText.short(granted), AddressText.short(granted), granted)
    }

    static func statusLine(secure: Bool, connected: Bool, host: String, loc: Loc) -> String {
        let pieces = [
            secure ? loc.t("explore.secureSite") : loc.t("connect.browser.a11yInsecure"),
            connected ? loc.t("explore.connectedTag") : nil,
        ].compactMap { $0 }
        return pieces.joined(separator: " · ")
    }

    // MARK: - Marks

    static func currentSite(_ engine: BrowserEngine) -> SiteModel {
        var model = site(
            host: engine.host,
            name: engine.title.isEmpty ? engine.host : engine.title,
            origin: engine.origin
        )
        model.subtitle = engine.host
        return model
    }

    static func site(from pinned: ExploreSiteWire) -> SiteModel {
        site(host: pinned.host, name: pinned.name.isEmpty ? pinned.host : pinned.name,
             origin: pinned.origin)
    }

    /// One site's drawn mark.
    ///
    /// The letter is the first grapheme of the host with `www.` dropped —
    /// every site under one domain would otherwise be a W. Upper-cased,
    /// because a tile of lower-case letters reads as a typo.
    static func site(host: String, name: String, origin: String) -> SiteModel {
        let bare = host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
        let letter = bare.first.map { String($0).uppercased() } ?? "?"
        return SiteModel(
            id: origin.isEmpty ? host : origin,
            name: name.isEmpty ? host : name,
            host: host,
            letter: letter,
            tint: tint(for: bare)
        )
    }

    /// A stable colour for a host, from the palette the fixtures already use.
    ///
    /// Not random and not stored: the same host is the same colour on every
    /// launch and on every device, which is what makes a tile recognisable at
    /// a glance. The known brands keep their own colours, so a person who has
    /// seen the gallery meets the same Uniswap pink.
    static func tint(for host: String) -> Color {
        if let known = Self.brands[host] { return known }
        let palette: [Color] = [
            BrandPalette.uniswap, BrandPalette.aave, BrandPalette.pancake,
            BrandPalette.polymarket, BrandPalette.opensea, BrandPalette.lido,
            BrandPalette.ens, BrandPalette.hyperliquid, BrandPalette.curve,
            BrandPalette.oneinch, BrandPalette.morpho, BrandPalette.safe,
        ]
        guard !host.isEmpty else { return BrandPalette.unknown }
        // FNV-1a over the host's bytes: short, stable across processes, and
        // not `hashValue`, which Swift seeds per-process — the same site would
        // change colour on every launch.
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in host.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x0000_0100_0000_01b3
        }
        return palette[Int(hash % UInt64(palette.count))]
    }

    private static let brands: [String: Color] = [
        "app.uniswap.org": BrandPalette.uniswap,
        "app.aave.com": BrandPalette.aave,
        "pancakeswap.finance": BrandPalette.pancake,
        "polymarket.com": BrandPalette.polymarket,
        "opensea.io": BrandPalette.opensea,
        "stake.lido.fi": BrandPalette.lido,
        "app.ens.domains": BrandPalette.ens,
        "app.hyperliquid.xyz": BrandPalette.hyperliquid,
        "curve.fi": BrandPalette.curve,
        "limitless.exchange": BrandPalette.limitless,
    ]
}
