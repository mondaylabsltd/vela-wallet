//
//  SendExecutor.swift
//  VelaWallet
//
//  The `send` machine's eighteen arms.
//
//  Ported from `app-android/.../feature/send/core/SendExecutor.kt` (spec 043
//  T027). This file is the ORDER — which read happens before which, what is
//  fatal and what falls back — and the order is `UserOpSpine`'s, which is the
//  desktop's `executor/user_op.rs`.
//
//  Nothing here prices, validates or classifies: the relay's words go to the
//  core to be classified, and the screen prints the core's sentence.
//

import Foundation
import VelaCore

@MainActor
final class SendExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "fetch_tokens", "clear_token_cache", "resolve_token_metadata", "add_network",
        "estimate_fee", "probe_treasury", "load_account_credential", "submit_user_op",
        "cancel_passkey_sign", "persist_tx_records", "track_submitted", "resolve_identity",
        "resolve_risk", "simulate_calls", "start_timer", "haptic", "show_alert", "close",
    ]

    /// What the screen owns: the tracker handoff, the alert surface, haptics,
    /// and leaving.
    struct Ports {
        var signingStarted: () -> Void = {}
        var trackSubmitted: (_ userOpHash: String, _ recordIds: [String], _ chainId: Int) -> Void = { _, _, _ in }
        var haptic: (String) -> Void = { _ in }
        var alert: ([String: Any]) -> Void = { _ in }
        var closed: () -> Void = {}
        var refreshBalances: () -> Void = {}
        /// The pending row is on disk: the feed re-reads, so the home shows it
        /// at submit (FR-006).
        var recordsPersisted: () -> Void = {}
    }

    private let store: VelaStore
    private let relay: RelayClient
    private let pool: RpcPool
    private let spine: UserOpSpine
    private let accounts: UserOpSpine.AccountPort
    private let fees: FeeStore
    private let identity: RecipientIdentity
    private let metadata: TokenMetadata
    /// The account list, for the one credential lookup this machine asks.
    private let accountStore: AccountStore
    /// The holdings the balance machine already read, and the person's chains.
    private let balances: () -> BalanceViewWire?
    private let networks: () -> NetViewWire?
    /// `var` because two of these close over the STORE, which is built after
    /// this executor — the store installs them once it exists.
    var ports: Ports

    /// The ceremony in flight, so `cancel_passkey_sign` can end it.
    private var signing: Task<String, Error>?

    init(
        store: VelaStore,
        relay: RelayClient,
        pool: RpcPool,
        spine: UserOpSpine,
        accounts: UserOpSpine.AccountPort,
        fees: FeeStore,
        identity: RecipientIdentity,
        metadata: TokenMetadata,
        accountStore: AccountStore,
        balances: @escaping () -> BalanceViewWire?,
        networks: @escaping () -> NetViewWire?,
        ports: Ports
    ) {
        self.store = store
        self.relay = relay
        self.pool = pool
        self.spine = spine
        self.accounts = accounts
        self.fees = fees
        self.identity = identity
        self.metadata = metadata
        self.accountStore = accountStore
        self.balances = balances
        self.networks = networks
        self.ports = ports
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "fetch_tokens":
            return fetchTokens()

        case "clear_token_cache":
            ports.refreshBalances()
            return CoreJSON.string(["type": "token_cache_cleared"])

        case "resolve_token_metadata":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let address = operation["address"] as? String ?? ""
            let meta = await metadata.resolve(chainId: chainId, addresses: [address])[
                address.lowercased()
            ]
            return CoreJSON.string([
                "type": "token_metadata",
                "meta": meta.map { ["symbol": $0.symbol, "decimals": $0.decimals] as [String: Any] }
                    .map { $0 as Any } ?? NSNull(),
            ])

        // The scanner is this operation's ONLY entry, and the scanner is 055.
        case "add_network":
            return CoreJSON.string(["type": "network_added", "outcome": ["type": "error"]])

        case "estimate_fee":
            return await estimateFee(operation)

        case "probe_treasury":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            return CoreJSON.string([
                "type": "treasury_probed",
                "probe": Self.probeWire(await relay.probeTreasury(chainId: chainId)),
            ])

        case "load_account_credential":
            let id = operation["account_id"] as? String ?? ""
            let record = await accountStore.loadAccounts()
                .first { ($0["id"] as? String) == id }
            return CoreJSON.string([
                "type": "account_credential",
                "public_key_hex": (record?["public_key_hex"] as? String)
                    .flatMap { $0.isEmpty ? nil : $0 }
                    .map { $0 as Any } ?? NSNull(),
            ])

        case "submit_user_op":
            return await submit(operation)

        case "cancel_passkey_sign":
            signing?.cancel()
            signing = nil
            return CoreJSON.string(["type": "passkey_cancel_acknowledged"])

        case "persist_tx_records":
            let records = (operation["records"] as? [[String: Any]] ?? []).map(Self.feedRow)
            TxRecords.writeRecords(records, store: store)
            ports.recordsPersisted()
            return CoreJSON.string(["type": "records_persisted"])

        case "track_submitted":
            ports.trackSubmitted(
                operation["user_op_hash"] as? String ?? "",
                operation["record_ids"] as? [String] ?? [],
                (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            )
            return CoreJSON.string(["type": "track_handed_off"])

        case "resolve_identity":
            let address = operation["address"] as? String ?? ""
            let resolved = await identity.resolve(address)
            return CoreJSON.string([
                "type": "identity_resolved",
                "identity": resolved.map { ["name": $0.name, "source": $0.source] as [String: Any] }
                    .map { $0 as Any } ?? NSNull(),
            ])

        case "resolve_risk":
            return await resolveRisk(operation)

        // No simulation engine on this base (055).
        case "simulate_calls":
            return CoreJSON.string(["type": "sim_resolved", "sim_json": NSNull()])

        case "start_timer":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string([
                "type": "timer_elapsed",
                "tag": operation["tag"] ?? NSNull(),
            ])

        case "haptic":
            ports.haptic((operation["kind"] as? String) ?? "")
            return CoreJSON.string(["type": "haptic_played"])

        case "show_alert":
            ports.alert(operation["kind"] as? [String: Any] ?? [:])
            return CoreJSON.string(["type": "alert_acknowledged"])

        case "close":
            ports.closed()
            return CoreJSON.string(["type": "closed"])

        default:
            print("[vela-wallet] send: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    // MARK: - Tokens

    /// The holdings the balance machine already read — **no second walk**.
    ///
    /// Reading the chains again here would be a second opinion about what
    /// somebody owns, arriving a moment later than the one on the home screen.
    private func fetchTokens() -> String {
        guard let balance = balances() else {
            return CoreJSON.string(["type": "tokens_loaded", "tokens": NSNull(), "chains": []])
        }
        let rows = networks()?.networks ?? []
        let chains = rows.map { row in
            [
                "network": "chain-\(row.chainId)",
                "chain_id": row.chainId,
                "native_symbol": row.nativeSymbol,
            ] as [String: Any]
        }
        let tokens = (balance.tokens + balance.unpricedTokens).map { token in
            [
                "network": "chain-\(token.chainId)",
                "chain_id": token.chainId,
                "symbol": token.symbol,
                "balance": token.balance,
                "decimals": token.decimals,
                "token_address": token.tokenAddress.map { $0 as Any } ?? NSNull(),
                "price_usd": token.priceUsd.map { $0 as Any } ?? NSNull(),
                "logo_urls": [String](),
                "spam": token.spam,
            ] as [String: Any]
        }
        return CoreJSON.string(["type": "tokens_loaded", "tokens": tokens, "chains": chains])
    }

    // MARK: - The fee, through the live session

    private func estimateFee(_ operation: [String: Any]) async -> String {
        // A batch takes precedence only when it HAS legs; an empty one would
        // otherwise silence the single call beside it.
        let batch = operation["batch"] as? [[String: Any]]
        let single = operation["tx"] as? [String: Any]
        let calls: [[String: Any]]
        if let batch, !batch.isEmpty { calls = batch }
        else if let single { calls = [single] }
        else { calls = [] }

        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let account = operation["account"] as? String ?? ""
        let publicKey = operation["public_key_hex"] as? String
        let deployed = await relay.isDeployed(chainId: chainId, address: account) ?? false

        let settled = await fees.quote(
            chainId: chainId,
            account: account,
            deployed: deployed,
            publicKeyAvailable: publicKey != nil,
            calls: calls,
            feeToken: operation["gas_fee_token"] as? String
        )
        guard let settled else {
            // Superseded by a newer request. The core still needs an answer for
            // THIS operation, and "we could not find out" is the honest one.
            return CoreJSON.string([
                "type": "fee_estimated",
                "outcome": ["type": "failed", "kind": "other"],
            ])
        }
        if let estimate = settled.fee, settled.failed == nil {
            return CoreJSON.string([
                "type": "fee_estimated",
                "outcome": ["type": "ok", "estimate": estimate.coreJSON],
            ])
        }
        return CoreJSON.string([
            "type": "fee_estimated",
            "outcome": ["type": "failed", "kind": settled.failed ?? "other"],
        ])
    }

    // MARK: - Submit

    private func submit(_ operation: [String: Any]) async -> String {
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let account = operation["account"] as? String ?? ""
        let calls = (operation["calls"] as? [[String: Any]] ?? []).map { call in
            UserOpCall(
                to: call["to"] as? String ?? "",
                value: call["value"] as? String ?? "0",
                data: call["data"] as? String ?? "0x"
            )
        }
        let quoted = (operation["quoted_fee"] as? [String: Any]).map { fee in
            UserOpSpine.Quoted(
                amount: fee["amount"] as? String ?? "",
                recipient: fee["recipient"] as? String ?? "",
                // The speed this fee was priced at (spec 069).
                tier: fee["tier"] as? String
            )
        }
        let task = Task<String, Error> { [spine, ports] in
            try await spine.submit(
                chainId: chainId,
                account: account,
                calls: calls,
                gasFeeToken: operation["gas_fee_token"] as? String,
                quotedFee: quoted,
                signingStarted: { ports.signingStarted() }
            )
        }
        signing = task
        defer { signing = nil }
        do {
            let hash = try await task.value
            return CoreJSON.string([
                "type": "submitted",
                "user_op_hash": hash,
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])
        } catch let refused as UserOpSpine.Refused {
            return CoreJSON.string([
                "type": "submit_failed", "failure": Self.failureWire(refused.failure),
            ])
        } catch {
            // A cancelled Task lands here. The core routes a cancel back to
            // confirm; anything else would raise an error surface over a
            // ceremony the person themselves stopped.
            return CoreJSON.string([
                "type": "submit_failed", "failure": ["type": "passkey_cancelled"],
            ])
        }
    }

    /// What the recipient is, as far as this shell can honestly say.
    ///
    /// **`is_contract` is a judgement the shell is asked for here and is NOT
    /// asked for in `contacts`** — there, `classify_recipient` hands the core
    /// the raw `eth_getCode` bytes and `contacts.rs` decides, because an
    /// EIP-7702 delegated EOA has code and is still a wallet (its invariant
    /// ⑦). This operation's wire is a `bool`, so every client answers the
    /// simple question and a delegated wallet reads as a contract.
    ///
    /// Answered the same way web, desktop and Android answer it, deliberately:
    /// a client that were uniquely clever here would disagree with the other
    /// three about the same address. The right fix is the core's predicate
    /// exported once — a bridge change this cut promised not to make — and it
    /// is recorded in results.md rather than quietly diverged.
    ///
    /// Nothing on the iOS send screen draws this today; it is answered because
    /// the core asks, not because a pixel depends on it.
    private func resolveRisk(_ operation: [String: Any]) async -> String {
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let address = operation["address"] as? String ?? ""
        let deployed = await relay.isDeployed(chainId: chainId, address: address)
        // Has this device ever paid them? The local store is the only honest
        // source, and it is the same one `contacts::load_send_history` reads.
        let paid = TxRecords.load(store: store).contains { record in
            (record["to"] as? String)?.lowercased() == address.lowercased()
        }
        return CoreJSON.string([
            "type": "risk_resolved",
            "risk": [
                "is_contract": deployed.map { $0 as Any } ?? NSNull(),
                "first_time": !paid,
            ] as [String: Any],
        ])
    }

    // MARK: - Wire helpers

    /// The feed's own camelCase row for a submitted send (data-model.md).
    static func feedRow(_ record: [String: Any]) -> [String: Any] {
        [
            "id": record["id"] as? String ?? "",
            "userOpHash": record["user_op_hash"] as? String ?? "",
            "txHash": record["tx_hash"] as? String ?? "",
            "from": record["from"] as? String ?? "",
            "to": record["to"] as? String ?? "",
            "toName": record["to_name"].flatMap { $0 is NSNull ? nil : $0 } ?? NSNull(),
            "value": record["value"] as? String ?? "",
            "symbol": record["symbol"] as? String ?? "",
            "decimals": (record["decimals"] as? NSNumber)?.intValue ?? 18,
            "logoUrls": record["logo_urls"] as? [String] ?? [],
            "chainId": (record["chain_id"] as? NSNumber)?.intValue ?? 0,
            // SECONDS. Writing milliseconds puts every send in the year 57000
            // and the feed's day grouping silently stops working.
            "timestamp": (record["timestamp_s"] as? NSNumber)?.doubleValue
                ?? Date().timeIntervalSince1970,
            "usd": record["usd"].flatMap { $0 is NSNull ? nil : $0 } ?? NSNull(),
            "status": "pending",
            "type": "send",
        ]
    }

    private static func probeWire(_ probe: RelayClient.TreasuryProbe) -> [String: Any] {
        switch probe {
        case .covered: return ["type": "covered"]
        case .uncovered: return ["type": "uncovered"]
        case .unknown: return ["type": "unknown"]
        case .lowFloat(let status): return ["type": "low_float", "status": status]
        }
    }

    private static func failureWire(_ failure: UserOpSpine.Failure) -> [String: Any] {
        switch failure {
        case .passkeyCancelled: return ["type": "passkey_cancelled"]
        case .relayerUnavailable: return ["type": "relayer_unavailable"]
        case .bundlerUnderfunded: return ["type": "bundler_underfunded"]
        case .other(let message):
            return ["type": "other", "message": message.map { $0 as Any } ?? NSNull()]
        }
    }

    /// What the core hears when an arm threw: nothing was done.
    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "fetch_tokens":
            return CoreJSON.string(["type": "tokens_loaded", "tokens": NSNull(), "chains": []])
        case "clear_token_cache":
            return CoreJSON.string(["type": "token_cache_cleared"])
        case "resolve_token_metadata":
            return CoreJSON.string(["type": "token_metadata", "meta": NSNull()])
        case "add_network":
            return CoreJSON.string(["type": "network_added", "outcome": ["type": "error"]])
        case "estimate_fee":
            return CoreJSON.string([
                "type": "fee_estimated", "outcome": ["type": "failed", "kind": "other"],
            ])
        case "probe_treasury":
            return CoreJSON.string(["type": "treasury_probed", "probe": ["type": "unknown"]])
        case "load_account_credential":
            return CoreJSON.string(["type": "account_credential", "public_key_hex": NSNull()])
        case "submit_user_op":
            return CoreJSON.string([
                "type": "submit_failed",
                "failure": ["type": "other", "message": NSNull()] as [String: Any],
            ])
        case "cancel_passkey_sign":
            return CoreJSON.string(["type": "passkey_cancel_acknowledged"])
        case "persist_tx_records":
            return CoreJSON.string(["type": "records_persisted"])
        case "track_submitted":
            return CoreJSON.string(["type": "track_handed_off"])
        case "resolve_identity":
            return CoreJSON.string(["type": "identity_resolved", "identity": NSNull()])
        case "resolve_risk":
            return CoreJSON.string(["type": "risk_resolved", "risk": NSNull()])
        case "simulate_calls":
            return CoreJSON.string(["type": "sim_resolved", "sim_json": NSNull()])
        case "start_timer":
            return CoreJSON.string(["type": "timer_elapsed", "tag": operation["tag"] ?? NSNull()])
        case "haptic":
            return CoreJSON.string(["type": "haptic_played"])
        case "show_alert":
            return CoreJSON.string(["type": "alert_acknowledged"])
        default:
            return CoreJSON.string(["type": "closed"])
        }
    }
}

/// `UserOpSpine.AccountPort` over the account store.
///
/// The whole key set goes out, pinned key first: a Safe's address is a function
/// of every key it was founded with, so a signature packed against a subset
/// verifies against a different account entirely.
@MainActor
struct SendAccountPort: UserOpSpine.AccountPort {
    let accounts: AccountStore

    func keyRoutesJson(of address: String) async -> String {
        guard let record = await record(for: address) else { return "[]" }
        let routes = (record["keys"] as? [[String: Any]] ?? []).map { key -> [String: String] in
            [
                "credential_id": key["credential_id"] as? String ?? key["credentialId"] as? String ?? "",
                "transports": key["transports"] as? String ?? "",
            ]
        }
        let data = (try? JSONSerialization.data(withJSONObject: routes)) ?? Data("[]".utf8)
        return String(decoding: data, as: UTF8.self)
    }

    func keys(of address: String) async -> [WalletKeyRecord] {
        guard let record = await record(for: address) else { return [] }
        let keys = (record["keys"] as? [[String: Any]] ?? []).compactMap { key -> WalletKeyRecord? in
            guard let id = key["credential_id"] as? String ?? key["credentialId"] as? String,
                  let hex = key["public_key_hex"] as? String ?? key["publicKeyHex"] as? String
            else { return nil }
            return WalletKeyRecord(credentialId: id, publicKeyHex: hex)
        }
        guard keys.isEmpty else { return keys }
        // A record written before the key set existed carries the scalars, and
        // the core reads that as a legacy single-key account.
        guard let id = record["id"] as? String,
              let hex = record["public_key_hex"] as? String ?? record["publicKeyHex"] as? String,
              !hex.isEmpty
        else { return [] }
        return [WalletKeyRecord(credentialId: id, publicKeyHex: hex)]
    }

    /// The pinned key's stored transports and method.
    ///
    /// The METHOD outranks the transport hints, which is why it is derived from
    /// them rather than defaulted: a caBLE credential carries the wide hint set,
    /// and a `platform` default would send somebody who signs on another phone
    /// looking for a security key they do not own.
    func routing(of address: String) async -> (transports: String, method: KeyMethod) {
        let record = await record(for: address)
        let first = (record?["keys"] as? [[String: Any]])?.first
        let transports = (first?["transports"] as? String) ?? ""
        let hints = Set(transports.split(separator: ",").map {
            $0.trimmingCharacters(in: .whitespaces)
        })
        let method: KeyMethod
        if hints.contains("hybrid") && !hints.contains("internal") { method = .hybrid }
        else if hints.contains("usb") || hints.contains("nfc") { method = .securityKey }
        else { method = .platform }
        return (transports, method)
    }

    private func record(for address: String) async -> [String: Any]? {
        await accounts.loadAccounts().first {
            ($0["address"] as? String)?.lowercased() == address.lowercased()
        }
    }
}
