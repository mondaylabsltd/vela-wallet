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
        "estimate_fee", "probe_treasury", "prewarm_fees", "load_account_credential", "submit_user_op",
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
        /// The Trusted Signer ended without a signature (spec 071). The core
        /// hears a cancelled ceremony — back to confirm, nothing sent — and
        /// the screen says which of the Trusted Signer's sentences applies.
        var trustedSignerEnded: (TrustedSignerNotice) -> Void = { _ in }
        /// What the asset list has so far, while `fetch_tokens` waits for its
        /// first round — `tokens_partial`, display-only (the store installs it).
        var tokensPartial: ([[String: Any]]) -> Void = { _ in }
    }

    /// How long `fetch_tokens` waits for the asset list's first round before
    /// it answers with what has arrived. The dashboard's own pull gives up at
    /// 20 s; a cold twelve-chain sweep is allowed a little longer here, since
    /// the alternative is "could not load" over money that is on its way.
    static let firstRoundWaitMs: Double = 30_000

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
    /// The round the balance machine last settled for this address in this
    /// run (`nil` = none yet) — the moment its tokens ARE the asset list, not
    /// a stream, and how "the next full load" is told from the last one.
    private let holdingsRound: (String) -> Int?
    /// Point the balance machine at this address (booting it if nothing has
    /// yet), so a round is on its way. Idempotent.
    private let openHoldings: (String) -> Void
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
        holdingsRound: @escaping (String) -> Int? = { _ in 0 },
        openHoldings: @escaping (String) -> Void = { _ in },
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
        self.holdingsRound = holdingsRound
        self.openHoldings = openHoldings
        self.ports = ports
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "fetch_tokens":
            return await fetchTokens(address: operation["address"] as? String ?? "")

        // The asset list re-reads; its new round reaches this screen as
        // `holdings_updated` (the store), like every other round does.
        case "clear_token_cache":
            ports.refreshBalances()
            return CoreJSON.string(["type": "token_cache_cleared"])

        // Answered at once; the reads run on without the core.
        case "prewarm_fees":
            prewarmFees(
                account: operation["account"] as? String ?? "",
                chainIds: (operation["chain_ids"] as? [NSNumber] ?? []).map(\.intValue)
            )
            return CoreJSON.string(["type": "fees_prewarmed"])

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
    ///
    /// **Never `null` for money that is merely on its way** (spec 078). The
    /// flow can open before the asset list has settled a round for this
    /// account — a cold start, a dashboard nobody booted — and answering
    /// `null` then raised "could not load" over a wallet that had simply not
    /// been read yet. So it waits for the first round, handing the picker what
    /// has arrived meanwhile (`tokens_partial`, as the home streams it).
    ///
    /// **And a round that reached nothing is read once more** (the desktop's
    /// rule, all four shells): one source has one failure mode — a round in
    /// which no chain answered, a proxy blip at launch — and it used to become
    /// "could not load tokens" at once, the picker empty until the next poll.
    /// So the dashboard is asked for ONE forced re-read (the home recovers with
    /// it) and the NEXT full load answers, whatever it holds; only a second
    /// load that reached nothing is the refusal.
    private func fetchTokens(address: String) async -> String {
        guard !address.isEmpty else { return tokensAnswer(balances(), address: address) }
        if holdingsRound(address) == nil { openHoldings(address) }
        var started = Date()
        var streamed: [BalanceTokenWire] = []
        /// `.some(round met)` once the one re-read is out (`nil` inside: it
        /// met no settled round, only an unreachable dashboard).
        var retriedFrom: Int?? = nil
        while Date().timeIntervalSince(started) * 1000 < Self.firstRoundWaitMs, !Task.isCancelled {
            let view = balances().flatMap { Self.sameAccount($0, address) ? $0 : nil }
            let round = holdingsRound(address)
            let unreachable = view?.unreachable == true
            if let view, round != nil || unreachable {
                let met: Int?? = .some(round)
                if !Self.reachedNothing(view) {
                    return tokensAnswer(view, address: address)
                }
                switch retriedFrom {
                case .none:
                    // The one re-read, and a fresh budget for it.
                    retriedFrom = met
                    started = Date()
                    ports.refreshBalances()
                case .some(let asked) where asked == round:
                    break // still the load that reached nothing: the re-read is out
                case .some:
                    return tokensAnswer(view, address: address) // the second empty load
                }
            }
            if let view, !view.tokens.isEmpty, view.tokens != streamed {
                streamed = view.tokens
                ports.tokensPartial(Self.sendTokens(view))
            }
            try? await Task.sleep(nanoseconds: 150_000_000)
        }
        return tokensAnswer(balances(), address: address)
    }

    /// A load that reached nothing: no holdings, and chains that did not
    /// answer (or nothing could be read at all). An account that holds
    /// nothing is an EMPTY list, which is an answer — not this.
    static func reachedNothing(_ balance: BalanceViewWire) -> Bool {
        balance.tokens.isEmpty && (!balance.failedChainIds.isEmpty || balance.unreachable == true)
    }

    /// `tokens_loaded` from the dashboard as it stands: `null` only for a load
    /// that reached nothing (or another account's), the holdings otherwise.
    private func tokensAnswer(_ balance: BalanceViewWire?, address: String) -> String {
        guard let balance, address.isEmpty || Self.sameAccount(balance, address),
              !Self.reachedNothing(balance)
        else {
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
        return CoreJSON.string([
            "type": "tokens_loaded", "tokens": Self.sendTokens(balance), "chains": chains,
        ])
    }

    /// The asset list's holdings in the send machine's vocabulary — ONE
    /// mapping, for the `fetch_tokens` answer, `tokens_partial` and
    /// `holdings_updated` alike, so the three can never describe the same
    /// holding two ways.
    ///
    /// **`tokens` only.** `unpriced_tokens` is a SUBSET of `tokens` (the
    /// detail sheet's "couldn't be priced" list), not its complement: adding
    /// the two listed every unpriceable holding twice in the picker — the
    /// same mistake `WalletLive.assetRows` documents for the home.
    static func sendTokens(_ balance: BalanceViewWire) -> [[String: Any]] {
        balance.tokens.map { token in
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
    }

    /// The dashboard is reading THIS account (addresses compare lower-cased).
    static func sameAccount(_ balance: BalanceViewWire, _ address: String) -> Bool {
        balance.address?.lowercased() == address.lowercased()
    }

    // MARK: - Fees, read ahead

    /// Warm the caches the fee executor reads, for each chain the person
    /// holds value on, while they are still choosing a token (spec 078).
    ///
    /// The SAME reads a quote makes, through the same `RelayClient` calls, so
    /// they land in the same caches — nothing is priced here and nothing comes
    /// back: the deployment read, the gas signals (no tip on Tempo, the core's
    /// `want_tip`), the relay's gas price for the tier in force (one response
    /// carries every tier), the in-band rows — and on Tempo the fee recipient
    /// instead of the gas price. Measured on the live relay those are 3–5 s of
    /// a 4.5–6 s first quote; read here, the quote a pick starts is left with
    /// its simulation. Every failure is swallowed: a read that did not warm is
    /// simply made again by the quote.
    private func prewarmFees(account: String, chainIds: [Int]) {
        guard !account.isEmpty, !chainIds.isEmpty else { return }
        let tier = fees.speed?.tier ?? "fast"
        let relay = self.relay
        Task {
            await withTaskGroup(of: Void.self) { group in
                for chainId in chainIds {
                    let tempo = isChainWithoutNativeCoin(chainId: UInt32(chainId))
                    group.addTask { _ = await relay.isDeployed(chainId: chainId, address: account) }
                    group.addTask { _ = await relay.gasSignals(chainId: chainId, wantTip: !tempo) }
                    group.addTask { _ = await relay.inBandQuotes(chainId: chainId, safe: account) }
                    if tempo {
                        group.addTask { _ = await relay.accountInfo(chainId: chainId, safe: account) }
                    } else {
                        group.addTask { _ = await relay.bundlerQuote(chainId: chainId, tier: tier) }
                    }
                }
            }
        }
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
            feeToken: operation["gas_fee_token"] as? String,
            // Nobody chose the coin on this form: the fee machine picks one
            // that can pay, and the estimate's `fee_asset` says which. A chip
            // tap makes the core send `false` from then on.
            autoFeeToken: operation["auto_fee_token"] as? Bool ?? false
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
            if case .trustedSigner(let notice) = refused.failure { ports.trustedSignerEnded(notice) }
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
    /// ⑦). This operation's wire is a `bool`, so the shell answers it with the
    /// web's `isContractAddress` rule (`RelayClient.isContract`): the 7702
    /// designator is a wallet, any other code a contract — the same answer
    /// web, desktop and Android give for the same address.
    ///
    /// Nothing on the iOS send screen draws this today; it is answered because
    /// the core asks, not because a pixel depends on it.
    private func resolveRisk(_ operation: [String: Any]) async -> String {
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let address = operation["address"] as? String ?? ""
        // A 7702-delegated EOA is a wallet (the web's `isContractAddress`).
        let deployed = await relay.isContract(chainId: chainId, address: address)
        // Has this device ever paid them? The local store is the only honest
        // source, and it is the same one `contacts::load_send_history` reads.
        let first = Self.firstTime(address, records: TxRecords.load(store: store))
        return CoreJSON.string([
            "type": "risk_resolved",
            "risk": [
                "is_contract": deployed.map { $0 as Any } ?? NSNull(),
                "first_time": first,
            ] as [String: Any],
        ])
    }

    /// `true` = this device never sent to `address`: the "first time sending
    /// here" tag, the poisoning defence. The web's `resolveRecipientRisk` /
    /// `hasPriorInteraction`, verbatim (Android's `FeedExecutor.hasSentTo`):
    /// `to` compared lower-cased, and only a `send`, a `dapp_tx` or a legacy
    /// row with no `type` counts — a receive or a signature is not a send.
    /// An unreadable store reads as no history (so the tag shows rather than
    /// hides); a non-address is never "first".
    static func firstTime(_ address: String, records: [[String: Any]]) -> Bool {
        guard address.range(of: "^0x[0-9a-fA-F]{40}$", options: .regularExpression) != nil
        else { return false }
        let lc = address.lowercased()
        let sent = records.contains { record in
            guard (record["to"] as? String)?.lowercased() == lc else { return false }
            guard let type = record["type"] else { return true }
            return (type as? String) == "send" || (type as? String) == "dapp_tx"
        }
        return !sent
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
        // Nothing was signed and the send may be signed another way: the
        // core's cancelled ceremony, which keeps the confirmation on screen.
        // The words are the screen's (`trustedSignerEnded`).
        case .trustedSigner: return ["type": "passkey_cancelled"]
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
        case "prewarm_fees":
            return CoreJSON.string(["type": "fees_prewarmed"])
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
            var route = [
                "credential_id": key["credential_id"] as? String ?? key["credentialId"] as? String ?? "",
                "transports": key["transports"] as? String ?? "",
            ]
            // Spec 075: a key minted or found through the Trusted Signer lives
            // behind that page, and `sign_route` will not find its way back
            // there without this. Dropping it here would silently send the
            // ceremony to a platform sheet that cannot see the key.
            if let origin = key["signer_origin"] as? String ?? key["signerOrigin"] as? String,
               !origin.isEmpty {
                route["signer_origin"] = origin
            }
            return route
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

    func name(of address: String) async -> String? {
        (await record(for: address)?["name"] as? String).flatMap { $0.isEmpty ? nil : $0 }
    }

    private func record(for address: String) async -> [String: Any]? {
        await accounts.loadAccounts().first {
            ($0["address"] as? String)?.lowercased() == address.lowercased()
        }
    }
}
