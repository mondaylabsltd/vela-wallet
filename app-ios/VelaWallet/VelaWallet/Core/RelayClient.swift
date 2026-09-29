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
//  ## The caches, and what clears them
//
//  In-band quotes for 8 s — a confirm screen re-quotes on every fee-token tap
//  and the relay would rather not be asked four times a second — and account
//  info for 30 s. `clearCaches()` is what `network_admin`'s
//  `clear_bundler_cache` has meant on the other clients and now means here; it
//  was a truthful no-op on this client until this file existed. The fee's own
//  inputs — gas signals, the relay's gas price, the simulation — are held for
//  the core's fee-signals window and dropped by `invalidateFeeSignals`. A
//  deployed account is remembered for good (a Safe cannot be un-deployed); an
//  undeployed one is asked every time, since the next send deploys it.
//
//  The caches are plain dictionaries and need no lock: this class is
//  `@MainActor`, so two readers cannot interleave. **And two readers cannot
//  both miss and both fetch** (spec 078): every fee read is single-flight
//  (`SingleFlight`), because the speed control prices one operation in three
//  sessions at the same instant, and three cold misses were three round trips
//  whose answers landed one row at a time.
//

import Foundation
import VelaCore

/// The relay client's view of the pool and the network. A protocol so tests
/// script answers without a socket.
@MainActor
protocol RelayPort {
    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome
    /// `call`, with whether any POST may have been acted on (spec 082 RA1) —
    /// what lets a submit say "not sent", and only then.
    func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult
    func bundlerBase(chainId: Int) async -> String?
    func bestRpcUrl(chainId: Int) async -> String?
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer
}

extension RelayPort {
    /// A port that knows only outcomes (a scripted one) cannot say whether a
    /// request left the device, so a give-up MAY have delivered: "not sent"
    /// is never a guess (contract §2). An answer — a value or a refusal — is
    /// a JSON reply, which is not.
    func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult {
        let outcome = await call(chainId: chainId, method: method, params: params, kind: kind)
        let maybe: Bool
        if case .failed = outcome { maybe = true } else { maybe = false }
        return RpcCallResult(outcome: outcome, maybeDelivered: maybe, heldErrorJson: nil)
    }
}

/// The real port: the pool for JSON-RPC and routing, `CoreHTTP` for REST.
@MainActor
struct PoolRelayPort: RelayPort {
    let pool: RpcPool

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        await pool.call(chainId: chainId, method: method, params: params, kind: kind)
    }

    func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult {
        await pool.callDetailed(chainId: chainId, method: method, params: params, kind: kind)
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

    /// What one submit came to, in the core's words (`user_op::submit_step`,
    /// spec 082 RA1). Three answers, and the middle one is the reason this
    /// type exists: a request that may have reached the relay and whose
    /// answer never came back is NOT "failed — try again" (G21, a paid
    /// double spend) and is not "sent" either.
    enum SubmitVerdict: Equatable {
        /// The relay holds the operation; its hash wins over the local one.
        case accepted(userOpHash: String)
        /// A POST may have been acted on and no answer came back: followed
        /// under the LOCAL hash, never retried as a new operation.
        case maybeSent(userOpHash: String)
        /// Nothing left the device (`rejection` nil), or the relay refused it
        /// and no earlier attempt can have delivered it — the core's class.
        case notSent(rejection: RelayRejection?)
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

    private let port: RelayPort
    /// The configured relay host, for when the pool names no base for a chain.
    private let builtinBase: () async -> String
    private let now: () -> Double
    /// The submit retry's pause; `nil` is the core's (`RetryAfter.delay_ms`),
    /// tests set zero.
    private let retryDelayMs: Double?

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
    /// The relay's simulation of one exact operation — chain, account,
    /// deployment and the calls byte for byte — for the same window (the
    /// core's `SIMULATION_CACHE_TTL_MS`, which IS `FEE_SIGNALS_CACHE_TTL_MS`,
    /// so the one binding serves both). Nothing it measures depends on speed:
    /// the session in force and every tier preview ask the identical question.
    private static let simulationTTLMs = feeSignalsTTLMs
    private var gasSignalsCache: [String: (signals: GasSignals, at: Double)] = [:]
    private var bundlerQuoteCache: [String: (quote: [String: Any], at: Double)] = [:]
    private var simulationCache: [String: (answer: String, at: Double)] = [:]
    private var feeSignalsEpoch: [Int: Int] = [:]
    /// Accounts known to be deployed, `chain:address`. Permanent: a Safe
    /// cannot be un-deployed, and asking again is a round trip on every quote.
    private var deployedAccounts: Set<String> = []

    /// One flight per question (see `SingleFlight`). Keys carry the chain's
    /// fee-signals epoch where a fresh reading was asked for, so a flight
    /// that started before the refresh never answers a caller after it.
    private let gasSignalsFlights = SingleFlight<String, GasSignals>()
    private let bundlerPriceFlights = SingleFlight<String, [String: Any]?>()
    private let inBandFlights = SingleFlight<String, [[String: Any]]?>()
    private let deploymentFlights = SingleFlight<String, DeploymentRead>()
    private let infoFlights = SingleFlight<String, AccountInfo?>()
    private let simulationFlights = SingleFlight<String, String>()

    /// How many relay gas-price reads actually went out — the test seam for
    /// "three tiers, one request".
    var bundlerPriceReads: Int { bundlerPriceFlights.started }

    /// Forget this chain's held readings, so the next quote run measures again.
    /// The simulation goes with them: a submit is about to change the
    /// account's state, and a refresh means "look again" for all of it.
    func invalidateFeeSignals(chainId: Int) {
        feeSignalsEpoch[chainId, default: 0] += 1
        gasSignalsCache = gasSignalsCache.filter { !$0.key.hasPrefix("\(chainId):") }
        bundlerQuoteCache = bundlerQuoteCache.filter { !$0.key.hasPrefix("\(chainId):") }
        simulationCache = simulationCache.filter { !$0.key.hasPrefix("\(chainId):") }
    }

    init(
        port: RelayPort,
        builtinBase: @escaping () async -> String = { NetDefaults.bundlerServiceURL },
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 },
        retryDelayMs: Double? = nil
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
        return await infoFlights.run(key) { await self.readAccountInfo(chainId: chainId, safe: safe, key: key) }
    }

    private func readAccountInfo(chainId: Int, safe: String, key: String) async -> AccountInfo? {
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
        return await inBandFlights.run(key) { await self.readInBandQuotes(chainId: chainId, safe: safe, key: key) }
    }

    private func readInBandQuotes(chainId: Int, safe: String, key: String) async -> [[String: Any]]? {
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
    ///
    /// The relay answers slow, standard AND fast in one response, so it is
    /// asked ONCE per chain and window and every tier is served from that
    /// answer: the speed control's three sessions used to ask it three times
    /// at the same instant, and their rows settled one after another. Each
    /// tier's row is held under the core's own rule (`bundlerQuoteCacheable`),
    /// so a degenerate row is never kept while its siblings are.
    func bundlerQuote(chainId: Int, tier: String) async -> [String: Any]? {
        let key = "\(chainId):\(tier)"
        if let held = bundlerQuoteCache[key], now() - held.at < Self.feeSignalsTTLMs {
            return held.quote
        }
        let epoch = feeSignalsEpoch[chainId, default: 0]
        let rows = await bundlerPriceFlights.run("\(chainId):\(epoch)") {
            await self.readBundlerPrices(chainId: chainId, epoch: epoch)
        }
        return rows?[tier] as? [String: Any]
    }

    /// Every tier the relay priced, parsed and — where the core says a row is
    /// a real measurement — held. `nil` when the relay did not answer.
    private func readBundlerPrices(chainId: Int, epoch: Int) async -> [String: Any]? {
        guard let result = await bundlerValue(
            chainId: chainId, method: "pimlico_getUserOperationGasPrice", params: []
        ) as? [String: Any] else { return nil }
        var rows: [String: Any] = [:]
        for (tier, raw) in result {
            guard let row = raw as? [String: Any], let maxFee = Self.decimalOfHex(row["maxFeePerGas"])
            else { continue }
            let quote: [String: Any] = [
                "max_fee_per_gas": maxFee,
                // The tip this tier is signed with — what the core turns into
                // the gas bid on screen (issue 684). Absent on a generic bundler.
                "max_priority_fee_per_gas": Self.decimalOfHex(row["maxPriorityFeePerGas"]).map { $0 as Any } ?? NSNull(),
                "network_fee_per_gas": Self.decimalOfHex(row["networkFeePerGas"]).map { $0 as Any } ?? NSNull(),
                "relayer_fee_per_gas": Self.decimalOfHex(row["relayerFeePerGas"]).map { $0 as Any } ?? NSNull(),
            ]
            rows[tier] = quote
            // Never a zero cap, which the core rejects as degenerate: "the
            // relay did not answer" is not a measurement worth holding.
            if bundlerQuoteCacheable(maxFeePerGas: maxFee), feeSignalsEpoch[chainId, default: 0] == epoch {
                bundlerQuoteCache["\(chainId):\(tier)"] = (quote, now())
            }
        }
        return rows
    }

    /// The relay's simulation of one exact operation, shared and held.
    ///
    /// `key` names the operation (the fee executor builds it from the account,
    /// the deployment and the calls byte for byte); `run` asks the relay and
    /// answers the fee machine's result JSON. Concurrent askers of the same
    /// operation get the one answer at the same moment; an answer `cacheable`
    /// approves is held for the fee-signals window and dropped with the
    /// signals (`invalidateFeeSignals`) — at every submit, among others.
    func sharedSimulation(
        chainId: Int, key: String, cacheable: (String) -> Bool, run: () async -> String
    ) async -> String {
        let slot = "\(chainId):\(key)"
        if let held = simulationCache[slot], now() - held.at < Self.simulationTTLMs {
            return held.answer
        }
        let epoch = feeSignalsEpoch[chainId, default: 0]
        return await simulationFlights.run("\(epoch):\(slot)") {
            let answer = await run()
            if cacheable(answer), feeSignalsEpoch[chainId, default: 0] == epoch {
                simulationCache[slot] = (answer, now())
            }
            return answer
        }
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

    /// `eth_sendUserOperation`, driven by the core's `submit_step` (spec 082
    /// RA1). Nothing here decides what an answer means:
    ///
    /// - every POST's "may this have been acted on" (the pool's
    ///   `maybe_delivered`, contract §2) is OR-ed over the whole submit, so a
    ///   refusal on attempt two after a lost reply on attempt one proves
    ///   nothing and reads "may have been sent";
    /// - the relay's error goes to the core as the JSON member it came as —
    ///   the `[existingHash:0x…]` marker is searched there first, before any
    ///   translation can eat it;
    /// - "currently processing" is the core's to retry, with the identical op.
    ///
    /// `localHash` is the operation's own EntryPoint v0.7 hash, computed
    /// before the first POST: what a may-have-been-sent op is followed under.
    ///
    /// `tier` is the speed the displayed fee was priced at, sent as the
    /// optional third parameter (spec 068's relay contract; iOS's since 069).
    /// A NAME, never a wei figure: the relay resolves it at submit time and
    /// clamps it between its inclusion floor and what the signed reimbursement
    /// funds. `nil` sends the pre-068 two-element params exactly, and the dead
    /// `rapid` is never sent — a relay refuses an unknown name with -32602.
    func sendUserOp(
        chainId: Int, opJson: String, localHash: String, tier: String? = nil
    ) async -> SubmitVerdict {
        guard let op = Self.object(fromJSON: opJson) else {
            // Never encoded, never sent.
            VelaLog.failure(.relay, kind: "not_sent", "chain=\(chainId) encode")
            return .notSent(rejection: .other(message: "the operation could not be encoded"))
        }
        let params = Self.submitParams(op, tier: tier)
        let started = Date()
        var attempt: UInt32 = 0
        var maybeDelivered = false
        VelaLog.notice(.relay, "submitting chain=\(chainId) hash=\(VelaLog.short(localHash))")
        while true {
            let result = await port.callDetailed(
                chainId: chainId, method: "eth_sendUserOperation", params: params, kind: "bundler"
            )
            maybeDelivered = maybeDelivered || result.maybeDelivered
            let step = userOpSubmitStep(
                reply: Self.submitReply(result), attempt: attempt,
                maybeDelivered: maybeDelivered, localHash: localHash
            )
            let attempts = attempt + 1
            switch step {
            case .retryAfter(let delayMs):
                attempt += 1
                let pause = retryDelayMs ?? Double(delayMs)
                try? await Task.sleep(nanoseconds: UInt64(max(0, pause) * 1_000_000))
            case .accepted(let hash):
                if hash.caseInsensitiveCompare(localHash) != .orderedSame {
                    // The relay's hash wins; a different one is worth a line.
                    VelaLog.notice(.relay, "userop.hash_mismatch local=\(VelaLog.short(localHash)) relay=\(VelaLog.short(hash))")
                }
                VelaLog.notice(.relay, "submit verdict=accepted hash=\(VelaLog.short(hash)) attempts=\(attempts) in=\(VelaLog.ms(since: started))")
                return .accepted(userOpHash: hash)
            case .maybeSent(let hash):
                VelaLog.failure(.relay, kind: "maybe_sent", "hash=\(VelaLog.short(hash)) attempts=\(attempts) in=\(VelaLog.ms(since: started))")
                return .maybeSent(userOpHash: hash)
            case .notSent(let rejection):
                VelaLog.failure(.relay, kind: "not_sent", "rejection=\(Self.rejectionName(rejection)) attempts=\(attempts)")
                return .notSent(rejection: rejection)
            }
        }
    }

    /// One POST's answer as the core's `SubmitReply`: the relay's result, its
    /// error member as it came, or no answer at all.
    ///
    /// A 200 with no error member is the relay's RESULT whatever it holds — a
    /// null, a word, an object — and whether that is a hash is the core's to
    /// say: `submit_step` reads one that is not as "may have been sent" (the
    /// relay spoke without refusing, so it may hold the op). Turning it into
    /// a made-up refusal here read NotSent — "failed, try again" over an op
    /// the relay may have queued (G21's double payment). The desktop and
    /// Android hand it over the same way.
    static func submitReply(_ result: RpcCallResult) -> UserOpSubmitReply {
        switch result.outcome {
        case .ok(let value):
            if let hash = value as? String { return .hash(hash: hash) }
            guard let value, !(value is NSNull) else { return .hash(hash: "") }
            return .hash(hash: jsonText(value) ?? "")
        case .rpcError(let code, let message):
            if let held = result.heldErrorJson { return .rpcError(errorJson: held) }
            var member: [String: Any] = ["message": message]
            if let code { member["code"] = code }
            return .rpcError(errorJson: CoreJSON.string(member))
        case .failed, .rangeCap:
            return .noAnswer
        }
    }

    /// The rejection's class for a log line — never the relay's sentence.
    private static func rejectionName(_ rejection: RelayRejection?) -> String {
        switch rejection {
        case nil: return "none"
        case .relayerUnavailable: return "relayer_unavailable"
        case .bundlerUnderfunded: return "bundler_underfunded"
        case .other: return "other"
        }
    }

    /// The chain's head, read once before a submit's first POST — where the
    /// tracker's relay-independent landing check starts (ruling 8). `nil`
    /// when the chain could not say; the core then scans below the head.
    func headBlock(chainId: Int) async -> UInt64? {
        guard let hex = await chainCall(chainId: chainId, method: "eth_blockNumber", params: []) as? String,
              hex.hasPrefix("0x"), hex.count > 2
        else { return nil }
        return UInt64(hex.dropFirst(2), radix: 16)
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

    /// The relay's view of an op with no receipt, through the ONE method name
    /// and parser the core owns (`userOpStatusMethod`, `parseUserOpStatus`,
    /// spec 082 RA7 — the relay serves `pimlico_getUserOperationStatus`, and
    /// the `eth_` name this file used to ask for answered -32601 every time,
    /// G13). `nil` for an older relay, a failure or a status the core does not
    /// know — which it reads as `status_unavailable`, never as a verdict.
    func userOpStatus(chainId: Int, userOpHash: String) async -> TrackStatusAnswer? {
        guard !userOpHash.isEmpty,
              let result = await bundlerValue(
                  chainId: chainId, method: userOpStatusMethod(), params: [userOpHash]
              ),
              let json = Self.jsonText(result)
        else { return nil }
        return parseUserOpStatus(json: json)
    }

    /// The relay-independent landing check (spec 082 ruling 8): the
    /// EntryPoint's `UserOperationEvent` for one op, in the window the core
    /// chose, and the chain's head — through the pool, answered AS IT CAME.
    /// The core decides what a range error is (T180) and what a found log
    /// means; this judges nothing. `fromBlock` nil asks for the head only.
    func findOpEvent(
        chainId: Int, entryPoint: String, topic0: String, userOpHash: String,
        fromBlock: UInt64?, toBlock: UInt64?
    ) async -> (logsJson: String?, errorJson: String?, head: UInt64?) {
        var logsJson: String?
        var errorJson: String?
        if let fromBlock {
            let filter: [String: Any] = [
                "address": entryPoint,
                "topics": [topic0, userOpHash],
                "fromBlock": "0x" + String(fromBlock, radix: 16),
                "toBlock": toBlock.map { "0x" + String($0, radix: 16) } ?? "latest",
            ]
            let result = await port.callDetailed(
                chainId: chainId, method: "eth_getLogs", params: [filter], kind: "rpc"
            )
            switch result.outcome {
            case .ok(let value):
                logsJson = Self.jsonText(value ?? NSNull())
            case .rpcError(let code, let message):
                // The endpoint's own error member, untouched: a range limit
                // halves the core's window; anything else retries it.
                var member: [String: Any] = ["message": message]
                if let code { member["code"] = code }
                errorJson = result.heldErrorJson ?? CoreJSON.string(member)
            case .rangeCap:
                errorJson = result.heldErrorJson
            case .failed:
                break // no answer: the core asks the same window next tick
            }
        }
        let head = await headBlock(chainId: chainId)
        return (logsJson, errorJson, head)
    }

    /// `eth_getTransactionReceipt` through the chain pool (spec 082 RJ4,
    /// EX13): the `result` as it came — `"null"` while the transaction is
    /// not mined — or `nil` when nobody answered. The core reads it; this
    /// judges nothing.
    func transactionReceiptJson(chainId: Int, txHash: String) async -> String? {
        guard !txHash.isEmpty else { return nil }
        guard case .ok(let value) = await port.call(
            chainId: chainId, method: "eth_getTransactionReceipt", params: [txHash], kind: "rpc"
        ) else { return nil }
        return Self.jsonText(value ?? NSNull())
    }

    /// Any JSON value as text; `nil` for what cannot be written.
    static func jsonText(_ value: Any) -> String? {
        if value is NSNull { return "null" }
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(withJSONObject: value)
        else {
            // A scalar (a string, a number) is valid JSON on its own.
            guard let data = try? JSONSerialization.data(
                withJSONObject: value, options: [.fragmentsAllowed]
            ) else { return nil }
            return String(data: data, encoding: .utf8)
        }
        return String(data: data, encoding: .utf8)
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
        return await gasSignalsFlights.run("\(key):\(epoch)") {
            await self.readGasSignals(chainId: chainId, wantTip: wantTip, key: key, epoch: epoch)
        }
    }

    private func readGasSignals(chainId: Int, wantTip: Bool, key: String, epoch: Int) async -> GasSignals {
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

    /// What the deployment read came to: an answer, or none — and whether
    /// none was a rate limit (spec 082 RJ13: the fee row then names the
    /// chain's node, never Vela's relay).
    enum DeploymentRead: Equatable {
        case deployed(Bool)
        case unread(rateLimited: Bool)
    }

    /// `eth_getCode` != `0x`; `nil` when the chain could not be asked.
    ///
    /// A `true` is remembered for good — a Safe cannot be un-deployed, and
    /// this read sat in front of every quote (0.8–1.0 s measured). A `false`
    /// or a `nil` is never held: the next send deploys the account, and an
    /// unreachable chain is not an answer.
    func isDeployed(chainId: Int, address: String) async -> Bool? {
        if case .deployed(let deployed) = await deploymentRead(chainId: chainId, address: address) {
            return deployed
        }
        return nil
    }

    /// `isDeployed`, saying why there is no answer when there is none.
    func deploymentRead(chainId: Int, address: String) async -> DeploymentRead {
        let key = "\(chainId):\(address.lowercased())"
        if deployedAccounts.contains(key) { return .deployed(true) }
        return await deploymentFlights.run(key) {
            let outcome = await self.port.call(
                chainId: chainId, method: "eth_getCode", params: [address, "latest"], kind: "rpc"
            )
            guard case .ok(let value) = outcome, let code = value as? String, code.hasPrefix("0x") else {
                if case .failed(let rateLimited) = outcome { return .unread(rateLimited: rateLimited) }
                return .unread(rateLimited: false)
            }
            let deployed = code.count > 2
            if deployed { self.deployedAccounts.insert(key) }
            return .deployed(deployed)
        }
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
}
