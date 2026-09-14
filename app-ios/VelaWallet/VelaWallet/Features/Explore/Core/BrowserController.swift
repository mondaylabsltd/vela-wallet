//
//  BrowserController.swift
//  VelaWallet
//
//  The browser's owner: three app-resident machines, one engine per open tab,
//  and the two maps that make an answer reach the page that asked.
//
//  Ported from `app-android/.../feature/browser/core/BrowserController.kt`
//  (spec 044). App-resident rather than screen-owned for the reason every
//  wallet-state machine here is: a request in flight must survive a sheet,
//  a tab switch and a trip to 设置, and a page that reloaded every time
//  somebody glanced at their balance would lose a half-finished swap.
//
//  ## Two delivery rules, and they are not the same rule
//
//  `answer` goes to the tab that **asked** (`requestTab`). `emit` goes to the
//  tab **in front of the person**. A page that is not visible is not told about
//  an account switch it never asked about, and an answer must never land in a
//  tab that did not ask the question.
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
extension DappPermissionsCore: CoreBridge {}

@MainActor
@Observable
final class BrowserController {

    // MARK: - The three machines

    private(set) var explore: ExploreViewWire = .empty
    private(set) var history: BhistViewWire = .empty
    private(set) var permissions: DpermViewWire = .empty

    private var exploreCore: CoreStore<ExploreViewWire>!
    private var historyCore: CoreStore<BhistViewWire>!
    private var permissionsCore: CoreStore<DpermViewWire>!

    private let exploreExecutor: ExploreExecutor
    private let historyExecutor: BhistExecutor
    private let browserExecutor: BrowserExecutor
    private let router: RequestRouter

    // MARK: - The engines

    /// One per open tab, keyed by the core's tab id.
    private var engines: [String: BrowserEngine] = [:]
    /// The engine in front of the person, if a tab with a page is selected.
    private(set) var current: BrowserEngine?
    /// Bumped whenever an engine's navigation state changes, so SwiftUI
    /// redraws chrome that reads a non-observable engine property.
    private(set) var engineTick = 0

    // MARK: - Routing

    /// Request id → the tab that asked. An answer goes there and nowhere else.
    private var requestTab: [String: String] = [:]
    /// Forwarded requests not yet answered. A navigation settles them all.
    private var openIds: [String] = []
    /// Visits recorded before the history store answered.
    private var queuedVisits: [[String: Any]] = []
    private var historyReady = false

    /// The chain the browser is on. Gnosis until somebody says otherwise —
    /// the chain this wallet's relay actually sponsors.
    private(set) var browserChain = 100

    // MARK: - Ports the app fills in

    struct Ports {
        var knownChains: () -> [Int] = { [] }
        var poolCall: (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> [String: Any]?
        = { _, _, _, _ in nil }
        /// A signing request, on its way to the per-request controller.
        var onSignRequest: (SignRequest) -> Void = { _ in }
        /// `nil` when the hash is not one this wallet minted.
        var receiptFor: (_ userOpHash: String) async -> RequestRouter.Receipt? = { _ in nil }
        /// The feed's store, for the "connected to" row.
        var writeRecords: ([[String: Any]]) -> Void = { _ in }
        /// The wallet moved to another chain because a page asked.
        var onChainSwitched: (Int) -> Void = { _ in }
    }

    struct SignRequest {
        let id: String
        let method: String
        let paramsJson: String
        let origin: String
        let transportId: String
        let chainId: Int
    }

    var ports: Ports

    private let bundle: Bundle
    private let now: () -> Double

    init(
        store: VelaStore,
        ports: Ports = Ports(),
        bundle: Bundle = .main,
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }
    ) {
        self.ports = ports
        self.bundle = bundle
        self.now = now
        self.exploreExecutor = ExploreExecutor(store: store)
        self.historyExecutor = BhistExecutor(store: store)
        self.browserExecutor = BrowserExecutor(store: store)
        self.router = RequestRouter()

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
        permissionsCore = CoreStore(
            bridge: DappPermissionsCore(),
            perform: { [browserExecutor] in await browserExecutor.perform($0) },
            onView: { [weak self] view in self?.permissions = view },
            onFault: { print("[vela-wallet] dapp_permissions fault: \($0)") }
        )

        wirePorts()
    }

    private func wirePorts() {
        historyExecutor.onLoaded = { [weak self] in self?.flushVisits() }

        browserExecutor.ports = BrowserExecutor.Ports(
            respond: { [weak self] id, json in self?.answer(id: id, json: json) },
            emit: { [weak self] json in self?.emit(json) },
            settleForwarded: { [weak self] code, message in
                self?.settleForwarded(code: code, message: message)
            },
            saveConnectionRecord: { [weak self] row in self?.ports.writeRecords([row]) },
            forward: { [weak self] id, method, paramsJson, origin in
                self?.forward(id: id, method: method, paramsJson: paramsJson, origin: origin)
            }
        )

        router.ports = RequestRouter.Ports(
            browserChain: { [weak self] in self?.browserChain ?? 100 },
            knownChains: { [weak self] in self?.ports.knownChains() ?? [] },
            switchChain: { [weak self] chainId in self?.switchChain(to: chainId) },
            poolCall: { [weak self] chainId, method, params, bundler in
                await self?.ports.poolCall(chainId, method, params, bundler) ?? nil
            },
            respond: { [weak self] id, json in self?.answer(id: id, json: json) },
            sign: { [weak self] id, method, paramsJson, origin in
                self?.raiseSigning(id: id, method: method, paramsJson: paramsJson, origin: origin)
            },
            receiptFor: { [weak self] hash in await self?.ports.receiptFor(hash) ?? nil }
        )
    }

    // MARK: - Lifecycle

    /// Boot the three machines. Idempotent.
    func start() {
        exploreCore.boot(CoreJSON.string(["type": "start"]))
        historyCore.boot(CoreJSON.string(["type": "start"]))
        permissionsCore.boot(CoreJSON.string(["type": "chain_changed", "chain_id": browserChain]))
    }

    /// The wallet's accounts, in the order the permissions machine needs them.
    ///
    /// `accounts_updated` → `account_switched` → `chain_changed`, and not
    /// because it reads nicely: a grant is pinned to an address, so the
    /// machine has to know the whole set before it can judge one, and it has
    /// to know the active one before it can re-pin.
    func accountsChanged(addresses: [String], active: String?) {
        permissionsCore.dispatch(CoreJSON.string([
            "type": "accounts_updated",
            "addresses": addresses.isEmpty ? NSNull() : addresses as Any,
        ]))
        if let active, !active.isEmpty {
            permissionsCore.dispatch(CoreJSON.string([
                "type": "account_switched", "address": active, "now_ms": now(),
            ]))
        }
        permissionsCore.dispatch(CoreJSON.string([
            "type": "chain_changed", "chain_id": browserChain,
        ]))
    }

    func switchChain(to chainId: Int) {
        guard chainId != browserChain else { return }
        browserChain = chainId
        permissionsCore.dispatch(CoreJSON.string(["type": "chain_changed", "chain_id": chainId]))
        ports.onChainSwitched(chainId)
    }

    /// The browser screen went away.
    func close() {
        permissionsCore.dispatch(CoreJSON.string(["type": "browser_closed"]))
    }

    // MARK: - Tabs and navigation

    /// Open a URL or a typed host. Waits for the mirror: a mutation dispatched
    /// before hydration is dropped by the core, which is how a deep link used
    /// to open nothing at all.
    func open(_ text: String) {
        let url = BrowserEngine.coerce(text)
        guard !url.isEmpty else { return }
        whenReady { [weak self] in
            guard let self else { return }
            if let selected = explore.selected {
                if engines[selected.id] != nil || selected.url != nil {
                    exploreCore.dispatch(CoreJSON.string([
                        "type": "tab_navigated", "id": selected.id, "url": url, "title": NSNull(),
                    ]))
                    engines[selected.id]?.load(url)
                    return
                }
                exploreCore.dispatch(CoreJSON.string([
                    "type": "tab_navigated", "id": selected.id, "url": url, "title": NSNull(),
                ]))
                return
            }
            exploreCore.dispatch(CoreJSON.string([
                "type": "tab_opened", "url": url, "title": NSNull(), "now_ms": now(),
            ]))
        }
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
        permissionsCore.dispatch(CoreJSON.string(["type": "consent_approved", "now_ms": now()]))
    }

    func consentRejected() {
        permissionsCore.dispatch(CoreJSON.string(["type": "consent_rejected"]))
    }

    /// `nil` revokes the origin in front of the person — the browser chip.
    /// A named origin revokes silently, because the page for it is not here.
    func revoke(origin: String? = nil) {
        permissionsCore.dispatch(CoreJSON.string([
            "type": "revoke_requested", "origin": origin.map { $0 as Any } ?? NSNull(),
        ]))
    }

    // MARK: - The request path

    private func forward(id: String, method: String, paramsJson: String, origin: String) {
        openIds.append(id)
        Task { [weak self] in
            await self?.router.route(id: id, method: method, paramsJson: paramsJson, origin: origin)
        }
    }

    private func raiseSigning(id: String, method: String, paramsJson: String, origin: String) {
        ports.onSignRequest(SignRequest(
            id: id,
            method: method,
            paramsJson: paramsJson,
            origin: origin,
            transportId: requestTab[id] ?? current?.id ?? "",
            chainId: browserChain
        ))
    }

    /// Deliver an answer to the tab that asked, falling back to the tab in
    /// front. A page whose tab closed mid-flight simply does not hear.
    func answer(id: String, json: [String: Any]) {
        openIds.removeAll { $0 == id }
        let engine = requestTab.removeValue(forKey: id).flatMap { engines[$0] } ?? current
        guard let engine else { return }
        ProviderBridge.deliver(json, to: engine.webView)
    }

    /// The signing controller's answers come back through here so they reach
    /// the page's own tab even after the sheet has taken the foreground.
    func answerFromSigning(transportId: String, id: String, json: [String: Any]) {
        openIds.removeAll { $0 == id }
        requestTab.removeValue(forKey: id)
        guard let engine = engines[transportId] ?? current else { return }
        ProviderBridge.deliver(json, to: engine.webView)
    }

    private func emit(_ json: [String: Any]) {
        guard let engine = current else { return }
        ProviderBridge.deliver(json, to: engine.webView)
    }

    private func settleForwarded(code: Int, message: String) {
        let pending = openIds
        openIds.removeAll()
        for id in pending {
            answer(id: id, json: BrowserExecutor.errorJson(id: id, code: code, message: message))
        }
    }

    // MARK: - Engines

    private func commitExplore(_ view: ExploreViewWire) {
        explore = view
        reconcile(view)
        flushReady()
    }

    /// Make the engines match the tabs.
    private func reconcile(_ view: ExploreViewWire) {
        let live = Set(view.tabs.map(\.id))
        for id in engines.keys where !live.contains(id) {
            engines.removeValue(forKey: id)
            if current?.id == id { current = nil }
        }

        guard let selected = view.selected else {
            current = nil
            return
        }
        guard let url = selected.url, !url.isEmpty else {
            // The start page's own tab: a tab with no site is not a page.
            current = nil
            return
        }

        let engine = engines[selected.id] ?? makeEngine(id: selected.id)
        if current !== engine {
            current = engine
            // The machine may have been born after the page loaded, and then
            // it knows no origin at all. Telling it again costs a dispatch and
            // saves a connect sheet that opens against nothing.
            if !engine.origin.isEmpty {
                permissionsCore.dispatch(CoreJSON.string([
                    "type": "navigation_started", "url": engine.url,
                ]))
            }
        }
        if engine.url != url, engine.webView.url?.absoluteString != url {
            engine.load(url)
        }
    }

    private func makeEngine(id: String) -> BrowserEngine {
        let engine = BrowserEngine(id: id, bundle: bundle)
        engine.onIncoming = { [weak self] incoming in self?.incoming(tabId: id, incoming) }
        engine.onNavigationStarted = { [weak self] url in
            self?.permissionsCore.dispatch(CoreJSON.string([
                "type": "navigation_started", "url": url,
            ]))
        }
        engine.onMeta = { [weak self] url, title, favicon in
            self?.recordVisit(url: url, title: title, favicon: favicon)
            guard let self, let tab = explore.selected, tab.id == id else { return }
            exploreCore.dispatch(CoreJSON.string([
                "type": "tab_navigated", "id": id, "url": url,
                "title": title.isEmpty ? NSNull() : title as Any,
            ]))
        }
        engine.onStateChanged = { [weak self] in self?.engineTick &+= 1 }
        engines[id] = engine
        return engine
    }

    private func incoming(tabId: String, _ incoming: ProviderIncoming) {
        requestTab[incoming.id] = tabId
        permissionsCore.dispatch(CoreJSON.string([
            "type": "provider_request",
            "id": incoming.id,
            "method": incoming.method,
            "params_json": incoming.paramsJson,
            // The WEB VIEW's URL through the core's own rule — never the
            // envelope's claim about itself.
            "origin": ProviderBridge.origin(of: incoming.url),
            // True by construction: the bridge posts only from the top frame.
            "is_main_frame": true,
        ]))
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
