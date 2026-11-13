//
//  ReceiveStanding.swift
//  VelaWallet
//
//  §37 (review 1-7): may this account be received at?
//
//  A multi-key wallet rebuilt at sign-in from a group only the key index
//  vouched for is saved `unconfirmed`: a lying index can hand back the
//  person's real credential beside somebody else's keys, and the address
//  derived from that group is one the other party co-owns. Such an account
//  signs, but shows NO address to receive at — no list, no code, no copy, no
//  share card, no payment request, no deposit watch — until the chain alone
//  confirms the group makes this very address (`refuted` once a chain proves
//  it another wallet's).
//
//  Every rule is the core's: `receive_standing` says whether the screen may
//  show an address and what it says instead; `registry_resolve_confirm_step`
//  walks the chain, its `eth_call`s answered by the SAME executor the
//  sign-in's key and unit walks use (`RegistryClient.confirmIdentity`);
//  `account_with_identity` is the record to save. This file only reads the
//  stored record, drives the walk, and saves what the core wrote.
//

import Foundation
import Observation
import VelaCore

/// `ReceiveStanding` as the core writes it (`receive_standing`).
struct ReceiveStandingWire: Decodable, Equatable {
    /// Show the address, its code and the payment request.
    let receivable: Bool
    /// `confirmed` / `unconfirmed` / `refuted`.
    let identity: String
    /// What the screen says instead (`receive.unconfirmed.*` /
    /// `receive.refuted.*`).
    var titleKey: String? = nil
    var bodyKey: String? = nil
    /// Check the chain again before the screen shows anything — and offer
    /// Check again.
    let checkFirst: Bool

    /// An address that is no stored Vela account (a multisig shown in front):
    /// this rule does not speak for it.
    static let notAnAccount = ReceiveStandingWire(receivable: true, identity: "confirmed", checkFirst: false)

    /// A stored record the core could not read (`receive_standing` → nil):
    /// no address either (the core's own reading), said as unconfirmed —
    /// never as somebody's address.
    static let unreadable = ReceiveStandingWire(
        receivable: false, identity: "unconfirmed",
        titleKey: "receive.unconfirmed.title", bodyKey: "receive.unconfirmed.body",
        checkFirst: false
    )

    /// The core's answer for one stored record.
    static func of(accountJson: String) -> ReceiveStandingWire {
        guard let json = receiveStanding(accountJson: accountJson),
              let standing = try? CoreJSON.decoder.decode(ReceiveStandingWire.self, from: Data(json.utf8))
        else { return .unreadable }
        return standing
    }
}

/// The Receive screen's gate, per address, for every deposit entry.
@MainActor
@Observable
final class ReceiveStandingStore {

    /// Keyed by lowercased address. Absent: not read yet — which shows no
    /// address either (fail closed); the read is a moment's.
    private(set) var standings: [String: ReceiveStandingWire] = [:]
    /// The addresses whose chain walk is out right now ("Checking on-chain…").
    private(set) var checking: Set<String> = []

    private let accounts: AccountStore
    /// The chain walk (`RegistryClient.confirmIdentity`). A seam so a test
    /// scripts the chain. `@MainActor`: this store calls it — and a function
    /// type left to Swift 6.2's default isolation imports
    /// `swift_getExtendedFunctionTypeMetadata`, which iOS 17 lacks
    /// (`check-ios17-function-metadata.mjs`).
    private let confirm: @MainActor @Sendable (_ publicKeyHex: String, _ address: String) async -> IdentityVerdict

    init(
        accounts: AccountStore,
        confirm: @escaping @MainActor @Sendable (_ publicKeyHex: String, _ address: String) async -> IdentityVerdict
    ) {
        self.accounts = accounts
        self.confirm = confirm
    }

    convenience init(accounts: AccountStore, registry: RegistryClient) {
        self.init(accounts: accounts, confirm: { publicKeyHex, address in
            await registry.confirmIdentity(publicKeyHex: publicKeyHex, address: address)
        })
    }

    /// What Receive may show for `address`, as last read; `nil` — not read
    /// yet.
    func standing(of address: String) -> ReceiveStandingWire? {
        standings[address.lowercased()]
    }

    /// Whether an address may be shown to receive at — `false` until read.
    func receivable(_ address: String) -> Bool {
        !address.isEmpty && standing(of: address)?.receivable == true
    }

    func isChecking(_ address: String) -> Bool {
        checking.contains(address.lowercased())
    }

    /// Read the stored record's standing.
    @discardableResult
    func refresh(_ address: String) async -> ReceiveStandingWire? {
        guard !address.isEmpty else { return nil }
        let standing = await record(of: address).map { ReceiveStandingWire.of(accountJson: Self.json($0)) }
            ?? .notAnAccount
        standings[address.lowercased()] = standing
        return standing
    }

    /// Receive opened (any deposit entry): read the standing and, when the
    /// core says to check first, walk the chain — the screen says "Checking
    /// on-chain…" meanwhile, and shows the address only once confirmed.
    func open(_ address: String) async {
        guard let standing = await refresh(address), standing.checkFirst else { return }
        await check(address)
    }

    /// App start: the active account, re-checked before first use when it
    /// is unconfirmed. Quiet — nothing is drawn for it.
    func recheckActive() async {
        let records = await accounts.loadAccounts()
        let index = await accounts.loadActiveIndex()
        guard records.indices.contains(index),
              let address = records[index]["address"] as? String
        else { return }
        await open(address)
    }

    /// Check again: the chain walk, and the record saved when it changed.
    func check(_ address: String) async {
        let key = address.lowercased()
        guard !address.isEmpty, !checking.contains(key) else { return }
        checking.insert(key)
        defer { checking.remove(key) }
        guard let record = await record(of: address),
              let publicKeyHex = Self.publicKeyHex(record),
              let stored = record["address"] as? String
        else {
            await refresh(address)
            return
        }
        let verdict = await confirm(publicKeyHex, stored)
        let was = record["identity"] as? String ?? "confirmed"
        if verdict.identity != was,
           let next = accountWithIdentity(accountJson: Self.json(record), identity: verdict.identity),
           let object = (try? JSONSerialization.jsonObject(with: Data(next.utf8))) as? [String: Any] {
            VelaLog.notice(.accounts, "identity \(was) -> \(verdict.identity) (verified_by \(verdict.verifiedBy))")
            await accounts.saveAccount(object)
        }
        await refresh(address)
    }

    // MARK: - The stored record

    private func record(of address: String) async -> [String: Any]? {
        await accounts.loadAccounts().first {
            ($0["address"] as? String)?.caseInsensitiveCompare(address) == .orderedSame
        }
    }

    /// One of the account's keys: its first founding key, else the scalar.
    static func publicKeyHex(_ record: [String: Any]) -> String? {
        let first = (record["keys"] as? [[String: Any]])?.first?["public_key_hex"] as? String
        let key = first ?? record["public_key_hex"] as? String
        return (key?.isEmpty ?? true) ? nil : key
    }

    private static func json(_ object: [String: Any]) -> String {
        (try? JSONSerialization.data(withJSONObject: object)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
    }
}
