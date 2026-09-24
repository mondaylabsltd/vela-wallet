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
        /// Spec 075: the Trusted Signer page this key lives behind; empty when it
        /// lives behind none. The core needs it to caption the row at all — a
        /// page runs its ceremony in a browser, so the authenticator reports
        /// `platform`, and a row that believed the report said "this device"
        /// about the one side of the page this wallet cannot reach (found by
        /// the Android device pass, 2026-09-22). Only the record knows it; the
        /// registry stores no page.
        var signerOrigin = ""
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
        /// Spec 075: the Trusted Signer page this key lives behind, as the core
        /// returned it; empty when the row is not behind one (the field is
        /// absent on the wire then). The row's method already says `trustedSigner`
        /// in that case — this says WHICH page.
        var signerOrigin = ""
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
    ///
    /// Each key's Trusted Signer page comes along (spec 075). It is read through
    /// `keyRoutesJson`, which already lifts `signer_origin` off the same record
    /// for the signing route: one reader of the record means the row and the
    /// ceremony cannot disagree about where a key lives.
    static func deviceKeys(
        of address: String, walletName: String, in accounts: UserOpSpine.AccountPort
    ) async -> [DeviceKey] {
        let pages = await signerOrigins(of: address, in: accounts)
        return await accounts.keys(of: address).enumerated().map { index, key in
            DeviceKey(
                publicKeyHex: key.publicKeyHex,
                name: index == 0 ? walletName : "",
                transports: "",
                signerOrigin: pages[key.credentialId] ?? ""
            )
        }
    }

    /// Each founding key's Trusted Signer page, by credential id; a key behind no
    /// page is simply absent. Keyed rather than positional — the route list and
    /// the key list come off one record but not through one filter, and a
    /// mismatch by one would hand a key somebody else's page.
    private static func signerOrigins(
        of address: String, in accounts: UserOpSpine.AccountPort
    ) async -> [String: String] {
        let json = await accounts.keyRoutesJson(of: address)
        let routes = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [[String: Any]] ?? []
        return routes.reduce(into: [:]) { pages, route in
            guard let id = route["credential_id"] as? String, !id.isEmpty,
                  let page = route["signer_origin"] as? String, !page.isEmpty
            else { return }
            pages[id] = page
        }
    }

    func read(address: String, device: [DeviceKey]) async -> Result {
        let empty = Result(source: .device, rows: [])
        let deviceJson = Self.json(device.map {
            [
                "public_key_hex": $0.publicKeyHex, "name": $0.name, "transports": $0.transports,
                // Spec 075: the core reads the empty string as "behind no page",
                // so it goes out on every key rather than only on some.
                "signer_origin": $0.signerOrigin,
            ]
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
                // Absent on the wire for every row but a Trusted Signer's.
                signerOrigin: text("signer_origin")
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
