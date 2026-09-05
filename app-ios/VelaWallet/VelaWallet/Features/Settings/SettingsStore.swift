//
//  SettingsStore.swift
//  VelaWallet
//
//  The `network_admin` machine, app-resident.
//
//  Residency matters more here than anywhere else in this cut. This machine
//  probes endpoints over the network and holds a search debounce; one that died
//  with the settings route would re-probe every chain each time somebody opened
//  设置, and a debounce armed against a dead core is an effect nobody will
//  answer.
//
//  It owns no rules. Which chains exist, whether one may be added, what makes an
//  RPC compatible, when an edit is held back — all of it is decided and tested
//  in Rust. This file is a lifetime, a decoded view, and the events the drawn
//  controls can raise.
//

import Foundation
import Observation
import VelaCore

/// uniffi generates one class per exported machine with no shared supertype.
/// Declared beside its user so adding a machine touches no shared file.
extension NetworkAdminCore: CoreBridge {}

@MainActor
@Observable
final class SettingsStore {

    /// The core's view, decoded. `nil` until the settings route boots the
    /// machine — a real state, and the one the fixture-shaped neutral surface
    /// renders from.
    private(set) var networkAdmin: NetViewWire?

    /// `true` once the core has read all four stores. Mutations sent before it
    /// are dropped by the core.
    var isLoaded: Bool { networkAdmin?.loaded == true }

    private let executor: NetworkAdminExecutor
    private var core: CoreStore<NetViewWire>!

    init(store: VelaStore, accounts: AccountStore) {
        self.executor = NetworkAdminExecutor(store: store, accounts: accounts)
        self.core = CoreStore(
            bridge: NetworkAdminCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.networkAdmin = view }
        )
    }

    /// Called from the settings route's `.task`. Idempotent: the machine reads
    /// its stores once and keeps them.
    func open() {
        core.boot(CoreJSON.string(["type": "started"]))
    }

    // MARK: - What the drawn controls raise

    /// The wizard's search box.
    func search(_ query: String) {
        core.dispatch(CoreJSON.string(["type": "search_input", "query": query]))
    }

    /// A row of the search results.
    func selectChain(_ chainId: Int, keepCustomRpc: Bool = false) {
        core.dispatch(CoreJSON.string([
            "type": "chain_selected",
            "chain_id": chainId,
            "keep_custom_rpc": keepCustomRpc,
        ]))
    }

    /// The wizard's RPC-override field.
    func editCustomRpc(_ value: String) {
        core.dispatch(CoreJSON.string(["type": "custom_rpc_edited", "value": value]))
    }

    /// 添加.
    ///
    /// `now_iso` is the shell's clock because the core has none; it dates the
    /// stored record. The core still refuses a candidate it never verified, so
    /// this cannot add a chain past a failed probe.
    func confirmAdd() {
        core.dispatch(CoreJSON.string(["type": "add_confirmed", "now_iso": Self.nowISO]))
    }

    func resetWizard() {
        core.dispatch(CoreJSON.string(["type": "wizard_reset"]))
    }

    func deleteNetwork(id: String) {
        core.dispatch(CoreJSON.string(["type": "delete_confirmed", "id": id]))
    }

    // MARK: - Lookups the screens need

    func network(id: String) -> NetNetworkRowWire? {
        networkAdmin?.networks.first { $0.id == id }
    }

    /// ISO-8601 with a `Z`, which is the shape the stored `addedAt` has carried
    /// since the Expo client wrote it.
    private static var nowISO: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: Date())
    }
}
