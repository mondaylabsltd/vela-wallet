//
//  CustomTokens.swift
//  VelaWallet
//
//  `vela.customTokens` — the tokens this wallet knows about.
//
//  Two machines write this key: `token_trust` admits one from an authenticated
//  receipt, and `manage_tokens` adds one somebody typed in. That is exactly the
//  shape the address book's `vela.serviceEndpoints` note warns about — two
//  independent writers on one key, both succeeding, one erasing the other — so
//  they reach it through here.
//
//  ## The id is the de-dupe, and it has one spelling
//
//  `{chainId}_{contractAddress}`, lowercased. `manage_tokens.rs` calls it "THE
//  dedupe key" and notes that the manual and auto-add paths each hand-rolled it
//  in the TypeScript. On this client they do not.
//
//  ## `networkName` is display vocabulary, and it is frozen at save time
//
//  The core never sees a chain's NAME — it deals in `chain_id` and takes the
//  name from the shell's registry snapshot. Storing it is a compatibility
//  requirement (every other client's `CustomToken` carries it), not a decision.
//

import Foundation

enum CustomTokens {

    /// `{chainId}_{contract}`, lowercased — the one spelling.
    static func id(chainId: Int, contract: String) -> String {
        "\(chainId)_\(contract.lowercased())"
    }

    @MainActor
    static func load(store: VelaStore) -> [[String: Any]] {
        store.readList(VelaStore.Key.customTokens)
    }

    /// Replace-by-id then append, exactly as every client's storage layer does.
    /// Answers whether anything was written — a token with no contract, no
    /// symbol or no chain is not a token, and the machines model that refusal.
    @MainActor
    static func save(_ token: [String: Any], store: VelaStore) -> Bool {
        guard let contract = token["contractAddress"] as? String, !contract.isEmpty,
              let chainId = (token["chainId"] as? NSNumber)?.intValue,
              let symbol = token["symbol"] as? String, !symbol.isEmpty
        else { return false }

        var record = token
        record["id"] = token["id"] as? String ?? id(chainId: chainId, contract: contract)
        record["networkName"] = token["networkName"] as? String
            ?? ChainCatalog.meta(chainId)?.displayName ?? ""
        record["symbol"] = symbol

        var stored = load(store: store).filter { ($0["id"] as? String) != record["id"] as? String }
        stored.append(record)
        store.writeList(VelaStore.Key.customTokens, stored)
        return true
    }

    /// Remove by id. Answers whether the row was there — the core keeps a row
    /// on a failed delete, and "it was already gone" is not a failure.
    @MainActor
    @discardableResult
    static func remove(id: String, store: VelaStore) -> Bool {
        let stored = load(store: store)
        let next = stored.filter { ($0["id"] as? String) != id }
        guard next.count != stored.count else { return false }
        store.writeList(VelaStore.Key.customTokens, next)
        return true
    }

    /// A stored token in the shape the wallet-state machines read.
    ///
    /// `networkName` does not travel: chain naming is the shell's word for a
    /// chain, and a core that carried it could disagree with the settings
    /// screen about what a network is called.
    static func toWire(_ stored: [String: Any]) -> [String: Any]? {
        guard let contract = stored["contractAddress"] as? String, !contract.isEmpty,
              let chainId = (stored["chainId"] as? NSNumber)?.intValue
        else { return nil }
        return [
            "id": stored["id"] as? String ?? id(chainId: chainId, contract: contract),
            "chain_id": chainId,
            "contract_address": contract,
            "symbol": stored["symbol"] as? String ?? "",
            "name": stored["name"] as? String ?? "",
            "decimals": (stored["decimals"] as? NSNumber)?.intValue ?? 18,
        ]
    }

    /// The reverse, for a core that hands one back to be persisted.
    static func fromWire(_ wire: [String: Any]) -> [String: Any] {
        let chainId = (wire["chain_id"] as? NSNumber)?.intValue ?? 0
        let contract = wire["contract_address"] as? String ?? ""
        return [
            "id": wire["id"] as? String ?? id(chainId: chainId, contract: contract),
            "chainId": chainId,
            "contractAddress": contract,
            "symbol": wire["symbol"] as? String ?? "",
            "name": wire["name"] as? String ?? "",
            "decimals": (wire["decimals"] as? NSNumber)?.intValue ?? 18,
            "networkName": wire["network_name"] as? String
                ?? ChainCatalog.meta(chainId)?.displayName ?? "",
        ]
    }
}
