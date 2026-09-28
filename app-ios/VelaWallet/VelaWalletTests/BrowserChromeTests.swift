//
//  BrowserChromeTests.swift
//  VelaWalletTests
//
//  Spec 079 US5: the browser's chrome is honest and complete — a lock with no
//  words, a consent that says what it asks and connects, network rows with
//  their logo and what the account holds there. Hermetic.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
import WebKit
@testable import VelaWallet

@MainActor
struct BrowserChromeTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func dbr(consent: Bool, secure: Bool = true) -> DbrViewWire {
        DbrViewWire(
            ready: true,
            consent: consent
                ? DbrConsentViewWire(tab: "t1", origin: "https://app.uniswap.org",
                                     methods: ["eth_requestAccounts"], address: me, chainId: 100)
                : nil,
            tabs: [DbrTabViewWire(tab: "t1", origin: "https://app.uniswap.org",
                                  connectedAddress: consent ? nil : me, chainId: 100,
                                  secure: secure, crashed: false)],
            sites: [], signing: nil, queuedSigning: 0
        )
    }

    private func connection(consent: Bool, holdings: [Int: String] = [:]) -> ConnectionModel {
        let view = dbr(consent: consent)
        return ExploreLive.connectionModel(
            dbr: view, tab: view.tabs.first, engine: nil, identity: (name: "Me", address: me),
            chainIds: [1, 100, 8453], holdings: holdings, loc: loc
        )
    }

    /// The consent names what it asks — "连接到 {host}" — and its approve word
    /// is every client's "连接", no longer "批准".
    @Test func theConsentSaysWhatItAsksAndConnects() {
        let asking = connection(consent: true)
        #expect(asking.title == loc.t("connect.browser.title", vars: ["host": "app.uniswap.org"]))
        #expect(asking.title.contains("app.uniswap.org"))
        #expect(asking.consent?.approve == loc.t("connect.browser.connect"))
        #expect(asking.consent?.approve != loc.t("connect.dapp.approve"))
        #expect(asking.statusLine.isEmpty, "no safety words while it asks")

        let connected = connection(consent: false)
        #expect(connected.consent == nil)
        #expect(connected.statusLine == loc.t("explore.connectedTag"), "已连接 is a fact, and it stays")
    }

    /// Every network row leads with its logo and shows what the account holds
    /// there, from the home screen's figures — nothing where it is not known.
    @Test func networkRowsCarryTheirLogoAndTheAccountsBalance() {
        let model = connection(consent: false, holdings: [100: "¥11.91", 8453: "¥2.78"])
        #expect(model.networkLogoUrl == Marks.chainLogoURL(100))
        #expect(model.networks.map(\.id) == [1, 100, 8453])
        #expect(model.networks.allSatisfy { $0.logoUrl == Marks.chainLogoURL($0.id) })
        #expect(model.networks.first { $0.id == 100 }?.amount == "¥11.91")
        #expect(model.networks.first { $0.id == 1 }?.amount == nil, "unknown is blank, never ¥0.00")
    }

    private func token(_ chain: Int, _ balance: String, price: Double?, spam: Bool = false) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chain, symbol: "X", name: "X", balance: balance, decimals: 18,
                         tokenAddress: nil, priceUsd: price, spam: spam)
    }

    private func balance(_ tokens: [BalanceTokenWire], failed: [Int] = [], hidden: Bool = false) -> BalanceViewWire {
        BalanceViewWire(
            address: me, displayTotalUsd: 1, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: hidden, refreshing: false, lastRefreshedAtMs: nil, tokens: tokens,
            unpricedTokens: [], failedChainIds: failed, rateLimitedChainIds: [], bannerChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    /// The holdings are the home screen's own figures, per network, in the
    /// display currency — and a failed chain, spam, an unpriced token, dust
    /// or a hidden balance says nothing rather than a zero.
    @Test func theHoldingsAreTheHomeScreensFiguresOrNothing() {
        let display = WalletLive.Display(code: "USD", rate: 1)
        let figures = WalletLive.networkHoldings(balance([
            token(100, "2", price: 1),
            token(100, "1.5", price: 2),
            token(100, "9999", price: 1, spam: true),
            token(1, "1", price: nil),
            token(8453, "0.001", price: 1),
            token(42161, "5", price: 1),
        ], failed: [42161]), display: display)
        #expect(figures[100]?.hasSuffix(Formats.number(5, minimumFractionDigits: 2, maximumFractionDigits: 2)) == true)
        #expect(figures[1] == nil, "unpriced")
        #expect(figures[8453] == nil, "under half a cent")
        #expect(figures[42161] == nil, "a chain whose read failed")
        #expect(WalletLive.networkHoldings(balance([token(100, "2", price: 1)], hidden: true), display: display).isEmpty,
                "a hidden balance stays hidden here too")
        #expect(WalletLive.networkHoldings(nil, display: display).isEmpty)
    }

    /// Spec 079 US6: a tab's card is the page as it was last seen; a start
    /// page keeps its drawing; a tab never photographed keeps the stand-in.
    @Test func eachTabsCardIsItsOwnPage() {
        let explore = ExploreViewWire(
            favorites: [], groups: [],
            tabs: [
                ExploreTabWire(id: "a", url: "https://app.uniswap.org/", title: "Uniswap", host: "app.uniswap.org"),
                ExploreTabWire(id: "b", url: "https://bscscan.com/", title: "BscScan", host: "bscscan.com"),
                ExploreTabWire(id: "c", url: nil, title: "", host: ""),
            ],
            selectedTab: "a", favoritesHidden: false, recentHidden: false,
            favoritesFull: false, tabsFull: false, ready: true
        )
        let shot = UIImage()
        let tabs = ExploreLive.tabs(explore: explore, snapshot: { $0 == "a" || $0 == "c" ? shot : nil }, loc: loc)
        #expect(tabs[0].snapshot === shot)
        #expect(tabs[1].snapshot == nil, "not photographed yet: the stand-in")
        #expect(tabs[2].snapshot == nil, "a start page keeps its drawing")
        #expect(tabs[0].site?.letter == "U")
    }

    /// Spec 079 US4: the page's chain cannot be reached → one line naming
    /// it; merely rate limited → nothing (a 429 is transient); answering
    /// again → gone, because the pool drops it from the failed set.
    @Test func theChainNoticeShowsForAFailedChainOnly() {
        // Spec 082 RF1: a chain one call's first pass could not reach at all
        // is named while the dApp still waits — not after three passes.
        let early = ExploreLive.chainNotice(chainId: 100, failed: [], unreached: [100], rateLimited: [], loc: loc)
        #expect(early == loc.t("explore.chainDown", vars: ["chain": "Gnosis"]))
        #expect(ExploreLive.chainNotice(chainId: 100, failed: [], unreached: [100], rateLimited: [100], loc: loc) == nil,
                "unreached but rate limited: still quiet")
        #expect(ExploreLive.chainNotice(chainId: 100, failed: [], unreached: [1], rateLimited: [], loc: loc) == nil)
        let down = ExploreLive.chainNotice(chainId: 100, failed: [100], rateLimited: [], loc: loc)
        #expect(down == loc.t("explore.chainDown", vars: ["chain": "Gnosis"]))
        #expect(down?.contains("Gnosis") == true)
        #expect(ExploreLive.chainNotice(chainId: 100, failed: [100], rateLimited: [100], loc: loc) == nil,
                "rate limiting stays quiet")
        #expect(ExploreLive.chainNotice(chainId: 100, failed: [1], rateLimited: [], loc: loc) == nil,
                "another chain's trouble is not this page's")
        #expect(ExploreLive.chainNotice(chainId: 100, failed: [], rateLimited: [], loc: loc) == nil,
                "answered again: the notice is gone")
        #expect(ExploreLive.chainNotice(chainId: nil, failed: [100], rateLimited: [], loc: loc) == nil)
    }

    /// The http lock is its own glyph — lucide `lock-open` — not the warning
    /// triangle, and the site menu carries no status words at all.
    @Test func anHttpPageGetsAnOpenLockAndNoWords() {
        #expect(LucideGlyph.lockOpen.svg.contains("9.9-1"), "lucide lock-open's shackle")
        #expect(LucideGlyph.lockOpen.svg != LucideGlyph.lock.svg)
        let model = ExploreLive.home(
            explore: .empty, history: .empty, dbr: dbr(consent: false, secure: false), engine: nil,
            identity: (name: "Me", address: me), loc: loc
        )
        if case .siteMenu(_, let statusLine, _) = model.menus.siteMenu {
            #expect(statusLine.isEmpty)
        } else {
            Issue.record("the site menu is a site menu")
        }
    }

    // MARK: - Spec 082

    private func engine() -> BrowserEngine {
        let engine = BrowserEngine(id: "tab-\(UUID().uuidString)")
        engine.loader = { _ in }
        engine.stopper = {}
        return engine
    }

    /// RE1 (G28): the bar names the page on screen. A typed address over a
    /// live page keeps the live page until the new one commits; the lock is
    /// the committed page's.
    @Test func aTypedAddressKeepsThePageOnScreenUntilItCommits() {
        let engine = engine()
        defer { engine.tearDown() }
        engine.reportedURL = { URL(string: "https://jumper.exchange/") }
        engine.committed()
        #expect(engine.bar.host == "jumper.exchange")
        #expect(engine.bar.lock == "closed")

        engine.load("https://app.uniswap.org/")
        engine.provisionalStarted(attempt: "https://app.uniswap.org/")
        // WebKit's own `url` is already the provisional one — the bar is not.
        engine.reportedURL = { URL(string: "https://app.uniswap.org/") }
        engine.metaChanged()
        #expect(engine.bar.host == "jumper.exchange", "the bar never names a page that has not arrived")
        #expect(engine.bar.lock == "closed")
        #expect(engine.host == "jumper.exchange")
        #expect(engine.loading, "the hairline says a load is on its way")

        engine.committed()
        #expect(engine.bar.host == "app.uniswap.org")
        #expect(engine.bar.url == "https://app.uniswap.org/")
    }

    /// A page's own `location.href` to a host that never answers never
    /// renames the bar over the live page — the spoofing shape (E-G28b);
    /// when it is given up the panel names the failed host with NO lock.
    @Test func aPagesOwnNavigationNeverRenamesTheBarAndAFailureHasNoLock() {
        let engine = engine()
        defer { engine.tearDown() }
        engine.reportedURL = { URL(string: "http://192.168.50.9:8137/") }
        engine.committed()
        #expect(engine.bar.host == "192.168.50.9:8137")

        // The page navigates itself (what `decidePolicyFor` reports).
        engine.requested("https://app.uniswap.org/")
        engine.provisionalStarted(attempt: "https://app.uniswap.org/")
        engine.reportedURL = { URL(string: "https://app.uniswap.org/") }
        engine.metaChanged()
        #expect(engine.bar.host == "192.168.50.9:8137")
        #expect(engine.bar.lock != "none", "the live page keeps its own lock")

        // Twenty seconds, no commit, WebKit never came alive: given up.
        engine.reportedProgress = { 0.1 }
        engine.watchdogFired(elapsedMs: browserLoadGiveUpMs())
        #expect(engine.failure != nil)
        #expect(engine.bar.host == "app.uniswap.org", "the panel names where it was going")
        #expect(engine.bar.lock == "none", "nothing from that host is on screen: no lock")
    }

    /// A fresh tab has nothing committed: the pending host, and no lock.
    @Test func aFreshTabNamesItsPendingLoadWithNoLock() {
        let engine = engine()
        defer { engine.tearDown() }
        engine.load("https://app.uniswap.org/swap")
        #expect(engine.bar.host == "app.uniswap.org")
        #expect(engine.bar.lock == "none")
        #expect(engine.bar.url == "https://app.uniswap.org/swap")
    }

    /// A single-page app moving itself (same origin, nothing pending) moves
    /// the committed address with it.
    @Test func aSinglePageAppsOwnMoveFollows() {
        let engine = engine()
        defer { engine.tearDown() }
        engine.reportedURL = { URL(string: "https://app.uniswap.org/swap") }
        engine.committed()
        engine.reportedURL = { URL(string: "https://app.uniswap.org/pool") }
        engine.metaChanged()
        #expect(engine.bar.url == "https://app.uniswap.org/pool")
    }

    /// A navigation that never leaves the document — a fragment, or Back /
    /// Forward across a single-page app's own history entries — gets no
    /// commit and no finish from WebKit, only the new URL while nothing
    /// loads. It is the page moving: the load it started ends there, or the
    /// Stop row, the watchdog and the pending page stay up for good and the
    /// bar stops following the page (082 review). Real WebKit, no network.
    @Test func aSameDocumentArrivalEndsItsLoad() async throws {
        let engine = BrowserEngine(id: "tab-\(UUID().uuidString)")
        defer { engine.tearDown() }
        engine.webView.loadHTMLString("<html><body>page</body></html>", baseURL: URL(string: "https://app.test/")!)
        // Generous: with the whole suite running, WebKit's content process
        // can take many seconds to come up. No network is involved.
        let loaded = Date().addingTimeInterval(30)
        while Date() < loaded, engine.committedURL == nil || engine.loading {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
        try #require(engine.committedURL == "https://app.test/", "the local page never committed")

        engine.load("https://app.test/#pool")
        let moved = Date().addingTimeInterval(15)
        while Date() < moved, engine.loading { try? await Task.sleep(nanoseconds: 20_000_000) }
        #expect(!engine.loading, "a fragment is not a load that never ends")
        #expect(engine.pendingURL == nil)
        #expect(!engine.watchdogArmed)
        #expect(engine.bar.url == "https://app.test/#pool")
        #expect(!ExploreLive.siteMenuItems(
            bookmarked: false, connected: false, loading: engine.loading, loc: loc
        ).contains { $0.id == "stop" })
    }

    /// Back across a single-page app's own entry (`goBack` names the entry it
    /// goes to as pending): WebKit only moves the URL, nothing loads — the
    /// back ends there. The same move while WebKit IS loading a new document
    /// is a load in flight and waits for its commit, and another origin is
    /// never taken for an arrival.
    @Test func backWithinTheDocumentArrivesAndALoadStillWaitsForItsCommit() {
        let engine = engine()
        defer { engine.tearDown() }
        var webKitLoading = false
        engine.reportedLoading = { webKitLoading }
        engine.reportedURL = { URL(string: "https://app.uniswap.org/pool") }
        engine.committed()

        // Back to /swap, a pushState entry: no commit will come.
        engine.requested("https://app.uniswap.org/swap")
        #expect(engine.loading)
        engine.reportedURL = { URL(string: "https://app.uniswap.org/swap") }
        engine.metaChanged()
        #expect(!engine.loading)
        #expect(engine.pendingURL == nil)
        #expect(!engine.watchdogArmed)
        #expect(engine.bar.url == "https://app.uniswap.org/swap")

        // A same-origin load of a new document: its URL shows while WebKit
        // loads — still pending, the bar on the page on screen.
        engine.requested("https://app.uniswap.org/explore")
        webKitLoading = true
        engine.reportedURL = { URL(string: "https://app.uniswap.org/explore") }
        engine.metaChanged()
        #expect(engine.loading)
        #expect(engine.bar.url == "https://app.uniswap.org/swap")
        engine.committed()
        webKitLoading = false
        #expect(engine.bar.url == "https://app.uniswap.org/explore")

        // Another origin is never an arrival, loading or not.
        engine.requested("https://evil.example/")
        engine.reportedURL = { URL(string: "https://evil.example/") }
        engine.metaChanged()
        #expect(engine.bar.host == "app.uniswap.org")
        #expect(engine.loading)
    }

    /// RE5: while a page loads, the site menu's refresh row is Stop.
    @Test func theRefreshRowIsStopWhileLoading() {
        let loading = ExploreLive.siteMenuItems(bookmarked: false, connected: false, loading: true, loc: loc)
        #expect(loading.contains { $0.id == "stop" && $0.label == loc.t("connect.dapp.stop") })
        #expect(!loading.contains { $0.id == "refresh" })
        let idle = ExploreLive.siteMenuItems(bookmarked: false, connected: false, loc: loc)
        #expect(idle.contains { $0.id == "refresh" })
        #expect(!idle.contains { $0.id == "stop" })
        #expect(loc.t("connect.dapp.stop") != "connect.dapp.stop")
    }

    /// RE6 (G9, G10): a site asking is named once — in the header's "连接到
    /// {host}" — and one sentence, `connect.browser.body`, stands above the
    /// answers with nothing under them.
    @Test func theConsentNamesTheSiteOnce() {
        let asking = connection(consent: true)
        let headline = ConnectionPanelView.headline(asking)
        #expect(headline == loc.t("connect.browser.title", vars: ["host": "app.uniswap.org"]))
        #expect(headline.components(separatedBy: "app.uniswap.org").count == 2, "the host, once")
        #expect(asking.explainer == loc.t("connect.browser.body"))
        #expect(asking.footnote.isEmpty, "no second explainer under the buttons")
        #expect(!asking.explainer.contains("app.uniswap.org"))

        // The connected panel is unchanged.
        let connected = connection(consent: false)
        #expect(ConnectionPanelView.headline(connected) == "app.uniswap.org")
        #expect(connected.explainer == loc.t("explore.connectionExplainer"))
        #expect(connected.footnote == loc.t("explore.autoRequestHint"))
    }

    /// RE7 (G8): a Recents row whose title is its host names it once; a
    /// real title keeps the host under it.
    @Test func aRecentWhoseTitleIsItsHostIsNamedOnce() {
        var same = ExploreLive.site(host: "192.168.50.9:8137", name: "192.168.50.9:8137", origin: "http://192.168.50.9:8137")
        same.subtitle = "192.168.50.9:8137"
        let once = SiteRowView.lines(same)
        #expect(once.name == "192.168.50.9:8137")
        #expect(once.second == nil)

        var titled = ExploreLive.site(host: "app.uniswap.org", name: "Uniswap", origin: "https://app.uniswap.org")
        titled.subtitle = "app.uniswap.org"
        let two = SiteRowView.lines(titled)
        #expect(two.name == "Uniswap")
        #expect(two.second == "app.uniswap.org")

        // Case alone does not make a second line.
        let cased = SiteRowView.lines(ExploreLive.site(host: "bscscan.com", name: "BSCSCAN.COM", origin: "https://bscscan.com"))
        #expect(cased.second == nil)
    }

    /// RE12 (G25): every card has one skeleton — a tab never shown since the
    /// launch draws its mark and host, not fake page bars — and the grid is
    /// top-aligned.
    @Test func everyTabCardHasOneSkeleton() {
        let site = ExploreLive.site(host: "app.uniswap.org", name: "Uniswap", origin: "https://app.uniswap.org")
        let dormant = TabModel(id: "a", title: "Uniswap", site: site, selected: false, startPage: false)
        #expect(TabPreview.of(dormant) == .site(host: "app.uniswap.org"))
        var shot = dormant
        shot.snapshot = UIImage()
        #expect(TabPreview.of(shot) == .snapshot)
        let start = TabModel(id: "s", title: "", site: nil, selected: false, startPage: true)
        #expect(TabPreview.of(start) == .startPage)
        #expect(ExploreTabsScreen.columns.allSatisfy { $0.alignment == .top })
    }
}
