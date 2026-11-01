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
        /// What the core matches the account's sign-in key against, to mark the
        /// row it signs with (`signs_here`).
        var credentialId = ""
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
        /// base64url, as the registry explorer prints it; empty from the device.
        var credentialId = ""
        /// The registry's 20-byte attestation summary, `0x`-hex; empty from the device.
        var attestationHex = ""
        /// The authenticator verified the person at registration; `nil` = nobody can vouch.
        var userVerified: Bool?
        /// The key this device signs with — the account's sign-in key
        /// (founder, 2026-09-26). The core marks at most one row.
        var signsHere = false
    }

    struct Result: Equatable {
        let source: Source
        let rows: [Row]
    }

    /// A handful of rounds; this only stops a contract bug from spinning.
    private static let maxRounds = 24

    private let ethCall: @MainActor (_ chainId: Int, _ to: String, _ data: String) async -> String?
    private let step: (
        _ address: String, _ deviceKeysJson: String, _ answersJson: String, _ signInCredential: String
    ) -> String

    init(
        ethCall: @escaping @MainActor (Int, String, String) async -> String?,
        step: @escaping (String, String, String, String) -> String = {
            walletKeysStep(address: $0, deviceKeysJson: $1, answersJson: $2, signInCredential: $3)
        }
    ) {
        self.ethCall = ethCall
        self.step = step
    }

    /// The record's keys as the walk wants them, each with its OWN name as
    /// the record keeps it (r10 follow-up: keys and accounts are different
    /// things). The account's name never stands in for a key's — handing it
    /// over as key 0's label is what drew "Interleave" over "Parallel One".
    /// A record from before the key set names none; the registry's metadata
    /// names them, and the core says "Key n" for one nobody named.
    ///
    /// No page rides along any more (spec 102): a row is captioned by where
    /// its key LIVES — this device, a phone, a USB key — never by the page it
    /// was made on, and an account on its own domain says so once, above the
    /// list (`settings.signing.keysOn`), from its signing plan.
    static func deviceKeys(
        of address: String, in accounts: UserOpSpine.AccountPort
    ) async -> [DeviceKey] {
        let names = await recordKeyNames(of: address, in: accounts)
        return await accounts.keys(of: address).map { key in
            DeviceKey(
                credentialId: key.credentialId,
                publicKeyHex: key.publicKeyHex,
                name: names[key.publicKeyHex.lowercased()] ?? "",
                transports: ""
            )
        }
    }

    /// Each key's own name in the stored record, by public key.
    private static func recordKeyNames(
        of address: String, in accounts: UserOpSpine.AccountPort
    ) async -> [String: String] {
        guard let json = await accounts.accountJson(of: address),
              let record = object(json)
        else { return [:] }
        var names: [String: String] = [:]
        for key in record["keys"] as? [[String: Any]] ?? [] {
            guard let hex = key["public_key_hex"] as? String ?? key["publicKeyHex"] as? String,
                  let name = key["name"] as? String
            else { continue }
            names[hex.lowercased()] = name
        }
        return names
    }

    /// The credential the account signs with (its plan's key route) — the row
    /// the core marks. Empty for a record that names none: it signs as it
    /// always did, with no single key, and no row is marked.
    static func signInCredential(
        of address: String, in accounts: UserOpSpine.AccountPort
    ) async -> String {
        guard let record = await accounts.accountJson(of: address),
              let key = SigningPlanWire.of(accountJson: record)?.key
        else { return "" }
        return key.credentialId
    }

    /// `accountName`: the account's own name, beside its keys — a key
    /// labelled with it is labelled with nothing (`step_for_account`).
    func read(
        address: String, accountName: String = "", device: [DeviceKey], signInCredential: String
    ) async -> Result {
        let empty = Result(source: .device, rows: [])
        let deviceJson = Self.deviceJson(accountName: accountName, device: device)
        var answers: [[String: Any]] = []
        for _ in 0..<Self.maxRounds {
            guard let next = Self.object(step(address, deviceJson, Self.json(answers), signInCredential))
            else { return empty }
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
        // Never settled: what the device alone says.
        return deviceOnly(device: device, signInCredential: signInCredential)
    }

    /// What this device's own record says, with nobody asked: the walk run
    /// with no address, so that it can ask no chain. The rows are the core's —
    /// in founding order, named as the registry's answer will name them where
    /// the record knows, the sign-in key marked — and carry no badge nobody
    /// has vouched for yet.
    ///
    /// Settings draws these from its first frame (iPhone walk 2026-10-10: the
    /// block was ONE skeleton row until the registry answered, then grew to
    /// the wallet's three keys and pushed the page down ~250 pt).
    func deviceOnly(accountName: String = "", device: [DeviceKey], signInCredential: String) -> Result {
        let deviceJson = Self.deviceJson(accountName: accountName, device: device)
        return Self.object(step("", deviceJson, "[]", signInCredential)).map(Self.parse)
            ?? Result(source: .device, rows: [])
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
                    // `null` = nobody could read the attestation: no badge (#207).
                    syncedKnown: synced != nil,
                    aaguid: text("aaguid"),
                    providerName: text("provider_name"),
                    method: KeyMethod(rawValue: text("method")) ?? .platform,
                    kind: KeyMethod(rawValue: text("method")) ?? .platform
                ),
                synced: synced,
                publicKeyHex: text("public_key_hex"),
                credentialId: text("credential_id"),
                attestationHex: text("attestation_hex"),
                userVerified: (key["user_verified"] as? NSNumber)?.boolValue,
                signsHere: (key["signs_here"] as? NSNumber)?.boolValue ?? false
            )
        }
        return Result(source: source, rows: rows)
    }

    /// The walk's device argument in its fuller shape (r10 follow-up):
    /// `{"account_name", "keys"}`, so the core can tell the account's name
    /// from a key's.
    private static func deviceJson(accountName: String, device: [DeviceKey]) -> String {
        json([
            "account_name": accountName,
            "keys": device.map {
                [
                    "credential_id": $0.credentialId,
                    "public_key_hex": $0.publicKeyHex, "name": $0.name, "transports": $0.transports,
                ]
            },
        ])
    }

    private static func json(_ value: Any) -> String {
        (try? JSONSerialization.data(withJSONObject: value)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
    }

    private static func object(_ json: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
    }
}
