//
//  PreparedGasSubmitTests.swift
//  VelaWalletTests
//
//  core-perf-1: the submit signs the gas the confirmed fee was priced on.
//
//  Between Confirm and the passkey the submit used to ask the relay to
//  simulate the operation again and measure its inner calls again — the two
//  slowest reads before the prompt, for limits the fee on screen was NOT
//  priced on. The fee machine now keeps what its run measured
//  (`FeeEstimate.prepared`), the send hands it down in `SubmitUserOp.prepared`
//  while it is fresh and for exactly these calls, and the spine applies it
//  through `userOpApplyEstimate` when the account is still in the deployment
//  state the quote measured. The nonce is read fresh either way.
//
//  Both paths, through the real `SendStore`, its executor, the real `send`
//  and `fee_policy` cores and the spine, as `RootView` wires them:
//
//  - the quote's gas is signed: no second simulation, and the operation
//    posted carries the quote's limits even though the relay would now answer
//    others;
//  - the account was deployed since the quote: the gas is measured again,
//    and what is posted is the new measurement.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct PreparedGasSubmitTests {

    private let fixture = TrustedSignerFixture()
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let feeRecipient = "0x7777777777777777777777777777777777777777"

    /// The scripted relay, with the operation it was asked to post.
    final class PostCapture: RelayPort {
        let inner: ScriptedRelayPort
        private(set) var posted: [[String: Any]] = []
        /// What the POST is answered — a lost reply unless a test says
        /// otherwise (§35: the relay's AA22).
        var sendAnswer = RpcCallResult(outcome: .failed(rateLimited: false), maybeDelivered: true, heldErrorJson: nil)
        init(_ inner: ScriptedRelayPort) { self.inner = inner }

        func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
            await callDetailed(chainId: chainId, method: method, params: params, kind: kind).outcome
        }

        func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult {
            if method == "eth_sendUserOperation" {
                if let op = params.first as? [String: Any] { posted.append(op) }
                // A lost reply by default: the op may have been sent, and
                // nothing more is asked of this test's relay.
                return sendAnswer
            }
            return await inner.callDetailed(chainId: chainId, method: method, params: params, kind: kind)
        }

        func bundlerBase(chainId: Int) async -> String? { "https://relay.test" }
        func bestRpcUrl(chainId: Int) async -> String? { "https://rpc.test" }
        func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer { .failed }
    }

    private func estimate(call: String) -> RpcOutcome {
        .ok(["verificationGasLimit": "0x30d40", "callGasLimit": call, "preVerificationGas": "0xc350"] as [String: Any])
    }

    private func scripted() -> ScriptedRelayPort {
        let scripted = ScriptedRelayPort()
        scripted.rpc["eth_getCode"] = .ok("0x")
        scripted.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        scripted.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        scripted.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        scripted.rpc["eth_maxPriorityFeePerGas"] = .ok("0x1")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x77359400", "maxPriorityFeePerGas": "0x1",
            "networkFeePerGas": "0x3b9aca00", "relayerFeePerGas": "0x3b9aca00",
        ]
        scripted.rpc["pimlico_getUserOperationGasPrice"] = .ok(
            ["slow": tier, "standard": tier, "fast": tier] as [String: Any]
        )
        scripted.rpc["vela_getInBandGasQuote"] = .ok([[
            "recipient": feeRecipient, "asset": "native", "feeToken": NSNull(), "decimals": 18,
            "symbol": "XDAI", "balance": "0xde0b6b3a7640000", "usdPrice": "1", "usdBalance": "1",
        ] as [String: Any]])
        scripted.rpc["eth_estimateUserOperationGas"] = estimate(call: "0x30d40")
        return scripted
    }

    private func balance() throws -> BalanceViewWire {
        let json = """
        {
          "address": "\(fixture.account)",
          "display_total_usd": 2.0, "balance_unknown": false, "balance_partial": false,
          "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": null,
          "tokens": [{
            "chain_id": 100, "symbol": "xDAI", "name": "xDAI",
            "balance": "2", "decimals": 18, "token_address": null,
            "price_usd": 1.0, "spam": false
          }],
          "unpriced_tokens": [],
          "failed_chain_ids": [], "rate_limited_chain_ids": [], "unreachable_networks": [],
          "holdings_loading": false, "cached_total_usd": null,
          "switcher": { "open": false, "loading": false, "balances": [] }
        }
        """
        return try CoreJSON.decode(BalanceViewWire.self, from: CoreJSON.object(json))
    }

    private func networks() throws -> NetViewWire {
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [{
            "id": "chain-100", "chain_id": 100, "display_name": "Gnosis",
            "native_symbol": "xDAI", "is_custom": false,
            "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
            "rpc_save_deferred": false
          }],
          "wizard": {
            "phase": "idle", "query": "", "custom_rpc": "", "suggestions": [],
            "chain_info": null, "compat": null, "error": null, "can_add": false
          }
        }
        """
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(json))
    }

    /// A send of 0.001 xDAI on Gnosis, at its confirm, quoted.
    private func confirm(_ scripted: ScriptedRelayPort) async throws -> (SendStore, PostCapture) {
        let port = PostCapture(scripted)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.pageRecordJson
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        let fixture = self.fixture
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        defaults.set("[\(fixture.pageRecordJson)]", forKey: VelaStore.Key.accounts)
        let store = VelaStore(defaults: defaults)
        let accountStore = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accountStore, offline: true)
        let held = try balance()
        let nets = try networks()
        let executor = SendExecutor(
            store: store, relay: relay, pool: pool, spine: spine, accounts: accounts,
            fees: FeeStore(relay: relay, accounts: accounts, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accountStore),
            metadata: TokenMetadata(store: store, pool: pool), accountStore: accountStore,
            balances: { held }, networks: { nets },
            ports: SendExecutor.Ports(trackSubmitted: { _ in }),
            clearanceWaitMs: 600_000
        )
        let send = SendStore(executor: executor)
        send.open(
            accountId: fixture.credentialHex, address: fixture.account, name: "Mine",
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        await Wait.until { !(send.view?.tokens.isEmpty ?? true) }
        send.selectToken(id: try #require(send.view?.tokens.first?.id))
        await Wait.until { send.view?.stage == .enterDetails }
        send.setRecipient(payee)
        send.setAmount("0.001")
        await Wait.until { send.view?.canContinue == true }
        send.advance()
        await Wait.until { send.view?.stage == .confirm && send.view?.canConfirm == true }
        return (send, port)
    }

    private func estimates(_ scripted: ScriptedRelayPort) -> Int {
        scripted.calls.filter { $0 == "eth_estimateUserOperationGas" }.count
    }

    private func gas(_ value: Any?) -> UInt64? {
        guard let hex = value as? String else { return nil }
        return UInt64(hex.dropFirst(hex.hasPrefix("0x") ? 2 : 0), radix: 16)
    }

    /// The quote measured the operation; the confirm signs that measurement.
    /// The relay is then made to answer other limits: had the submit asked it
    /// again, the operation posted would carry them.
    @Test func theSubmitSignsTheQuotesGasWithoutMeasuringAgain() async throws {
        let scripted = scripted()
        let (send, port) = try await confirm(scripted)
        let prepared = try #require(send.view?.fee?.prepared, "the quote's measured gas did not reach the send")
        #expect(prepared.deployed == false)
        #expect(prepared.calls.count == 1)
        let quoted = estimates(scripted)
        #expect(quoted >= 1, "the quote never simulated")

        scripted.rpc["eth_estimateUserOperationGas"] = estimate(call: "0x7a120")
        send.slideConfirm()
        await Wait.until { !port.posted.isEmpty }
        let op = try #require(port.posted.first)

        #expect(estimates(scripted) == quoted, "the submit simulated again before the passkey")
        #expect(gas(op["callGasLimit"]) == UInt64(prepared.callGasLimit),
                "the posted operation is not the gas the fee was priced on")
        #expect(gas(op["verificationGasLimit"]) == UInt64(prepared.verificationGasLimit))
        #expect(gas(op["preVerificationGas"]) == UInt64(prepared.preVerificationGas))
        // The nonce and the deployment were still read for this submit.
        #expect(scripted.calls.filter { $0 == "eth_getCode" }.count >= 2)
    }

    /// The account was deployed between the quote and the confirm: gas
    /// measured for an undeployed account is not used — the submit measures
    /// again, as it always did, and posts that.
    @Test func aQuoteForAnotherDeploymentIsMeasuredAgain() async throws {
        let scripted = scripted()
        let (send, port) = try await confirm(scripted)
        let prepared = try #require(send.view?.fee?.prepared)
        #expect(prepared.deployed == false)
        let quoted = estimates(scripted)

        scripted.rpc["eth_getCode"] = .ok("0x6080604052")
        scripted.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "5")
        scripted.rpc["eth_estimateUserOperationGas"] = estimate(call: "0x7a120")
        send.slideConfirm()
        await Wait.until { !port.posted.isEmpty }
        let op = try #require(port.posted.first)

        #expect(estimates(scripted) == quoted + 1, "the submit signed gas measured for another deployment")
        #expect(gas(op["callGasLimit"]) != UInt64(prepared.callGasLimit))
        #expect(gas(op["nonce"]) == 5, "the nonce was not read fresh")
    }

    /// The estimate the fee card hands the send keeps the measurement on the
    /// way through this shell (`FeeEstimateWire.coreJSON`): dropping it was
    /// what made every submit measure again.
    @Test func theEstimateKeepsItsMeasurementThroughTheShell() throws {
        let json: [String: Any] = [
            "chain_id": 100, "total_wei": "1", "max_fee_per_gas": "1", "total_gas": "1",
            "deployed": false, "quoted": true, "fee_asset": ["type": "native"], "fee_recipient": feeRecipient,
            "prepared": [
                "calls": [["to": payee, "value": "1", "data": "0x"]], "deployed": false,
                "verification_gas_limit": "2000000", "call_gas_limit": "200000",
                "pre_verification_gas": "50000", "settlement_gas": NSNull(),
            ] as [String: Any],
        ]
        let wire = try CoreJSON.decode(FeeEstimateWire.self, from: json)
        let prepared = try #require(wire.prepared)
        #expect(prepared.callGasLimit == "200000")
        #expect(prepared.settlementGas == nil)
        let back = try #require(wire.coreJSON["prepared"] as? [String: Any])
        #expect(back["call_gas_limit"] as? String == "200000")
        #expect((back["calls"] as? [[String: Any]])?.first?["to"] as? String == payee)
        // Absent stays absent: nothing invented for an estimate without one.
        var bare = json
        bare.removeValue(forKey: "prepared")
        #expect(try CoreJSON.decode(FeeEstimateWire.self, from: bare).coreJSON["prepared"] == nil)
    }

    // MARK: - §35: every SafeOp expires

    /// The window the posted operation was signed with: the signature's
    /// first 12 bytes, `validAfter(6) ‖ validUntil(6)` big-endian.
    private func window(_ op: [String: Any]) throws -> (after: UInt64, until: UInt64) {
        let signature = try #require(op["signature"] as? String)
        let hex = signature.hasPrefix("0x") ? String(signature.dropFirst(2)) : signature
        try #require(hex.count >= 24, "\(signature)")
        let after = try #require(UInt64(hex.prefix(12), radix: 16))
        let until = try #require(UInt64(hex.dropFirst(12).prefix(12), radix: 16))
        return (after, until)
    }

    /// §35 (review 1-6): the operation a send posts is signed with its
    /// window — open now (validAfter 0) and closed 30 minutes from the
    /// moment it was stamped, just before its hash. Through the real send,
    /// fee and spine, as the app wires them: the page's request and the
    /// envelope both carry it, or the core would have signed nothing.
    @Test func theOperationPostedExpiresThirtyMinutesAfterItIsSigned() async throws {
        let scripted = scripted()
        let (send, port) = try await confirm(scripted)
        let before = UInt64(Date().timeIntervalSince1970)
        send.slideConfirm()
        await Wait.until { !port.posted.isEmpty }
        let after = UInt64(Date().timeIntervalSince1970)
        let (validAfter, validUntil) = try window(try #require(port.posted.first))
        #expect(validAfter == 0, "open from the start")
        #expect(validUntil >= before + 1800 && validUntil <= after + 1800,
                "closes 30 minutes after it was stamped: \(validUntil) vs \(before)…\(after)")
    }

    /// §35: the relay's `AA22 expired or not due` — the window closed before
    /// the EntryPoint saw it. Nothing was sent: the send machine (the REAL
    /// one, hearing the spine's `expired`) says `expired`, and the confirm
    /// draws it as a used nonce is drawn — "Not sent", calmly, its sentence,
    /// and Review again, which signs anew with a window of its own.
    @Test func anOperationThatExpiredIsNotSentCalmlyAndReviewedAgain() async throws {
        let scripted = scripted()
        let (send, port) = try await confirm(scripted)
        port.sendAnswer = RpcCallResult(
            outcome: .rpcError(code: -32500, message: "AA22 expired or not due"),
            maybeDelivered: false, heldErrorJson: nil
        )
        send.slideConfirm()
        await Wait.until { send.view?.txStatus == "error" }
        let view = try #require(send.view)
        #expect(view.txStatus == "error")
        #expect(view.txError == "expired", "\(String(describing: view.txError))")
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(SendLive.confirmNotice(view, loc: en) == en.t("componentsUi.signing.expired"))
        #expect(SendLive.confirmNotice(view, loc: en) == "It expired before it was submitted — nothing was sent.")
        #expect(SendLive.confirmNoticeTitle(view, loc: en) == en.t("componentsTx.receipt.statusNotSent"))
        #expect(port.posted.count == 1, "one POST, never a second by itself")
    }
}
