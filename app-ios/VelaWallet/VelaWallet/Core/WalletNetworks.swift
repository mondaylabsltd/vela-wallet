//
//  WalletNetworks.swift
//  VelaWallet
//
//  The networks this wallet has, as the core lists them: `network_admin`'s
//  rows, the built-ins in the core's order and then the person's own.
//
//  The receive list, the chain filters and a holding's chain name were built
//  from `ChainCatalog.chains` — the 24 built-ins — so a network the person
//  added had no receive row, no filter row and a blank name beside its
//  tokens. Android, the web and the desktop list from the core's networks;
//  this is that list on iOS.
//
//  Until the networks machine has read its stores (an empty list) the shipped
//  catalogue stands in, so a cold screen still lists every built-in.
//

import Foundation

struct WalletNetworks {

    /// In the core's order. A built-in is the catalogue's own entry; a
    /// network the person added is one made from its row — its id, name,
    /// coin and URLs. That entry is for NAMING the network (receive, filters,
    /// marks); routing reads the core's rows, never this.
    let chains: [ChainMeta]

    /// The shipped catalogue alone — what every list shows before the
    /// networks machine has answered, and what the tests build with.
    static let builtin = WalletNetworks(chains: ChainCatalog.chains)

    init(chains: [ChainMeta]) {
        self.chains = chains
    }

    /// `network_admin`'s rows. Nothing yet → the catalogue.
    init(_ rows: [NetNetworkRowWire]?) {
        guard let rows, !rows.isEmpty else {
            self = .builtin
            return
        }
        chains = rows.map { row in
            if !row.isCustom, let shipped = ChainCatalog.meta(row.chainId) { return shipped }
            return ChainMeta(
                id: row.id,
                displayName: row.displayName,
                chainId: row.chainId,
                apiNetworkId: "",
                nativeSymbol: row.nativeSymbol,
                isL2: false,
                rpcURL: row.rpcUrl,
                explorerURL: row.explorerUrl,
                gasModel: .native
            )
        }
    }

    /// The network with this id: the list's, else the catalogue's (a
    /// built-in is always nameable, even from a list that has not loaded).
    func meta(_ chainId: Int) -> ChainMeta? {
        chains.first { $0.chainId == chainId } ?? ChainCatalog.meta(chainId)
    }
}
