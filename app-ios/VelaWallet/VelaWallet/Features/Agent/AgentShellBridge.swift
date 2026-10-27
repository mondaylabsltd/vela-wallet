//
//  AgentShellBridge.swift
//  VelaWallet
//
//  What this app does for its agent (`AgentShell`, spec 104 phase 7): its
//  views, its stores' calls, its screens, its send.
//
//  Every call comes from the agent's own thread and blocks it while the work
//  hops to the main actor, where the stores live — except the ones made under
//  `AgentStore.reviewing`, which the main actor makes itself and are answered
//  where they are. `perform` waits on its own: an action can take a while (a
//  network checked, a token looked up), and it runs on the agent's blocking
//  pool, not on the thread the connection lives on.
//

import Foundation
import VelaCore

nonisolated final class AgentShellBridge: AgentShell, @unchecked Sendable {
    /// Weak: the store owns the core, which owns this.
    weak var store: AgentStore?

    // MARK: - The wallet, read

    func views() -> String {
        onMain { store?.graph.map(Self.views) ?? "{}" }
    }

    /// The views as JSON, under the names the core's `World` reads — the
    /// machines' own views, whole. While nobody is signed in only the
    /// session: the other machines belong to an account.
    @MainActor
    static func views(_ graph: AppGraph) -> String {
        var parts = ["\"session\":\(graph.session.viewJson ?? "null")"]
        if !graph.session.view.address.isEmpty {
            parts.append("\"contacts\":\(graph.contacts.viewJson ?? "null")")
            parts.append("\"balance\":\(graph.wallet.viewJson ?? "null")")
            parts.append("\"feed\":\(graph.activity.viewJson ?? "null")")
            // The ledger has not read its stores yet: nothing to judge by.
            if graph.settings.networkAdmin?.loaded == true, let networks = graph.settings.networksJson {
                parts.append("\"networks\":\(networks)")
            }
            parts.append("\"currency\":\(graph.settings.currencyJson ?? "null")")
            parts.append("\"fee_tier\":\(graph.settings.feeTierJson ?? "null")")
        }
        parts.append("\"preferences\":\(CoreJSON.string(preferences(graph.preferences)))")
        // The space in front (spec 105): the agent reads and drafts in it.
        parts.append("\"network_space\":\"\(graph.space.current.rawValue)\"")
        return "{\(parts.joined(separator: ","))}"
    }

    /// The preferences in the catalog's spellings. The phone shares no usage
    /// statistics, so `analytics` is always off.
    @MainActor
    static func preferences(_ prefs: Preferences) -> [String: Any] {
        [
            "theme": prefs.theme.rawValue,
            "language": prefs.language,
            "text_scale": prefs.textScale.rawValue,
            "number_format": prefs.numberFormat.rawValue,
            "date_format": prefs.dateFormat.rawValue,
            "time_format": prefs.timeFormat.rawValue,
            "analytics": false,
        ]
    }

    func page() -> String {
        onMain { store?.page?.name() ?? "welcome" }
    }

    func signing() -> String? {
        onMain { store?.page?.signing() }
    }

    func changed() {
        // Never inline: the core may say so from inside a call the main actor
        // is making.
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.store?.refresh() }
        }
    }

    // MARK: - Doing

    func perform(action: String) -> String {
        guard !Thread.isMainThread else {
            return AgentPerform.refused("not_ready", "the wallet could not do it from here")
        }
        let answer = Answer()
        let done = DispatchSemaphore(value: 0)
        Task { @MainActor [weak self] in
            if let store = self?.store, let graph = store.graph {
                answer.value = await AgentPerform(graph: graph, agent: store, page: store.page).perform(action)
            } else {
                answer.value = AgentPerform.refused("not_ready", "the wallet is closing")
            }
            done.signal()
        }
        done.wait()
        return answer.value
    }

    func reveal(screen: String) {
        onMain {
            guard let screen = Self.object(screen) else { return }
            _ = store?.page?.show(screen)
        }
    }

    // MARK: - The send

    func sendOpen() -> AgentSendOpen? {
        onMain {
            guard let send = store?.graph?.send, send.inJourney, let view = send.viewJson else { return nil }
            return AgentSendOpen(session: UInt64(send.journey), view: view)
        }
    }

    func sendAlert() -> String? {
        onMain {
            guard let alert = store?.graph?.send.alert else { return nil }
            return alert["type"] as? String ?? "alert"
        }
    }

    func sendStart(params: String) -> UInt64? {
        guard let params = Self.object(params) else { return nil }
        let before: Int? = onMain {
            guard let store, let send = store.graph?.send, let page = store.page else { return nil }
            let before = send.journey
            page.openSend(params)
            return before
        }
        guard let before else { return nil }
        // Every entry into Send reads the account record first: the journey
        // starts a moment later.
        return wait(ms: 5_000) {
            guard let send = store?.graph?.send, send.inJourney, send.journey != before else { return nil }
            return UInt64(send.journey)
        }
    }

    func sendDispatch(event: String) {
        onMain {
            guard let event = Self.object(event) else { return }
            store?.graph?.send.dispatch(event)
        }
    }

    func sendClose() {
        // What is in flight first — a Done already sent closes by itself, and
        // a second one would land on the next journey.
        _ = wait(ms: 1_500) { (store?.graph?.send.isIdle ?? true) ? true : nil }
        onMain {
            guard let send = store?.graph?.send, send.inJourney else { return }
            send.done()
        }
        // Gone with its screens, so the next send opens on a clean stack.
        _ = wait(ms: 3_000) {
            guard let store, let send = store.graph?.send else { return true }
            return !send.inJourney && !(store.page?.sendShowing() ?? false) ? true : nil
        }
    }

    func account() -> String {
        onMain { store?.graph?.session.view.address ?? "" }
    }

    func balancesSettled(account: String) -> Bool {
        onMain { store?.graph?.wallet.hasSettled(account) ?? false }
    }

    func holdings() -> String {
        onMain { Self.json(CoreFacts(json: store?.graph?.wallet.viewJson)["tokens"].raw) }
    }

    func tracker() -> String {
        onMain { Self.json(CoreFacts(json: store?.graph?.tracker.viewJson)["entries"].raw) }
    }

    func networkId(chainId: UInt32) -> String {
        // `SendExecutor.sendTokens`' own spelling of a token's network.
        "chain-\(chainId)"
    }

    // MARK: -

    private static func object(_ text: String) -> [String: Any]? {
        text.data(using: .utf8).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
    }

    private static func json(_ list: Any?) -> String {
        guard let list = list as? [Any],
              let data = try? JSONSerialization.data(withJSONObject: list),
              let text = String(data: data, encoding: .utf8)
        else { return "[]" }
        return text
    }

    /// Run `body` on the main actor and answer with it: inline when already
    /// there, else hopping and waiting.
    private func onMain<T: Sendable>(_ body: @MainActor () -> T) -> T {
        if Thread.isMainThread { return MainActor.assumeIsolated { body() } }
        return DispatchQueue.main.sync { MainActor.assumeIsolated { body() } }
    }

    /// Look on the main actor until `read` answers, or `ms` pass. Off the
    /// main thread only.
    private func wait<T: Sendable>(ms: Int, _ read: @MainActor () -> T?) -> T? {
        var waited = 0
        while true {
            if let value = onMain(read) { return value }
            if waited >= ms || Thread.isMainThread { return nil }
            Thread.sleep(forTimeInterval: 0.05)
            waited += 50
        }
    }

    /// A value handed back from the main actor to the thread waiting for it.
    private final class Answer: @unchecked Sendable {
        var value = ""
    }
}
