//
//  RegistryNameLookup.swift
//  VelaWallet
//
//  The name a Vela user registered for a wallet, found by its ADDRESS — the
//  iOS transport for `vela_core::registry_lookup` (issue 191).
//
//  The v2 index cannot be asked about an address: `?walletRef=` answers 400,
//  because an address is `f(all founding keys)` and no single entry owns one.
//  `RegistryClient.queryByWalletRef` asked exactly that for two specs, and the
//  index rung of `RecipientIdentity` silently never answered. The name is
//  reached through the chain instead — the Safe's `SafeWebAuthnSharedSigner`
//  configuration → `?publicKey=` → `?unitId=` — and every rule of that walk
//  (which chain first, which unit is believed, what a malformed blob means,
//  how long a verdict may be kept) is the core's, shared with the other three
//  shells. What is left here is what only a shell can do: perform the requests
//  and keep the verdict for as long as the core says.
//

import Foundation
import VelaCore

@MainActor
final class RegistryNameLookup {
    /// How one GET against the index ended.
    enum IndexAnswer {
        case ok(String)
        /// The index said 404: an answer.
        case notFound
        /// Unreachable, timed out, any other status: nobody said anything.
        case failed
    }

    /// `vela.indexName:<address>` — web's key and shape: `{ hit: { name,
    /// publicKey } }` or `{ missAt }`.
    static let cachePrefix = "vela.indexName:"
    /// `registry_lookup::MISS_TTL_MS` — a miss is only true for now.
    static let missTTLMs: Double = 6 * 60 * 60 * 1000
    /// A lookup is a handful of rounds; this only stops a contract bug from spinning.
    private static let maxRounds = 16

    private let store: VelaStore
    /// `eth_call` on one chain: the RAW `result` hex — a bare `0x` included,
    /// which is a chain saying "no such contract here" — or `nil` when nobody
    /// answered. The distinction is the core's caching rule.
    private let ethCall: (_ chainId: Int, _ to: String, _ data: String) async -> String?
    private let indexGet: (_ path: String) async -> IndexAnswer
    /// `registryNameStep` from the core; a seam so tests can script it.
    private let step: (_ address: String, _ answersJson: String) -> String
    private let now: () -> Double

    init(
        store: VelaStore,
        ethCall: @escaping (Int, String, String) async -> String?,
        indexGet: @escaping (String) async -> IndexAnswer,
        step: @escaping (String, String) -> String = { registryNameStep(address: $0, answersJson: $1) },
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }
    ) {
        self.store = store
        self.ethCall = ethCall
        self.indexGet = indexGet
        self.step = step
        self.now = now
    }

    func name(for address: String) async -> String? {
        let key = address.lowercased()
        guard key.count == 42, key.hasPrefix("0x") else { return nil }

        let stored = store.readObject(Self.cachePrefix + key)
        if let hit = stored["hit"] as? [String: Any], let name = hit["name"] as? String, !name.isEmpty {
            return name
        }
        if let missAt = (stored["missAt"] as? NSNumber)?.doubleValue, now() - missAt < Self.missTTLMs {
            return nil
        }

        var answers: [[String: Any]] = []
        for _ in 0..<Self.maxRounds {
            guard let transcript = try? JSONSerialization.data(withJSONObject: answers),
                  let next = try? JSONSerialization.jsonObject(
                      with: Data(step(key, String(decoding: transcript, as: UTF8.self)).utf8)
                  ) as? [String: Any]
            else { return nil }

            if next["type"] as? String == "ask" {
                let requests = next["requests"] as? [[String: Any]] ?? []
                // "Together, if the shell can": the second tier is six chains.
                let round = await withTaskGroup(of: [String: Any].self) { group in
                    for request in requests {
                        group.addTask { [weak self] in await self?.perform(request) ?? [:] }
                    }
                    var collected: [[String: Any]] = []
                    for await answer in group where !answer.isEmpty { collected.append(answer) }
                    return collected
                }
                answers.append(contentsOf: round)
                continue
            }

            let found = next["found"] as? [String: Any]
            let name = (found?["name"] as? String).flatMap { $0.isEmpty ? nil : $0 }
            switch next["remember"] as? String {
            case "forever":
                if let name {
                    store.writeObject(Self.cachePrefix + key, [
                        "hit": ["name": name, "publicKey": found?["public_key"] as? String ?? ""],
                    ])
                }
            case "briefly":
                if name == nil { store.writeObject(Self.cachePrefix + key, ["missAt": now()]) }
            default:
                break
            }
            return name
        }
        return nil
    }

    private func perform(_ request: [String: Any]) async -> [String: Any] {
        let id = request["id"] as? String ?? ""
        func answer(_ outcome: String, _ body: String? = nil) -> [String: Any] {
            ["id": id, "outcome": outcome, "body": body ?? NSNull()]
        }
        switch request["type"] as? String {
        case "eth_call":
            guard let chainId = (request["chain_id"] as? NSNumber)?.intValue,
                  let to = request["to"] as? String, let data = request["data"] as? String,
                  let result = await ethCall(chainId, to, data)
            else { return answer("failed") }
            return answer("ok", result)
        case "index_get":
            guard let path = request["path"] as? String else { return answer("failed") }
            switch await indexGet(path) {
            case .ok(let body): return answer("ok", body)
            case .notFound: return answer("not_found")
            case .failed: return answer("failed")
            }
        default:
            return answer("failed")
        }
    }
}
