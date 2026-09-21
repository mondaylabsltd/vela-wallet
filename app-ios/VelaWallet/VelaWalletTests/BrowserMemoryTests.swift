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

    /// A site's letter is the host's, `www.` dropped and upper-cased.
    ///
    /// Without dropping `www.` every site under one domain is a W, which makes
    /// the grid unreadable at exactly the size it is drawn at.
    @Test func aSitesLetterIsItsHostsWithoutTheWww() {
        #expect(ExploreLive.site(host: "www.example.com", name: "", origin: "").letter == "E")
        #expect(ExploreLive.site(host: "app.uniswap.org", name: "", origin: "").letter == "A")
        #expect(ExploreLive.site(host: "", name: "", origin: "").letter == "?")
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

    /// **An http page is SAID to be insecure**, not merely left unpraised.
    ///
    /// 056 asserted the weaker claim — that the line names the host and does
    /// not say "secure" — because it believed no corpus sentence existed for
    /// this. 058's copy ruler found `connect.browser.a11yInsecure`, which
    /// Android has used since 044 and which is translated everywhere. Saying
    /// nothing about an unencrypted page is not neutral: it reads as fine.
    @Test func anInsecureOriginIsCalledInsecure() {
        let secureLabel = loc().t("explore.secureSite")
        let insecureLabel = loc().t("connect.browser.a11yInsecure")
        let insecure = ExploreLive.statusLine(
            secure: false, connected: true, host: "127.0.0.1:8137", loc: loc()
        )
        #expect(!insecure.contains(secureLabel))
        #expect(insecure.contains(insecureLabel))
        // The sentence is a sentence, not the key echoed back (FR-005's
        // failure signal would pass a `contains` check against itself).
        #expect(insecureLabel != "connect.browser.a11yInsecure")
        #expect(insecure.contains(loc().t("explore.connectedTag")))

        let secure = ExploreLive.statusLine(
            secure: true, connected: false, host: "app.uniswap.org", loc: loc()
        )
        #expect(secure == secureLabel)
    }

    /// **A page that cannot be reached says so** (058).
    ///
    /// Both failure callbacks used to set `loading = false` and nothing else,
    /// so an unreachable dApp was a white rectangle under an empty address
    /// bar. On the founder's iPhone that was `app.uniswap.org`, silent, for
    /// sixty seconds. The reason is the SYSTEM's, verbatim: "the host could
    /// not be found" and "the request timed out" are different problems and a
    /// person debugging their own network needs the difference.
    @Test func aFailedNavigationIsDescribedInTheSystemsOwnWords() {
        let timedOut = NSError(
            domain: NSURLErrorDomain, code: NSURLErrorTimedOut,
            userInfo: [NSLocalizedDescriptionKey: "The request timed out."]
        )
        let described = BrowserEngine.describe(timedOut)
        #expect(described.contains("timed out"))
        // The code is carried too: it is what turns "it did not work" into
        // something somebody can look up.
        #expect(described.contains("\(NSURLErrorTimedOut)"))
    }

    /// **A cancelled navigation is not a failure.**
    ///
    /// Every redirect chain and every in-flight navigation a page replaces
    /// cancels the last one. Drawing "couldn't load" there would put an error
    /// over a page that is loading perfectly well.
    @Test func aCancelledNavigationIsNotAFailure() {
        let cancelled = NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled,
                                userInfo: [:])
        #expect(BrowserEngine.describe(cancelled) == BrowserEngine.cancelled)

        // And a non-URL error keeps whatever the system called it.
        let other = NSError(domain: "vela.test", code: 7,
                            userInfo: [NSLocalizedDescriptionKey: "something else"])
        #expect(BrowserEngine.describe(other) == "something else")
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
