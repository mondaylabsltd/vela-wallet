//
//  RegistryChainReader.swift
//  VelaWallet
//
//  The registry CONTRACT, read directly — what sign-in falls back to when the
//  index service cannot be reached (spec 062).
//
//  The index is a cache of the contract. When it is gone — never when it
//  answers; a refusal is an answer — the same two read questions are put to the
//  contract itself: on Gnosis, where the record lives, then on Ethereum, where
//  a person may have backed it up precisely for this. `vela_core::registry_chain`
//  plans the calls and answers in the index's own JSON shapes, so every guard in
//  `RegistryClient` runs unchanged on either source. What is left here is what
//  only a shell can do: perform the `eth_call`s.
//

import Foundation
import VelaCore

struct RegistryChainReader: Sendable {
    /// Where the index's own unit ids come from.
    static let homeChain = 100

    /// The RAW `result` hex, or `nil` when that chain did not answer.
    let ethCall: @Sendable (_ chainId: Int, _ to: String, _ data: String) async -> String?

    /// The `?publicKey=` body and the chain that gave it, or `nil` when nobody answered.
    func keyProfile(_ publicKeyHex: String) async -> (chainId: Int, body: [String: Any])? {
        guard let plan = Self.object(registryChainKeyPlan(publicKeyHex: publicKeyHex)),
              let chains = plan["chains"] as? [NSNumber]
        else { return nil }
        for chainId in chains.map(\.intValue) {
            guard let results = await read(chainId, plan),
                  let profile = Self.object(registryChainKeyStatus(hasEntryHex: results[0], groupsHex: results[1]))
            else { continue }
            // A chain that knows the key founded nothing is only believed when
            // it is the record's home: Ethereum holds what somebody backed up.
            let founded = ((profile["groups"] as? [String: Any])?["unitIds"] as? [Any])?.count ?? 0
            if chainId != Self.homeChain && founded == 0 { continue }
            return (chainId, profile)
        }
        return nil
    }

    /// The `?unitId=` body from ONE chain — unit ids are per deployment.
    func unit(chainId: Int, unitId: UInt32) async -> [String: Any]? {
        guard let plan = Self.object(registryChainUnitPlan(unitId: unitId)),
              let results = await read(chainId, plan)
        else { return nil }
        return Self.object(registryChainUnit(unitId: unitId, unitHex: results[0], membersHex: results[1]))
    }

    /// Both calls of a plan on one chain; `nil` unless BOTH were answered.
    private func read(_ chainId: Int, _ plan: [String: Any]) async -> [String]? {
        guard let calls = plan["calls"] as? [[String: Any]], calls.count == 2,
              let firstTo = calls[0]["to"] as? String, let firstData = calls[0]["data"] as? String,
              let secondTo = calls[1]["to"] as? String, let secondData = calls[1]["data"] as? String
        else { return nil }
        async let first = ethCall(chainId, firstTo, firstData)
        async let second = ethCall(chainId, secondTo, secondData)
        guard let a = await first, let b = await second else { return nil }
        return [a, b]
    }

    private static func object(_ json: String?) -> [String: Any]? {
        guard let json else { return nil }
        return (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
    }
}
