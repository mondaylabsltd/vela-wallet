//
//  ExplorerLinks.swift
//  VelaWallet
//
//  在区块浏览器中查看 — the three drawn buttons that did nothing.
//
//  Ported from `app-web/vela-wallet/src/lib/services/networks.ts`
//  (`explorerBaseURL` / `explorerTxURL` / `explorerAddressURL`), with one
//  deliberate deviation.
//
//  ## No base, no link — web's fallback is not ported
//
//  Web's two convenience wrappers fall back to `etherscan.io` for a chain with
//  no configured explorer, and its own `explorerBaseURL` comment says why that
//  is only a display convenience: *"security surfaces show NO link rather than
//  a misleading one"*. Sending somebody to Ethereum's explorer to look up a
//  Gnosis transaction is exactly the misleading one — they would find nothing
//  and reasonably conclude their money had vanished. So a chain without an
//  explorer has no link, and the caller draws nothing.
//
//  ## Custom networks count
//
//  A chain the person added themselves carries its own `explorerURL` in
//  `vela.customNetworks`. Reading only the twelve built-ins would leave every
//  added network's transactions unlinkable.
//

import Foundation

enum ExplorerLinks {

    /// The chain's explorer base, trailing slash stripped. `nil` for a chain
    /// nobody has an explorer for.
    @MainActor
    static func base(chainId: Int, store: VelaStore) -> String? {
        if let builtin = ChainCatalog.meta(chainId)?.explorerURL, !builtin.isEmpty {
            return trimmed(builtin)
        }
        for network in store.readList(VelaStore.Key.customNetworks) {
            guard (network["chainId"] as? NSNumber)?.intValue == chainId,
                  let url = network["explorerURL"] as? String, !url.isEmpty
            else { continue }
            return trimmed(url)
        }
        return nil
    }

    @MainActor
    static func tx(chainId: Int, hash: String, store: VelaStore) -> URL? {
        guard !hash.isEmpty, let base = base(chainId: chainId, store: store) else { return nil }
        return URL(string: "\(base)/tx/\(hash)")
    }

    @MainActor
    static func address(chainId: Int, _ address: String, store: VelaStore) -> URL? {
        guard !address.isEmpty, let base = base(chainId: chainId, store: store) else { return nil }
        return URL(string: "\(base)/address/\(address)")
    }

    /// A token's own page. Explorers put ERC-20s under `/token/`; a native coin
    /// has no contract and therefore no page, which the caller reads as "no
    /// link" rather than linking to the chain's homepage.
    @MainActor
    static func token(chainId: Int, contract: String?, store: VelaStore) -> URL? {
        guard let contract, !contract.isEmpty,
              let base = base(chainId: chainId, store: store)
        else { return nil }
        return URL(string: "\(base)/token/\(contract)")
    }

    private static func trimmed(_ url: String) -> String {
        url.hasSuffix("/") ? String(url.dropLast()) : url
    }
}
