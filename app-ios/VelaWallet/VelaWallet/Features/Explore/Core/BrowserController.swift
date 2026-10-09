//
//  BrowserController.swift
//  VelaWallet
//
//  The browser's owner: three app-resident machines and one engine per open
//  tab.
//
//  App-resident rather than screen-owned for the reason every wallet-state
//  machine here is: a request in flight must survive a sheet, a tab switch
//  and a trip to 设置, and a page that reloaded every time somebody glanced at
//  their balance would lose a half-finished swap.
//
//  ## It routes nothing (spec 070)
//
//  Until 070 this file kept a request→tab map, a list of open ids, a global
//  `browserChain` and two delivery rules ("answer the tab that asked, falling
//  back to the front tab"; "emit to the front tab"). Every tab bug the audit
//  found lived in that bookkeeping: a background tab's navigation settled the
//  front tab's requests, answers fell back to the wrong page, a background
//  tab's approval announced an address to the front page. `dapp_browser` owns
//  all of it now, per tab and per DOCUMENT. This class tells the core what
//  WebKit said — with the tab it said it in — and posts back exactly what the
//  core addresses to exactly the tab it names. There is no `current` fallback
//  anywhere on the page channel.
//
//  ## Leaving 探索 is not closing anything
//
//  The page keeps running and its requests stay open; only closing a tab,
//  navigating, or the renderer dying settles them. (The old `browser_closed`
//  on `onDisappear` cleared the connected chip on every trip to the wallet and
//  never restored it.)
//
//  ## Which tabs keep their page is the core's (spec 099 FR-004)
//
//  Every open tab keeps its own `WKWebView` and its page keeps running when
//  another tab is in front. How many may stay alive is not this file's call:
//  after any tab change, any change of the browser machine's view and on the
//  system's memory warning, it hands the core's `browserEnginePlan` the strip,
//  the recency, the busy tabs and the engines it has, and lets go of exactly
//  the ones the plan names. A suspended tab keeps its URL, title and last
//  picture; the browser machine hears that its page went (the same two events
//  a page that goes while its tab stays always meant); selecting it loads the
//  page again, and the chrome says it was reloaded to save memory until the
//  person leaves that tab.
//
//  ## The history wait is not superstition
//
//  `browser_history` publishes no `ready` flag, and a visit recorded before
//  its store answers is dropped silently — `BrowserWireDriftTests` proves it.
//  So visits queue until `BhistExecutor` says the mirror is live.
//

import Foundation
import Observation
import UIKit
import WebKit
import VelaCore

extension ExploreSitesCore: CoreBridge {}
extension BrowserHistoryCore: CoreBridge {}
extension DappBrowserCore: CoreBridge {}

@MainActor
@Observable
final class BrowserController {

    // MARK: - The three machines

    private(set) var explore: ExploreViewWire = .empty
    private(set) var history: BhistViewWire = .empty
    /// The in-app browser's decisions: consent, per-tab facts, connected sites.
    private(set) var dbr: DbrViewWire = .empty

    private var exploreCore: CoreStore<ExploreViewWire>!
    private var historyCore: CoreStore<BhistViewWire>!
    private var dbrCore: CoreStore<DbrViewWire>!

    /// How many effects the in-app browser's machine has in flight — an
    /// answer it is still waiting on (a chain read, a store write). What a test
    /// waits on instead of a clock: `CoreDriver.isIdle`, counted.
    var dbrInFlight: Int { dbrCore.inFlight }

    private let exploreExecutor: ExploreExecutor
    private let historyExecutor: BhistExecutor
    private let dbrExecutor: DbrExecutor

    // MARK: - The engines

    /// One per open tab, keyed by the explore machine's tab id — the same id
    /// `dapp_browser` knows the tab by.
    private var engines: [String: BrowserEngine] = [:]
    /// The engine in front of the person, if a tab with a page is selected.
    /// For DRAWING only: nothing on the page channel ever falls back to it.
    private(set) var current: BrowserEngine?

    /// The tab whose page the person ASKED to see (DESIGN N — Android's
    /// `wanted`, the same latch per tab).
    ///
    /// Tabs survive a launch, and restoring one is right — landing inside it
    /// is not. The owner opened 探索 on an iPhone and a page from a previous
    /// session loaded itself: a tab kept from two days earlier, at a
    /// `127.0.0.1` address that is nothing on this device now, so the browser
    /// opened onto an error nobody asked for (2026-09-23).
    ///
    /// The first fix was one flag for the run — "a page has been asked for" —
    /// and it woke tabs nobody had asked for once it was set: a page opened
    /// from outside (a `velawallet://open` link, a launch URL, the
    /// external-page sheet) set it before the saved tabs had loaded, so the
    /// restored tab in front loaded yesterday's dApp behind the new one —
    /// scripts running, a consent it could raise; and closing a page woke the
    /// neighbour the core selected, behind the home.
    ///
    /// So `reconcile` MINTS an engine — and runs a page's scripts — only for
    /// this tab, while it is the one selected: the tab a resume row, the
    /// switcher or a waiting request brought forward, the tab an open went
    /// to, a tab an open made. A restored tab, one left behind the home, the
    /// neighbour a close selects: dormant until somebody resumes it. The ask
    /// lapses when the selection moves off its tab without one, and when
    /// Explore lands on its home (`landedHome`). Engines that exist are kept
    /// either way — this gates making one, never keeping one.
    private var wanted: String?
    /// An open went to a NEW tab the core has yet to make: the tabs there
    /// before it. The selected tab that is not one of them is the one asked
    /// for (`reconcile` reads it once).
    private var wantOpened: Set<String>?
    /// Bumped whenever an engine's navigation state changes, so SwiftUI
    /// redraws chrome that reads a non-observable engine property.
    private(set) var engineTick = 0

    /// The wallet's networks, as last told to the core — the chains a site
    /// may be switched to from the connection panel.
    private(set) var chainIds: [Int] = []

    /// Settings' debug mode (spec 091), as the app last said. Which pages are
    /// offered the wallet is the core's rule; this is only the fact it reads.
    private(set) var debugMode = false

    /// Tabs whose engine `browserEnginePlan` let go of (spec 099 FR-004),
    /// with their last picture for the switcher.
    private var suspended: [String: UIImage?] = [:]
    /// The tab whose page was loaded again because it had been suspended —
    /// the status line says so until the person leaves it.
    private(set) var reloadedTab: String?
    /// A plan is being applied: the browser machine's view it changes does
    /// not start another.
    @ObservationIgnored private var planning = false
    /// What each tab's status line last said when the person dismissed it
    /// (or opened its details): gone until it would say something new.
    private(set) var statusSeen: [String: String] = [:]
    /// The tab whose status panel is open, as the browser machine was told.
    private(set) var inspectedTab: String?
    @ObservationIgnored private var memoryObserver: NSObjectProtocol?
    #if DEBUG
    /// Each engine as it is made, before its first load: a test stands in
    /// for its loader and sees exactly which page each tab was asked to load.
    @ObservationIgnored var engineMadeForTesting: ((BrowserEngine) -> Void)?
    #endif

    /// Visits recorded before the history store answered.
    private var queuedVisits: [[String: Any]] = []
    private var historyReady = false

    // MARK: - Ports the app fills in

    struct Ports {
        /// One call through the wallet's own pool — the body, or why there is
        /// none (every endpoint rate-limiting, or none answering).
        var poolCall: @MainActor (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> DbrExecutor.Read
        = { _, _, _, _ in .unanswered(rateLimited: false) }
        /// The transaction a user operation landed in (the relay's receipt).
        var resolveUserOp: @MainActor (_ chainId: Int, _ userOpHash: String) async -> String? = { _, _ in nil }
        /// Open the signing sheet for a request. `nil` = there is no sheet to
        /// open, and the request is refused rather than left waiting.
        var onForwardToSigning: ((DbrForward) -> Void)?
        /// The page behind a sheet is gone: close that sheet, answer nothing.
        var onCancelSigning: (_ tab: String, _ id: String) -> Void = { _, _ in }
        /// The feed's store, for the "connected to" row.
        var writeRecords: ([[String: Any]]) -> Void = { _ in }
        /// Spec 100: carry a page's add-network request to `network_admin`
        /// (`dapp_add_requested`) — the operation as the core sent it. `nil`
        /// = nobody can show the sheet, and the request is declined rather
        /// than left waiting.
        var onForwardToAddNetwork: (([String: Any]) -> Void)?
        /// Spec 100: the page behind an add-network sheet is gone.
        var onCancelAddNetwork: (_ tab: String, _ id: String) -> Void = { _, _ in }
    }

    var ports: Ports

    private let now: () -> Double

    init(
        store: VelaStore,
        ports: Ports = Ports(),
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }
    ) {
        self.ports = ports
        self.now = now
        self.exploreExecutor = ExploreExecutor(store: store)
        self.historyExecutor = BhistExecutor(store: store)
        self.dbrExecutor = DbrExecutor(store: store, now: now)

        exploreCore = CoreStore(
            bridge: ExploreSitesCore(),
            perform: { [exploreExecutor] in await exploreExecutor.perform($0) },
            onView: { [weak self] view in self?.commitExplore(view) },
            onFault: { VelaLog.failure(.browser, kind: "explore_sites_fault", VelaLog.error($0)) }
        )
        historyCore = CoreStore(
            bridge: BrowserHistoryCore(),
            perform: { [historyExecutor] in await historyExecutor.perform($0) },
            onView: { [weak self] view in self?.history = view },
            onFault: { VelaLog.failure(.browser, kind: "browser_history_fault", VelaLog.error($0)) }
        )
        dbrCore = CoreStore(
            bridge: DappBrowserCore(),
            perform: { [dbrExecutor] in await dbrExecutor.perform($0) },
            onView: { [weak self] view in self?.commitDbr(view) },
            onFault: { VelaLog.failure(.browser, kind: "dapp_browser_fault", VelaLog.error($0)) },
            neutralAnswer: { DbrExecutor.neutralAnswer($0) }
        )

        wirePorts()
        // Spec 099 FR-004: the system asked for memory — every tab but the
        // one in front and the busy ones lets its page go.
        memoryObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.didReceiveMemoryWarningNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.memoryWarning() }
        }
    }

    private func wirePorts() {
        historyExecutor.onLoaded = { [weak self] in self?.flushVisits() }

        dbrExecutor.ports = DbrExecutor.Ports(
            deliver: { [weak self] tab, json in self?.engines[tab]?.deliver(json) },
            poolCall: { [weak self] chainId, method, params, bundler in
                guard let self else { return .unanswered(rateLimited: false) }
                return await self.ports.poolCall(chainId, method, params, bundler)
            },
            resolveUserOp: { [weak self] chainId, hash in
                await self?.ports.resolveUserOp(chainId, hash) ?? nil
            },
            forwardToSigning: { [weak self] forward in self?.forwardToSigning(forward) },
            cancelSigning: { [weak self] tab, id in self?.ports.onCancelSigning(tab, id) },
            saveConnectionRecord: { [weak self] row in self?.ports.writeRecords([row]) },
            forwardToAddNetwork: { [weak self] operation in self?.forwardToAddNetwork(operation) },
            cancelAddNetwork: { [weak self] tab, id in self?.ports.onCancelAddNetwork(tab, id) }
        )
    }

    // MARK: - Lifecycle

    /// Boot the three machines. Idempotent.
    func start() {
        exploreCore.boot(CoreJSON.string(["type": "start"]))
        historyCore.boot(CoreJSON.string(["type": "start"]))
        startConnections()
    }

    /// Boot the browser core alone, for Settings' list of connected sites.
    /// Idempotent, and harmless before any page exists: it only lists the
    /// stored sites.
    func startConnections() {
        guard dbrCore.boot(CoreJSON.string(["type": "start"])) else { return }
        // Right behind `start`, before any engine exists to send a page
        // message: what is dispatched before the boot is dropped.
        tellCoreDebugMode()
    }

    /// Settings' debug mode, at construction and on every change (spec 091).
    ///
    /// The core hears it (`debug_mode_changed`) and decides which pages are
    /// offered the wallet. Turning it OFF takes the wallet from a LAN page at
    /// once: the core retires every document it no longer offers — its open
    /// requests answered 4900, a sheet showing one closed. Every open tab
    /// installs the matching script, which a WebView reads when a document
    /// starts: a page already open without the wallet gets it at its next
    /// load (reload or navigation), never mid-page. New tabs start with it.
    func setDebugMode(_ on: Bool) {
        guard on != debugMode else { return }
        debugMode = on
        tellCoreDebugMode()
        for engine in engines.values { engine.setDebugMode(on) }
    }

    /// Dropped before the core boots; `startConnections` says it then.
    private func tellCoreDebugMode() {
        dbrCore.dispatch(CoreJSON.string(["type": "debug_mode_changed", "on": debugMode]))
    }

    /// The wallet's chains. A site may switch or add only to one of these.
    func networksChanged(_ chainIds: [Int]) {
        self.chainIds = chainIds
        dbrCore.dispatch(CoreJSON.string(["type": "networks_changed", "chain_ids": chainIds]))
    }

    /// The wallet's accounts, then the active one.
    ///
    /// All of them first: a grant whose account left the wallet is dropped
    /// only against a KNOWN list (an empty one is "not known yet", never
    /// "nobody"). Then the active one — every grant follows it, and each open
    /// tab of a re-pinned site hears `accountsChanged`. Repeating it with the
    /// same account changes nothing.
    func accountsChanged(addresses: [String], active: String?) {
        dbrCore.dispatch(CoreJSON.string([
            "type": "accounts_updated",
            "addresses": addresses.isEmpty ? NSNull() : addresses as Any,
        ]))
        if let active, !active.isEmpty {
            dbrCore.dispatch(CoreJSON.string([
                "type": "account_switched", "address": active, "now_ms": now(),
            ]))
        }
    }

    // MARK: - A page from outside (spec 088 FR-004)

    /// A page another app or website asked this wallet to open, and the host
    /// the person is asked about.
    struct ExternalPage: Identifiable, Equatable {
        let url: String
        let host: String
        var id: String { url }
    }

    /// `velawallet://open?url=…` waits here for the person. Nothing loads —
    /// and no page meets the provider — until they say yes to the host on the
    /// sheet (`answerExternal`).
    var externalPage: ExternalPage?

    /// Ask before opening a page from outside. The core decides whether the
    /// link may be opened at all (https, a plain host) and which host to show;
    /// anything else is dropped in silence, like any link this app does not
    /// handle.
    func askToOpen(_ url: String) {
        guard let host = dappExternalPageHost(url: url) else { return }
        externalPage = ExternalPage(url: url, host: host)
    }

    /// The sheet's answer: open the page as asked, or forget it. `true` when
    /// a page is now opening.
    @discardableResult
    func answerExternal(_ open: Bool) -> Bool {
        guard let page = externalPage else { return false }
        externalPage = nil
        guard open else { return false }
        self.open(page.url)
        return true
    }

    // MARK: - Tabs and navigation

    /// Open what somebody typed: a URL, a host, or a search.
    ///
    /// Through the core's `dappBrowserInput`, so all three browsers read the
    /// address bar the same way. Waits for the mirror: a mutation dispatched
    /// before hydration is dropped by the core, which is how a deep link used
    /// to open nothing at all.
    ///
    /// WHICH tab it goes into is the core's (`browserOpenTarget`, DESIGN N):
    /// the page's own tab only when its address bar was used (`onPage`);
    /// from the home or from outside, the selected start-page tab or a NEW
    /// tab — never over a live dApp. Until 2026-10 this navigated the
    /// selected tab whatever it held, so a site opened from the home
    /// replaced the dApp the person had left there. A picked site (`.site`)
    /// that a tab is already on comes back as it was left. A full strip
    /// (24 tabs) has no room for a new one, and the core then names a tab it
    /// can spare — a start page, else the one used longest ago, never the
    /// dApp just left — which is selected before it loads.
    ///
    /// Only the tab the open goes to is asked for (`wanted`), and only once
    /// the open runs — after the saved tabs have loaded: the tab that was in
    /// front when they loaded stays as dormant as it came.
    func open(_ text: String, kind: ExploreOpenKind = .address, onPage: Bool = false) {
        guard let url = dappBrowserInput(text: text) else { return }
        whenReady { [weak self] in
            guard let self else { return }
            let shown = onPage ? explore.selectedTab : nil
            // A full strip is the core's too: it never answers a new tab
            // the explore machine would drop, but a tab it can spare.
            let target = Self.openTarget(view: explore, shown: shown, onPage: onPage, url: url, kind: kind)
            switch target {
            case .load(let id):
                load(url, into: id)
            case .resume(let id):
                VelaLog.notice(.browser, "open resumes tab=\(id)")
                selectTab(id)
            case .newTab:
                // The tab this makes is the one asked for: `reconcile` finds
                // it as the selected tab that was not here before — inside
                // this dispatch, which commits the view before it returns.
                wantOpened = Set(explore.tabs.map(\.id))
                exploreCore.dispatch(CoreJSON.string([
                    "type": "tab_opened", "url": url, "title": NSNull(), "now_ms": now(),
                ]))
                wantOpened = nil
            }
        }
    }

    /// The core's `load`: `id` takes `url`, and is the tab asked for.
    private func load(_ url: String, into id: String) {
        wanted = id
        if explore.selectedTab != id {
            exploreCore.dispatch(CoreJSON.string(["type": "tab_selected", "id": id]))
        }
        exploreCore.dispatch(CoreJSON.string([
            "type": "tab_navigated", "id": id, "url": url, "title": NSNull(),
        ]))
        // An engine already showing a page is told directly; a start
        // page gets its engine from `reconcile`, which loads the URL.
        engines[id]?.load(url)
    }

    /// Open a site the person tapped, by the id the tile or row carries.
    ///
    /// A favourite's `url` can be deeper than its origin, because that is
    /// where the person actually works. Recents carry the exact URL for the
    /// same reason. Either is a SITE to the core: a tab already on it is
    /// brought back rather than loaded over.
    func openSite(id: String) {
        if let pinned = explore.favorites.first(where: { $0.origin == id }) {
            open(pinned.url, kind: .site)
            return
        }
        if let visited = history.entries.first(where: { $0.origin == id }) {
            open(visited.url, kind: .site)
            return
        }
        open(id, kind: .site)
    }

    // MARK: - Where Explore lands, and where an opened site goes (DESIGN N)

    /// What Explore shows for `entry`, now — the core's `exploreLanding`
    /// over this strip and the tab whose request waits on the person.
    func landing(_ entry: ExploreEntry) -> ExploreLanding {
        Self.landing(view: explore, entry: entry, waiting: waitingTab)
    }

    /// The tab whose request is in front of the person — the consent, the
    /// signature or the add-network sheet, in the core's order
    /// (`browserWaitingTab`).
    var waitingTab: String? { Self.waitingTab(dbr) }

    /// The tab to mark as "this tab" (`browserLitTab`): the one in front
    /// while its page shows; over the home, only a selected start page — a
    /// dApp left for the wallet waits unlit.
    func litTab(onPage: Bool) -> String? {
        Self.litTab(view: explore, shown: onPage ? explore.selectedTab : nil, onPage: onPage)
    }

    /// The core's landing rule. Home for anything it cannot read: the home
    /// closes and loads nothing, so it is the side to fall on.
    static func landing(view: ExploreViewWire, entry: ExploreEntry, waiting: String?) -> ExploreLanding {
        guard let json = exploreLanding(viewJson: view.stripJSON, entry: entry.rawValue, waiting: waiting),
              let answer = try? CoreJSON.object(json),
              answer["type"] as? String == "tab", let id = answer["id"] as? String
        else { return .home }
        return .tab(id)
    }

    /// The core's open rule. A new tab for anything it cannot read: that
    /// replaces nothing.
    static func openTarget(
        view: ExploreViewWire, shown: String?, onPage: Bool, url: String, kind: ExploreOpenKind
    ) -> ExploreOpenTarget {
        guard let json = browserOpenTarget(
                viewJson: view.stripJSON, shown: shown, onPage: onPage, url: url, kind: kind.rawValue
              ),
              let answer = try? CoreJSON.object(json)
        else { return .newTab }
        switch (answer["type"] as? String, answer["id"] as? String) {
        case ("load", let id?): return .load(id)
        case ("resume", let id?): return .resume(id)
        default: return .newTab
        }
    }

    static func litTab(view: ExploreViewWire, shown: String?, onPage: Bool) -> String? {
        browserLitTab(viewJson: view.stripJSON, shown: shown, onPage: onPage)
    }

    /// `browserWaitingTab` over the three sheets' tabs — all the rule reads
    /// of the browser machine's view.
    static func waitingTab(_ dbr: DbrViewWire) -> String? {
        var sheets: [String: Any] = [:]
        if let consent = dbr.consent { sheets["consent"] = ["tab": consent.tab] }
        if let signing = dbr.signing { sheets["signing"] = ["tab": signing.tab] }
        if let adding = dbr.addingNetwork { sheets["adding_network"] = ["tab": adding.tab] }
        return browserWaitingTab(dappViewJson: CoreJSON.string(sheets))
    }

    func newTab() {
        whenReady { [weak self] in
            guard let self else { return }
            exploreCore.dispatch(CoreJSON.string([
                "type": "tab_opened", "url": NSNull(), "title": NSNull(), "now_ms": now(),
            ]))
        }
    }

    /// A tab the person asked to see — a resume row, the switcher, an open
    /// that resumes, a request waiting in it: it gets its page, live as it
    /// was left, or loaded again when it had none.
    func selectTab(_ id: String) {
        // Asked for, even when it is the tab already selected: a resume row
        // is how a person reaches the page a launch left dormant.
        wanted = id
        guard explore.selectedTab != id else {
            reconcile(explore)
            return
        }
        exploreCore.dispatch(CoreJSON.string(["type": "tab_selected", "id": id]))
    }

    /// Explore landed on its home: nothing is shown, so no tab is asked for.
    /// The tab left in front keeps the engine it has; none is woken behind
    /// the home until a resume (Android's `landedHome`).
    func landedHome() {
        wanted = nil
    }

    func closeTab(_ id: String) {
        exploreCore.dispatch(CoreJSON.string(["type": "tab_closed", "id": id]))
    }

    /// Close several tabs at once (spec 099 — Chrome's "close other tabs",
    /// "close tabs to the right", "close all tabs").
    ///
    /// Which ones is the core's (`exploreTabsClosedBy`), and one `tabs_closed`
    /// closes them in one write: a selection that survives stays, a closed one
    /// moves to the nearest surviving tab on its right, else its left, and
    /// none left is the start page. Nothing else is done here: each closed
    /// tab leaves the explore view, and `reconcile` tears its engine down and
    /// tells the browser machine `tab_closed` — exactly as for a single close.
    /// Returns the ids it closed; none means nothing was sent.
    @discardableResult
    func closeTabs(_ scope: ExploreTabCloseScope) -> [String] {
        let ids = Self.tabsClosed(by: scope, strip: Self.stripJSON(explore.tabs))
        guard !ids.isEmpty else { return [] }
        VelaLog.notice(.browser, "close tabs n=\(ids.count)")
        exploreCore.dispatch(CoreJSON.string(["type": "tabs_closed", "ids": ids]))
        return ids
    }

    func closeAllTabs() {
        closeTabs(.all)
    }

    /// The strip as the core reads it (`ExploreView.tabs`, JSON).
    static func stripJSON(_ tabs: [ExploreTabWire]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: tabs.map(\.wire)),
              let text = String(data: data, encoding: .utf8)
        else { return "[]" }
        return text
    }

    /// The tabs `scope` takes from `strip` (`stripJSON`), in strip order — the
    /// core's `tabs_closed_by`. Empty when the core cannot read the input:
    /// closing nothing is the safe side.
    static func tabsClosed(by scope: ExploreTabCloseScope, strip: String) -> [String] {
        guard let json = exploreTabsClosedBy(tabsJson: strip, scopeJson: CoreJSON.string(scope.wire)),
              let data = json.data(using: .utf8),
              let ids = (try? JSONSerialization.jsonObject(with: data)) as? [String]
        else { return [] }
        return ids
    }

    func goBack() { current?.goBack() }
    func goForward() { current?.goForward() }
    func reload() { current?.reload() }
    /// Stop the load in flight (spec 082 RE5): the committed page stays.
    func stop() { current?.stop() }

    /// The network came back (spec 082 RE3): every failed page starts its
    /// count again, and the one in front is asked for again when its class is
    /// one a returning network can clear.
    func networkCameBack() {
        for engine in engines.values { engine.networkCameBack() }
    }

    // MARK: - What is on screen (spec 079)

    /// Whether the browsing view is showing — a failed page retries by
    /// itself only while it is the page in front, 探索 is on screen and the
    /// app is active (FR-012).
    private var browsingVisible = false
    private var appActive = true

    func browsingVisible(_ visible: Bool) {
        browsingVisible = visible
        syncOnScreen()
    }

    func appActive(_ active: Bool) {
        appActive = active
        for engine in engines.values { engine.setAppActive(active) }
    }

    /// A tab's page as it was last seen (spec 079), for the switcher.
    /// A suspended tab keeps the picture it had when its page was let go.
    func snapshot(of tab: String) -> UIImage? {
        if let engine = engines[tab] { return engine.snapshot }
        return suspended[tab] ?? nil
    }

    /// Photograph the page in front, then `done` — the tab switcher opens on
    /// a card that shows it. Never waits long: a snapshot that does not come
    /// back within a moment is not worth a stuck button.
    func snapshotCurrent(then done: @escaping () -> Void) {
        guard let current else { return done() }
        var finished = false
        let finish = {
            guard !finished else { return }
            finished = true
            done()
        }
        current.captureSnapshot(finish)
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 300_000_000)
            finish()
        }
    }

    private func syncOnScreen() {
        for engine in engines.values {
            let shown = browsingVisible && engine === current
            if engine.onScreen != shown { engine.setOnScreen(shown) }
        }
    }

    /// The core's facts about the tab in front.
    var currentTab: DbrTabViewWire? { dbr.tab(explore.selectedTab) }

    // MARK: - Favourites and the start page's sections

    func addFavorite(url: String, title: String?) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "favorite_added", "url": url,
            "title": title.map { $0 as Any } ?? NSNull(), "now_ms": now(),
        ]))
    }

    func removeFavorite(origin: String) {
        exploreCore.dispatch(CoreJSON.string(["type": "favorite_removed", "origin": origin]))
    }

    /// The star: remove a site that is already a favourite, add one that is
    /// not.
    ///
    /// Spec 082 RE1: the star acts on what the bar names — the page on
    /// screen, never where a load in flight is going. Under a failure panel
    /// that is the address that failed, and the engine's title is still the
    /// page before's: the name is the core's rule (`browserPinnedTitle`) —
    /// that site's last good title, else its host (issue #329).
    func toggleFavorite() {
        guard let engine = current else { return }
        let url = engine.bar.url
        let origin = ProviderBridge.origin(of: url)
        guard !url.isEmpty, !origin.isEmpty else { return }
        if explore.favorites.contains(where: { $0.origin == origin }) {
            removeFavorite(origin: origin)
        } else {
            addFavorite(url: url, title: browserPinnedTitle(url: url, lastGood: engine.lastVisit))
        }
    }

    func renameFavorite(origin: String, name: String) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "favorite_renamed", "origin": origin, "name": name,
        ]))
    }

    func setSystemGroupHidden(_ group: String, hidden: Bool) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "system_group_hidden_set", "group": group, "hidden": hidden,
        ]))
    }

    func clearRecent() {
        historyCore.dispatch(CoreJSON.string(["type": "clear_all"]))
    }

    func deleteRecent(origin: String) {
        historyCore.dispatch(CoreJSON.string(["type": "delete_origin", "origin": origin]))
    }

    // MARK: - Connections

    func consentApproved() {
        dbrCore.dispatch(CoreJSON.string(["type": "consent_approved", "now_ms": now()]))
    }

    func consentRejected() {
        dbrCore.dispatch(CoreJSON.string(["type": "consent_rejected", "now_ms": now()]))
    }

    /// Disconnect one site — from the connection panel, the site menu or
    /// Settings. Every open tab of that origin hears `accountsChanged []` and
    /// `disconnect`.
    func revoke(origin: String) {
        guard !origin.isEmpty else { return }
        dbrCore.dispatch(CoreJSON.string(["type": "revoke_requested", "origin": origin]))
    }

    /// Settings → Storage → dApp connections: every grant, in the live
    /// session too — not only on disk until the next launch.
    func revokeAll() {
        dbrCore.dispatch(CoreJSON.string(["type": "revoke_all"]))
    }

    /// The person picked a network for a site in the connection panel.
    func pickSiteChain(origin: String, chainId: Int) {
        guard !origin.isEmpty else { return }
        dbrCore.dispatch(CoreJSON.string([
            "type": "site_chain_picked", "origin": origin, "chain_id": chainId,
        ]))
    }

    // MARK: - Signing

    /// The signing pipeline's answer to a forwarded request, exactly once.
    ///
    /// `payload` is `sign_request`'s own `SignResponsePayload` — the core
    /// builds the page's message from it. `userOpHash` is set when the page
    /// was answered with a user-operation hash (a receipt that did not land
    /// in time), so the page's later receipt polls can be translated.
    func signingAnswered(tab: String, id: String, payload: [String: Any], userOpHash: String?) {
        dbrCore.dispatch(CoreJSON.string([
            "type": "signing_answered",
            "tab": tab,
            "id": id,
            "payload": payload,
            "user_op_hash": userOpHash.map { $0 as Any } ?? NSNull(),
            "now_ms": now(),
        ]))
    }

    // MARK: - Adding a network (spec 100)

    /// The add-network sheet's ending for a forwarded request — `outcome` is
    /// `network_admin`'s `DappAddOutcome`, carried as it came; the core answers
    /// the page, exactly once.
    func addNetworkAnswered(tab: String, id: String, outcome: [String: Any]) {
        dbrCore.dispatch(CoreJSON.string([
            "type": "add_network_answered",
            "tab": tab,
            "id": id,
            "outcome": outcome,
            "now_ms": now(),
        ]))
    }

    private func forwardToAddNetwork(_ operation: [String: Any]) {
        let tab = operation["tab"] as? String ?? ""
        let id = operation["id"] as? String ?? ""
        // The sheet names the site; the page behind it should be the one that asked.
        if explore.tabs.contains(where: { $0.id == tab }) { selectTab(tab) }
        guard let open = ports.onForwardToAddNetwork else {
            addNetworkAnswered(tab: tab, id: id, outcome: ["type": "declined"])
            return
        }
        open(operation)
    }

    /// The error a request gets when there is no sheet to show it on. -32002,
    /// the code a busy wallet answers; the kind only picks default words, and
    /// the message is given.
    static func busyPayload() -> [String: Any] {
        ["type": "err", "code": -32002, "kind": "submit_failed", "message": "Another request is open"]
    }

    private func forwardToSigning(_ forward: DbrForward) {
        // The sheet names the site; the page behind it should be the one that
        // asked, not whichever tab happened to be in front.
        if explore.tabs.contains(where: { $0.id == forward.tab }) { selectTab(forward.tab) }
        guard let open = ports.onForwardToSigning else {
            signingAnswered(tab: forward.tab, id: forward.id, payload: Self.busyPayload(), userOpHash: nil)
            return
        }
        open(forward)
    }

    // MARK: - Engines

    private func commitExplore(_ view: ExploreViewWire) {
        explore = view
        reconcile(view)
        planEngines()
        flushReady()
    }

    private func commitDbr(_ view: DbrViewWire) {
        dbr = view
        // A request that settled may free a tab to be let go; one that opened
        // keeps it (the core's `busy`).
        planEngines()
    }

    // MARK: - Which engines stay (spec 099 FR-004)

    /// The core's input: the strip, the tab in front, the recency, the busy
    /// tabs and the engines that exist — `EngineInput`.
    static func engineInput(
        explore: ExploreViewWire, dbr: DbrViewWire, live: [String], pressure: Bool
    ) -> [String: Any] {
        [
            "tabs": explore.tabs.map(\.id),
            "selected": explore.selectedTab.map { $0 as Any } ?? NSNull(),
            "recent": explore.recentTabs,
            "busy": dbr.tabs.filter(\.busy).map(\.tab),
            "live": live,
            "pressure": pressure,
        ]
    }

    /// The tabs the core's `browserEnginePlan` says to let go of now. Empty
    /// when the core cannot read the input — keeping a page is the safe side.
    static func enginePlan(_ input: [String: Any]) -> [String] {
        guard let json = browserEnginePlan(inputJson: CoreJSON.string(input)),
              let plan = try? CoreJSON.object(json)
        else { return [] }
        return plan["suspend"] as? [String] ?? []
    }

    /// The system asked the app to free memory.
    func memoryWarning() {
        VelaLog.notice(.browser, "memory warning live=\(engines.count)")
        planEngines(pressure: true)
    }

    /// Ask the core which engines to let go of, and let go of exactly those.
    private func planEngines(pressure: Bool = false) {
        guard !planning, !engines.isEmpty else { return }
        planning = true
        defer { planning = false }
        let input = Self.engineInput(
            explore: explore, dbr: dbr, live: engines.keys.sorted(), pressure: pressure
        )
        for id in Self.enginePlan(input) { suspend(id) }
    }

    /// Let `id`'s page go and keep its tab: URL and title stay in the strip,
    /// the picture stays for the switcher.
    private func suspend(_ id: String) {
        guard let engine = engines.removeValue(forKey: id) else { return }
        if current === engine { current = nil }
        suspended[id] = engine.snapshot
        engine.tearDown()
        VelaLog.notice(.browser, "suspend tab=\(id) (memory)")
        // Its document is gone; its tab is not, and its next page must be
        // answered — so not `tab_closed`, whose id is never a page again. A
        // load of nothing that no document greets retires the page it
        // replaces, exactly as a page that goes while its tab stays.
        for event in Self.pageGoneEvents(tab: id, nowMs: now()) {
            dbrCore.dispatch(CoreJSON.string(event))
        }
    }

    /// What the browser machine hears when `tab`'s page goes and the tab
    /// stays (the desktop's `page_gone_events`).
    static func pageGoneEvents(tab: String, nowMs: Double) -> [[String: Any]] {
        [
            ["type": "navigation_started", "tab": tab, "url": "about:blank", "now_ms": nowMs],
            ["type": "load_finished", "tab": tab, "url": "about:blank", "now_ms": nowMs],
        ]
    }

    // MARK: - The tab's status (spec 099 FR-014)

    /// The tab's status panel opened: the browser machine carries that tab's
    /// whole record until it closes.
    func inspectorOpened(tab: String) {
        inspectedTab = tab
        dbrCore.dispatch(CoreJSON.string(["type": "inspector_opened", "tab": tab]))
    }

    func inspectorClosed() {
        guard inspectedTab != nil else { return }
        inspectedTab = nil
        dbrCore.dispatch(CoreJSON.string(["type": "inspector_closed"]))
    }

    /// The person dismissed (or opened) the line that said `seen` for `tab`:
    /// it stays away until it would say something else.
    func dismissStatus(tab: String, seen: String) {
        statusSeen[tab] = seen
    }

    /// Make the engines match the tabs.
    ///
    /// An engine loads its tab's URL once, when it is made. After that the
    /// PAGE is the authority on where it is — a redirect, a link, a
    /// `pushState` — and the tab's record follows it (`onMeta`), never the
    /// other way round: pushing the record back into a page mid-redirect
    /// would reload it in a loop.
    private func reconcile(_ view: ExploreViewWire) {
        let live = Set(view.tabs.map(\.id))
        for (id, engine) in engines where !live.contains(id) {
            engines.removeValue(forKey: id)
            if current === engine { current = nil }
            engine.tearDown()
            dbrCore.dispatch(CoreJSON.string(["type": "tab_closed", "tab": id, "now_ms": now()]))
        }
        // A suspended tab that closed has no engine to tear down, but its
        // record in the browser machine goes the same way.
        for id in suspended.keys where !live.contains(id) {
            suspended.removeValue(forKey: id)
            dbrCore.dispatch(CoreJSON.string(["type": "tab_closed", "tab": id, "now_ms": now()]))
        }
        statusSeen = statusSeen.filter { live.contains($0.key) }
        // The reloaded line lasts until the person leaves that tab.
        if let reloadedTab, reloadedTab != view.selectedTab { self.reloadedTab = nil }
        // The new tab an open made is the page that was asked for.
        if let before = wantOpened, let id = view.selectedTab, !before.contains(id) {
            wanted = id
            wantOpened = nil
        }
        // An ask is for its tab while that tab is in front. The selection
        // moving with no ask — a close that selects a neighbour, a start page
        // opened — asks for nothing.
        if let wanted, wanted != view.selectedTab { self.wanted = nil }

        defer { syncOnScreen() }
        guard let selected = view.selected, let url = selected.url, !url.isEmpty else {
            // No tab, or the start page's own tab: a tab with no site is not
            // a page.
            current = nil
            return
        }
        if let engine = engines[selected.id] {
            current = engine
            return
        }
        guard selected.id == wanted else {
            // Nobody asked for this tab's page — restored at launch, left
            // behind the home, the neighbour of a closed tab: it stays
            // dormant, and Explore shows its home. A resume wakes it — see
            // `wanted`.
            current = nil
            return
        }
        let engine = makeEngine(id: selected.id)
        engine.setAppActive(appActive)
        current = engine
        // Woken: its page was let go to save memory, and loads again — said,
        // never silent (spec 099 FR-004).
        if suspended.removeValue(forKey: selected.id) != nil {
            reloadedTab = selected.id
            VelaLog.notice(.browser, "wake tab=\(selected.id) reloaded (was suspended)")
        }
        engine.load(url)
    }

    private func makeEngine(id: String) -> BrowserEngine {
        let engine = BrowserEngine(id: id, debugMode: debugMode)
        engine.onPageMessage = { [weak self] frameOrigin, isMainFrame, body in
            guard let self else { return }
            dbrCore.dispatch(CoreJSON.string([
                "type": "page_message",
                "tab": id,
                "frame_origin": frameOrigin,
                "is_main_frame": isMainFrame,
                "message_json": body,
                "now_ms": now(),
            ]))
        }
        engine.onNavigationStarted = { [weak self] url in
            guard let self else { return }
            dbrCore.dispatch(CoreJSON.string([
                "type": "navigation_started", "tab": id, "url": url, "now_ms": now(),
            ]))
        }
        engine.onLoadFinished = { [weak self] url in
            guard let self else { return }
            dbrCore.dispatch(CoreJSON.string([
                "type": "load_finished", "tab": id, "url": url, "now_ms": now(),
            ]))
        }
        engine.onRendererGone = { [weak self] in
            guard let self else { return }
            dbrCore.dispatch(CoreJSON.string(["type": "renderer_gone", "tab": id, "now_ms": now()]))
        }
        engine.onMeta = { [weak self] url, title in
            self?.exploreCore.dispatch(CoreJSON.string([
                "type": "tab_navigated", "id": id, "url": url,
                "title": title.isEmpty ? NSNull() : title as Any,
            ]))
        }
        engine.onVisited = { [weak self] url, title, favicon in
            self?.recordVisit(url: url, title: title, favicon: favicon)
            // Issue #425: the site's good load titles a favourite its host
            // stands in for — one pinned while it had failed, or one whose
            // stored name (an error page's) the core reset on hydration.
            self?.exploreCore.dispatch(Self.pageLoaded(url: url, title: title))
        }
        engine.onStateChanged = { [weak self] in self?.engineTick &+= 1 }
        engines[id] = engine
        #if DEBUG
        engineMadeForTesting?(engine)
        #endif
        return engine
    }

    /// The `page_loaded` event for a load the core's visit rule accepted
    /// (`browserLoadVisit` — never the engine's error page): its address and
    /// its title, `""` read as no title.
    static func pageLoaded(url: String, title: String) -> String {
        CoreJSON.string([
            "type": "page_loaded", "url": url,
            "title": title.isEmpty ? NSNull() : title as Any,
        ])
    }

    // MARK: - The history queue

    private func recordVisit(url: String, title: String, favicon: String) {
        let visit: [String: Any] = [
            "type": "visit_recorded",
            "url": url,
            "title": title.isEmpty ? NSNull() : title as Any,
            "favicon": favicon.isEmpty ? NSNull() : favicon as Any,
            "now_ms": now(),
        ]
        guard historyReady else {
            queuedVisits.append(visit)
            return
        }
        historyCore.dispatch(CoreJSON.string(visit))
    }

    private func flushVisits() {
        historyReady = true
        let queued = queuedVisits
        queuedVisits.removeAll()
        for visit in queued { historyCore.dispatch(CoreJSON.string(visit)) }
    }

    // MARK: - Waiting for the mirror

    private var readyWork: [() -> Void] = []

    /// Run now if the explore mirror is live, else when it becomes live.
    ///
    /// Every mutation dispatched before hydration is dropped by the core, so
    /// an intent that arrives from a deep link or a fast finger has to wait —
    /// the defect Android found when a deep link raced the document load and
    /// no page appeared.
    private func whenReady(_ work: @escaping () -> Void) {
        if explore.ready {
            work()
            return
        }
        readyWork.append(work)
    }

    private func flushReady() {
        guard explore.ready, !readyWork.isEmpty else { return }
        let queued = readyWork
        readyWork.removeAll()
        for work in queued { work() }
    }
}

#if DEBUG
extension BrowserController {
    /// Tests drive the REAL core through the REAL executor with a fake page:
    /// what WebKit would have reported for `tab`, without a web view.
    func pageMessageForTesting(tab: String, frameOrigin: String, isMainFrame: Bool = true, body: String) {
        dbrCore.dispatch(CoreJSON.string([
            "type": "page_message", "tab": tab, "frame_origin": frameOrigin,
            "is_main_frame": isMainFrame, "message_json": body, "now_ms": now(),
        ]))
    }

    func navigationForTesting(tab: String, url: String, finished: Bool) {
        dbrCore.dispatch(CoreJSON.string([
            "type": finished ? "load_finished" : "navigation_started", "tab": tab, "url": url,
            "now_ms": now(),
        ]))
    }

    func tabClosedForTesting(tab: String) {
        dbrCore.dispatch(CoreJSON.string(["type": "tab_closed", "tab": tab, "now_ms": now()]))
    }

    func rendererGoneForTesting(tab: String) {
        dbrCore.dispatch(CoreJSON.string(["type": "renderer_gone", "tab": tab, "now_ms": now()]))
    }

    /// The read deadline in place of the core's 30 s.
    func readDeadlineForTesting(_ ms: Double?) { dbrExecutor.deadlineForTesting = ms }

    /// The tabs whose engines were let go of.
    var suspendedForTesting: Set<String> { Set(suspended.keys) }

    /// The engine a tab has, if one was made.
    func engineForTesting(_ tab: String) -> BrowserEngine? { engines[tab] }

    /// Where `deliver` goes when there is no web view — a test's sink.
    func deliverForTesting(_ sink: @escaping (_ tab: String, _ messageJson: String) -> Void) {
        dbrExecutor.ports.deliver = sink
    }
}
#endif
