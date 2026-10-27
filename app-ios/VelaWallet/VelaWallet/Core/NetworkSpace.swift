//
//  NetworkSpace.swift
//  VelaWallet
//
//  The mainnet space and the testnet space (spec 105).
//
//  One device-wide switch: which networks the whole wallet shows. The mainnet
//  space holds networks whose assets have real value; the testnet space,
//  testnets and developer networks with nothing of real value. Every list a
//  person sees — balances and the total, assets, activity, receive, send's
//  networks, the chains a dApp can use — is the space's, never the whole set.
//
//  Which space a stored value names is the core's to say (`networkSpaceRead`).
//  Which space a network belongs to is the built-in table's word (the
//  catalogue's `testnet`, pinned to the core's by `ChainCatalogTests`), else
//  the person's for a network they added (`isTestnet`), else — a chain nobody
//  vouched for — the testnet space, the core's `chain_is_testnet`.
//

import Foundation
import Observation
import VelaCore

enum NetworkSpaceKind: String {
    case mainnet, testnet

    var other: NetworkSpaceKind { self == .mainnet ? .testnet : .mainnet }
}

@Observable
final class NetworkSpace {

    private(set) var current: NetworkSpaceKind
    /// The switch prompt is up (the header label, Settings → Networks).
    var switcherOpen = false

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
        current = Self.read(store)
    }

    var testnet: Bool { current == .testnet }

    /// The person's switch (through the `set_network_space` intent), or the
    /// agent's once the person allowed it.
    func set(_ space: NetworkSpaceKind) {
        store.writeString(VelaStore.Key.networkSpace, space.rawValue)
        guard space != current else { return }
        current = space
    }

    /// The stored space, as the core reads it: anything unreadable is mainnet.
    static func read(_ store: VelaStore) -> NetworkSpaceKind {
        NetworkSpaceKind(rawValue: networkSpaceRead(stored: store.readString(VelaStore.Key.networkSpace)))
            ?? .mainnet
    }

    /// Whether `chainId` belongs to the testnet space.
    static func isTestnet(_ chainId: Int, customs: [[String: Any]]) -> Bool {
        if let builtin = ChainCatalog.meta(chainId) { return builtin.testnet }
        let custom = customs.first { ($0["chainId"] as? NSNumber)?.intValue == chainId }
        return custom?["isTestnet"] as? Bool ?? true
    }

    /// The chains of the space in front: the built-ins, then the person's
    /// own, those of the space alone — what the executors read balances on
    /// and watch for receipts. Read from the store each time, so a switch
    /// holds for the next read without anyone telling the executor.
    static func chainIds(store: VelaStore) -> [Int] {
        let testnet = read(store) == .testnet
        let customs = store.readList(VelaStore.Key.customNetworks)
        var ids = ChainCatalog.chains.filter { $0.testnet == testnet }.map(\.chainId)
        for network in customs {
            guard let chainId = (network["chainId"] as? NSNumber)?.intValue,
                  ChainCatalog.meta(chainId) == nil,
                  !ids.contains(chainId),
                  isTestnet(chainId, customs: customs) == testnet
            else { continue }
            ids.append(chainId)
        }
        return ids
    }
}
