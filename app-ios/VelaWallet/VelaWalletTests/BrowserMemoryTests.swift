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

    /// Issue #465: a document saved before custom groups went — one that
    /// listed a favourite, an empty "New group" a phone made — loads through
    /// this store with every favourite and the hidden Recent; the view this
    /// build decodes has no groups, Manage groups is the two sections, and
    /// the next write leaves `groups` out of the store.
    @Test func aDocumentFromBeforeIssue465KeepsEveryFavouriteAndLosesItsGroups() async throws {
        let store = freshStore("explore-pre-465")
        store.writeObject(ExploreExecutor.key, [
            "favorites": [
                ["origin": "https://curve.fi", "url": "https://curve.fi/", "host": "curve.fi",
                 "name": "Curve", "renamed": false, "added_ms": 1_759_051_384_000],
                ["origin": "https://polymarket.com", "url": "https://polymarket.com/",
                 "host": "polymarket.com", "name": "Polymarket", "renamed": false,
                 "added_ms": 1_759_051_385_000],
            ],
            "groups": [
                ["id": "g-1759051390000", "name": "Trading",
                 "members": ["https://curve.fi"], "hidden": false, "created_ms": 1_759_051_390_000],
                ["id": "g-1759051391000", "name": "New group", "members": [String](),
                 "hidden": true, "created_ms": 1_759_051_391_000],
            ],
            "tabs": [[String: Any]](),
            "selected_tab": NSNull(),
            "hidden_system": ["recent"],
            "name_rule": 1,
        ])
        let executor = ExploreExecutor(store: store)
        let core = ExploreSitesCore()

        func operation(_ result: String, _ type: String) throws -> (id: UInt64, op: [String: Any]) {
            let effects = try CoreJSON.object(result)["effects"] as? [[String: Any]] ?? []
            let effect = try #require(effects.first { ($0["operation"] as? [String: Any])?["type"] as? String == type })
            return (try #require((effect["id"] as? NSNumber)?.uint64Value),
                    try #require(effect["operation"] as? [String: Any]))
        }

        let read = try operation(try core.dispatch(eventJson: CoreJSON.string(["type": "start"])), "read_explore")
        let loaded = try core.resolveEffect(effectId: read.id, resultJson: await executor.perform(read.op))
        let view = try CoreJSON.decode(
            ExploreViewWire.self, from: try CoreJSON.object(loaded)["view"] as? [String: Any] ?? [:]
        )
        #expect(view.ready)
        #expect(view.favorites.map(\.origin) == ["https://curve.fi", "https://polymarket.com"])
        #expect(view.recentHidden, "a hidden section stays hidden")
        #expect(!view.favoritesHidden)
        guard case .groupManage(_, let rows) = ExploreLive.groupManage(explore: view, loc: loc()) else {
            Issue.record("Manage groups is not the group sheet"); return
        }
        #expect(rows.map(\.id) == ["favorites", "recent"])

        let renamed = try core.dispatch(eventJson: CoreJSON.string([
            "type": "favorite_renamed", "origin": "https://curve.fi", "name": "Stables",
        ]))
        _ = await executor.perform(try operation(renamed, "write_explore").op)
        let stored = store.readObject(ExploreExecutor.key)
        #expect(stored["groups"] == nil, "the next write drops the groups")
        #expect((stored["favorites"] as? [[String: Any]])?.count == 2, "every favourite is kept")
        #expect(stored["hidden_system"] as? [String] == ["recent"])
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
        tabs: [ExploreTabWire] = [],
        selected: String? = nil,
        favoritesHidden: Bool = false,
        recentHidden: Bool = false,
        favoritesFull: Bool = false
    ) -> ExploreViewWire {
        ExploreViewWire(
            favorites: favorites, tabs: tabs, selectedTab: selected,
            favoritesHidden: favoritesHidden, recentHidden: recentHidden,
            favoritesFull: favoritesFull, tabsFull: false, ready: true
        )
    }

    private func entry(_ origin: String, _ host: String, title: String) -> BhistEntryWire {
        BhistEntryWire(origin: origin, url: origin + "/", host: host, title: title,
                       favicon: "", lastVisitedMs: 1_757_000_000_000)
    }

    /// Recent is the one section under Favorites (issue #465: no custom
    /// groups), and hidden it draws nothing while keeping everything.
    @Test func recentIsTheOnlySectionAndHiddenItDrawsNothing() {
        let history = BhistViewWire(entries: [entry("https://app.aave.com", "app.aave.com", title: "Aave")])

        let shown = ExploreLive.groups(explore: view(), history: history, loc: loc())
        #expect(shown.map(\.id) == ["recent"])
        #expect(shown.first?.sites.map(\.host) == ["app.aave.com"])

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

    /// Issue #330 (Android; iOS drew the same page): with Favorites and
    /// Recent hidden, Explore showed the search field alone — the Favorites
    /// heading's Edit was the only way to Manage groups, and it went with the
    /// hidden section. The heading stays (no tiles), so hidden groups can
    /// always be shown again; a page with no favourites yet keeps it too.
    @Test func everyGroupHiddenKeepsTheWayBackToManageGroups() {
        let history = BhistViewWire(entries: [entry("https://app.aave.com", "app.aave.com", title: "Aave")])
        let me = (name: "Me", address: "0x88cca0eedbf2c4426110bbfc998f048689266894")
        func page(_ explore: ExploreViewWire) -> ExploreHomeModel {
            ExploreLive.home(explore: explore, history: history, dbr: .empty, engine: nil, identity: me, loc: loc())
        }

        let hidden = page(view(favorites: [site("https://app.uniswap.org", "app.uniswap.org")],
                               favoritesHidden: true, recentHidden: true))
        #expect(hidden.favorites?.action == loc().t("explore.edit"), "its Edit opens Manage groups")
        #expect(hidden.favorites?.tiles.isEmpty == true, "a hidden Favorites draws no tiles")
        #expect(hidden.groups.isEmpty)
        #expect(hidden.empty == nil, "not the empty start page")

        let favoritesOnly = page(view(favorites: [site("https://app.uniswap.org", "app.uniswap.org")],
                                      favoritesHidden: true))
        #expect(favoritesOnly.favorites?.action == loc().t("explore.edit"))
        #expect(favoritesOnly.groups.map(\.id) == ["recent"], "Recent's heading offers Clear, never Manage")

        let none = page(view(recentHidden: true))
        #expect(none.favorites?.action == loc().t("explore.edit"), "no favourites yet: the heading stays")
        #expect(none.favorites?.tiles.count == 1, "over its add tile")

        let shown = page(view(favorites: [site("https://app.uniswap.org", "app.uniswap.org")]))
        #expect(shown.favorites?.tiles.count == 2, "shown again, the tiles come back")
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
                // Spec 097 E: no grant says so, rather than going blank.
                #expect(line == loc().t(connected ? "explore.connectedTag" : "home.connEmptyTitle"))
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

    /// Spec 097 E (S3): a site the core holds no grant for — never connected,
    /// or just disconnected — says so, and the panel offers nothing that
    /// implies access: no "can see your address", no Disconnect, no "requests
    /// appear here", no account it sees. The desktop pass found the panel
    /// saying all of it after a disconnect, with only the status word gone.
    @Test func aSiteThatIsNotConnectedSaysSoAndOffersNoAccess() {
        let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
        let tab = DbrTabViewWire(tab: "t1", origin: "https://app.aave.com", connectedAddress: nil,
                                 chainId: 56, secure: true, crashed: false)
        let panel = ExploreLive.connectionModel(
            dbr: DbrViewWire(ready: true, consent: nil, tabs: [tab], sites: [], signing: nil, queuedSigning: 0),
            tab: tab, engine: nil, identity: (name: "Me", address: me), chainIds: [1, 56], loc: loc()
        )
        #expect(panel.connected == false)
        #expect(panel.statusLine == loc().t("home.connEmptyTitle"))
        #expect(panel.explainer.isEmpty, "it can see nothing")
        #expect(panel.footnote.isEmpty, "no requests come from a site with no grant")
        #expect(panel.consent == nil)
        #expect(panel.networks.map(\.id) == [1, 56], "the network it reads is still the site's to change")

        let connected = DbrTabViewWire(tab: "t1", origin: "https://app.aave.com", connectedAddress: me,
                                       chainId: 56, secure: true, crashed: false)
        let granted = ExploreLive.connectionModel(
            dbr: DbrViewWire(ready: true, consent: nil, tabs: [connected], sites: [], signing: nil, queuedSigning: 0),
            tab: connected, engine: nil, identity: (name: "Me", address: me), loc: loc()
        )
        #expect(granted.connected)
        #expect(granted.statusLine == loc().t("explore.connectedTag"))
        #expect(granted.explainer == loc().t("explore.connectionExplainer"))
        #expect(granted.footnote == loc().t("explore.autoRequestHint"))
    }

    // MARK: - Logo misses (spec 082 T120, RE10, W20)

    /// A 404 is a miss for the session; a 503 — or a connection that never
    /// answered — is asked again after the core's minute, and at once when
    /// the network comes back.
    @Test func aLogoMissLastsAsLongAsTheCoreSays() {
        let clock = Clock()
        LogoStore.now = { clock.ms }
        defer {
            LogoStore.now = { Date().timeIntervalSince1970 * 1000 }
            LogoStore.forgetAll()
        }
        let gone = "https://data.example/logos/gone.png"
        let busy = "https://data.example/logos/busy.png"
        let silent = "https://data.example/logos/silent.png"
        LogoStore.recordMiss(gone, status: 404, notAnImage: false)
        LogoStore.recordMiss(busy, status: 503, notAnImage: false)
        LogoStore.recordMiss(silent, status: nil, notAnImage: false)
        #expect(LogoStore.isMissed(gone))
        #expect(LogoStore.isMissed(busy))
        #expect(LogoStore.isMissed(silent))

        clock.ms += 60_001
        #expect(LogoStore.isMissed(gone), "a 404 stays missed for the session")
        #expect(!LogoStore.isMissed(busy), "a 503 is asked again after a minute")
        #expect(!LogoStore.isMissed(silent))

        LogoStore.recordMiss(busy, status: 503, notAnImage: false)
        let epoch = LogoEpoch.shared.value
        LogoStore.networkCameBack()
        #expect(!LogoStore.isMissed(busy), "the network came back: asked again at once")
        #expect(LogoStore.isMissed(gone))
        #expect(LogoEpoch.shared.value == epoch + 1, "the logos on screen ask again")
        #expect(markMissTtlMs(kind: "transport", status: 404) == nil)
        #expect(markMissTtlMs(kind: "transport", status: 503) == 60_000)
    }

    /// W20 (082 review): logos that failed a moment apart behind a dead
    /// network each come back after their minute, on screen, with no tap —
    /// not only the first. The one re-ask used to fire at the FIRST miss's
    /// expiry, when every later miss was still a few ms from its own, and
    /// nothing asked again after it: E-W20's "wait 60 s" brought back one logo.
    @Test func everyTransientMissIsAskedAgainNotOnlyTheFirst() {
        let clock = Clock()
        LogoStore.now = { clock.ms }
        defer {
            LogoStore.now = { Date().timeIntervalSince1970 * 1000 }
            LogoStore.forgetAll()
        }
        LogoStore.forgetAll()
        let start = clock.ms
        let first = "https://data.example/logos/first.png"
        let second = "https://data.example/logos/second.png"
        let gone = "https://data.example/logos/gone.png"
        LogoStore.recordMiss(first, status: 503, notAnImage: false)
        clock.ms += 40
        LogoStore.recordMiss(second, status: nil, notAnImage: false)
        LogoStore.recordMiss(gone, status: 404, notAnImage: false)
        #expect(LogoStore.reaskDueMs == start + 60_000, "the earliest miss is asked first")

        // The first re-ask: the first logo asks again, the second is 40 ms
        // from its own minute — and gets its own re-ask.
        clock.ms = start + 60_000
        let epoch = LogoEpoch.shared.value
        LogoStore.reaskFired()
        #expect(LogoEpoch.shared.value == epoch + 1)
        #expect(!LogoStore.isMissed(first))
        #expect(LogoStore.isMissed(second))
        #expect(LogoStore.reaskDueMs == start + 60_040, "the second logo's minute is asked for too")

        clock.ms = start + 60_040
        LogoStore.reaskFired()
        #expect(LogoEpoch.shared.value == epoch + 2)
        #expect(!LogoStore.isMissed(second))
        #expect(LogoStore.isMissed(gone), "a 404 is never asked again")
        #expect(LogoStore.reaskDueMs == nil, "nothing left to wait for")
    }
}

@MainActor
final class Clock {
    var ms: Double = 1_757_000_000_000
}
