//
//  BrowserMemoryTests.swift
//  VelaWalletTests
//
//  What the browser remembers, and how it is drawn.
//
//  The two store executors and `ExploreLive`. Everything here is hermetic: the
//  machines are real, the store is a real `UserDefaults` suite, and nothing
//  reaches a network.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BrowserMemoryTests {

    private func freshStore(_ name: String) -> VelaStore {
        let suite = "vela.tests.\(name).\(UUID().uuidString)"
        UserDefaults().removePersistentDomain(forName: suite)
        return VelaStore(defaults: UserDefaults(suiteName: suite)!)
    }

    private func loc() -> Loc { Loc(overrideTag: "en", preferredLanguages: []) }

    // MARK: - The two stores

    /// A document that was never written answers **absent**, not empty.
    ///
    /// The core builds its own default from `null` and would deserialise a
    /// document with no fields from `{}`. Two different bugs, one of which
    /// would show somebody a start page that could never be added to.
    @Test func anUnwrittenExploreDocumentIsAbsentRatherThanEmpty() async {
        let executor = ExploreExecutor(store: freshStore("explore"))
        let answer = try! CoreJSON.object(await executor.perform(["type": "read_explore"]))
        #expect(answer["type"] as? String == "loaded")
        #expect(answer["doc"] is NSNull)
    }

    @Test func theExploreDocumentRoundTrips() async {
        let store = freshStore("explore-roundtrip")
        let executor = ExploreExecutor(store: store)
        _ = await executor.perform([
            "type": "write_explore",
            "doc": ["favorites": [["origin": "https://app.uniswap.org"]], "groups": []],
        ])
        let answer = try! CoreJSON.object(await executor.perform(["type": "read_explore"]))
        let doc = answer["doc"] as? [String: Any]
        #expect((doc?["favorites"] as? [[String: Any]])?.count == 1)
    }

    /// Clearing **deletes the key**. "Cleared" and "empty" are different facts
    /// on disk, and the other three clients read this document.
    @Test func clearingHistoryRemovesTheKeyRatherThanWritingAnEmptyList() async {
        let store = freshStore("history")
        let executor = BhistExecutor(store: store)
        _ = await executor.perform(["type": "write_history", "entries": [[
            "origin": "https://app.uniswap.org", "url": "https://app.uniswap.org/swap",
            "host": "app.uniswap.org", "title": "Uniswap", "favicon": "",
            "last_visited_ms": 1_757_000_000_000,
        ]]])
        #expect(store.hasKey(BhistExecutor.key))

        _ = await executor.perform(["type": "remove_history"])
        #expect(!store.hasKey(BhistExecutor.key), "an empty list is not the same as a cleared one")
    }

    /// A corrupt list answers `[]` — the TS `catch { [] }`, ported. Distinct
    /// from `explore`'s `null`, and the core's own doc says so for each.
    @Test func anUnreadableHistoryAnswersAnEmptyListRatherThanFailing() async {
        let store = freshStore("history-corrupt")
        store.writeString(BhistExecutor.key, "{not json")
        let executor = BhistExecutor(store: store)
        let answer = try! CoreJSON.object(await executor.perform(["type": "read_history"]))
        #expect((answer["entries"] as? [[String: Any]])?.isEmpty == true)
    }

    // MARK: - `ExploreLive`

    private func site(_ origin: String, _ host: String, name: String = "") -> ExploreSiteWire {
        ExploreSiteWire(origin: origin, url: origin + "/swap", host: host,
                        name: name.isEmpty ? host : name, renamed: false, addedMs: 0)
    }

    private func view(
        favorites: [ExploreSiteWire] = [],
        groups: [ExploreGroupWire] = [],
        tabs: [ExploreTabWire] = [],
        selected: String? = nil,
        favoritesHidden: Bool = false,
        recentHidden: Bool = false,
        favoritesFull: Bool = false
    ) -> ExploreViewWire {
        ExploreViewWire(
            favorites: favorites, groups: groups, tabs: tabs, selectedTab: selected,
            favoritesHidden: favoritesHidden, recentHidden: recentHidden,
            favoritesFull: favoritesFull, tabsFull: false, ready: true
        )
    }

    private func entry(_ origin: String, _ host: String, title: String) -> BhistEntryWire {
        BhistEntryWire(origin: origin, url: origin + "/", host: host, title: title,
                       favicon: "", lastVisitedMs: 1_757_000_000_000)
    }

    /// Recents come first, and a hidden system group draws nothing while
    /// keeping everything.
    @Test func recentsLeadAndAHiddenSystemGroupDrawsNothing() {
        let history = BhistViewWire(entries: [entry("https://app.aave.com", "app.aave.com", title: "Aave")])

        let shown = ExploreLive.groups(
            explore: view(groups: [ExploreGroupWire(
                id: "trading", name: "Trading", hidden: false,
                sites: [site("https://curve.fi", "curve.fi")]
            )]),
            history: history, loc: loc()
        )
        #expect(shown.map(\.id) == ["recent", "trading"])

        let hidden = ExploreLive.groups(
            explore: view(recentHidden: true), history: history, loc: loc()
        )
        #expect(hidden.isEmpty, "a hidden system group keeps everything and draws nothing")
    }

    /// The grid is full: **no** add affordance rather than one that refuses.
    @Test func aFullGridOffersNoAddTile() {
        let full = ExploreLive.home(
            explore: view(favorites: [site("https://app.aave.com", "app.aave.com")],
                          favoritesFull: true),
            history: .empty, dbr: .empty, engine: nil,
            identity: (name: "Me", address: "0x88cca0eedbf2c4426110bbfc998f048689266894"),
            loc: loc()
        )
        #expect(full.favorites?.tiles.count == 1)
        #expect(!(full.favorites?.tiles.contains { if case .add = $0 { true } else { false } } ?? true))

        let room = ExploreLive.home(
            explore: view(favorites: [site("https://app.aave.com", "app.aave.com")]),
            history: .empty, dbr: .empty, engine: nil,
            identity: (name: "Me", address: "0x88cca0eedbf2c4426110bbfc998f048689266894"),
            loc: loc()
        )
        #expect(room.favorites?.tiles.count == 2)
    }

    /// A site's letter is the core's rule (spec 079): the host's first letter
    /// or digit after a leading `www.`, `app.` or `m.`, upper-cased.
    ///
    /// Without dropping `www.` every site under one domain is a W, which makes
    /// the grid unreadable at exactly the size it is drawn at — and without
    /// dropping `app.` Uniswap was an A (Android pass, F16).
    @Test func aSitesLetterIsTheCoresRule() {
        #expect(ExploreLive.site(host: "www.example.com", name: "", origin: "").letter == "E")
        #expect(ExploreLive.site(host: "app.uniswap.org", name: "", origin: "").letter == "U")
        #expect(ExploreLive.site(host: "m.x.io", name: "", origin: "").letter == "X")
        #expect(ExploreLive.site(host: "127.0.0.1:8137", name: "", origin: "").letter == "1")
        #expect(ExploreLive.site(host: "", name: "", origin: "").letter == "?")
    }

    /// A site's avatar tries its own icon — the one it named when visited,
    /// then the usual https places — and never asks over plain http.
    @Test func aSitesAvatarTriesItsOwnIconOverHttpsOnly() {
        let site = ExploreLive.site(host: "app.uniswap.org", name: "Uniswap", origin: "https://app.uniswap.org")
        #expect(site.iconUrls == ["https://app.uniswap.org/apple-touch-icon.png", "https://app.uniswap.org/favicon.ico"])
        #expect(ExploreLive.site(host: "127.0.0.1:8137", name: "", origin: "http://127.0.0.1:8137").iconUrls.isEmpty)
        let recorded = ExploreLive.iconUrls(recorded: "https://app.uniswap.org/favicon.png", origin: "https://app.uniswap.org")
        #expect(recorded.first == "https://app.uniswap.org/favicon.png", "the recorded icon leads")
        #expect(recorded.count == 3)
        #expect(ExploreLive.iconUrls(recorded: "http://evil.test/i.png", origin: "http://evil.test").isEmpty)
    }

    /// The same host is the same colour on every launch.
    ///
    /// Not `hashValue`: Swift seeds it per process, so a tile would change
    /// colour every time the app started — which is the opposite of the point
    /// of a recognisable mark.
    @Test func aSitesTintIsStableAndKnownBrandsKeepTheirOwn() {
        #expect(ExploreLive.tint(for: "app.uniswap.org") == BrandPalette.uniswap)
        #expect(ExploreLive.tint(for: "curve.fi") == BrandPalette.curve)
        let once = ExploreLive.tint(for: "some-dapp.example")
        let again = ExploreLive.tint(for: "some-dapp.example")
        #expect(once == again)
        #expect(ExploreLive.tint(for: "") == BrandPalette.unknown)
    }

    /// **A lock, and only a lock** (spec 079, owner: "你标记的安全站点 只是https
    /// 而已，并不代表这个站点真的安全 … 用一把锁代表 https 和非https 就行了，不文字
    /// 标记"). The line under a site says only what is a fact about the
    /// connection — "已连接" — and never "安全站点" or "不安全站点"; the http
    /// lock carries `connect.browser.a11yInsecure` for a screen reader.
    @Test func theStatusLineMakesNoClaimAboutTheSite() {
        let words = [loc().t("explore.secureSite"), loc().t("connect.browser.a11yInsecure")]
        for secure in [true, false] {
            for connected in [true, false] {
                let line = ExploreLive.statusLine(secure: secure, connected: connected, host: "x.io", loc: loc())
                for word in words { #expect(!line.contains(word), "\(line) makes a claim about the site") }
                #expect(line == (connected ? loc().t("explore.connectedTag") : ""))
            }
        }
    }

    /// **A page that cannot be reached says so** (058) — and since 079 it
    /// says WHY in the corpus's words, chosen by the core from the failure's
    /// class, not the system's English sentence and a code. The engine's
    /// transitions are in `BrowserLoadTests`; this pins the table it reads.
    @Test func aFailedNavigationIsClassifiedByTheCore() {
        let timedOut = browserLoadClassify(
            platform: "apple", code: Int64(NSURLErrorTimedOut), domain: NSURLErrorDomain, certificate: false
        )
        #expect(timedOut?.class == "timeout")
        #expect(timedOut?.reasonKey == "explore.loadOffline")
        #expect(timedOut?.autoRetry == true)
        let notFound = browserLoadClassify(
            platform: "apple", code: Int64(NSURLErrorCannotFindHost), domain: NSURLErrorDomain, certificate: false
        )
        #expect(notFound?.reasonKey == "explore.loadNotFound")
        #expect(notFound?.autoRetry == false, "a typo does not heal")
        let certificate = browserLoadClassify(
            platform: "apple", code: Int64(NSURLErrorServerCertificateUntrusted), domain: NSURLErrorDomain,
            certificate: false
        )
        #expect(certificate?.reasonKey == "explore.loadCertificate")
        #expect(certificate?.autoRetry == false, "a certificate failure is never retried")
    }

    /// **A cancelled navigation is not a failure.**
    ///
    /// Every redirect chain and every in-flight navigation a page replaces
    /// cancels the last one. Drawing "couldn't load" there would put an error
    /// over a page that is loading perfectly well. WebKit's own "frame load
    /// interrupted" is the same fact.
    @Test func aCancelledNavigationIsNotAFailure() {
        #expect(browserLoadClassify(
            platform: "apple", code: Int64(NSURLErrorCancelled), domain: NSURLErrorDomain, certificate: false
        ) == nil)
        #expect(browserLoadClassify(
            platform: "apple", code: 102, domain: "WebKitErrorDomain", certificate: false
        ) == nil)
    }

    /// A tab with no title is drawn with its host, never blank.
    @Test func aTitlelessTabIsDrawnWithItsHost() {
        let rows = ExploreLive.tabs(
            explore: view(
                tabs: [
                    ExploreTabWire(id: "t1", url: "https://curve.fi/", title: "", host: "curve.fi"),
                    ExploreTabWire(id: "t2", url: nil, title: "", host: ""),
                ],
                selected: "t1"
            ),
            loc: loc()
        )
        #expect(rows.first?.title == "curve.fi")
        #expect(rows.first?.selected == true)
        #expect(rows.last?.startPage == true, "a tab with no url is the start page")
        #expect(rows.last?.site == nil, "and it is drawn with the wallet's own mark")
    }

    /// The connection panel leads with the **origin**, not with the site's
    /// claim about itself.
    @Test func theConsentSurfaceLeadsWithTheOrigin() {
        let panel = ExploreLive.connectionModel(
            dbr: DbrViewWire(
                ready: true,
                consent: DbrConsentViewWire(
                    tab: "t1", origin: "https://app.uniswap.org", methods: ["eth_requestAccounts"],
                    address: "0x88cca0eedbf2c4426110bbfc998f048689266894", chainId: 1
                ),
                tabs: [], sites: [], signing: nil, queuedSigning: 0
            ),
            tab: nil,
            engine: nil,
            identity: (name: "Me", address: "0x88cca0eedbf2c4426110bbfc998f048689266894"),
            loc: loc()
        )
        #expect(panel.site.host == "app.uniswap.org")
        #expect(panel.site.name == "app.uniswap.org", "the host IS the name here — a site's own name is a claim")
        #expect(panel.account.seed == "0x88cca0eedbf2c4426110bbfc998f048689266894")
    }
}
