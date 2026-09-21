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
//  ## The history wait is not superstition
//
//  `browser_history` publishes no `ready` flag, and a visit recorded before
//  its store answers is dropped silently — `BrowserWireDriftTests` proves it.
//  So visits queue until `BhistExecutor` says the mirror is live.
//

import Foundation
import Observation
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
    /// Bumped whenever an engine's navigation state changes, so SwiftUI
    /// redraws chrome that reads a non-observable engine property.
    private(set) var engineTick = 0

    /// The wallet's networks, as last told to the core — the chains a site
    /// may be switched to from the connection panel.
    private(set) var chainIds: [Int] = []

    /// Visits recorded before the history store answered.
    private var queuedVisits: [[String: Any]] = []
    private var historyReady = false

    // MARK: - Ports the app fills in

    struct Ports {
        /// One call through the wallet's own pool — `["result": …]`,
        /// `["error": …]`, or `nil` when no endpoint answered.
        var poolCall: (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> [String: Any]?
        = { _, _, _, _ in nil }
        /// The transaction a user operation landed in (the relay's receipt).
        var resolveUserOp: (_ chainId: Int, _ userOpHash: String) async -> String? = { _, _ in nil }
        /// Open the signing sheet for a request. `nil` = there is no sheet to
        /// open, and the request is refused rather than left waiting.
        var onForwardToSigning: ((DbrForward) -> Void)?
        /// The page behind a sheet is gone: close that sheet, answer nothing.
        var onCancelSigning: (_ tab: String, _ id: String) -> Void = { _, _ in }
        /// The feed's store, for the "connected to" row.
        var writeRecords: ([[String: Any]]) -> Void = { _ in }
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
            onFault: { print("[vela-wallet] explore_sites fault: \($0)") }
        )
        historyCore = CoreStore(
            bridge: BrowserHistoryCore(),
            perform: { [historyExecutor] in await historyExecutor.perform($0) },
            onView: { [weak self] view in self?.history = view },
            onFault: { print("[vela-wallet] browser_history fault: \($0)") }
        )
        dbrCore = CoreStore(
            bridge: DappBrowserCore(),
            perform: { [dbrExecutor] in await dbrExecutor.perform($0) },
            onView: { [weak self] view in self?.dbr = view },
            onFault: { print("[vela-wallet] dapp_browser fault: \($0)") },
            neutralAnswer: { DbrExecutor.neutralAnswer($0) }
        )

        wirePorts()
    }

    private func wirePorts() {
        historyExecutor.onLoaded = { [weak self] in self?.flushVisits() }

        dbrExecutor.ports = DbrExecutor.Ports(
            deliver: { [weak self] tab, json in self?.engines[tab]?.deliver(json) },
            poolCall: { [weak self] chainId, method, params, bundler in
                await self?.ports.poolCall(chainId, method, params, bundler) ?? nil
            },
            resolveUserOp: { [weak self] chainId, hash in
                await self?.ports.resolveUserOp(chainId, hash) ?? nil
            },
            forwardToSigning: { [weak self] forward in self?.forwardToSigning(forward) },
            cancelSigning: { [weak self] tab, id in self?.ports.onCancelSigning(tab, id) },
            saveConnectionRecord: { [weak self] row in self?.ports.writeRecords([row]) }
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
        dbrCore.boot(CoreJSON.string(["type": "start"]))
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

    // MARK: - Tabs and navigation

    /// Open what somebody typed: a URL, a host, or a search.
    ///
    /// Through the core's `dappBrowserInput`, so all three browsers read the
    /// address bar the same way. Waits for the mirror: a mutation dispatched
    /// before hydration is dropped by the core, which is how a deep link used
    /// to open nothing at all.
    func open(_ text: String) {
        guard let url = dappBrowserInput(text: text) else { return }
        whenReady { [weak self] in
            guard let self else { return }
            if let selected = explore.selected {
                exploreCore.dispatch(CoreJSON.string([
                    "type": "tab_navigated", "id": selected.id, "url": url, "title": NSNull(),
                ]))
                // An engine already showing a page is told directly; a start
                // page gets its engine from `reconcile`, which loads the URL.
                engines[selected.id]?.load(url)
                return
            }
            exploreCore.dispatch(CoreJSON.string([
                "type": "tab_opened", "url": url, "title": NSNull(), "now_ms": now(),
            ]))
        }
    }

    /// Open a site the person tapped, by the id the tile or row carries.
    ///
    /// A favourite's `url` can be deeper than its origin, because that is
    /// where the person actually works. Recents carry the exact URL for the
    /// same reason.
    func openSite(id: String) {
        if let pinned = explore.favorites.first(where: { $0.origin == id }) {
            open(pinned.url)
            return
        }
        if let visited = history.entries.first(where: { $0.origin == id }) {
            open(visited.url)
            return
        }
        open(id)
    }

    func newTab() {
        whenReady { [weak self] in
            guard let self else { return }
            exploreCore.dispatch(CoreJSON.string([
                "type": "tab_opened", "url": NSNull(), "title": NSNull(), "now_ms": now(),
            ]))
        }
    }

    func selectTab(_ id: String) {
        guard explore.selectedTab != id else { return }
        exploreCore.dispatch(CoreJSON.string(["type": "tab_selected", "id": id]))
    }

    func closeTab(_ id: String) {
        exploreCore.dispatch(CoreJSON.string(["type": "tab_closed", "id": id]))
    }

    func closeAllTabs() {
        for tab in explore.tabs {
            exploreCore.dispatch(CoreJSON.string(["type": "tab_closed", "id": tab.id]))
        }
    }

    func goBack() { current?.goBack() }
    func goForward() { current?.goForward() }
    func reload() { current?.reload() }

    /// The core's facts about the tab in front.
    var currentTab: DbrTabViewWire? { dbr.tab(explore.selectedTab) }

    // MARK: - Favourites and groups

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
    func toggleFavorite() {
        guard let engine = current, !engine.url.isEmpty, !engine.origin.isEmpty else { return }
        if explore.favorites.contains(where: { $0.origin == engine.origin }) {
            removeFavorite(origin: engine.origin)
        } else {
            addFavorite(url: engine.url, title: engine.title.isEmpty ? nil : engine.title)
        }
    }

    func renameFavorite(origin: String, name: String) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "favorite_renamed", "origin": origin, "name": name,
        ]))
    }

    func createGroup(name: String) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "group_created", "name": name, "now_ms": now(),
        ]))
    }

    func deleteGroup(id: String) {
        exploreCore.dispatch(CoreJSON.string(["type": "group_deleted", "id": id]))
    }

    func setGroupHidden(id: String, hidden: Bool) {
        exploreCore.dispatch(CoreJSON.string([
            "type": "group_hidden_set", "id": id, "hidden": hidden,
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
        dbrCore.dispatch(CoreJSON.string(["type": "consent_rejected"]))
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
        ]))
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
        flushReady()
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
            dbrCore.dispatch(CoreJSON.string(["type": "tab_closed", "tab": id]))
        }

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
        let engine = makeEngine(id: selected.id)
        current = engine
        engine.load(url)
    }

    private func makeEngine(id: String) -> BrowserEngine {
        let engine = BrowserEngine(id: id)
        engine.onPageMessage = { [weak self] frameOrigin, isMainFrame, body in
            self?.dbrCore.dispatch(CoreJSON.string([
                "type": "page_message",
                "tab": id,
                "frame_origin": frameOrigin,
                "is_main_frame": isMainFrame,
                "message_json": body,
            ]))
        }
        engine.onNavigationStarted = { [weak self] url in
            self?.dbrCore.dispatch(CoreJSON.string([
                "type": "navigation_started", "tab": id, "url": url,
            ]))
        }
        engine.onLoadFinished = { [weak self] url in
            self?.dbrCore.dispatch(CoreJSON.string([
                "type": "load_finished", "tab": id, "url": url,
            ]))
        }
        engine.onRendererGone = { [weak self] in
            self?.dbrCore.dispatch(CoreJSON.string(["type": "renderer_gone", "tab": id]))
        }
        engine.onMeta = { [weak self] url, title in
            self?.exploreCore.dispatch(CoreJSON.string([
                "type": "tab_navigated", "id": id, "url": url,
                "title": title.isEmpty ? NSNull() : title as Any,
            ]))
        }
        engine.onVisited = { [weak self] url, title, favicon in
            self?.recordVisit(url: url, title: title, favicon: favicon)
        }
        engine.onStateChanged = { [weak self] in self?.engineTick &+= 1 }
        engines[id] = engine
        return engine
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
            "is_main_frame": isMainFrame, "message_json": body,
        ]))
    }

    func navigationForTesting(tab: String, url: String, finished: Bool) {
        dbrCore.dispatch(CoreJSON.string([
            "type": finished ? "load_finished" : "navigation_started", "tab": tab, "url": url,
        ]))
    }

    func tabClosedForTesting(tab: String) {
        dbrCore.dispatch(CoreJSON.string(["type": "tab_closed", "tab": tab]))
    }

    func rendererGoneForTesting(tab: String) {
        dbrCore.dispatch(CoreJSON.string(["type": "renderer_gone", "tab": tab]))
    }

    /// Where `deliver` goes when there is no web view — a test's sink.
    func deliverForTesting(_ sink: @escaping (_ tab: String, _ messageJson: String) -> Void) {
        dbrExecutor.ports.deliver = sink
    }
}
#endif
