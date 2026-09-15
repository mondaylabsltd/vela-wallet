//
//  ManageTokensStore.swift
//  VelaWallet
//
//  The `manage_tokens` machine, and the one interaction decision this client
//  makes differently from the web panel.
//
//  ## Why the search happens by itself
//
//  Web's panel has a "Search Token" button next to the address field. The drawn
//  iOS sheet (T3) has **one** CTA — 添加到钱包 — so the search cannot be a
//  button here without inventing one. It runs when the address becomes valid,
//  once per address: the core still owns validity, still owns which chains
//  answered and still owns admission; what moved is only *when the shell asks*.
//
//  That is a real deviation from the TypeScript and it is recorded here rather
//  than smoothed over. The corpus already carries 正在搜索所有网络… for exactly
//  this state, which is what the drawing expects to show while it runs.
//
//  ## The registry is the shell's
//
//  `DetectRequested` carries the network list because the core has no registry:
//  the twelve built-ins plus whatever the person added, which is the same list
//  the balance read sweeps.
//

import Foundation
import Observation
import VelaCore

extension ManageTokensCore: CoreBridge {}

@MainActor
@Observable
final class ManageTokensStore {

    private(set) var view: MtokViewWire?

    private let store: VelaStore
    private let executor: ManageTokensExecutor
    private var core: CoreStore<MtokViewWire>!
    /// The address the automatic search has already been run for, so typing
    /// one more character does not re-sweep twelve chains.
    private var detectedFor: String?

    init(store: VelaStore, pool: RpcPool, onInvalidate: @escaping () -> Void = {}) {
        self.store = store
        self.executor = ManageTokensExecutor(store: store, pool: pool,
                                             onInvalidate: onInvalidate)
        self.core = CoreStore(
            bridge: ManageTokensCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view },
            onFault: { print("[vela-wallet] manage_tokens fault: \($0)") }
        )
    }

    /// The panel opened — load what is already added.
    func open() {
        core.boot(CoreJSON.string(["type": "start"]))
    }

    /// Every keystroke, and a scanned address after the shell extracts it.
    func input(_ text: String) {
        detectedFor = nil
        core.dispatch(CoreJSON.string(["type": "address_input", "s": text]))
        // The core has re-judged validity by now (its render is synchronous),
        // so this reads the answer rather than guessing at it.
        detectIfReady()
    }

    /// Sweep every network for the address, once.
    private func detectIfReady() {
        guard let view, view.addressValid, !view.detecting,
              detectedFor != view.inputAddress
        else { return }
        detectedFor = view.inputAddress
        core.dispatch(CoreJSON.string([
            "type": "detect_requested", "networks": networks(),
        ]))
    }

    /// 添加到钱包 on a found card.
    func save(chainId: Int) {
        core.dispatch(CoreJSON.string(["type": "save_requested", "chain_id": chainId]))
    }

    /// The trash button on an already-added row. Drawn nowhere yet — the sheet
    /// has no manage list — but the machine's, and reachable the moment one is
    /// drawn.
    func delete(id: String) {
        core.dispatch(CoreJSON.string(["type": "delete_requested", "id": id]))
    }

    /// The twelve built-ins plus the person's own networks, in registry order —
    /// `getAllNetworksSync()`'s answer, which is shell domain.
    private func networks() -> [[String: Any]] {
        var rows = ChainCatalog.chains.map { chain in
            ["chain_id": chain.chainId, "name": chain.displayName] as [String: Any]
        }
        var seen = Set(ChainCatalog.chains.map(\.chainId))
        for network in store.readList(VelaStore.Key.customNetworks) {
            guard let chainId = (network["chainId"] as? NSNumber)?.intValue,
                  seen.insert(chainId).inserted
            else { continue }
            rows.append([
                "chain_id": chainId,
                "name": network["displayName"] as? String ?? "\(chainId)",
            ])
        }
        return rows
    }
}
