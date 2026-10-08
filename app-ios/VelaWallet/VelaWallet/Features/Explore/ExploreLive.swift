//
//  ExploreLive.swift
//  VelaWallet
//
//  The browser's screens, built from what the machines say instead of from
//  `ExploreFixtures`.
//
//  Three views go in — `explore_sites` (favourites, tabs),
//  `browser_history` (recents) and `dapp_browser` (the consent sheet and, per
//  tab, the origin, the connected account, the chain, the lock and whether the
//  page crashed) — and the drawn `ExploreHomeModel` comes out. The fixtures stay exactly where
//  they are: E1–E7 are gallery states, and a gallery that ran somebody else's
//  JavaScript would not be a gallery.
//
//  ## A site's mark: its own icon, its letter until then
//
//  Spec 022 drew a letter only, so as not to ask a third party for every tile.
//  The owner ruled otherwise in 2026-09 (https only, no referrer) and spec 079
//  applied it here: a site shows the icon it named when visited, then the
//  usual https places (`SigningLive.siteIconUrls`), and its letter — the
//  core's rule, `browserSiteLetter`, so `app.uniswap.org` is "U" — until one
//  lands and when none does. The tint is still a function of the host: the
//  same host always the same colour, without asking anybody.
//

import SwiftUI
import VelaCore

enum ExploreLive {

    /// The whole start page, the tab strip and the browser chrome.
    static func home(
        explore: ExploreViewWire,
        history: BhistViewWire,
        dbr: DbrViewWire,
        engine: BrowserEngine?,
        identity: (name: String, address: String),
        chainIds: [Int] = [],
        /// Spec 079: what the account holds per network, in the display
        /// currency (`WalletLive.networkHoldings`) — never fetched here.
        holdings: [Int: String] = [:],
        /// Spec 079: each tab's last snapshot, by tab id.
        snapshot: (String) -> UIImage? = { _ in nil },
        loc: Loc
    ) -> ExploreHomeModel {
        let populated = !explore.favorites.isEmpty || !history.entries.isEmpty
            || !explore.tabs.isEmpty
        // The core's facts about the tab in front — never another tab's.
        let tab = engine == nil ? nil : dbr.tab(explore.selectedTab)

        var tiles: [TileModel] = explore.favorites.map { .site(site(from: $0)) }
        // The grid is full: draw NO add affordance rather than one that
        // refuses. The core publishes the cap so the shell never counts.
        if !explore.favoritesFull { tiles.append(.add(loc.t("explore.add"))) }

        let connection = connectionModel(
            dbr: dbr, tab: tab, engine: engine, identity: identity,
            chainIds: chainIds, holdings: holdings, loc: loc
        )
        let bookmarked = engine.map { current in
            explore.favorites.contains { $0.origin == current.origin }
        } ?? false
        let siteMenu = ExploreSheet.siteMenu(
            site: engine.map(currentSite) ?? ExploreFixtures.uniswap,
            // Spec 079 (owner): the lock alone — no "安全站点", no words.
            statusLine: "",
            items: siteMenuItems(
                bookmarked: bookmarked, connected: tab?.connectedAddress != nil,
                loading: engine?.loading ?? false, loc: loc
            )
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
            // The Favorites heading stays on any page with something on it —
            // no favourites yet, or Favorites hidden: its Edit is the way to
            // Manage groups, and with both sections hidden the page was left
            // with the search field alone and no way back (issue #330).
            // Hidden, it loses its tiles, not its heading — the desktop's rule
            // (078 W-11).
            favorites: populated
                ? (title: loc.t("explore.favorites"), action: loc.t("explore.edit"),
                   tiles: explore.favoritesHidden ? [] : tiles)
                : nil,
            groups: groups(explore: explore, history: history, loc: loc),
            browser: browserModel(
                explore: explore, tab: tab, engine: engine,
                identity: identity
            ),
            tabs: tabs(explore: explore, snapshot: snapshot, loc: loc),
            tabsScreen: TabsScreenCopy(
                title: loc.t("explore.tabs"), done: loc.t("explore.done"),
                newTab: loc.t("explore.newTab"), closeAll: loc.t("explore.closeAllTabs"),
                close: loc.t("explore.closeTab"),
                closeOthers: loc.t("explore.closeOtherTabs"),
                closeRight: loc.t("explore.closeTabsToRight")
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

    /// The ⋯ sheet's items, saying what a tap will do NOW: the star's row
    /// removes a site that is already a favourite, Disconnect is offered only
    /// to a site that is connected, and — spec 082 RE5 — while a page loads
    /// the refresh row is Stop.
    static func siteMenuItems(
        bookmarked: Bool, connected: Bool, loading: Bool = false, loc: Loc
    ) -> [SiteMenuItem] {
        ExploreFixtures.siteMenuItems(loc).compactMap { item in
            switch item.id {
            case "refresh" where loading:
                return SiteMenuItem(id: "stop", icon: "close", label: loc.t("connect.dapp.stop"))
            case "favorite" where bookmarked:
                return SiteMenuItem(id: item.id, icon: "starSolid",
                                    label: loc.t("explore.removeFromFavorites"))
            case "disconnect" where !connected:
                return nil
            default:
                return item
            }
        }
    }

    // MARK: - Sections

    /// The sections under Favorites: 最近 alone (issue #465 — there are no
    /// custom groups).
    ///
    /// Recents comes from a different machine (`browser_history`), which is
    /// why it is assembled here. Hidden, it keeps everything and draws
    /// nothing.
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
                    // The icon the page named when it was visited, then the
                    // usual places (spec 079). Https only.
                    site.iconUrls = iconUrls(recorded: entry.favicon, origin: entry.origin)
                    return site
                },
                hidden: false
            ))
        }

        return rows
    }

    /// Manage groups: exactly Favorites and Recent dApps, each with its eye
    /// (issue #465). Recent carries no second word — "System" said nothing
    /// once every row is one.
    static func groupManage(explore: ExploreViewWire, loc: Loc) -> ExploreSheet {
        .groupManage(
            title: loc.t("explore.manageGroups"),
            rows: [
                GroupManageRow(
                    id: "favorites", title: loc.t("explore.favorites"),
                    meta: loc.t("explore.siteCount", vars: ["n": String(explore.favorites.count)]),
                    hidden: explore.favoritesHidden
                ),
                GroupManageRow(
                    id: "recent", title: loc.t("explore.recent"),
                    meta: nil, hidden: explore.recentHidden
                ),
            ]
        )
    }

    // MARK: - Tabs

    static func tabs(
        explore: ExploreViewWire, snapshot: (String) -> UIImage? = { _ in nil }, loc: Loc
    ) -> [TabModel] {
        let models = explore.tabs.map { tab in
            let isStart = (tab.url ?? "").isEmpty
            var model = TabModel(
                id: tab.id,
                title: isStart ? loc.t("explore.startPage") : displayTitle(tab),
                site: isStart ? nil : site(
                    host: tab.host, name: displayTitle(tab),
                    origin: tab.url.map { ProviderBridge.origin(of: $0) } ?? tab.host
                ),
                selected: explore.selectedTab == tab.id,
                startPage: isStart
            )
            // A start page keeps its drawing; a page shows itself.
            if !isStart { model.snapshot = snapshot(tab.id) }
            return model
        }
        return offeringCloses(models, strip: explore.tabs)
    }

    /// Which batch closes each card's long-press menu offers (spec 099): the
    /// ones whose scope the core says takes at least one tab of `strip`.
    static func offeringCloses(_ models: [TabModel], strip: [ExploreTabWire]) -> [TabModel] {
        let json = BrowserController.stripJSON(strip)
        return models.map { tab in
            var tab = tab
            tab.closesOthers = !BrowserController.tabsClosed(by: .others(keep: tab.id), strip: json).isEmpty
            tab.closesRight = !BrowserController.tabsClosed(by: .right(of: tab.id), strip: json).isEmpty
            return tab
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
        tab: DbrTabViewWire?,
        engine: BrowserEngine?,
        identity: (name: String, address: String)
    ) -> BrowserModel {
        BrowserModel(
            url: engine?.url ?? "",
            host: engine?.host ?? "",
            // The core's judgement of the tab's origin, never a prefix check
            // here: a loopback dev server is not "insecure", and a page that
            // has not said hello yet is not "secure" either.
            secure: tab?.secure ?? false,
            connected: tab?.connectedAddress != nil,
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
        dbr: DbrViewWire,
        tab: DbrTabViewWire?,
        engine: BrowserEngine?,
        identity: (name: String, address: String),
        chainIds: [Int] = [],
        holdings: [Int: String] = [:],
        loc: Loc
    ) -> ConnectionModel {
        // The origin that is ASKING outranks the one in front. They are
        // normally the same; when a background tab asks, or a page navigates
        // with a request still open, they are not, and the sheet must name
        // the asker.
        let consent = dbr.consent
        let origin = consent?.origin ?? tab?.origin ?? engine?.origin ?? ""
        let host = BrowserEngine.hostOf(origin: origin)
        // The **origin** is the fact. A site's name and its icon are claims it
        // makes about itself, and a consent sheet that led with the claim
        // would be a sheet somebody can dress up.
        let asked = site(host: host, name: host, origin: origin)
        // The asker's own facts: the tab that asked, when it is not in front.
        let facts = consent.flatMap { dbr.tab($0.tab) } ?? tab
        let secure = facts?.secure ?? false
        let connected = consent == nil && tab?.connectedAddress != nil
        let chainId = consent?.chainId ?? tab?.chainId ?? 1

        // The account the site SEES — the grant's — or, while it asks, the
        // one a grant would be made for. Named when it is this wallet's
        // active account, which it is unless a switch is still landing.
        let shown = consent?.address ?? tab?.connectedAddress ?? identity.address
        let isActive = shown.caseInsensitiveCompare(identity.address) == .orderedSame

        return ConnectionModel(
            // A site that is ASKING is named in the title — "连接到 {host}".
            // The anti-phishing line: the sheet's first sentence is the origin
            // the request came from, not a generic heading a person skims.
            // Android has titled it this way since 044.
            title: consent == nil
                ? loc.t("explore.connectionTitle")
                : loc.t("connect.browser.title", vars: ["host": host]),
            site: asked,
            // Asking: no words under the host (the title asks). Otherwise
            // the core's fact about the grant — "已连接", or that there is
            // none (spec 097 E).
            statusLine: consent == nil
                ? statusLine(secure: secure, connected: connected, host: host, loc: loc)
                : "",
            // WHICH account this site holds, not which one the wallet is on.
            //
            // A grant is pinned to the address it was given to, so the two can
            // differ — and they differ exactly when it matters: after an
            // account switch, or on a grant made before one. The panel named
            // `identity` regardless, so a device found it saying
            // "Parallel One · 0x88cC…6894" over a page that had been handed
            // `0xA9aE…2B`. The identicon goes with it — `seed: shown` — because
            // that artwork is the anti-forgery mark, and a mark for the wrong
            // account is worse than none.
            //
            // A consent card has no grant yet, so `shown` falls through to the
            // account that is about to get one, which IS the active one.
            account: (
                name: isActive ? identity.name : AddressText.short(shown),
                address: AddressText.short(shown),
                seed: shown
            ),
            switchLabel: loc.t("explore.switchAccount"),
            networkLabel: loc.t("explore.network"),
            network: (
                name: chainName(chainId),
                dot: SettingsLive.chainColor(chainId)
            ),
            // When a site is ASKING, the one sentence is the one written for
            // exactly that moment — what a connection is, and what it is not
            // — above the answers, and nothing under them (spec 082 RE6: two
            // explainers said it twice). Connected: unchanged.
            // Not connected (spec 097 E): no sentence about what the site can
            // see — it can see nothing.
            explainer: consent != nil
                ? loc.t("connect.browser.body")
                : connected ? loc.t("explore.connectionExplainer") : "",
            disconnect: loc.t("explore.disconnect"),
            footnote: connected ? loc.t("explore.autoRequestHint") : "",
            secure: secure,
            // The approve word every client uses (spec 079): "连接" — it was
            // "批准" here alone.
            consent: consent == nil ? nil : (
                approve: loc.t("connect.browser.connect"),
                reject: loc.t("connect.dapp.reject")
            ),
            connected: connected,
            origin: origin,
            chainId: chainId,
            // The wallet's own networks, and nothing a page named — each with
            // its logo and what the account holds there (spec 079).
            networks: origin.isEmpty ? [] : chainIds.map { id in
                NetworkChoiceModel(
                    id: id, name: chainName(id), dot: SettingsLive.chainColor(id),
                    logoUrl: Marks.chainLogoURL(id), amount: holdings[id]
                )
            },
            networkLogoUrl: Marks.chainLogoURL(chainId)
        )
    }

    /// Spec 079 US4: the one-line notice when the page's chain cannot be
    /// reached — its chain is in the pool's failed set and not merely rate
    /// limited (a 429 is transient and stays quiet, the wallet's standing
    /// rule). `nil` when there is nothing to say; it goes by itself when the
    /// chain answers again, because the pool drops it from the set.
    ///
    /// Spec 082 RF1: `failed ∪ unreached ∖ rate-limited` — a chain one call's
    /// first pass could not reach at all is named while the dApp still waits,
    /// not after three passes (G33). The home banner keeps `failed` alone.
    static func chainNotice(
        chainId: Int?, failed: [Int], unreached: [Int] = [], rateLimited: [Int], loc: Loc
    ) -> String? {
        guard let chainId, failed.contains(chainId) || unreached.contains(chainId),
              !rateLimited.contains(chainId)
        else { return nil }
        return loc.t("explore.chainDown", vars: ["chain": chainName(chainId)])
    }

    private static func chainName(_ chainId: Int) -> String {
        ChainCatalog.meta(chainId)?.displayName ?? String(chainId)
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
    ///
    /// Spec 079 (owner: "用一把锁代表 https 和非https 就行了，不文字标记"): the
    /// lock beside it says the scheme, and this line says only what is a
    /// fact about the connection — "已连接" — or nothing. Neither "安全站点"
    /// nor "不安全站点" is drawn; the http lock carries
    /// `connect.browser.a11yInsecure` for a screen reader.
    ///
    /// Spec 097 E: a site with no grant says so — "No active connection"
    /// (`home.connEmptyTitle`) — where the line used to go blank over a panel
    /// that still said what the site could see.
    static func statusLine(secure: Bool, connected: Bool, host: String, loc: Loc) -> String {
        loc.t(connected ? "explore.connectedTag" : "home.connEmptyTitle")
    }

    // MARK: - Marks

    static func currentSite(_ engine: BrowserEngine) -> SiteModel {
        var model = site(
            host: engine.host,
            name: engine.title.isEmpty ? engine.host : engine.title,
            origin: engine.origin
        )
        model.subtitle = engine.host
        // The icon this page named, when it named one (spec 079).
        model.iconUrls = iconUrls(recorded: engine.favicon, origin: engine.origin)
        return model
    }

    static func site(from pinned: ExploreSiteWire) -> SiteModel {
        site(host: pinned.host, name: pinned.name.isEmpty ? pinned.host : pinned.name,
             origin: pinned.origin)
    }

    /// Spec 100: the add-network sheet a page opened, in Settings' own words
    /// wherever they fit. The core decided everything drawn here — which
    /// chain, whose name and coin, the verdict, whether Add acts; this picks
    /// the line.
    static func addNetwork(_ view: NetDappAddViewWire, loc: Loc) -> AddNetworkSheetModel {
        let chain = String(view.chainId)
        var rows: [(String, String)] = [
            (loc.t("addToken.labelName"), view.name.isEmpty ? loc.t("addToken.chainId", vars: ["chainId": chain]) : view.name),
            (loc.t("addToken.labelChainId"), chain),
        ]
        if !view.nativeSymbol.isEmpty { rows.append((loc.t("addToken.labelNativeToken"), view.nativeSymbol)) }
        if let host = view.rpcHost { rows.append((loc.t("addToken.labelRpcUrl"), host)) }
        if let host = view.explorerHost { rows.append((loc.t("addToken.labelExplorer"), host)) }
        let verdict = view.phase == .ready || view.phase == .notCompatible
        var checks: [CheckItemModel] = []
        if verdict, let compat = view.compat, compat.rpcFailure == nil {
            checks = compat.contracts.map { CheckItemModel(label: $0.name, ok: $0.deployed) }
                + [CheckItemModel(label: loc.t("settingsModals.addNetwork.checkSigner"), ok: compat.p256Available == true)]
        }
        let pill: StatusPillModel?
        let note: String?
        switch view.phase {
        case .checking:
            pill = StatusPillModel(tone: .neutral, label: loc.t("settingsModals.addNetwork.checkingCompatibility"))
            note = nil
        case .ready:
            pill = StatusPillModel(tone: .ok, label: loc.t("settingsModals.addNetwork.compatible"))
            note = (view.compat?.multiKeyReady == false) ? loc.t("settingsModals.addNetwork.singleKeyOnly") : nil
        case .notCompatible:
            pill = StatusPillModel(tone: .error, label: loc.t("settingsModals.addNetwork.incompatible"))
            note = loc.t("settingsModals.addNetwork.incompatibleHint")
        case .checkFailed:
            pill = StatusPillModel(tone: .warn, label: loc.t("settingsModals.addNetwork.unableToVerify"))
            note = nil
        case .wrongRpc:
            pill = nil
            note = loc.t("assets.rpcFixWrongChain", vars: [
                "actual": view.reportedChainId.map(String.init) ?? "", "expected": chain,
            ])
        case .noRpc:
            pill = nil
            note = loc.t("componentsUi.browserStatus.reason.badRpc")
        }
        let decided = view.phase == .notCompatible || view.phase == .wrongRpc || view.phase == .noRpc
        return AddNetworkSheetModel(
            id: "\(view.tab)/\(view.id)",
            title: loc.t("settingsModals.addNetwork.modalTitle"),
            lead: loc.t("connect.browser.addLead", vars: ["host": view.host]),
            site: site(host: view.host, name: view.host, origin: view.origin),
            rows: rows.map { AddNetworkSheetModel.Row(label: $0.0, value: $0.1) },
            fromSite: view.fromSite ? loc.t("connect.browser.addFromSite") : nil,
            pill: pill,
            checksTitle: checks.isEmpty ? nil : loc.t("settingsModals.addNetwork.compatibilityCheck"),
            checks: checks,
            note: note,
            add: view.canAdd ? loc.t("settingsModals.addNetwork.addNetworkBtn") : nil,
            retry: view.phase == .checkFailed ? loc.t("settingsModals.addNetwork.retry") : nil,
            setupTool: view.phase == .notCompatible ? loc.t("settingsModals.addNetwork.openChainSetupTool") : nil,
            dismiss: decided ? loc.t("common.done") : loc.t("connect.browser.cancel")
        )
    }

    /// One site's mark: its own icon (spec 079), and its letter until then.
    ///
    /// The letter is the core's rule (`browserSiteLetter`): the first letter
    /// or digit after a leading `www.`, `app.` or `m.` — every site under one
    /// domain would otherwise be a W, and `app.uniswap.org` an A. The tint
    /// keeps its own rule (`www.` dropped) so a known brand keeps its colour.
    static func site(host: String, name: String, origin: String) -> SiteModel {
        let bare = host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
        return SiteModel(
            id: origin.isEmpty ? host : origin,
            name: name.isEmpty ? host : name,
            host: host,
            letter: browserSiteLetter(host: host),
            tint: tint(for: bare),
            iconUrls: SigningLive.siteIconUrls(origin: origin)
        )
    }

    /// Where a site's icon is, best first: the one the page named when it
    /// was visited (https only), then the usual https places. Nothing over
    /// plain http, where anybody on the path could answer with a brand.
    static func iconUrls(recorded: String?, origin: String) -> [String] {
        let named = recorded.flatMap { $0.hasPrefix("https://") ? $0 : nil }
        var urls: [String] = named.map { [$0] } ?? []
        for url in SigningLive.siteIconUrls(origin: origin) where !urls.contains(url) {
            urls.append(url)
        }
        return urls
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
