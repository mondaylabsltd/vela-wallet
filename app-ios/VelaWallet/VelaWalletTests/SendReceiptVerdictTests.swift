//
//  SendReceiptVerdictTests.swift
//  VelaWalletTests
//
//  Spec 082 T108 (RA4, RA10): the wallet's own Send when the relay's reply
//  was lost, and when the relay never had the op.
//
//  - The submit arm reports `submitted{maybe_sent, submit_block}` under the
//    op's local hash — never a failure for money that may be on its way.
//  - The receipt draws `maybe_sent` as on its way ("it may have been sent —
//    don't send it again", the op hash, no Retry) and `not_sent` as a failure
//    with nothing spent.
//  - The tracker's verdict reaches the receipt through the core's one mapping.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SendReceiptVerdictTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let op = "0x" + String(repeating: "d4", count: 32)

    /// A receipt on a real core's view, with its status overridden.
    private func receiptView(_ status: String) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "receipt"
        object["user_op_hash"] = op
        object["tx_status"] = "submitted"
        object["selected_token"] = [
            "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0,
            "logo_urls": [], "spam": false,
        ]
        object["receipt"] = [
            "status": status, "hold_reason": NSNull(), "kind": NSNull(), "transfers": [],
            "amount": "0.001", "usd_value": 0.001, "typical_inclusion_s": 5,
            "submitted_at_ms": NSNull(),
        ] as [String: Any]
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func drawn() throws -> SendReceiptModel {
        guard case .sendReceipt(let model) = WalletFlowFixtures.build(.sd4b, loc: loc).base else {
            throw Missing()
        }
        return model
    }

    private struct Missing: Error {}

    /// A lost reply: on its way, in the sentence that says don't send it
    /// again — the op hash to look it up by, and a close that leaves it
    /// running. Never failed, never a Retry.
    @Test func aMaybeSentReceiptSaysItMayHaveBeenSent() throws {
        let view = try receiptView("maybe_sent")
        #expect(SendLive.flowState(view, feeSheetOpen: false) == .sd4b)
        let receipt = SendLive.receipt(view, display: .usd, on: try drawn(), loc: loc)
        #expect(receipt.stage == .submitted)
        #expect(receipt.title == loc.t("send.txSubmitting"))
        #expect(receipt.captions == [loc.t("componentsUi.signing.maybeSent")])
        #expect(receipt.hash?.value == op)
        #expect(receipt.hash?.label == loc.t("componentsTx.receipt.userOpHash"))
        #expect(receipt.cta == loc.t("send.txCloseBackground"))
        #expect(!receipt.ctaAccent)
    }

    /// The relay never had it: failed, nothing spent — the generic sentence
    /// (true now), never the revert's "the fee may still have been taken".
    @Test func aNotSentReceiptIsAFailureWithNothingSpent() throws {
        let view = try receiptView("not_sent")
        #expect(SendLive.flowState(view, feeSheetOpen: false) == .sd4b)
        let receipt = SendLive.receipt(view, display: .usd, on: try drawn(), loc: loc)
        #expect(receipt.stage == .failed)
        #expect(receipt.captions == [loc.t("send.txErrorGeneric")])
        #expect(!receipt.captions.contains(loc.t("componentsTx.receipt.failedHint")))
        #expect(receipt.cta == loc.t("componentsTx.receipt.done"))
    }

    /// The tracker's verdicts, in the core's one mapping: a never-sent op is
    /// a failure marked `not_sent`; a may-have-been-sent op still waiting says
    /// nothing; the relay acknowledging it is its own verdict.
    @Test func theTrackersVerdictIsTheCoresMapping() throws {
        func outcome(_ status: String, _ outcomeName: String) throws -> [String: Any]? {
            let entry = TrackEntryWire(
                userOpHash: op, chainId: 100, recordIds: ["s1"], status: status, txHash: nil,
                polling: true, submittedAtMs: 1, outcome: outcomeName
            )
            return try sendReceiptOutcomeOf(trackEntryJson: entry.coreJSON).map { try CoreJSON.object($0) }
        }
        let notSent = try #require(try outcome("not_sent", "final"))
        #expect(notSent["type"] as? String == "failed")
        #expect(notSent["not_sent"] as? Bool == true)
        #expect(try outcome("pending", "maybe_sent") == nil, "nothing new to say while it may have been sent")
        #expect(try outcome("pending", "landing")?["type"] as? String == "acknowledged")
        #expect(try outcome("fee_held", "landing")?["type"] as? String == "fee_held")
    }

    /// The submit arm: a mute relay after the signature is `submitted` with
    /// `maybe_sent` and the head read before the POST — recorded and tracked
    /// like any submit, never a `submit_failed`.
    @Test func theSubmitArmReportsAMaybeSentOp() async throws {
        let fixture = TrustedSignerFixture()
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_blockNumber"] = .ok("0x2dc6c00")
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        port.detailed["eth_sendUserOperation"] = [
            RpcCallResult(outcome: .failed(rateLimited: false), maybeDelivered: true, heldErrorJson: nil),
        ]
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.recordJson(signedInWith: UserOpSpine.trustedSignerMethod)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accountStore = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accountStore)
        let executor = SendExecutor(
            store: store, relay: relay, pool: pool, spine: spine, accounts: accounts,
            fees: FeeStore(relay: relay, accounts: accounts, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accountStore),
            metadata: TokenMetadata(store: store, pool: pool), accountStore: accountStore,
            balances: { nil }, networks: { nil }, ports: SendExecutor.Ports(),
            // The clearance below is a Task hop: on a starved runner that
            // is seconds, not the core's milliseconds (`Waits.swift`).
            clearanceWaitMs: 600_000
        )
        // The core's write-ahead, played by the test (RJ1): the POST is
        // cleared once the op is signed.
        executor.ports.opSigned = { [weak executor] hash, _ in
            Task { @MainActor in _ = await executor?.perform(["type": "clear_to_post", "user_op_hash": hash]) }
        }
        let reply = try CoreJSON.object(await executor.perform([
            "type": "submit_user_op", "chain_id": 100, "account": fixture.account,
            "public_key_hex": NSNull(),
            "calls": [["to": fixture.account, "value": "1000", "data": "0x"] as [String: Any]],
            "gas_fee_token": NSNull(),
            "quoted_fee": ["amount": "1000", "recipient": fixture.account] as [String: Any],
        ]))
        #expect(reply["type"] as? String == "submitted")
        #expect(reply["maybe_sent"] as? Bool == true)
        #expect((reply["submit_block"] as? NSNumber)?.uint64Value == 0x2dc6c00)
        #expect((reply["user_op_hash"] as? String)?.count == 66)
    }

    /// Ruling 1, the Android finding (e7b8cdd4) checked on this client: the
    /// send core keeps "signing" — and its Cancel — from the passkey until
    /// the relay answers, so a Cancel can land while the POST is out. The
    /// POST must run to its verdict and the op be reported: dropping it would
    /// leave an op the relay may hold with no record and no tracker, the
    /// slide one swipe from paying twice.
    @Test func aCancelWhileThePostIsOutStillReportsTheOp() async throws {
        let fixture = TrustedSignerFixture()
        let scripted = ScriptedRelayPort()
        scripted.rpc["eth_getCode"] = .ok("0x")
        scripted.rpc["eth_blockNumber"] = .ok("0x2dc6c00")
        scripted.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        let relayHash = "0x" + String(repeating: "cd", count: 32)
        scripted.detailed["eth_sendUserOperation"] = [
            RpcCallResult(outcome: .ok(relayHash), maybeDelivered: false, heldErrorJson: nil),
        ]
        let port = HeldPostPort(scripted)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.recordJson(signedInWith: UserOpSpine.trustedSignerMethod)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accountStore = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accountStore)
        let executor = SendExecutor(
            store: store, relay: relay, pool: pool, spine: spine, accounts: accounts,
            fees: FeeStore(relay: relay, accounts: accounts, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accountStore),
            metadata: TokenMetadata(store: store, pool: pool), accountStore: accountStore,
            balances: { nil }, networks: { nil }, ports: SendExecutor.Ports(),
            // The clearance below is a Task hop: on a starved runner that
            // is seconds, not the core's milliseconds (`Waits.swift`).
            clearanceWaitMs: 600_000
        )
        // The core's write-ahead, played by the test (RJ1): the POST is
        // cleared once the op is signed.
        executor.ports.opSigned = { [weak executor] hash, _ in
            Task { @MainActor in _ = await executor?.perform(["type": "clear_to_post", "user_op_hash": hash]) }
        }
        let submit = Task {
            try CoreJSON.object(await executor.perform([
                "type": "submit_user_op", "chain_id": 100, "account": fixture.account,
                "public_key_hex": NSNull(),
                "calls": [["to": fixture.account, "value": "1000", "data": "0x"] as [String: Any]],
                "gas_fee_token": NSNull(),
                "quoted_fee": ["amount": "1000", "recipient": fixture.account] as [String: Any],
            ]))
        }
        // On state, not a clock (`Waits.swift`): the suite's time limit
        // reports a POST that never goes out.
        await Wait.until { port.holding }
        try #require(port.holding, "the POST never went out")

        // The person taps Cancel while the relay has the POST.
        _ = await executor.perform(["type": "cancel_passkey_sign"])
        port.release()

        let reply = try await submit.value
        #expect(reply["type"] as? String == "submitted", "a cancel after the passkey dropped the op: \(reply)")
        #expect(reply["user_op_hash"] as? String == relayHash)
        #expect(scripted.calls.filter { $0 == "eth_sendUserOperation" }.count == 1, "one POST, never a second")
    }
}

/// A relay port that holds the submit POST until released — the moment a
/// person can still tap Cancel.
@MainActor
final class HeldPostPort: RelayPort {
    private let inner: ScriptedRelayPort
    private var held: CheckedContinuation<Void, Never>?
    private(set) var holding = false

    init(_ inner: ScriptedRelayPort) { self.inner = inner }

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        await callDetailed(chainId: chainId, method: method, params: params, kind: kind).outcome
    }

    func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult {
        if method == "eth_sendUserOperation" {
            await withCheckedContinuation { continuation in
                held = continuation
                holding = true
            }
        }
        return await inner.callDetailed(chainId: chainId, method: method, params: params, kind: kind)
    }

    func release() {
        holding = false
        held?.resume()
        held = nil
    }

    func bundlerBase(chainId: Int) async -> String? { await inner.bundlerBase(chainId: chainId) }
    func bestRpcUrl(chainId: Int) async -> String? { await inner.bestRpcUrl(chainId: chainId) }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer {
        await inner.restGet(url: url, xRpcUrl: xRpcUrl)
    }
}
