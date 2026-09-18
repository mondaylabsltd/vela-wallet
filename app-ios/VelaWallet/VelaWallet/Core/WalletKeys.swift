//
//  WalletKeys.swift
//  VelaWallet
//
//  Which passkeys control a wallet — the iOS transport for
//  `vela_core::wallet_keys` (spec 062).
//
//  A person offered "back up your keys" is owed the sight of them first. The
//  walk reads the wallet's founding set from the registry CONTRACT (Gnosis,
//  then the Ethereum backup; never our index), names each key and the vault
//  holding it, and falls back to what this device's account record remembers
//  when no chain answers. Every rule is the core's; this carries `eth_call`s.
//

import Foundation
import VelaCore

@MainActor
final class WalletKeys {
    enum Source: Equatable {
        /// The registry answered: every field is filled.
        case registry
        /// No chain answered: the device's own memory, and no sync badges.
        case device
        /// A chain answered and holds no record for this address. Nothing was
        /// unreachable, and nothing may say so.
        case notRegistered
    }

    /// One key as the account record holds it, in founding order.
    struct DeviceKey: Equatable {
        let publicKeyHex: String
        let name: String
        let transports: String
    }

    struct Row: Equatable {
        /// In the create flow's shape — what `PasskeyProviderMark` draws from.
        let key: CreateKeyRow
        /// `nil` when only the device answered: nobody can vouch for a badge.
        let synced: Bool?
        let publicKeyHex: String
    }

    struct Result: Equatable {
        let source: Source
        let rows: [Row]
    }

    /// A handful of rounds; this only stops a contract bug from spinning.
    private static let maxRounds = 24

    private let ethCall: (_ chainId: Int, _ to: String, _ data: String) async -> String?
    private let step: (_ address: String, _ deviceKeysJson: String, _ answersJson: String) -> String

    init(
        ethCall: @escaping (Int, String, String) async -> String?,
        step: @escaping (String, String, String) -> String = {
            walletKeysStep(address: $0, deviceKeysJson: $1, answersJson: $2)
        }
    ) {
        self.ethCall = ethCall
        self.step = step
    }

    /// The record's keys as the walk wants them. The record keeps no per-key
    /// label, so only key 0 — whose name IS the wallet's — arrives named; the
    /// registry's metadata names the rest.
    static func deviceKeys(
        of address: String, walletName: String, in accounts: UserOpSpine.AccountPort
    ) async -> [DeviceKey] {
        await accounts.keys(of: address).enumerated().map { index, key in
            DeviceKey(publicKeyHex: key.publicKeyHex, name: index == 0 ? walletName : "", transports: "")
        }
    }

    func read(address: String, device: [DeviceKey]) async -> Result {
        let empty = Result(source: .device, rows: [])
        let deviceJson = Self.json(device.map {
            ["public_key_hex": $0.publicKeyHex, "name": $0.name, "transports": $0.transports]
        })
        var answers: [[String: Any]] = []
        for _ in 0..<Self.maxRounds {
            guard let next = Self.object(step(address, deviceJson, Self.json(answers))) else { return empty }
            guard next["type"] as? String == "ask" else { return Self.parse(next) }
            let requests = next["requests"] as? [[String: Any]] ?? []
            let round = await withTaskGroup(of: [String: Any].self) { group in
                for request in requests {
                    group.addTask { [weak self] in await self?.perform(request) ?? [:] }
                }
                var collected: [[String: Any]] = []
                for await answer in group where !answer.isEmpty { collected.append(answer) }
                return collected
            }
            answers.append(contentsOf: round)
        }
        // Never settled: what the device alone says, asked with no address so
        // that it can ask nobody.
        return Self.object(step("", deviceJson, "[]")).map(Self.parse) ?? empty
    }

    private func perform(_ request: [String: Any]) async -> [String: Any] {
        let id = request["id"] as? String ?? ""
        guard request["type"] as? String == "eth_call",
              let chainId = (request["chain_id"] as? NSNumber)?.intValue,
              let to = request["to"] as? String, let data = request["data"] as? String,
              let result = await ethCall(chainId, to, data)
        else { return ["id": id, "outcome": "failed", "body": NSNull()] }
        return ["id": id, "outcome": "ok", "body": result]
    }

    private static func parse(_ done: [String: Any]) -> Result {
        let source: Source = switch done["source"] as? String {
        case "registry": .registry
        case "not_registered": .notRegistered
        default: .device
        }
        let rows = (done["keys"] as? [[String: Any]] ?? []).map { key -> Row in
            let synced = (key["synced"] as? NSNumber)?.boolValue
            func text(_ name: String) -> String { key[name] as? String ?? "" }
            return Row(
                key: CreateKeyRow(
                    name: text("name"),
                    authenticatorAttachment: text("authenticator_attachment"),
                    transports: text("transports"),
                    confirmed: true,
                    synced: synced ?? true,
                    aaguid: text("aaguid"),
                    providerName: text("provider_name"),
                    method: KeyMethod(rawValue: text("method")) ?? .platform
                ),
                synced: synced,
                publicKeyHex: text("public_key_hex")
            )
        }
        return Result(source: source, rows: rows)
    }

    private static func json(_ value: Any) -> String {
        (try? JSONSerialization.data(withJSONObject: value)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
    }

    private static func object(_ json: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
    }
}
