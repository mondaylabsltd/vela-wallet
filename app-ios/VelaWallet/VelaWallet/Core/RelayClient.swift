//
//  RelayClient.swift
//  VelaWallet
//
//  The relay (bundler) and the chain, as the send path talks to them.
//
//  Ported from `app-android/.../feature/send/core/RelayClient.kt` (spec 043,
//  research D8), which is itself the desktop's `executor/relay.rs`. **Transport
//  only.** No number is computed here: quantities cross as decimal strings for
//  the core to price, and the relay's sentences cross for the core to classify.
//
//  ## Every call is routed, including the relay's own
//
//  JSON-RPC goes through `RpcPool` with `kind: "bundler"`, so bans and
//  cooldowns apply to the relay exactly as they do to a chain endpoint. REST
//  goes to the bundler base **the pool names for that chain** (the core's
//  invariant ③: account-info must resolve to the same bundler the pool would
//  submit to — Tempo's gas reimbursement is paid to that bundler's per-Safe
//  EOA, and reading it from a different one reimburses the wrong address),
//  with the pool's best RPC URL riding `X-Rpc-Url` so the relay reads the
//  chain through the endpoint this wallet picked.
//
//  ## Two caches, and what clears them
//
//  In-band quotes for 8 s — a confirm screen re-quotes on every fee-token tap
//  and the relay would rather not be asked four times a second — and account
//  info for 30 s. `clearCaches()` is what `network_admin`'s
//  `clear_bundler_cache` has meant on the other clients and now means here; it
//  was a truthful no-op on this client until this file existed.
//
//  The caches are plain dictionaries and need no lock: this class is
//  `@MainActor`, so two readers cannot interleave. Two callers CAN both miss
//  and both fetch, which is what Android's lock also allows — it guards the
//  map, never the round trip — and both write the same answer.
//

import Foundation
import VelaCore

/// The relay client's view of the pool and the network. A protocol so tests
/// script answers without a socket.
@MainActor
protocol RelayPort {
    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome
    func bundlerBase(chainId: Int) async -> String?
    func bestRpcUrl(chainId: Int) async -> String?
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer
}

/// The real port: the pool for JSON-RPC and routing, `CoreHTTP` for REST.
@MainActor
struct PoolRelayPort: RelayPort {
    let pool: RpcPool

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        await pool.call(chainId: chainId, method: method, params: params, kind: kind)
    }

    func bundlerBase(chainId: Int) async -> String? { await pool.bundlerBase(chainId: chainId) }

    func bestRpcUrl(chainId: Int) async -> String? { await pool.bestRpcUrl(chainId: chainId) }

    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer {
        // Spec 081 FR-007: the wallet no longer names its preferred RPC endpoint
        // to the relay — that URL can carry a provider API key, and the relay
        // reads `x-vela-rpc-url`, never this header.
        _ = xRpcUrl
        return await CoreHTTP.getREST(url, headers: [:], timeout: CoreHTTP.Timeout.networkCheck)
    }
}

@MainActor
final class RelayClient {

    /// What the treasury probe found, in the core's vocabulary.
    enum TreasuryProbe {
        case covered
        case uncovered
        case unknown
        case lowFloat(status: [String: Any])
    }

    /// What the relay knows about this account on this chain.
    struct AccountInfo {
        let depositAddress: String
        let settlementRecipient: String?
        let status: String

        /// Where an in-band fee leg pays: the settlement recipient, else the
        /// deposit address.
        var feeRecipient: String? {
            settlementRecipient ?? (depositAddress.isEmpty ? nil : depositAddress)
        }
    }

    /// What one bundler call came back as.
    ///
    /// Three outcomes rather than an optional, because the relay's **sentence**
    /// is the thing the core classifies (`relay_error_message`,
    /// `parse_existing_user_op_hash`, `classify_relay_rejection`). Collapsing a
    /// refusal into `nil` would turn "you already have this operation pending"
    /// into "the relay is unreachable", and the wallet would submit it again.
    enum BundlerAnswer {
        case value(Any?)
        case refused(message: String)
        case unreachable
    }

    enum EstimateAnswer {
        case estimated(verificationGasLimit: String, callGasLimit: String, preVerificationGas: String)
        /// The relay answered and refused; the sentence is for the log and the
        /// core, never for arithmetic here.
        case refused(String)
        case unreachable
    }

    enum SubmitAnswer {
        case accepted(userOpHash: String)
        /// The relay's error as JSON — the core turns it into a sentence and a
        /// class.
        case rejected(errorJson: String)
        case unreachable
    }

    enum ReceiptAnswer {
        case unreachable
        case pending
        case resolved(confirmed: Bool, txHash: String, sender: String?, logs: [[String: Any]])
    }

    struct GasSignals: Equatable {
        let ethGasPrice: String?
        let baseFee: String?
        let priorityFee: String?
    }

    private static let quoteTTLMs: Double = 8_000
    private static let infoTTLMs: Double = 30_000
    private static let submitMaxRetries = 3
    static let submitRetryDelayMs: Double = 3_000

    private let port: RelayPort
    /// The configured relay host, for when the pool names no base for a chain.
    private let builtinBase: () async -> String
    private let now: () -> Double
    /// The submit retry's pause; tests set zero.
    private let retryDelayMs: Double

    private var quoteCache: [String: (quotes: [[String: Any]], at: Double)] = [:]
    private var infoCache: [String: (info: AccountInfo, at: Double)] = [:]

    // The fee's inputs, held still (issue 212; iOS's since spec 069). The fee
    // session re-samples on every quote run, and since 069 up to three
    // sessions price one send at once, one per speed. Uncached, a recipient
    // edit re-rolled the gas price, and three tiers priced on three readings
    // could not be compared. A COMPLETE reading is held for the core's window
    // (`fee_policy::FEE_SIGNALS_CACHE_TTL_MS`, 15 s) per chain, what counts as
    // complete is the core's rule too, and it is dropped by the refresh control,
    // a failed quote and every submit (`invalidateFeeSignals`). The epoch
    // stops a read that was in flight when somebody asked for a fresh one
    // from landing its older answer in the cache.
    private static let feeSignalsTTLMs = Double(feeSignalsCacheTtlMs())
    private var gasSignalsCache: [String: (signals: GasSignals, at: Double)] = [:]
    private var bundlerQuoteCache: [String: (quote: [String: Any], at: Double)] = [:]
    private var feeSignalsEpoch: [Int: Int] = [:]

    /// Forget this chain's held readings, so the next quote run measures again.
    func invalidateFeeSignals(chainId: Int) {
        feeSignalsEpoch[chainId, default: 0] += 1
        gasSignalsCache = gasSignalsCache.filter { !$0.key.hasPrefix("\(chainId):") }
        bundlerQuoteCache = bundlerQuoteCache.filter { !$0.key.hasPrefix("\(chainId):") }
    }

    init(
        port: RelayPort,
        builtinBase: @escaping () async -> String = { NetDefaults.bundlerServiceURL },
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 },
        retryDelayMs: Double = RelayClient.submitRetryDelayMs
    ) {
        self.port = port
        self.builtinBase = builtinBase
        self.now = now
        self.retryDelayMs = retryDelayMs
    }

    // MARK: - REST

    private func restGet(chainId: Int, path: String) async -> CoreHTTP.RestAnswer {
        // The pool's answer when it has one; otherwise the CONFIGURED relay —
        // `NetDefaults` is the last resort, not the fallback. Spelled out
        // rather than `??` because the right-hand side is an async call and
        // `??` takes a non-async autoclosure.
        var base: String
        if let pooled = await port.bundlerBase(chainId: chainId) {
            base = pooled
        } else {
            base = await builtinBase()
        }
        while base.hasSuffix("/") { base.removeLast() }
        let xRpcUrl = await port.bestRpcUrl(chainId: chainId)
        return await port.restGet(url: base + path, xRpcUrl: xRpcUrl)
    }

    /// `GET /v1/treasury/{chain}`.
    ///
    /// **404 means "this chain is not covered", never "down".** The two lead to
    /// different screens: one says the network has no gas sponsorship at all,
    /// the other says try again in a minute.
    func probeTreasury(chainId: Int) async -> TreasuryProbe {
        let data: [String: Any]
        switch await restGet(chainId: chainId, path: "/v1/treasury/\(chainId)") {
        case .ok(let json): data = json
        case .status(let code): return code == 404 ? .uncovered : .unknown
        case .failed: return .unknown
        }
        let addressRaw = data["address"] as? String
        guard Self.isAddress(addressRaw), let address = addressRaw else { return .unknown }
        let bootstrapNeeded = data["bootstrapNeeded"] as? Bool ?? false
        let status: [String: Any] = [
            "chain_id": chainId,
            "address": address,
            "asset": (data["asset"] as? String) == "pathUSD"
                ? ["type": "path_usd"] : ["type": "native"],
            "balance": Self.decimalOfAny(data["balance"]) ?? "0",
            "floor": Self.decimalOfAny(data["floor"]) ?? "0",
            "bootstrap_needed": bootstrapNeeded,
        ]
        return bootstrapNeeded ? .lowFloat(status: status) : .covered
    }

    func accountInfo(chainId: Int, safe: String) async -> AccountInfo? {
        let key = "\(chainId):\(safe.lowercased())"
        if let cached = infoCache[key], now() - cached.at < Self.infoTTLMs { return cached.info }
        guard case .ok(let data) = await restGet(
            chainId: chainId, path: "/v1/account/\(chainId)/\(safe.lowercased())"
        ) else { return nil }
        let recipient = data["settlementRecipient"] as? String
        let info = AccountInfo(
            depositAddress: data["activeDepositAddress"] as? String ?? "",
            // A corrupted field degrades to the deposit-address fallback; it
            // must never poison the fee leg.
            settlementRecipient: Self.isAddress(recipient) ? recipient : nil,
            status: (data["status"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "UNKNOWN"
        )
        infoCache[key] = (info, now())
        return info
    }

    // MARK: - Bundler JSON-RPC

    private func bundlerCall(chainId: Int, method: String, params: [Any]) async -> BundlerAnswer {
        switch await port.call(chainId: chainId, method: method, params: params, kind: "bundler") {
        case .ok(let value): return .value(value)
        case .rpcError(_, let message): return .refused(message: message)
        case .failed, .rangeCap: return .unreachable
        }
    }

    /// The value of a bundler answer, for the reads whose only honest
    /// degradation is "I could not find out" — the core models every one of
    /// them with an optional field.
    private func bundlerValue(chainId: Int, method: String, params: [Any]) async -> Any? {
        if case .value(let value) = await bundlerCall(chainId: chainId, method: method, params: params) {
            return value
        }
        return nil
    }

    /// `vela_getInBandGasQuote`: the fee assets the relay accepts for this
    /// account, with what it holds of each. `nil` = the relay could not be
    /// asked, or answered nothing usable.
    ///
    /// Without a native USD price only the payable native row survives, as on
    /// every other client — a stablecoin cannot be converted without one.
    func inBandQuotes(chainId: Int, safe: String) async -> [[String: Any]]? {
        let key = "\(chainId):\(safe.lowercased())"
        if let cached = quoteCache[key], now() - cached.at < Self.quoteTTLMs { return cached.quotes }
        guard let result = await bundlerValue(
            chainId: chainId,
            method: "vela_getInBandGasQuote",
            params: [["safeAddress": safe]]
        ) as? [[String: Any]] else { return nil }
        let quotes = result.compactMap(Self.quoteRow)
        let nativeUnpriced = quotes
            .first { Self.assetKind($0) == "native" }
            .map { $0["usd_price"] is NSNull } ?? false
        let usable = nativeUnpriced ? quotes.filter { Self.assetKind($0) == "native" } : quotes
        guard !usable.isEmpty else { return nil }
        quoteCache[key] = (usable, now())
        return usable
    }

    private static func assetKind(_ quote: [String: Any]) -> String? {
        quote["asset"] as? String
    }

    /// One `vela_getInBandGasQuote` row, or nothing if it is not well-formed.
    ///
    /// The desktop's `parse_quote_row`, field for field, **including the two
    /// shapes that caught Android's first device run**: the relay writes
    /// `balance` as HEX and `feeToken` as JSON null on the native row. Reading
    /// the balance as a decimal drops every row, silently.
    private static func quoteRow(_ row: [String: Any]) -> [String: Any]? {
        let recipientRaw = row["recipient"] as? String
        guard isAddress(recipientRaw), let recipient = recipientRaw else { return nil }
        let kind: String
        switch row["asset"] as? String {
        case "native": kind = "native"
        case "erc20": kind = "erc20"
        default: return nil
        }
        let feeTokenRaw = row["feeToken"] as? String
        let feeToken = isAddress(feeTokenRaw) ? feeTokenRaw : nil
        guard let decimals = (row["decimals"] as? NSNumber)?.intValue, decimals >= 0 else { return nil }
        let symbol = (row["symbol"] as? String)?.trimmingCharacters(in: .whitespaces) ?? ""
        guard !symbol.isEmpty else { return nil }
        let usdPrice = decimalText(row["usdPrice"])
        // USD values are conversion metadata: native gas prices from gas units
        // alone, a stablecoin needs its own price or it cannot be converted.
        if kind == "erc20", feeToken == nil || usdPrice == nil { return nil }
        return [
            "recipient": recipient,
            // A PLAIN STRING. `FeeAssetKind` is a fieldless enum, not a tagged
            // union — `{"type":"native"}` is rejected by serde, and a rejected
            // result means the fee machine waits for an answer that already
            // came. It showed up as 估算中… forever, with no error anywhere.
            "asset": kind,
            "fee_token": kind == "erc20" ? (feeToken.map { $0 as Any } ?? NSNull()) : NSNull(),
            "balance": decimalOfAny(row["balance"]) ?? "0",
            "decimals": decimals,
            "symbol": symbol,
            "usd_balance": decimalText(row["usdBalance"]) ?? "0",
            "usd_price": usdPrice.map { $0 as Any } ?? NSNull(),
        ]
    }

    /// `pimlico_getUserOperationGasPrice`, one tier.
    func bundlerQuote(chainId: Int, tier: String) async -> [String: Any]? {
        let key = "\(chainId):\(tier)"
        if let held = bundlerQuoteCache[key], now() - held.at < Self.feeSignalsTTLMs {
            return held.quote
        }
        let epoch = feeSignalsEpoch[chainId, default: 0]
        guard let result = await bundlerValue(
            chainId: chainId, method: "pimlico_getUserOperationGasPrice", params: []
        ) as? [String: Any],
            let row = result[tier] as? [String: Any],
            let maxFee = Self.decimalOfHex(row["maxFeePerGas"])
        else { return nil }
        let quote: [String: Any] = [
            "max_fee_per_gas": maxFee,
            // The tip this tier is signed with — what the core turns into the
            // gas bid on screen (issue 684). Absent on a generic bundler.
            "max_priority_fee_per_gas": Self.decimalOfHex(row["maxPriorityFeePerGas"]).map { $0 as Any } ?? NSNull(),
            "network_fee_per_gas": Self.decimalOfHex(row["networkFeePerGas"]).map { $0 as Any } ?? NSNull(),
            "relayer_fee_per_gas": Self.decimalOfHex(row["relayerFeePerGas"]).map { $0 as Any } ?? NSNull(),
        ]
        // Never a zero cap, which the core rejects as degenerate: "the relay
        // did not answer" is not a measurement worth holding.
        if bundlerQuoteCacheable(maxFeePerGas: maxFee), feeSignalsEpoch[chainId, default: 0] == epoch {
            bundlerQuoteCache[key] = (quote, now())
        }
        return quote
    }

    /// `eth_estimateUserOperationGas` for a draft's relay JSON.
    ///
    /// A refusal and an unreachable relay are kept apart because the spine
    /// treats them differently: only a batch carrying a real contract call
    /// stops on a failed estimate.
    func estimateUserOpGas(chainId: Int, opJson: String) async -> EstimateAnswer {
        guard let op = Self.object(fromJSON: opJson) else {
            return .refused("The operation could not be encoded")
        }
        let result: [String: Any]
        switch await bundlerCall(
            chainId: chainId,
            method: "eth_estimateUserOperationGas",
            params: [op, entryPointAddress()]
        ) {
        case .value(let value):
            guard let object = value as? [String: Any] else {
                return .refused("Failed to estimate gas — empty result")
            }
            result = object
        case .refused(let message):
            return .refused(message.isEmpty ? "Gas estimation failed" : message)
        case .unreachable:
            return .unreachable
        }
        guard let verification = Self.decimalOfHex(result["verificationGasLimit"]) else {
            return .refused("verificationGasLimit missing")
        }
        guard let call = Self.decimalOfHex(result["callGasLimit"]) else {
            return .refused("callGasLimit missing")
        }
        guard let preVerification = Self.decimalOfHex(result["preVerificationGas"]) else {
            return .refused("preVerificationGas missing")
        }
        return .estimated(
            verificationGasLimit: verification,
            callGasLimit: call,
            preVerificationGas: preVerification
        )
    }

    /// `eth_sendUserOperation`, retried up to three times while the relay says
    /// it is busy — the web's loop, and the one place this file waits.
    ///
    /// A rejection is returned as the relay's own sentence wrapped in an error
    /// envelope, untouched: the core reads it (`relay_error_message`), looks
    /// for an operation already pending for this nonce
    /// (`parse_existing_user_op_hash` — an idempotent re-submit, never a second
    /// spend) and classifies the rest. No Swift here decides what a refusal
    /// means.
    ///
    /// `tier` is the speed the displayed fee was priced at, sent as the
    /// optional third parameter (spec 068's relay contract; iOS's since 069).
    /// A NAME, never a wei figure: the relay resolves it at submit time and
    /// clamps it between its inclusion floor and what the signed reimbursement
    /// funds. `nil` sends the pre-068 two-element params exactly, and the dead
    /// `rapid` is never sent — a relay refuses an unknown name with -32602.
    func sendUserOp(chainId: Int, opJson: String, tier: String? = nil) async -> SubmitAnswer {
        guard let op = Self.object(fromJSON: opJson) else {
            return .rejected(errorJson: Self.errorEnvelope("the operation could not be encoded"))
        }
        let params = Self.submitParams(op, tier: tier)
        var attempt = 0
        while true {
            switch await bundlerCall(
                chainId: chainId,
                method: "eth_sendUserOperation",
                params: params
            ) {
            case .value(let value):
                guard let hash = value as? String, hash.hasPrefix("0x") else {
                    return .rejected(errorJson: Self.errorEnvelope("the relay accepted nothing"))
                }
                return .accepted(userOpHash: hash)
            case .unreachable:
                return .unreachable
            case .refused(let message):
                let busy = message.contains("currently processing") || message.contains("Retry later")
                guard busy, attempt < Self.submitMaxRetries else {
                    return .rejected(errorJson: Self.errorEnvelope(message))
                }
                attempt += 1
                try? await Task.sleep(nanoseconds: UInt64(max(0, retryDelayMs) * 1_000_000))
            }
        }
    }

    /// `eth_getUserOperationReceipt`: unreachable, not yet, or landed with its
    /// logs.
    func userOpReceipt(chainId: Int, userOpHash: String) async -> ReceiptAnswer {
        guard !userOpHash.isEmpty else { return .unreachable }
        guard case .value(let result) = await bundlerCall(
            chainId: chainId, method: "eth_getUserOperationReceipt", params: [userOpHash]
        ) else { return .unreachable }
        guard let object = result as? [String: Any] else { return .pending }
        guard let receipt = object["receipt"] as? [String: Any],
              let txHash = receipt["transactionHash"] as? String, txHash.hasPrefix("0x")
        else { return .pending }
        let logs = (receipt["logs"] as? [[String: Any]] ?? []).compactMap(Self.trustLog)
        // `success` absent means success; only an explicit `false` is failure.
        let confirmed = (object["success"] as? NSNumber)?.boolValue ?? true
        return .resolved(
            confirmed: confirmed,
            txHash: txHash,
            sender: (object["sender"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            logs: logs
        )
    }

    private static func trustLog(_ log: [String: Any]) -> [String: Any]? {
        guard let address = log["address"] as? String, !address.isEmpty,
              let topics = log["topics"] as? [Any]
        else { return nil }
        return [
            "address": address,
            "topics": topics.compactMap { ($0 as? String).flatMap { $0.isEmpty ? nil : $0 } },
            "data": (log["data"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x",
        ]
    }

    /// `eth_getUserOperationStatus` (a Vela extension): `nil` for an older
    /// relay or a failure — which the core reads as `status_unavailable`, never
    /// as a verdict.
    func userOpStatus(chainId: Int, userOpHash: String) async -> (status: String, stage: String?)? {
        guard !userOpHash.isEmpty else { return nil }
        guard let result = await bundlerValue(
            chainId: chainId, method: "eth_getUserOperationStatus", params: [userOpHash]
        ) as? [String: Any] else { return nil }
        let known = [
            "not_found", "queued", "not_submitted", "submitted",
            "rejected", "included", "failed",
        ]
        guard let status = result["status"] as? String, known.contains(status) else { return nil }
        return (status, (result["last_executor_stage"] as? String).flatMap { $0.isEmpty ? nil : $0 })
    }

    // MARK: - The chain reads the send path needs

    /// A chain read whose only honest degradation is `nil`.
    ///
    /// A JSON-RPC error reads as `nil` here too, deliberately: a nonce the node
    /// refused to compute is not a nonce. The spine treats that `nil` as fatal
    /// **before** the passkey prompt, which is why it is read first.
    private func chainCall(chainId: Int, method: String, params: [Any]) async -> Any? {
        if case .ok(let value) = await port.call(
            chainId: chainId, method: method, params: params, kind: "rpc"
        ) { return value }
        return nil
    }

    /// The three raw gas signals, each `nil` when unreadable; the core prices
    /// with what it has, and only a failed `eth_gasPrice` triggers its default.
    func gasSignals(chainId: Int, wantTip: Bool) async -> GasSignals {
        let key = "\(chainId):gas:\(wantTip)"
        if let held = gasSignalsCache[key], now() - held.at < Self.feeSignalsTTLMs {
            return held.signals
        }
        let epoch = feeSignalsEpoch[chainId, default: 0]
        let gasPrice = await chainCall(chainId: chainId, method: "eth_gasPrice", params: [])
        let block = await chainCall(
            chainId: chainId, method: "eth_getBlockByNumber", params: ["latest", false]
        )
        // Tempo has no priority fee to ask for, and asking corrupts the
        // stablecoin reimbursement — the core says `want_tip: false` there.
        let tip: Any? = wantTip
            ? await chainCall(chainId: chainId, method: "eth_maxPriorityFeePerGas", params: [])
            : nil
        let signals = GasSignals(
            ethGasPrice: Self.decimalOfHex(gasPrice),
            baseFee: Self.decimalOfHex((block as? [String: Any])?["baseFeePerGas"]),
            priorityFee: Self.decimalOfHex(tip)
        )
        // Only a COMPLETE reading is held: every leg that was asked for
        // answered, and the price is positive. A block that ANSWERED without a
        // base fee is a real pre-London reading; one that did not answer is a
        // failed leg.
        let complete = gasSignalsCacheable(
            ethGasPrice: signals.ethGasPrice, blockAnswered: block is [String: Any],
            wantTip: wantTip, priorityFee: signals.priorityFee
        )
        if complete, feeSignalsEpoch[chainId, default: 0] == epoch {
            gasSignalsCache[key] = (signals, now())
        }
        return signals
    }

    /// `[userOperation, entryPoint, tier?]`: two elements, or three with a
    /// speed. Only the three offered names ever reach the relay.
    static func submitParams(_ op: Any, tier: String?) -> [Any] {
        guard let tier, ["fast", "standard", "slow"].contains(tier) else {
            return [op, entryPointAddress()]
        }
        return [op, entryPointAddress(), tier]
    }

    /// `EntryPoint.getNonce(sender, 0)` as a hex quantity; `nil` when
    /// unreadable — which the spine treats as fatal before any prompt.
    func nonce(chainId: Int, sender: String) async -> String? {
        guard let selector = try? functionSelector(signature: "getNonce(address,uint192)") else {
            return nil
        }
        let data = "0x" + selector.map { String(format: "%02x", $0) }.joined()
            + Self.addressWord(sender) + String(repeating: "0", count: 64)
        guard let result = await chainCall(
            chainId: chainId,
            method: "eth_call",
            params: [["to": entryPointAddress(), "data": data], "latest"]
        ) as? String, result.hasPrefix("0x"), result.count > 2 else { return nil }
        // Re-minted as a canonical quantity: the node answers a padded 32-byte
        // word and the bundler wants `0x0`, not `0x000…0`.
        guard let decimal = TokenReads.scaled(hex: result, decimals: 0) else { return nil }
        return Self.hexQuantity(decimal: decimal)
    }

    /// `eth_getCode` != `0x`; `nil` when the chain could not be asked.
    func isDeployed(chainId: Int, address: String) async -> Bool? {
        guard let code = await chainCall(
            chainId: chainId, method: "eth_getCode", params: [address, "latest"]
        ) as? String, code.hasPrefix("0x") else { return nil }
        return code.count > 2
    }

    /// The recipient-risk answer's `is_contract` — NOT `isDeployed`, which asks
    /// whether the SENDER's Safe exists. The web's `isContractAddress` (and
    /// Android's `RelayClient.isContract`): an EIP-7702-delegated EOA carries
    /// `0xef0100 ++ implAddr` and is a person's WALLET, never badged a contract;
    /// any other code is a contract; `nil` when the chain could not be asked.
    func isContract(chainId: Int, address: String) async -> Bool? {
        guard let code = await chainCall(
            chainId: chainId, method: "eth_getCode", params: [address, "latest"]
        ) as? String, code.hasPrefix("0x") else { return nil }
        return Self.codeIsContract(code)
    }

    /// The rule alone, for tests: 7702 designator = wallet, other code = contract.
    static func codeIsContract(_ code: String) -> Bool {
        if code.range(of: "^0x[eE][fF]0100[0-9a-fA-F]{40}$", options: .regularExpression) != nil { return false }
        return code.count > 2
    }

    /// The bundler base this chain's REST calls would use. Test seam: the
    /// live suite prints it, because "which relay did it ask" is the first
    /// question when a quote does not come back.
    func bundlerBaseForTest(chainId: Int) async -> String? {
        await port.bundlerBase(chainId: chainId)
    }

    /// What `network_admin`'s `clear_bundler_cache` means on this client.
    func clearCaches() {
        quoteCache.removeAll()
        infoCache.removeAll()
    }

    // MARK: - Coercions

    static func isAddress(_ value: String?) -> Bool {
        guard let value, value.count == 42, value.hasPrefix("0x") else { return false }
        return value.dropFirst(2).allSatisfy(\.isHexDigit)
    }

    static func addressWord(_ address: String) -> String {
        let body = address.hasPrefix("0x") ? String(address.dropFirst(2)) : address
        return String(repeating: "0", count: max(0, 64 - body.count)) + body.lowercased()
    }

    /// A hex quantity as a decimal string; `nil` when absent or not hex.
    static func decimalOfHex(_ value: Any?) -> String? {
        guard let text = value as? String, text.hasPrefix("0x") else { return nil }
        return TokenReads.scaled(hex: text.count > 2 ? text : "0x0", decimals: 0)
    }

    /// `parseBigIntHex`: a `0x` hex, a bare hex, or a JSON number — anything
    /// else is nothing. The relay writes `balance` as hex and the desktop's
    /// parser accepts all three, so this does too.
    static func decimalOfAny(_ value: Any?) -> String? {
        if let number = value as? NSNumber { return String(max(0, number.int64Value)) }
        guard let text = value as? String else { return nil }
        let body = text.hasPrefix("0x") ? String(text.dropFirst(2)) : text
        guard !body.isEmpty, body.allSatisfy(\.isHexDigit) else { return nil }
        return TokenReads.scaled(hex: body, decimals: 0)
    }

    /// A decimal the relay wrote as a string or a number, kept **as written**.
    ///
    /// Parsed only to reject nonsense; the string that goes on is the original,
    /// because a round trip through `Double` is how a price loses its last
    /// digits.
    static func decimalText(_ value: Any?) -> String? {
        let raw: String
        if let text = value as? String { raw = text.trimmingCharacters(in: .whitespaces) }
        else if let number = value as? NSNumber { raw = number.stringValue }
        else { return nil }
        guard let parsed = Double(raw), parsed.isFinite, parsed >= 0 else { return nil }
        return raw
    }

    /// A decimal string as a minimal hex quantity (`"0"` → `"0x0"`), in string
    /// arithmetic — a nonce can exceed what a `UInt64` holds.
    static func hexQuantity(decimal: String) -> String {
        var digits = Array(decimal.compactMap(\.wholeNumberValue))
        guard !digits.isEmpty else { return "0x0" }
        var out = ""
        while digits.contains(where: { $0 != 0 }) {
            var remainder = 0
            var next: [Int] = []
            for digit in digits {
                let value = remainder * 10 + digit
                next.append(value / 16)
                remainder = value % 16
            }
            out.append(Character(String(remainder, radix: 16)))
            digits = Array(next.drop { $0 == 0 })
        }
        return out.isEmpty ? "0x0" : "0x" + String(out.reversed())
    }

    static func object(fromJSON text: String) -> [String: Any]? {
        guard let data = text.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    static func errorEnvelope(_ message: String) -> String {
        CoreJSON.string(["message": message])
    }
}
