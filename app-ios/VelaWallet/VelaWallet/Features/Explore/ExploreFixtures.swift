//
//  ExploreFixtures.swift
//  VelaWallet
//
//  Canonical explore fixtures (spec 022, data-model.md §2 — the single canon
//  all four platforms port; web reference: src/lib/explore/fixtures.ts).
//  Site names, hosts, group titles and the demo page are verbatim mock
//  content (never translated); every label resolves through the corpus.
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

    private static func withSubtitle(_ site: SiteModel, _ subtitle: String) -> SiteModel {
        var copy = site
        copy.subtitle = subtitle
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
            // Custom group titles and blurbs are what the person typed — mock
            // content, verbatim, never translated (the spec-015 rule).
            GroupModel(id: "trading", title: "交易", kind: .custom, action: .menu,
                       sites: [withSubtitle(curve, "稳定币兑换"),
                               withSubtitle(hyperliquid, "永续合约交易")], hidden: false),
            GroupModel(id: "prediction", title: "预测市场", kind: .custom, action: .menu,
                       sites: [withSubtitle(polymarket, "事件预测市场"),
                               withSubtitle(limitless, "预测市场")], hidden: false),
        ]
    }

    private static func tabs(_ loc: Loc, selected: String) -> [TabModel] {
        [
            TabModel(id: "uniswap", title: uniswap.name, site: uniswap,
                     selected: selected == "uniswap", startPage: false),
            TabModel(id: "polymarket", title: polymarket.name, site: polymarket,
                     selected: selected == "polymarket", startPage: false),
            TabModel(id: "start", title: loc.t("explore.startPage"), site: nil,
                     selected: selected == "start", startPage: true),
        ]
    }

    /// E6's site menu, in mock order.
    static func siteMenuItems(_ loc: Loc) -> [SiteMenuItem] {
        [
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

    static func groupManage(_ loc: Loc) -> ExploreSheet {
        .groupManage(
            title: loc.t("explore.manageGroups"),
            rows: [
                GroupManageRow(id: "favorites", title: loc.t("explore.favorites"),
                               meta: loc.t("explore.siteCount", vars: ["n": "8"]),
                               system: true, hidden: false),
                GroupManageRow(id: "recent", title: loc.t("explore.recent"),
                               meta: loc.t("explore.systemGroup"), system: true, hidden: false),
                GroupManageRow(id: "trading", title: "交易",
                               meta: loc.t("explore.siteCount", vars: ["n": "4"]),
                               system: false, hidden: false),
                GroupManageRow(id: "prediction", title: "预测市场",
                               meta: loc.t("explore.siteCount", vars: ["n": "2"]),
                               system: false, hidden: false),
            ],
            newGroup: loc.t("explore.newGroup")
        )
    }

    /// Every phone state (E1–E7).
    static func buildMobileState(_ state: ExploreStateId, loc: Loc) -> ExploreHomeModel {
        let populated = state != .e1
        let browsing = state == .e4 || state == .e6 || state == .e7
        let view: ExploreView = browsing ? .browsing : (state == .e5 ? .tabs : .start)

        let tiles: [TileModel] = favorites.map { .site($0) } + [.add(loc.t("explore.add"))]
        // Spec 079 (owner): no "安全站点" — the lock alone.
        let siteMenu = ExploreSheet.siteMenu(site: uniswap, statusLine: "",
                                             items: siteMenuItems(loc))
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
            tabCountLabel: populated ? "2" : nil,
            searchPlaceholder: loc.t("explore.searchPlaceholder"),
            scanLabel: loc.t("explore.scan"),
            empty: populated ? nil : (title: loc.t("explore.startTitle"),
                                      caption: loc.t("explore.startHint"),
                                      cta: loc.t("explore.startCta")),
            favorites: populated ? (title: loc.t("explore.favorites"),
                                    action: loc.t("explore.edit"), tiles: tiles) : nil,
            groups: populated ? groups(loc) : [],
            browser: BrowserModel(
                url: uniswap.host, host: uniswap.host, secure: true, connected: true,
                canBack: true, canForward: false, bookmarked: false,
                account: (name: WalletFixtures.identity.name,
                          seed: WalletFixtures.identity.addressFull),
                tabCount: 2, page: demoPage
            ),
            // E5 opens the switcher FROM a page, so the page's tab is the
            // selected one — the mock's accent border is on Uniswap.
            tabs: tabs(loc, selected: browsing || state == .e5 ? "uniswap" : "start"),
            tabsScreen: TabsScreenCopy(
                title: loc.t("explore.tabs"), done: loc.t("explore.done"),
                newTab: loc.t("explore.newTab"), closeAll: loc.t("explore.closeAllTabs"),
                close: loc.t("explore.closeTab")
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
