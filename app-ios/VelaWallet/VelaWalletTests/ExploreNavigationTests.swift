//
//  ExploreNavigationTests.swift
//  VelaWalletTests
//
//  DESIGN N on iOS: where Explore lands and where an opened site goes are
//  the core's rules (`browser_tabs.rs` — `exploreLanding`,
//  `browserOpenTarget`, `browserLitTab`, `browserWaitingTab`). These tests
//  check the shell asks them the right question — the strip it hands over,
//  the entry, the waiting tab, the kind of open — and does what they answer:
//  the REAL core through the REAL controller, with pages on a port that
//  refuses at once (nothing here is about a page loading).
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ExploreNavigationTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func tab(_ id: String, _ url: String?) -> ExploreTabWire {
        ExploreTabWire(id: id, url: url, title: id, host: url.flatMap { URL(string: $0)?.host } ?? "")
    }

    /// A strip: the dApp `a`, the dApp `b`, the start page `s`.
    private func strip(selected: String?, recent: [String] = []) -> ExploreViewWire {
        var view = ExploreViewWire(
            favorites: [],
            tabs: [tab("a", "https://app.uniswap.org/swap"), tab("b", "https://polymarket.com/"), tab("s", nil)],
            selectedTab: selected, favoritesHidden: false, recentHidden: false,
            favoritesFull: false, tabsFull: false, ready: true
        )
        view.recentTabs = recent
        return view
    }

    // MARK: - Where Explore lands

    /// Entering Explore — from another section, or 探索 again while a page is
    /// up — is the home, even with a live dApp in front. A page opened from
    /// outside is that page; the home until its tab has one.
    @Test func enteringLandsOnTheHomeAndAnOpenedPageOnItself() {
        let onDapp = strip(selected: "a")
        #expect(BrowserController.landing(view: onDapp, entry: .section, waiting: nil) == .home)
        #expect(BrowserController.landing(view: onDapp, entry: .reselect, waiting: nil) == .home)
        #expect(BrowserController.landing(view: onDapp, entry: .pageOpened, waiting: nil) == .tab("a"))

        let onStart = strip(selected: "s")
        #expect(BrowserController.landing(view: onStart, entry: .pageOpened, waiting: nil) == .home,
                "the open has not reached the view yet: nothing to land on")
    }

    /// A tab whose request waits on the person wins over every entry — the
    /// sheet's page comes back with it. A tab the strip does not hold, or one
    /// with no page, is no reason to leave the home.
    @Test func aWaitingRequestBringsItsPage() {
        let view = strip(selected: "s")
        #expect(BrowserController.landing(view: view, entry: .section, waiting: "b") == .tab("b"))
        #expect(BrowserController.landing(view: view, entry: .reselect, waiting: "b") == .tab("b"))
        #expect(BrowserController.landing(view: view, entry: .section, waiting: "gone") == .home)
        #expect(BrowserController.landing(view: view, entry: .section, waiting: "s") == .home)
    }

    /// What waits, in the core's order: the consent, then the signature,
    /// then the add-network sheet.
    @Test func theWaitingTabIsTheCoresOrder() {
        func dbr(consent: String? = nil, signing: String? = nil, adding: String? = nil) -> DbrViewWire {
            var view = DbrViewWire(
                ready: true,
                consent: consent.map {
                    DbrConsentViewWire(tab: $0, origin: "https://dapp.example", methods: ["eth_requestAccounts"],
                                       address: nil, chainId: 1)
                },
                tabs: [], sites: [],
                signing: signing.map { DbrSigningViewWire(tab: $0, id: "1") },
                queuedSigning: 0
            )
            view.addingNetwork = adding.map { DbrSigningViewWire(tab: $0, id: "2") }
            return view
        }
        #expect(BrowserController.waitingTab(.empty) == nil)
        #expect(BrowserController.waitingTab(dbr(adding: "c")) == "c")
        #expect(BrowserController.waitingTab(dbr(signing: "b", adding: "c")) == "b")
        #expect(BrowserController.waitingTab(dbr(consent: "a", signing: "b", adding: "c")) == "a")
    }

    // MARK: - Where an opened site goes

    /// From the home, an address never replaces the live dApp in front: it
    /// goes into a selected start page, else a new tab. The page's own bar
    /// loads in that page.
    @Test func anOpenFromTheHomeNeverReplacesALiveDapp() {
        let url = "https://app.aave.com/"
        #expect(BrowserController.openTarget(view: strip(selected: "a"), shown: nil, onPage: false,
                                             url: url, kind: .address) == .newTab)
        #expect(BrowserController.openTarget(view: strip(selected: "s"), shown: nil, onPage: false,
                                             url: url, kind: .address) == .load("s"))
        #expect(BrowserController.openTarget(view: strip(selected: "a"), shown: "a", onPage: true,
                                             url: url, kind: .address) == .load("a"))
    }

    /// A picked site a tab is already on comes back as it was left; a typed
    /// address on the same site names a page, and never switches.
    @Test func aPickedSiteResumesTheTabAlreadyOnIt() {
        let view = strip(selected: "a")
        #expect(BrowserController.openTarget(view: view, shown: nil, onPage: false,
                                             url: "https://polymarket.com/markets", kind: .site) == .resume("b"))
        #expect(BrowserController.openTarget(view: view, shown: nil, onPage: false,
                                             url: "https://polymarket.com/markets", kind: .address) == .newTab)
    }

    /// "This tab" in the switcher: the page's own tab while it shows; over the
    /// home only a selected start page — a dApp left for the wallet is unlit.
    @Test func theLitTabIsThePageOrAStartPage() {
        #expect(BrowserController.litTab(view: strip(selected: "a"), shown: "a", onPage: true) == "a")
        #expect(BrowserController.litTab(view: strip(selected: "a"), shown: nil, onPage: false) == nil)
        #expect(BrowserController.litTab(view: strip(selected: "s"), shown: nil, onPage: false) == "s")
    }

    /// The strip the rules read is the view's own: tabs (a start page's url
    /// null), the one in front, the recency.
    @Test func theStripCarriesWhatTheRulesRead() throws {
        let json = try CoreJSON.object(strip(selected: "b", recent: ["b", "a"]).stripJSON)
        #expect(json["selected_tab"] as? String == "b")
        #expect(json["recent_tabs"] as? [String] == ["b", "a"])
        let tabs = try #require(json["tabs"] as? [[String: Any]])
        #expect(tabs.map { $0["id"] as? String } == ["a", "b", "s"])
        #expect(tabs[2]["url"] is NSNull)
        #expect(try CoreJSON.object(ExploreViewWire.empty.stripJSON)["selected_tab"] is NSNull)
    }

    // MARK: - The controller does what the core answers

    /// End to end through the controller and the real explore machine:
    /// an address from the home opens a NEW tab beside a live dApp, which
    /// keeps its page; a picked site already open comes back with no new tab
    /// and no navigation; the page's own bar loads in place.
    ///
    /// Each tab is a real WKWebView (see DebugModeTests): the limit only stops
    /// a hang.
    @Test(.timeLimit(.minutes(5)))
    func theControllerOpensWhereTheCoreSays() async throws {
        let h = BrowserHarness()
        h.browser.start()
        await Wait.until { h.browser.explore.ready }

        // Port 9 refuses at once.
        h.browser.open("http://127.0.0.1:9/one")
        await Wait.until { h.browser.explore.selected?.url == "http://127.0.0.1:9/one" }
        let first = try #require(h.browser.explore.selectedTab)
        let pages = { h.browser.explore.tabs.filter { $0.url != nil } }

        // From the home: a new tab — the dApp in front is not replaced.
        h.browser.open("http://localhost:9/two")
        await Wait.until { pages().count == 2 && h.browser.explore.selectedTab != first }
        #expect(h.browser.explore.tabs.first { $0.id == first }?.url == "http://127.0.0.1:9/one",
                "the dApp left in front keeps its page")
        let second = try #require(h.browser.explore.selectedTab)

        // A picked site a tab is already on: that tab, as it was left.
        h.browser.open("http://127.0.0.1:9/elsewhere", kind: .site)
        await Wait.until { h.browser.explore.selectedTab == first }
        #expect(pages().count == 2, "resumed, not opened again")
        #expect(h.browser.explore.tabs.first { $0.id == first }?.url == "http://127.0.0.1:9/one",
                "resuming loads nothing over the page")

        // The same site typed from the home names a page: a new tab.
        h.browser.open("http://127.0.0.1:9/typed")
        await Wait.until { pages().count == 3 }
        #expect(h.browser.explore.tabs.first { $0.id == first }?.url == "http://127.0.0.1:9/one")
        #expect(h.browser.explore.tabs.first { $0.id == second }?.url == "http://localhost:9/two")

        // The page's own address bar: in that page.
        let third = try #require(h.browser.explore.selectedTab)
        h.browser.open("http://127.0.0.1:9/next", onPage: true)
        await Wait.until { h.browser.explore.selected?.url == "http://127.0.0.1:9/next" }
        #expect(h.browser.explore.selectedTab == third)
        #expect(pages().count == 3)

        // Entering Explore again is the home; a page opened from outside is it.
        #expect(h.browser.landing(.section) == .home)
        #expect(h.browser.landing(.reselect) == .home)
        #expect(h.browser.landing(.pageOpened) == .tab(third))
        // From the home nothing is lit; on the page, its own tab.
        #expect(h.browser.litTab(onPage: false) == nil)
        #expect(h.browser.litTab(onPage: true) == third)
    }

    /// A new (start-page) tab takes the next address from the home: no third
    /// tab, and the dApp beside it untouched.
    @Test(.timeLimit(.minutes(5)))
    func aStartPageTakesTheNextOpen() async throws {
        let h = BrowserHarness()
        h.browser.start()
        await Wait.until { h.browser.explore.ready }
        h.browser.open("http://127.0.0.1:9/one")
        await Wait.until { h.browser.explore.selected?.url == "http://127.0.0.1:9/one" }
        let dapp = try #require(h.browser.explore.selectedTab)

        h.browser.newTab()
        await Wait.until { h.browser.explore.tabs.count == 2 && h.browser.explore.selected?.url == nil }
        let start = try #require(h.browser.explore.selectedTab)
        h.browser.open("http://localhost:9/two")
        await Wait.until { h.browser.explore.tabs.first { $0.id == start }?.url == "http://localhost:9/two" }
        #expect(h.browser.explore.tabs.count == 2)
        #expect(h.browser.explore.tabs.first { $0.id == dapp }?.url == "http://127.0.0.1:9/one")
    }

    /// A full strip takes no new tab (the core drops `tab_opened` at its
    /// cap), so the core's open target answers the selected tab: an open
    /// from the home loads there, as the desktop does, rather than doing
    /// nothing at all. The shell has no rule of its own for it.
    @Test func aFullStripIsAnsweredWithTheSelectedTab() {
        func strip(tabs count: Int) -> ExploreViewWire {
            var view = ExploreViewWire(
                favorites: [], tabs: (1...count).map { tab("t\($0)", "https://site\($0).example/") },
                selectedTab: "t3", favoritesHidden: false, recentHidden: false,
                favoritesFull: false, tabsFull: count >= 24, ready: true
            )
            view.recentTabs = ["t3"]
            return view
        }
        let full = strip(tabs: 24)
        for kind in [ExploreOpenKind.address, .site] {
            #expect(BrowserController.openTarget(view: full, shown: nil, onPage: false,
                                                 url: "https://late.example/", kind: kind) == .load("t3"))
        }
        // A picked site already in the strip still comes back as it was left.
        #expect(BrowserController.openTarget(view: full, shown: nil, onPage: false,
                                             url: "https://site7.example/", kind: .site) == .resume("t7"))
        // One short of the cap: a new tab, as ever.
        #expect(BrowserController.openTarget(view: strip(tabs: 23), shown: nil, onPage: false,
                                             url: "https://late.example/", kind: .address) == .newTab)
    }

    /// The same, end to end through the controller and the real explore
    /// machine: the open lands in the selected tab of a full strip.
    @Test(.timeLimit(.minutes(5)))
    func aFullStripStillOpens() async throws {
        let tabs: [[String: Any]] = (1...24).map { n in
            ["id": "t\(n)", "url": "https://site\(n).example/", "title": "t\(n)", "host": "site\(n).example"]
        }
        let h = BrowserHarness(seed: { store in
            store.writeObject(ExploreExecutor.key, ["tabs": tabs, "selected_tab": "t3"])
        })
        h.browser.start()
        await Wait.until { h.browser.explore.ready }
        #expect(h.browser.explore.tabsFull)

        h.browser.open("http://127.0.0.1:9/full")
        await Wait.until { h.browser.explore.tabs.first { $0.id == "t3" }?.url == "http://127.0.0.1:9/full" }
        #expect(h.browser.explore.tabs.count == 24)
        #expect(h.browser.explore.selectedTab == "t3")
    }

    // MARK: - The home's resume section

    /// The rows are the core's `resumable`, in its order; the header counts
    /// every tab (start pages too) and its action is the switcher. Nothing
    /// before the mirror is live, nothing with no page open.
    @Test func theResumeSectionIsTheCoresRows() throws {
        var view = strip(selected: "s", recent: ["b", "a", "s"])
        view.resumable = [view.tabs[1], view.tabs[0]]
        let section = try #require(ExploreLive.resume(explore: view, history: .empty, loc: loc))
        #expect(section.title == "3 tabs open")
        #expect(section.title == loc.t("explore.openTabs", count: 3))
        // A plural: one tab is "1 tab open", never "1 tabs open".
        let one = try #require(ExploreLive.resumeSection(rows: section.tabs, tabCount: 1, loc: loc))
        #expect(one.title == "1 tab open")
        #expect(section.action == loc.t("explore.tabs"))
        #expect(section.tabs.map(\.id) == ["b", "a"], "never re-sorted here")
        #expect(section.tabs.map(\.site.host) == ["polymarket.com", "app.uniswap.org"])
        #expect(section.tabs.allSatisfy { $0.site.subtitle == $0.site.host })

        var notReady = ExploreViewWire(
            favorites: [], tabs: view.tabs, selectedTab: "s", favoritesHidden: false,
            recentHidden: false, favoritesFull: false, tabsFull: false, ready: false
        )
        notReady.resumable = view.resumable
        #expect(ExploreLive.resume(explore: notReady, history: .empty, loc: loc) == nil)

        view.resumable = []
        #expect(ExploreLive.resume(explore: view, history: .empty, loc: loc) == nil)
    }

    /// The section is one pointer in the home's model, which sits at the
    /// screen-model stack budget (`ScreenModelStackTests`).
    @Test func theSectionIsOnePointer() {
        #expect(MemoryLayout<ResumeSectionModel?>.size == MemoryLayout<Int>.size)
    }

    /// The live home carries the section, and no tab count of its own.
    @Test func theLiveHomeCarriesTheSection() {
        var view = strip(selected: "s")
        view.resumable = [view.tabs[0]]
        let home = ExploreLive.home(
            explore: view, history: .empty, dbr: .empty, engine: nil,
            identity: (name: "Me", address: "0x1111111111111111111111111111111111111111"), loc: loc
        )
        #expect(home.resume?.tabs.map(\.id) == ["a"])
        #expect(home.browser.tabCount == 3)
    }

    // MARK: - The bar's words

    @Test func theBarSaysTheCountAndTheConnection() {
        #expect(AddressBarView.countText(1) == "1")
        #expect(AddressBarView.countText(24) == "24")
        #expect(AddressBarView.countText(100) == "99+")
        #expect(AddressBarView.accountName("Account", connected: true, tag: "Connected") == "Account, Connected")
        #expect(AddressBarView.accountName("Account", connected: false, tag: "Connected") == "Account")
    }

    /// A visit is an entry and a serial: the same entry twice is two visits.
    @Test func everyEntryIsANewVisit() {
        var visit = ExploreVisit(entry: .section)
        let before = visit
        visit.enter(.section)
        #expect(visit != before)
        visit.enter(.reselect)
        #expect(visit.entry == .reselect)
        #expect(ExploreEntry.pageOpened.rawValue == "page_opened")
    }
}
