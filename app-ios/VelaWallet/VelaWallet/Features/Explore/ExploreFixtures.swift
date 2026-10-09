//
//  ExploreFixtures.swift
//  VelaWallet
//
//  Canonical explore fixtures (spec 022, data-model.md §2 — the single canon
//  all four platforms port; web reference: src/lib/explore/fixtures.ts).
//  Site names, hosts and the demo page are verbatim mock content (never
//  translated); every label resolves through the corpus. Issue #465: no
//  custom groups — the start page's sections are Favorites and Recent alone.
//  Pure data + assembly: no fetching, no URL parsing, no business state.
//

import SwiftUI

enum ExploreFixtures {
    // MARK: - Canon

    static let uniswap = SiteModel(id: "uniswap", name: "Uniswap", host: "app.uniswap.org",
                                   letter: "U", tint: BrandPalette.uniswap)
    static let aave = SiteModel(id: "aave", name: "Aave", host: "app.aave.com",
                                letter: "A", tint: BrandPalette.aave)
    static let pancake = SiteModel(id: "pancake", name: "PancakeSwap", host: "pancakeswap.finance",
                                   letter: "P", tint: BrandPalette.pancake)
    static let polymarket = SiteModel(id: "polymarket", name: "Polymarket", host: "polymarket.com",
                                      letter: "P", tint: BrandPalette.polymarket)
    static let opensea = SiteModel(id: "opensea", name: "OpenSea", host: "opensea.io",
                                   letter: "O", tint: BrandPalette.opensea)
    static let lido = SiteModel(id: "lido", name: "Lido", host: "stake.lido.fi",
                                letter: "L", tint: BrandPalette.lido)
    static let ens = SiteModel(id: "ens", name: "ENS", host: "app.ens.domains",
                               letter: "E", tint: BrandPalette.ens)
    static let hyperliquid = SiteModel(id: "hyperliquid", name: "Hyperliquid",
                                       host: "app.hyperliquid.xyz", letter: "H",
                                       tint: BrandPalette.hyperliquid)
    static let curve = SiteModel(id: "curve", name: "Curve", host: "curve.fi",
                                 letter: "C", tint: BrandPalette.curve)
    static let limitless = SiteModel(id: "limitless", name: "Limitless", host: "limitless.exchange",
                                     letter: "L", tint: BrandPalette.limitless)

    /// The favourites grid, in mock order (E2/DE2).
    static let favorites: [SiteModel] = [uniswap, aave, pancake, polymarket, opensea, lido, ens]

    static let network = (name: "Ethereum", dot: ChainPalette.ethereum)

    private static func withMeta(_ site: SiteModel, _ meta: String) -> SiteModel {
        var copy = site
        copy.meta = meta
        copy.subtitle = site.host
        return copy
    }

    /// The page the browser shows. Fixture content: the site's words, not ours.
    static let demoPage = DemoPageModel(
        title: "兑换",
        fields: [.init(value: "0.5", symbol: "ETH"), .init(value: "1,280.42", symbol: "USDC")],
        cta: "兑换",
        ctaTint: BrandPalette.uniswap
    )

    // MARK: - Assembly

    private static func groups(_ loc: Loc) -> [GroupModel] {
        [
            GroupModel(id: "recent", title: loc.t("explore.recent"), kind: .recent,
                       action: .clear, sites: [withMeta(hyperliquid, "刚刚")], hidden: false),
        ]
    }

    /// The sites open in tabs, in strip order — the web board's phone strip
    /// (DESIGN N): one more than the desktop's, so the home shows its resume
    /// section full, three rows under a header that counts four tabs (the
    /// start page is one of them). The order is also the recency the fixture
    /// assumes: Uniswap was left last.
    static let tabSites: [SiteModel] = [uniswap, polymarket, aave]

    /// The strip: one tab per site, then the start page's own tab.
    private static func strip(_ loc: Loc, sites: [SiteModel]) -> [ExploreTabWire] {
        sites.map { site in
            ExploreTabWire(id: site.id, url: "https://\(site.host)", title: site.name, host: site.host)
        } + [ExploreTabWire(id: "start", url: nil, title: loc.t("explore.startPage"), host: "")]
    }

    private static func tabs(_ loc: Loc, strip: [ExploreTabWire], selected: String) -> [TabModel] {
        let sites = Dictionary(uniqueKeysWithValues: tabSites.map { ($0.id, $0) })
        let models = strip.map { tab in
            TabModel(id: tab.id, title: tab.title, site: sites[tab.id],
                     selected: selected == tab.id, startPage: tab.url == nil)
        }
        // Spec 099: the long-press menu asks the core, as the live one does.
        return ExploreLive.offeringCloses(models, strip: strip)
    }

    /// The home's resume section, the shape the core hands the live one:
    /// the tabs with a page, most recent first (the strip's order is the
    /// fixture's recency), at most three, under a header counting every tab.
    /// The canon's own marks — the letters — so the gallery asks nobody for
    /// an icon.
    private static func resume(_ loc: Loc, strip: [ExploreTabWire]) -> ResumeSectionModel? {
        let sites = Dictionary(uniqueKeysWithValues: tabSites.map { ($0.id, $0) })
        let rows = strip.filter { $0.url != nil }.prefix(resumeShown).compactMap { tab -> ResumeTabModel? in
            guard var site = sites[tab.id] else { return nil }
            site.subtitle = site.host
            return ResumeTabModel(id: tab.id, site: site)
        }
        return ExploreLive.resumeSection(rows: Array(rows), tabCount: strip.count, loc: loc)
    }

    /// The most rows the resume section draws — the core's
    /// `explore_sites::RESUME_SHOWN`, which decides it for the live screen;
    /// mirrored here only so the fixture is the shape the core hands over.
    static let resumeShown = 3

    /// E6's site menu, in mock order. Forward leads (DESIGN N: it moved here
    /// from the old bottom toolbar), greyed while there is nothing ahead.
    static func siteMenuItems(_ loc: Loc, canForward: Bool = false) -> [SiteMenuItem] {
        [
            SiteMenuItem(id: "forward", icon: "arrowRight", label: loc.t("explore.forward"),
                         disabled: !canForward),
            SiteMenuItem(id: "refresh", icon: "refreshCw", label: loc.t("explore.refresh")),
            SiteMenuItem(id: "share", icon: "share2", label: loc.t("explore.share")),
            SiteMenuItem(id: "copy", icon: "copy", label: loc.t("explore.copyLink")),
            SiteMenuItem(id: "favorite", icon: "star", label: loc.t("explore.addToFavorites")),
            SiteMenuItem(id: "system", icon: "externalLink",
                          label: loc.t("explore.openInSystemBrowser")),
            SiteMenuItem(id: "disconnect", icon: "power", label: loc.t("explore.disconnect")),
            SiteMenuItem(id: "close", icon: "close", label: loc.t("explore.closePage")),
        ]
    }

    static func connection(_ loc: Loc) -> ConnectionModel {
        ConnectionModel(
            title: loc.t("explore.connectionTitle"),
            site: uniswap,
            // Spec 079 (owner): the lock alone says https; "已连接" is a fact
            // about the connection, not a claim about the site.
            statusLine: loc.t("explore.connectedTag"),
            account: (name: WalletFixtures.identity.name,
                      address: WalletFixtures.identity.addressDisplay,
                      seed: WalletFixtures.identity.addressFull),
            switchLabel: loc.t("explore.switchAccount"),
            networkLabel: loc.t("explore.network"),
            network: network,
            explainer: loc.t("explore.connectionExplainer"),
            disconnect: loc.t("explore.disconnect"),
            footnote: loc.t("explore.autoRequestHint")
        )
    }

    /// E3: Favorites and Recent dApps, an eye each — all Manage groups holds
    /// (issue #465).
    static func groupManage(_ loc: Loc) -> ExploreSheet {
        .groupManage(
            title: loc.t("explore.manageGroups"),
            rows: [
                GroupManageRow(id: "favorites", title: loc.t("explore.favorites"),
                               meta: loc.t("explore.siteCount", count: 8),
                               hidden: false),
                GroupManageRow(id: "recent", title: loc.t("explore.recent"),
                               meta: nil, hidden: false),
            ]
        )
    }

    /// Every phone state (E1–E7).
    static func buildMobileState(_ state: ExploreStateId, loc: Loc) -> ExploreHomeModel {
        let populated = state != .e1
        let browsing = state == .e4 || state == .e6 || state == .e7
        let view: ExploreView = browsing ? .browsing : (state == .e5 ? .tabs : .start)

        let tiles: [TileModel] = favorites.map { .site($0) } + [.add(loc.t("explore.add"))]
        let canForward = false
        // Spec 079 (owner): no "安全站点" — the lock alone.
        let siteMenu = ExploreSheet.siteMenu(site: uniswap, statusLine: "",
                                             items: siteMenuItems(loc, canForward: canForward))
        // E1 is the first visit: the start page is the only tab there is.
        let openTabs = strip(loc, sites: populated ? tabSites : [])
        let connectionModel = connection(loc)

        let sheet: ExploreSheet? = switch state {
        case .e3: groupManage(loc)
        case .e6: siteMenu
        case .e7: .connection(connectionModel)
        default: nil
        }

        return ExploreHomeModel(
            state: state,
            view: view,
            title: loc.t("explore.title"),
            searchPlaceholder: loc.t("explore.searchPlaceholder"),
            scanLabel: loc.t("explore.scan"),
            empty: populated ? nil : (title: loc.t("explore.startTitle"),
                                      caption: loc.t("explore.startHint"),
                                      cta: loc.t("explore.startCta")),
            resume: resume(loc, strip: openTabs),
            favorites: populated ? (title: loc.t("explore.favorites"),
                                    action: loc.t("explore.edit"), tiles: tiles) : nil,
            groups: populated ? groups(loc) : [],
            browser: BrowserModel(
                url: "https://\(uniswap.host)/swap", host: uniswap.host, secure: true, connected: true,
                canBack: true, canForward: canForward, bookmarked: false,
                account: (name: WalletFixtures.identity.name,
                          seed: WalletFixtures.identity.addressFull),
                tabCount: openTabs.count, page: demoPage
            ),
            // E5 opens the switcher FROM a page, so the page's tab is the
            // selected one — the mock's accent border is on Uniswap.
            tabs: tabs(loc, strip: openTabs, selected: browsing || state == .e5 ? "uniswap" : "start"),
            tabsScreen: TabsScreenCopy(
                title: loc.t("explore.tabs"), done: loc.t("explore.done"),
                newTab: loc.t("explore.newTab"), closeAll: loc.t("explore.closeAllTabs"),
                close: loc.t("explore.closeTab"),
                closeOthers: loc.t("explore.closeOtherTabs"),
                closeRight: loc.t("explore.closeTabsToRight")
            ),
            sheet: sheet,
            menus: (groupManage: groupManage(loc), siteMenu: siteMenu, connection: connectionModel),
            nav: TabsModel(
                wallet: loc.t("componentsUi.mainNav.wallet"),
                contacts: loc.t("componentsUi.mainNav.contacts"),
                explore: loc.t("componentsUi.mainNav.explore"),
                settings: loc.t("componentsUi.mainNav.settings")
            )
        )
    }
}

extension ExploreHomeModel {
    /// The signed-in wallet's identity over the fixture's (spec 019's swap).
    /// A connection panel naming a stranger's account would be the wallet
    /// lying about what it just granted.
    func withIdentity(name: String, address: String) -> ExploreHomeModel {
        var copy = self
        copy.browser.account = (name: name, seed: address)
        var connection = copy.menus.connection
        connection.account = (name: name, address: Self.shorten(address), seed: address)
        copy.menus.connection = connection
        return copy
    }

    /// `0x14fB1f…D1eA5c` — the phones' own short form (spec 015).
    static func shorten(_ address: String) -> String {
        guard address.count > 14 else { return address }
        return "\(address.prefix(6))…\(address.suffix(4))"
    }
}
