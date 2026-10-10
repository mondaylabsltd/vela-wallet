//
//  DappSigningTests.swift
//  VelaWalletTests
//
//  What a page asks for, and what the wallet does about it.
//
//  The request executor's reading of a request, the signing controller's
//  assembly, and the sheet the four machines produce. (The router's
//  allowlist moved into the core with spec 070 — `dapp_rpc::classify`, pinned
//  by `tests/app_dapp_browser.rs` — and its tests with it.)
//  Hermetic: no network, no ceremony, no device.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - Reading a request

@MainActor
struct SignRequestReadingTests {

    /// `eth_sendTransaction` is one call; `wallet_sendCalls` is many — and an
    /// **empty batch is not a batch**.
    @Test func theCallsARequestCarries() {
        let single = SignExecutor.callsOf(
            method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0xabc","value":"0x38d7ea4c68000","data":"0x"}]"#
        )
        #expect(single?.count == 1)
        // Hex on the wire, decimal to the core.
        #expect(single?.first?.value == "1000000000000000")

        let batch = SignExecutor.callsOf(
            method: "wallet_sendCalls",
            paramsJson: #"[{"calls":[{"to":"0xa","value":"0x1"},{"to":"0xb"}]}]"#
        )
        #expect(batch?.count == 2)
        #expect(batch?.last?.value == "0", "an absent value is zero, not a failure")
        #expect(batch?.last?.data == "0x")

        #expect(SignExecutor.callsOf(method: "wallet_sendCalls", paramsJson: #"[{"calls":[]}]"#) == nil)
        #expect(SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: "[]") == nil)
        #expect(SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: "not json") == nil)
    }

    /// Spec 096 F1: the core's value table, through this shell's reading —
    /// PancakeSwap's `0xaa87bee538000` is 0.003 BNB; `"0x"` is zero (this
    /// shell refused it); decimal text, bare hex and a JSON number are
    /// refused (this shell read a number as ZERO); a batch with one bad leg
    /// is refused whole (this shell dropped the leg and sent the rest).
    @Test func aCallValueIsTheCoresReading() {
        let table: [(String, String?)] = [
            (#""0xaa87bee538000""#, "3000000000000000"),
            (#""0xAA87BEE538000""#, "3000000000000000"),
            (#""0x""#, "0"),
            (#""""#, "0"),
            ("null", "0"),
            (#""0""#, "0"),
            ("0", "0"),
            (#""0x\#(String(repeating: "f", count: 64))""#,
             "115792089237316195423570985008687907853269984665640564039457584007913129639935"),
            (#""0x1\#(String(repeating: "0", count: 64))""#, nil),
            (#""1000""#, nil),
            (#""aa87bee538000""#, nil),
            ("1000", nil),
            (#""0X1f""#, nil),
        ]
        for (value, wei) in table {
            let calls = SignExecutor.callsOf(
                method: "eth_sendTransaction",
                paramsJson: #"[{"to":"0x13f4EA83D0bd40E75C8222255bc855a974568Dd4","value":\#(value),"data":"0x3593564c"}]"#
            )
            #expect(calls?.first?.value == wei, "\(value)")
        }
        let batch = SignExecutor.callsOf(
            method: "wallet_sendCalls",
            paramsJson: #"[{"calls":[{"to":"0xa","value":"0x1"},{"to":"0xb","value":"1"}]}]"#
        )
        #expect(batch == nil, "every leg or none")
    }

    /// An odd-length hex value is still a number.
    ///
    /// A dApp writes `0x1`. A decoder that needs whole bytes and is handed one
    /// nibble answers nothing, and the request dies with "carried no
    /// transaction this wallet could read".
    @Test func anOddLengthHexValueIsStillANumber() {
        let call = SignExecutor.callsOf(
            method: "eth_sendTransaction", paramsJson: #"[{"to":"0xabc","value":"0x1"}]"#
        )
        #expect(call?.first?.value == "1")
    }

    /// `personal_sign` signs `params[0]`; `eth_sign` signs `params[1]`.
    ///
    /// The params are swapped between the two, and signing the wrong one
    /// produces a signature that verifies against nothing.
    @Test func aMessageIsHashedTheWayThePagesVerifierWill() {
        // "Hello, Vela" — the EIP-191 envelope over 11 bytes.
        let personal = SignExecutor.messageHash(
            method: "personal_sign", paramsJson: #"["0x48656c6c6f2c2056656c61","0xabc"]"#
        )
        let ethSign = SignExecutor.messageHash(
            method: "eth_sign", paramsJson: #"["0xabc","0x48656c6c6f2c2056656c61"]"#
        )
        #expect(personal != nil)
        #expect(personal == ethSign, "same payload, params swapped — the same digest")

        let expected = keccak256(data: Data("\u{19}Ethereum Signed Message:\n11".utf8) + Data("Hello, Vela".utf8))
        #expect(personal == expected)

        // A non-hex payload is signed as its own UTF-8 bytes.
        let text = SignExecutor.messageHash(method: "personal_sign", paramsJson: #"["hello","0xabc"]"#)
        #expect(text == keccak256(data: Data("\u{19}Ethereum Signed Message:\n5".utf8) + Data("hello".utf8)))
    }

    /// **A late receipt is not a confirmation** (issue 262). The core hears
    /// `receipt_pending` with the op hash and keeps the record pending; only
    /// a receipt in time is `succeeded` with the tx hash.
    @Test func aLateReceiptIsReportedPendingNeverSucceeded() {
        let late = SignExecutor.afterReceiptWait(userOpHash: "0xop", receipt: nil)
        #expect(late["type"] as? String == "receipt_pending")
        #expect(late["user_op_hash"] as? String == "0xop")
        #expect(late["result"] == nil)

        let inTime = SignExecutor.afterReceiptWait(userOpHash: "0xop", receipt: "0xtx")
        #expect(inTime["type"] as? String == "succeeded")
        #expect(inTime["result"] as? String == "0xtx")
    }

    /// A signature moves nothing, so its feed row claims no value and no
    /// symbol. A row with a value shows up in the feed as money.
    @Test func aSignatureRecordCarriesNoMoney() {
        let row = SignExecutor.recordRow([
            "record_id": "dapp-1-msg", "kind": "sign_message", "method": "personal_sign",
            "params_json": "[]", "result": "", "from": "0xme", "chain_id": 100,
            "now_ms": 1_757_000_000_000, "status": "pending", "user_op_hash": "",
            "dapp_origin": "https://x.test",
        ], nativeSymbol: "xDAI")
        #expect(row["type"] as? String == "sign_message")
        #expect(row["value"] as? String == "0")
        #expect((row["symbol"] as? String)?.isEmpty == true)
        // SECONDS. Milliseconds here puts every signature in the year 57000.
        #expect(row["timestamp"] as? Int == 1_757_000_000)
    }

    /// Spec 093: the request is stored as the CORE cut it — `stored_request`
    /// and `request_truncated`, verbatim — never a cut of the shell's own;
    /// the call itself is still read from the whole request.
    @Test func aTransactionRecordCarriesTheCallAndStoresTheCoresCut() {
        let long = String(repeating: "a", count: 9000)
        let cut = #"[{"to":"0x76875e","value":"0x38d7ea4c68000","data":"0xaaaa"}]"#
        let row = SignExecutor.recordRow([
            "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e","value":"0x38d7ea4c68000","data":"0x\#(long)"}]"#,
            "result": "", "from": "0xme", "chain_id": 100, "now_ms": 1_757_000_000_000,
            "status": "pending", "user_op_hash": "0xcf9f", "dapp_origin": "https://x.test",
            "stored_request": cut, "request_truncated": true,
        ], nativeSymbol: "xDAI")
        #expect(row["type"] as? String == "dapp_tx")
        #expect(row["to"] as? String == "0x76875e")
        #expect(row["symbol"] as? String == "xDAI")
        #expect(row["requestTruncated"] as? Bool == true)
        #expect(row["signedRequest"] as? String == cut, "the core's cut, not the shell's")
    }
}

// MARK: - The controller's assembly

@MainActor
struct SigningAssemblyTests {

    /// **Displayed is signed.** When the guard rewrote an approval, the
    /// rewritten params are what the approve carries — never the original
    /// request.
    @Test func theApproveCarriesTheGuardsRewriteRatherThanTheRequest() {
        let rewritten = #"[{"to":"0xtoken","data":"0x095ea7b3capped"}]"#
        var guardView = GuardViewWire.empty
        guardView = GuardViewWire(
            surface: .approvalEditor, detected: nil, meta: guardView.meta, editor: nil,
            confirmAllowed: true, rewrittenParamsJson: rewritten, unlimitedConsented: false, increaseTotal: nil,
            decimalsUnverified: false, expired: false, batch: nil
        )
        let opts = SigningController.approveOpts(fee: nil, clear: .empty, guard: guardView)
        #expect(opts["params_override_json"] as? String == rewritten)

        // Nothing rewritten is `null`, and that is NOT a green light: the
        // untouched params still meet the core's own refusal at submit.
        let untouched = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect(untouched["params_override_json"] is NSNull)
    }

    /// The displayed fee travels verbatim into the signature.
    @Test func theQuotedFeeTravelsWithTheApproval() {
        let fee = FeeViewWire(
            busy: false, failed: nil,
            fee: FeeEstimateWire(
                chainId: 100, totalWei: "2100000000000000", maxFeePerGas: "1500000000",
                totalGas: "120000", deployed: true, quoted: true,
                feeAsset: .native,
                feeRecipient: "0xrelay"
            ),
            stale: false, feeToken: nil, options: [], confirmFeeReady: true
        )
        let opts = SigningController.approveOpts(fee: fee, clear: .empty, guard: .empty)
        let quoted = opts["quoted_fee"] as? [String: Any]
        #expect(quoted?["amount"] as? String == "2100000000000000")
        #expect(quoted?["recipient"] as? String == "0xrelay")
        #expect(opts["max_fee_per_gas"] as? String == "1500000000")
    }

    /// Issue #262: the approve signs the coin that was picked, in THAT coin's
    /// units — the send core's own rule (`submit_user_op`).
    @Test func theApproveCarriesThePickedStablecoinAndItsOwnAmount() {
        let usdt = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
        let picked = FeeViewWire(
            busy: false, failed: nil,
            fee: FeeEstimateWire(
                chainId: 1, totalWei: "0", maxFeePerGas: "1", totalGas: "300000",
                deployed: false, quoted: true,
                feeAsset: .erc20(token: usdt, decimals: 6, amount: "1020000", symbol: "USDT"),
                feeRecipient: "0xrelay"
            ),
            stale: false, feeToken: usdt, options: [], confirmFeeReady: true
        )
        let opts = SigningController.approveOpts(fee: picked, clear: .empty, guard: .empty)
        #expect(opts["gas_fee_token"] as? String == usdt)
        #expect((opts["quoted_fee"] as? [String: Any])?["amount"] as? String == "1020000")

        let native = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect(native["gas_fee_token"] is NSNull)
        // No recipient, no quoted fee: an in-band leg with nowhere to pay is not one.
        let noRecipient = FeeEstimateWire(
            chainId: 1, totalWei: "5", maxFeePerGas: "1", totalGas: "1", deployed: true,
            quoted: false, feeAsset: .native, feeRecipient: nil
        )
        #expect(SigningController.quotedFee(noRecipient) == nil)
    }

    /// Each method reaches its own rung of the clear-signing ladder.
    @Test func eachMethodStartsTheRightResolution() {
        let tx = SigningController.clearKickoff(
            method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0xabc","data":"0xdeadbeef","value":"0x1"}]"#,
            chainId: 100, origin: "https://x.test"
        )
        #expect(tx?["type"] as? String == "resolve_transaction")
        #expect(tx?["to"] as? String == "0xabc")
        #expect(tx?["data"] as? String == "0xdeadbeef")

        // 089 S1: a batch goes over WHOLE — the core reads every call.
        let batchParams = #"[{"calls":[{"to":"0xfirst","data":"0x01"},{"to":"0xsecond"}]}]"#
        let batch = SigningController.clearKickoff(
            method: "wallet_sendCalls", paramsJson: batchParams, chainId: 100, origin: nil
        )
        #expect(batch?["type"] as? String == "resolve_batch")
        #expect(batch?["params_json"] as? String == batchParams)
        #expect(batch?["to"] == nil, "never call 1 alone")

        // A transaction is its OWN call — a stray `calls` beside it is not read.
        let stray = SigningController.clearKickoff(
            method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0xsigned","data":"0x095ea7b3ff","calls":[{"to":"0xdecoy","value":"0x1"}]}]"#,
            chainId: 100, origin: nil
        )
        #expect(stray?["to"] as? String == "0xsigned")
        #expect(stray?["data"] as? String == "0x095ea7b3ff")

        let typed = SigningController.clearKickoff(
            method: "eth_signTypedData_v4",
            paramsJson: #"["0xme","{\"primaryType\":\"Permit\"}"]"#,
            chainId: 100, origin: nil
        )
        #expect(typed?["type"] as? String == "resolve_typed_data")

        let message = SigningController.clearKickoff(
            method: "personal_sign", paramsJson: #"["0x48","0xme"]"#,
            chainId: 100, origin: "https://x.test"
        )
        #expect(message?["type"] as? String == "message_presented")
        #expect(message?["request_origin"] as? String == "https://x.test")

        #expect(SigningController.clearKickoff(
            method: "eth_blockNumber", paramsJson: "[]", chainId: 100, origin: nil
        ) == nil)
    }

    /// The sheet decodes the request's ONE document, where its method carries
    /// it — the core's reading, the same bytes the passkey signs (audit
    /// 2026-10-01). Two documents, in either order, give the sheet nothing.
    @Test func theSheetDecodesTheOneDocumentTheCoreSigns() {
        let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let doc = #"{"types":{"EIP712Domain":[],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{},"message":{"contents":"hi"}}"#
        let quoted = String(data: try! JSONSerialization.data(withJSONObject: [doc]), encoding: .utf8)!.dropFirst().dropLast()
        for (method, params) in [
            ("eth_signTypedData_v4", "[\"\(account)\",\(quoted)]"),
            ("eth_signTypedData_v3", "[\"\(account)\",\(doc)]"),
            ("eth_signTypedData", "[\(quoted),\"\(account)\"]"),
            ("eth_signTypedData_v1", "[\(doc),\"\(account)\"]"),
        ] {
            #expect(SigningController.typedDataOf(method: method, paramsJson: params).contains("\"Mail\""), "\(method)")
        }
        // The audit's shapes: nothing to show, and nothing is signed either.
        #expect(SigningController.typedDataOf(method: "eth_signTypedData_v4", paramsJson: "[\(doc),\(doc)]").isEmpty)
        #expect(SigningController.typedDataOf(method: "eth_signTypedData", paramsJson: "[\(doc),\(doc)]").isEmpty)
        #expect(SignExecutor.messageHash(method: "eth_signTypedData", paramsJson: "[\(doc),\(doc)]") == nil)
        #expect(SigningController.typedDataOf(method: "eth_signTypedData_v4", paramsJson: "[]").isEmpty)
    }
}

// MARK: - Displayed is signed (FR-017)

/// Captures the bytes the ceremony was asked to sign.
@MainActor
final class ChallengeCapturingSigner: UserOpSigner {
    private(set) var challenge: Data?

    func sign(
        challenge: Data, credentialIdHex: String?, transports: String, method: KeyMethod
    ) async throws -> Assertion {
        self.challenge = challenge
        throw PasskeyFailure(kind: .cancelled, message: "")
    }
}

@MainActor
struct DisplayedIsSignedTests {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let token = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
    private let spender = "0x1111111111111111111111111111111111111111"

    /// `approve(spender, amount)` calldata.
    private func approve(_ amountWord: String) -> String {
        "0x095ea7b3" + String(repeating: "0", count: 24) + String(spender.dropFirst(2)) + amountWord
    }

    private func challenge(forParams paramsJson: String) async -> Data? {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        // A call WITH calldata must be estimated, and an estimate that fails
        // is fatal for a contract call — correctly, since submitting an
        // un-estimated contract call is how money is burned on a revert. So
        // the estimate is scripted; without it the spine stops before the
        // ceremony and this test measures nothing.
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40",
            "callGasLimit": "0x30d40",
            "preVerificationGas": "0xc350",
        ] as [String: Any])
        let signer = ChallengeCapturingSigner()
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: ScriptedAccounts(),
            signer: { signer }
        )
        guard let calls = SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: paramsJson)
        else { return nil }
        _ = try? await spine.submit(
            chainId: 100, account: golden, calls: calls,
            // A USABLE quote: `quoted_fee_usable` refuses a zero amount with
            // no recipient, and the spine then stops before the ceremony —
            // which is correct, and would make this test measure nothing.
            gasFeeToken: nil, quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: golden),
            writeAhead: { _, _ in true }
        )
        return signer.challenge
    }

    /// **What is displayed is what is signed.**
    ///
    /// The guard rewrites an unlimited approval to a finite cap, and the
    /// person is shown the cap. If the signature were computed over anything
    /// but those exact bytes — a cached draft, the original params, a
    /// re-derivation — this passes and somebody grants a stranger their whole
    /// balance while reading "100 USDC".
    ///
    /// The property is checked the only way that cannot be faked: the same
    /// pipeline, two calldatas, two different challenges. A challenge that did
    /// not depend on the calldata would be identical for both.
    @Test func theChallengeFollowsTheCalldataThatWasShown() async {
        let unlimited = #"[{"to":"\#(token)","value":"0x0","data":"\#(approve(String(repeating: "f", count: 64)))"}]"#
        // 100 USDC at six decimals.
        let capped = #"[{"to":"\#(token)","value":"0x0","data":"\#(approve(String(format: "%064x", 100_000_000)))"}]"#

        let unlimitedChallenge = await challenge(forParams: unlimited)
        let cappedChallenge = await challenge(forParams: capped)

        #expect(unlimitedChallenge?.count == 32)
        #expect(cappedChallenge?.count == 32)
        #expect(unlimitedChallenge != cappedChallenge,
                "the ceremony signed the same bytes for two different approvals — the challenge is not a function of the calldata")
    }

    /// And the capped calldata is what the approve hands on: the guard's
    /// rewrite reaches the spine, not the request the page sent.
    @Test func theGuardsRewriteIsWhatTheSubmitReads() {
        let capped = #"[{"to":"\#(token)","value":"0x0","data":"\#(approve(String(format: "%064x", 100_000_000)))"}]"#
        var guardView = GuardViewWire.empty
        guardView = GuardViewWire(
            surface: .approvalEditor, detected: nil, meta: guardView.meta, editor: nil,
            confirmAllowed: true, rewrittenParamsJson: capped, unlimitedConsented: false, increaseTotal: nil,
            decimalsUnverified: false, expired: false, batch: nil
        )
        let override = SigningController.approveOpts(
            fee: nil, clear: .empty, guard: guardView
        )["params_override_json"] as? String

        #expect(override == capped)
        let calls = SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: override ?? "[]")
        #expect(calls?.first?.data == approve(String(format: "%064x", 100_000_000)))
        #expect(calls?.first?.data.hasSuffix(String(repeating: "f", count: 64)) == false,
                "the unlimited word must not survive into the call")
    }
}

// MARK: - The controller, end to end

@MainActor
struct SigningControllerTests {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let token = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"

    private func controller(_ store: VelaStore) -> SigningController {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        return SigningController(
            wallet: (address: golden, credentialId: "cred-1"),
            relay: relay,
            accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore(), offline: true),
            ports: SigningController.Ports(knownChains: { [100] })
        )
    }

    /// **Choosing a cap on the editor must reach the guard through the
    /// controller**, not only through the core.
    ///
    /// Device-found: 撤销 was tapped on the phone and the sheet went on saying
    /// 无限额. The core answers `preset_selected` correctly — proved in
    /// `BrowserWireDriftTests` — so the question was whether the controller
    /// carries it, and a screenshot cannot answer that.
    @Test func choosingACapReachesTheGuardThroughTheController() async {
        let suite = "vela.tests.signing.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let unlimited = "0x095ea7b3"
            + String(repeating: "0", count: 24) + String(repeating: "1", count: 40)
            + String(repeating: "f", count: 64)

        let controller = self.controller(store)
        controller.open(SigningController.Incoming(
            id: "req-1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"\#(token)","value":"0x0","data":"\#(unlimited)"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        ))

        #expect(controller.guardView.surface == .approvalEditor)
        // 2026-09-26: kept as the site asked until the person picks a cap.
        #expect(controller.guardView.editor?.choice == .unlimited)
        #expect(controller.guardView.confirmAllowed)
        #expect(controller.guardView.unlimitedConsented)

        controller.guardPreset("revoke")

        #expect(controller.guardView.editor?.choice == .revoke,
                "the chip's event never reached the guard through the controller")
        #expect(controller.guardView.confirmAllowed)
        #expect(!controller.guardView.unlimitedConsented, "a cap withdraws the consent")
        #expect(controller.guardView.rewrittenParamsJson != nil,
                "a capped approval must produce params to sign")
    }

    /// **A batch leg's chip must reach THAT leg.** The single approval's
    /// `preset_selected` is ignored by the core on a batch, which is how the
    /// leg cards were drawn with chips that did nothing.
    @Test func aBatchLegsChipReachesItsOwnLeg() async {
        let suite = "vela.tests.signing.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let unlimited = "0x095ea7b3"
            + String(repeating: "0", count: 24) + String(repeating: "1", count: 40)
            + String(repeating: "f", count: 64)

        let controller = self.controller(store)
        controller.open(SigningController.Incoming(
            id: "req-b", method: "wallet_sendCalls",
            paramsJson: #"[{"version":"1.0","chainId":"0x64","calls":[{"to":"\#(token)","value":"0x0","data":"\#(unlimited)"}]}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        ))

        #expect(controller.guardView.surface == .batch)
        #expect(controller.guardView.batch?.legs.first?.choice == .unlimited, "kept as asked by default")
        #expect(controller.guardView.unlimitedConsented)

        controller.guardLegPreset(0, "revoke")

        #expect(controller.guardView.batch?.legs.first?.choice == .revoke,
                "the leg chip never reached its leg")
        #expect(!controller.guardView.unlimitedConsented)
        #expect(controller.guardView.rewrittenParamsJson != nil, "a revoked leg is re-encoded")
    }
}

// MARK: - The sheet

@MainActor
struct SigningLiveTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// G49 (RJ15): a 1000-wei outflow is written exactly — never "xDAI −0" —
    /// with U+2212; a zero change is not drawn at all. The figure is the
    /// core's `formatSignedTokenAmount`, in the person's number preset.
    @Test func aSignedAmountNeverReadsMinusZero() throws {
        let before = Formats.current
        defer { Formats.current = before }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        let sim = try CoreJSON.decode(TrustSimViewWire.self, from: [
            "ready": true,
            "judgments": [
                ["type": "native", "delta": "-1000"],
                ["type": "erc20_trusted", "token": "0xusdc", "delta": "0", "symbol": "USDC", "decimals": 6],
                ["type": "erc20_trusted", "token": "0xusdc", "delta": "2500000", "symbol": "USDC", "decimals": 6],
            ] as [[String: Any]],
        ])
        var ctx = context()
        ctx.sim = sim
        ctx.simulation = .answered
        guard case .balances(_, let rows, _, _)? = SigningLive.balanceBlocks(isTransaction: true, context: ctx).first
        else {
            Issue.record("no balance block")
            return
        }
        #expect(rows.count == 2, "the zero change is not drawn: \(rows.map(\.delta))")
        let dust = try #require(rows.first)
        #expect(dust.symbol == "xDAI")
        #expect(dust.delta != "\u{2212}0" && dust.delta != "-0")
        #expect(dust.delta.hasPrefix("\u{2212}"), "U+2212, not a hyphen: \(dust.delta)")
        #expect(dust.delta == formatSignedTokenAmount(deltaBaseUnits: "-1000", decimals: 18, preset: "comma_dot"))
        #expect(rows.last?.delta == "+2.5")
        #expect(SimDeltas.deltaText("0", decimals: 18) == nil)
    }

    /// Issue #314: the wallet's own key backup leads with its outcome. Its
    /// intent is the headline (the sheet draws `dappOwn` that way) and a
    /// simulation that moves nothing is folded into the technical details —
    /// it was a bordered "Balance changes" card weighing as much as the
    /// outcome. A dApp's sheet keeps the card, and a simulation with something
    /// to say is never folded away, not even on the wallet's own.
    @Test func theWalletsOwnBackupFoldsASimulationThatMovesNothing() throws {
        let registry = "0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9"
        // The core's reading of the backup since the rows say the network:
        // every word a term (`label_term`), so no shell relabels them.
        let result = try CoreJSON.decode(ClearSignResultWire.self, from: [
            "intent": "Back up public keys", "intent_term": "intentBackUpPublicKeys",
            "contract_name": "Vela passkey registry", "fields": [
                ["label": "Network", "label_term": "labelNetwork", "value": "Ethereum", "format": "raw",
                 "warning": false, "unverified": false, "role": "generic", "detail": false, "expired": false],
                ["label": "Address", "label_term": "labelAddress", "value": "0x88cca0…266894",
                 "format": "addressName", "warning": false, "unverified": false, "role": "generic",
                 "detail": false, "expired": false, "address": "0x88cca0eedbf2c4426110bbfc998f048689266894"],
                ["label": "Public keys", "label_term": "labelPublicKeys", "value": "1", "format": "raw",
                 "warning": false, "unverified": false, "role": "generic", "detail": false, "expired": false],
            ],
            "risk": "safe", "contract_address": registry, "verified": true, "provenance": "built_in",
            "sign_type": "transaction", "partial": false, "best_effort": false, "to_own_token": false,
        ])
        let backup = clear(surface: .clearSign, result: result)
        let params = #"[{"to":"0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9","data":"0xcd438f9b","value":"0x0"}]"#
        func sheet(own: Bool, simulation: SigningController.Simulation, judgments: [[String: Any]] = []) throws -> SigningModel {
            var ctx = context()
            ctx.sim = try CoreJSON.decode(TrustSimViewWire.self, from: ["ready": true, "judgments": judgments])
            ctx.simulation = simulation
            var request = SigningController.Incoming(
                id: "r", method: "eth_sendTransaction", paramsJson: params,
                origin: "https://getvela.app", transportId: own ? SigningLive.walletTransport : "tab-1", chainId: 1
            )
            request.firstParty = own
            return SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc),
                request: request,
                sign: .empty, clear: backup, guard: .empty, fee: nil, context: ctx
            )
        }
        func hasBalances(_ model: SigningModel) -> Bool {
            model.blocks.contains { if case .balances = $0 { return true } else { return false } }
        }

        let own = try sheet(own: true, simulation: .answered)
        #expect(own.dappOwn)
        #expect(!hasBalances(own), "the no-change card is still on the sheet")
        #expect(own.tech.simResult?.label == loc.t("componentsUi.signing.simResultLabel"))
        #expect(own.tech.simResult?.value == loc.t("componentsUi.signing.balanceNoAssetsMove"))

        let dapp = try sheet(own: false, simulation: .answered)
        #expect(hasBalances(dapp), "a dApp keeps its balance card")
        #expect(dapp.tech.simResult == nil)

        let reverts = try sheet(
            own: true,
            simulation: .notice(risk: "danger", key: "componentsUi.signing.simWillFail", reason: nil)
        )
        #expect(reverts.blocks.contains { if case .warning(.danger, _) = $0 { return true } else { return false } },
                "a revert is never folded away")
        #expect(reverts.tech.simResult == nil)
        let moves = try sheet(
            own: true, simulation: .answered,
            judgments: [["type": "native", "delta": "-1000000000000000"]]
        )
        #expect(hasBalances(moves), "a balance that would move stays on the sheet")
        #expect(moves.tech.simResult == nil)
    }

    /// The wallet's own backup, on the REAL core: the `register()` bytes the
    /// core's own test reads (`registry-register-unit10.hex`), on Ethereum.
    /// The rows are Network, Address, Wallet name and Keys included, in the
    /// reader's words, because the core names each one — the index relabel
    /// that called row 0 "Registered as" is gone — and the confirm reads the
    /// intent ("Copy this wallet's record": what it is, not a backup that
    /// could bring a key back).
    ///
    /// Whose sheet it is comes from the request alone: the same bytes from a
    /// page are a page's sheet, with the same honest rows.
    @Test func theBackupsRowsAreTheCoresWordsInTheReadersLanguage() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let hex = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent().deletingLastPathComponent()
                .deletingLastPathComponent().deletingLastPathComponent()
                .appendingPathComponent("rust/crates/vela-core/tests/fixtures/registry-register-unit10.hex"),
            encoding: .utf8
        ).trimmingCharacters(in: .whitespacesAndNewlines)
        let tx: [[String: Any]] = [[
            "to": "0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9", "data": "0x\(hex)", "value": "0x0",
        ]]
        let params = String(decoding: try JSONSerialization.data(withJSONObject: tx), as: UTF8.self)
        let kickoff = try #require(SigningController.clearKickoff(
            method: "eth_sendTransaction", paramsJson: params, chainId: 1, origin: nil
        ))
        let core = ClearSigningCore()
        let result = try core.dispatch(eventJson: CoreJSON.string(kickoff))
        let view = try CoreJSON.decode(
            ClearSigningViewWire.self, from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:]
        )
        #expect(view.result?.intentTerm == "intentBackUpPublicKeys")

        func sheet(own: Bool) -> SigningModel {
            let base = context()
            let ctx = SigningLive.Context(
                loc: zh, chainName: "Ethereum", chainDot: .blue, nativeSymbol: "ETH",
                walletName: base.walletName, walletAddress: base.walletAddress
            )
            var request = SigningController.Incoming(
                id: "r", method: "eth_sendTransaction", paramsJson: params,
                origin: own ? "https://getvela.app" : "https://evil.example",
                transportId: own ? SigningLive.walletTransport : "tab-1", chainId: 1
            )
            request.firstParty = own
            return SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: zh), request: request,
                sign: .empty, clear: view, guard: .empty, fee: nil, context: ctx
            )
        }
        for own in [true, false] {
            let model = sheet(own: own)
            #expect(model.dappOwn == own)
            #expect(intents(model.blocks) == [zh.t("componentsUi.signing.intentBackUpPublicKeys")])
            let drawn = rows(model.blocks)
            #expect(drawn.map(\.label) == [
                zh.t("componentsUi.signing.labelNetwork"),
                zh.t("componentsUi.signing.labelAddress"),
                zh.t("componentsUi.signing.labelWalletName"),
                zh.t("componentsUi.signing.labelPublicKeys"),
            ], "rows: \(drawn.map(\.label))")
            // PR 3: the wallet's name is a row (it goes onto a second public
            // chain), and the count sits under "keys included".
            #expect(drawn.map(\.label) == ["网络", "地址", "钱包名称", "包含的钥匙"])
            #expect(drawn.first?.value == "Ethereum", "the network the backup goes to is a row")
            #expect(drawn.allSatisfy { !$0.label.hasPrefix("settingsModals.") && !$0.label.hasPrefix("componentsUi.") })
            #expect(model.confirm?.action == "复制钱包记录", "the confirm reads the intent, not a bare 确认")
            // The wallet's own sheet has no requester header: the intent is
            // its title (drawn once, in the header row with the ✕), and the
            // technical details do not name the wallet's own contract to it.
            #expect(model.headline?.text == (own ? "复制钱包记录" : nil))
            #expect(model.formBlocks.count == model.blocks.count - (own ? 1 : 0))
            #expect(model.tech.summary == (own ? nil : "Vela passkey registry"))
        }
    }

    /// The REAL core's reading of a batch, kicked off as the sheet kicks it off.
    private func resolvedBatch(_ paramsJson: String) throws -> ClearSigningViewWire {
        let event = try #require(SigningController.clearKickoff(
            method: "wallet_sendCalls", paramsJson: paramsJson, chainId: 100, origin: nil
        ))
        let core = ClearSigningCore()
        let result = try core.dispatch(eventJson: CoreJSON.string(event))
        return try CoreJSON.decode(ClearSigningViewWire.self, from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:])
    }

    private func cards(_ blocks: [SigningBlock]) -> [(title: String?, rows: [SigningRow], tone: SigningTone)] {
        blocks.compactMap { block in
            if case .card(let title, let rows, let tone) = block { return (title, rows, tone) }
            return nil
        }
    }

    /// 089 S1 — THE defect: `[1 wei → A, 1 xDAI → B]` read "Send 0.000…1 xDAI
    /// to A" and signed both. Every call is its own card, the total is said,
    /// and the headline is the batch's, never call 1's "Send".
    @Test func aTwoCallBatchDrawsBothCallsAndTheirTotal() throws {
        let a = "0x7687c0bc1dd2b9d7e9a5b1b4e1b0cbd8e0c3d141"
        let b = "0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c"
        let view = try resolvedBatch(
            #"[{"chainId":"0x64","calls":[{"to":"\#(a)","value":"0x1"},{"to":"\#(b)","value":"0xde0b6b3a7640000"}]}]"#
        )
        #expect(view.surface == .batch)
        #expect(view.result == nil && view.plainSend == nil, "no single reading stands for the batch")
        let blocks = SigningLive.blocks(clear: view, to: a, valueHex: "0x1", dataBytes: 0, context: context())
        #expect(intents(blocks) == [loc.t("componentsUi.signing.batchIntent")])
        let drawn = cards(blocks)
        #expect(drawn.count == 2)
        let send = loc.t("componentsUi.signing.intentSend")
        #expect(drawn[0].title == loc.t("componentsUi.signing.batchStep", vars: ["index": "1", "action": send]))
        #expect(drawn[0].rows.first?.value == "\u{2212}0.000000000000000001 xDAI")
        #expect(drawn[0].rows.last?.value.lowercased() == a)
        #expect(drawn[1].title == loc.t("componentsUi.signing.batchStep", vars: ["index": "2", "action": send]))
        #expect(drawn[1].rows.first?.value == "\u{2212}1 xDAI")
        #expect(drawn[1].rows.last?.value.lowercased() == b)
        let total = blocks.compactMap { block -> SigningRow? in
            if case .rows(let rows) = block { return rows.first }
            return nil
        }.first
        #expect(total?.label == loc.t("send.splitTotalLabel"))
        #expect(total?.value == "\u{2212}1.000000000000000001 xDAI")
        #expect(SigningLive.confirmLabel(clear: view, loc: loc) == loc.t("componentsUi.signing.confirmLabel"))
    }

    /// 089 S1: a call nobody could read is a card that says so, never dropped;
    /// a batch whose SECOND call is an unlimited approve reads danger in that
    /// card and the headline, and a cap on that call's leg is what ITS card
    /// reads (the guard's legs are the calls, in order).
    @Test func anUnreadableCallSaysSoAndASecondCallCapIsItsOwn() throws {
        let view = try CoreJSON.decode(ClearSigningViewWire.self, from: [
            "resolving": false, "resolved": true, "result": NSNull(), "message": NSNull(),
            "surface": "batch", "confirm": ["type": "confirm"], "blind_typed": NSNull(),
            "danger_haptic": false,
            "batch": [
                "calls": [
                    ["index": 1, "surface": "blind_transaction", "result": NSNull(), "plain_send": NSNull(),
                     "to": "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c", "data_bytes": 36,
                     "value_wei": "0", "amount": "0", "risk": "caution"],
                    ["index": 2, "surface": "clear_sign", "plain_send": NSNull(),
                     "result": [
                        "intent": "Approve", "intent_term": "intentApprove", "contract_name": NSNull(),
                        "owner": NSNull(), "risk": "danger", "contract_address": "0xdd", "verified": false,
                        "provenance": "standard", "sign_type": "transaction", "partial": false,
                        "best_effort": false, "to_own_token": false,
                        "fields": [[
                            "label": "Amount", "value": "Unlimited", "format": "tokenAmount",
                            "token_address": NSNull(), "warning": true, "unverified": false, "role": "send_amount",
                            "detail": false, "expired": false, "address": NSNull(), "usd_value": NSNull(),
                            "label_term": "labelAmount", "value_term": "valueUnlimited",
                        ]],
                     ] as [String: Any],
                     "to": "0xdd", "data_bytes": 68, "value_wei": "0", "amount": "0", "risk": "danger"],
                ] as [[String: Any]],
                "total_value_wei": "0", "total_amount": "0", "risk": "danger",
            ] as [String: Any],
        ])
        let blocks = SigningLive.blocks(clear: view, to: nil, valueHex: nil, dataBytes: 0, context: context())
        let drawn = cards(blocks)
        let undecoded = loc.t("componentsUi.signing.blindDecodeWarning", vars: ["bytes": "36"])
        #expect(drawn[0].title == loc.t("componentsUi.signing.batchStep", vars: ["index": "1", "action": undecoded]))
        #expect(drawn[0].rows.map(\.label) == [loc.t("componentsUi.signing.interactingLabel")])
        #expect(drawn[1].tone == .danger)
        // …and whom the call goes to: inside a batch nothing else says it.
        #expect(drawn[1].rows.last?.label == loc.t("componentsUi.signing.interactingLabel"))
        #expect(drawn[1].rows.last?.value == "0xdd")
        if case .intent(_, let tone)? = blocks.first { #expect(tone == .danger) } else { Issue.record("no headline") }

        let capped = try CoreJSON.decode(GuardViewWire.self, from: [
            "surface": "batch", "detected": NSNull(),
            "meta": ["symbol": "…", "decimals": 18, "verified": false, "loading": false],
            "editor": NSNull(), "confirm_allowed": true, "rewritten_params_json": NSNull(),
            "unlimited_consented": false, "unlimited_warning": false, "increase_total": NSNull(), "decimals_unverified": false,
            "expired": false,
            "batch": [
                "legs": [
                    ["to": "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c", "approval": NSNull(),
                     "meta": ["symbol": "…", "decimals": 18, "verified": false, "loading": false],
                     "editor": NSNull(), "choice": NSNull(), "needs_editor": false, "needs_choice": false,
                     "grants_broad": false],
                    ["to": "0xdd",
                     "approval": [
                        "kind": "erc20_approve", "token_address": "0xdd", "spender": "0x1111",
                        "amount_raw": "f", "amount_bits": 256, "is_unbounded": true, "is_boolean_grant": false,
                        "is_reducing": false, "editable": true, "block_reason": NSNull(), "deadline": NSNull(),
                        "locus": ["type": "calldata_word", "word_index": 1],
                     ] as [String: Any],
                     "meta": ["symbol": "USDC", "decimals": 6, "verified": true, "loading": false],
                     "editor": [
                        "mode": "custom", "custom_text": "250", "error": NSNull(),
                        "choice": ["type": "amount", "amount_raw": "250000000"],
                        "display_amount_raw": "250000000", "requested_finite": false,
                        "requested_unlimited": true, "has_balance_cap": false, "revoke_offered": true,
                        "balance_raw": NSNull(),
                     ] as [String: Any],
                     "choice": ["type": "amount", "amount_raw": "250000000"],
                     "needs_editor": true, "needs_choice": false, "grants_broad": false],
                ] as [[String: Any]],
                "any_uncapped": false, "any_to_own_token": false, "all_settled": true,
            ] as [String: Any],
        ])
        let shown = SigningLive.cappedApproval(view, guard: capped)
        #expect(shown.batch?.calls[1].result?.fields.first?.value == "250 USDC")
        #expect(shown.batch?.calls[1].risk == .caution, "an approve, capped, is a caution")
        #expect(shown.batch?.risk == .caution)
    }

    private func context() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894",
            origin: "https://x.test"
        )
    }

    private func clear(
        surface: ClearSurface, resolved: Bool = true,
        result: ClearSignResultWire? = nil, confirm: ClearConfirmWire = .confirm,
        plainSend: ClearPlainSendWire? = nil
    ) -> ClearSigningViewWire {
        ClearSigningViewWire(
            resolving: false, resolved: resolved, result: result, message: nil,
            surface: surface, confirm: confirm, blindTyped: nil, dangerHaptic: false,
            plainSend: plainSend
        )
    }

    /// The core's verdict for a request, as the live sheet would receive it.
    private func resolved(value: Any?) throws -> ClearSigningViewWire {
        let params: [[String: Any]] = [[
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x",
            "value": value ?? NSNull(),
        ]]
        let paramsJson = String(decoding: try JSONSerialization.data(withJSONObject: params), as: UTF8.self)
        let call = SigningController.firstCall(paramsJson: paramsJson)
        let core = ClearSigningCore()
        let result = try core.dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": call?.to as Any? ?? NSNull(), "data": call?.data as Any? ?? NSNull(),
            "value": call?.value as Any? ?? NSNull(),
            "chain_id": 100, "locale": SigningController.defaultLocale,
        ]))
        return try CoreJSON.decode(ClearSigningViewWire.self, from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:])
    }

    private func intents(_ blocks: [SigningBlock]) -> [String] {
        blocks.compactMap { block -> String? in
            if case .intent(let text, _) = block { return text }
            return nil
        }
    }

    /// **A plain native transfer is not a contract interaction** — and since
    /// spec 082 the core says so (RC1): the look is the one the phones always
    /// drew (发送 · −0.001 xDAI · 接收方), from the core's exact card.
    @Test func aNoCalldataTransferIsDrawnAsASendRatherThanAsABlindCall() throws {
        let view = try resolved(value: "0x38d7ea4c68000")
        #expect(view.surface == .plainSend)
        let blocks = SigningLive.blocks(
            clear: view, to: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            valueHex: "0x38d7ea4c68000", dataBytes: 0, context: context()
        )
        #expect(intents(blocks) == [loc.t("componentsUi.signing.intentSend")])
        let amount = blocks.compactMap { block -> AmountLine? in
            if case .amount(let line, _, _) = block { return line }
            return nil
        }.first
        #expect(amount?.sign == "−", "the amount is the first thing a person needs to see")
        #expect(amount?.value == "0.001")
        #expect(amount?.symbol == "xDAI", "the coin is the fee row's (RC5)")
        #expect(blocks.contains { if case .party = $0 { true } else { false } })
        #expect(SigningLive.confirmLabel(clear: view, loc: loc) == loc.t("componentsUi.signing.confirmSend"))
    }

    /// Zero with no calldata: the same card, "0 xDAI" with no minus, and a
    /// neutral 确认 — never "confirm send" (RC3).
    @Test func aZeroValuePlainSendReadsZeroWithANeutralConfirm() throws {
        let view = try resolved(value: "0x0")
        #expect(view.surface == .plainSend)
        #expect(view.plainSend?.noValue == true)
        let blocks = SigningLive.blocks(
            clear: view, to: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            valueHex: "0x0", dataBytes: 0, context: context()
        )
        let amount = blocks.compactMap { block -> AmountLine? in
            if case .amount(let line, _, _) = block { return line }
            return nil
        }.first
        #expect(amount?.sign == "")
        #expect(amount?.value == "0")
        #expect(SigningLive.confirmLabel(clear: view, loc: loc) == loc.t("componentsUi.signing.confirmLabel"))
    }

    /// A JSON number is passed to the core as text (RC6), which refuses to
    /// print it (RC4): the blind card, never a calm "0".
    @Test func aNumericValueIsTheBlindCardNeverACalmZero() throws {
        #expect(SigningController.valueText(NSNumber(value: 1000)) == "1000")
        #expect(SigningController.valueText(NSNull()) == nil)
        let view = try resolved(value: NSNumber(value: 1000))
        #expect(view.surface == .blindTransaction)
        #expect(view.plainSend == nil)
        let blocks = SigningLive.blocks(
            clear: view, to: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            valueHex: "1000", dataBytes: 0, context: context()
        )
        #expect(intents(blocks) == [loc.t("componentsUi.signing.intentContractCall")])
    }

    /// A real contract call with calldata gets the blind card, honestly.
    @Test func anUndecodableCallSaysSoWithItsSize() {
        let blocks = SigningLive.blocks(
            clear: clear(surface: .blindTransaction), to: "0xrouter",
            valueHex: "0x0", dataBytes: 412, context: context()
        )
        #expect(blocks.contains { block in
            if case .warning(_, let text) = block { return text.contains("412") }
            return false
        }, "the byte count is the only honest measure of what nobody could read")
    }

    /// **The core names a descriptor's words; the sheet says them in the
    /// reader's language.** The founder read "Approve / Amount / Spender /
    /// Unlimited" in English on a Chinese sheet (2026-09-27). Decoded from the
    /// core's own JSON shape, so the snake_case terms are proved to arrive.
    @Test func namedDescriptorWordsReadInTheReadersLanguage() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        func field(_ label: String, _ labelTerm: Any, _ value: String, _ valueTerm: Any, warning: Bool, role: String) -> [String: Any] {
            ["label": label, "label_term": labelTerm, "value": value, "value_term": valueTerm,
             "format": warning ? "tokenAmount" : "raw", "token_address": NSNull(), "warning": warning,
             "unverified": false, "role": role, "detail": false, "expired": false,
             "address": NSNull(), "usd_value": NSNull()]
        }
        let view = try CoreJSON.decode(ClearSigningViewWire.self, from: [
            "resolving": false, "resolved": true, "surface": "clear_sign",
            "confirm": ["type": "confirm_intent", "intent": "Approve", "intent_term": "intentApprove"],
            "message": NSNull(), "blind_typed": NSNull(), "danger_haptic": false,
            "result": [
                "intent": "Approve", "intent_term": "intentApprove",
                "contract_name": NSNull(), "owner": NSNull(), "risk": "danger",
                "contract_address": "0xa0b8", "verified": false, "provenance": "standard",
                "sign_type": "transaction", "partial": false, "best_effort": false, "to_own_token": false,
                "fields": [
                    field("Amount", "labelAmount", "Unlimited", "valueUnlimited", warning: true, role: "generic"),
                    field("Spender", "labelSpender", "0x1111", NSNull(), warning: false, role: "spender"),
                    field("Referral code", NSNull(), "abc", NSNull(), warning: false, role: "generic"),
                ],
            ],
        ])
        let shown = SigningLive.localizedTerms(view, loc: zh)
        let result = try #require(shown.result)
        #expect(result.intent == zh.t("componentsUi.signing.intentApprove"))
        #expect(result.intent != "Approve")
        #expect(result.fields.map(\.label) == [
            zh.t("componentsUi.signing.labelAmount"), zh.t("componentsUi.signing.labelSpender"), "Referral code",
        ])
        #expect(result.fields.map(\.value) == [zh.t("componentsUi.signing.valueUnlimited"), "0x1111", "abc"])
        // The confirm says the core's word for an intent it has no verb of its own for.
        #expect(SigningLive.confirmLabel(clear: view, loc: zh) == zh.t("componentsUi.signing.intentApprove"))
    }

    /// **The confirm's verb is a corpus string, never a raw intent id.**
    ///
    /// Android shipped a button reading 确认send — the id concatenated onto a
    /// prefix. Asserting the absence of the id would be wrong in English,
    /// where "Confirm send" legitimately contains "send"; the honest check is
    /// that the label IS the corpus value for that intent. Driven in Chinese,
    /// where an id that leaked through would be unmistakable.
    @Test func theConfirmVerbIsAlwaysCorpusWords() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let expected = [
            "send": "confirmSend", "swap": "confirmSwap",
            "deposit": "confirmDeposit", "withdraw": "confirmWithdraw",
        ]
        for (intent, key) in expected {
            let label = SigningLive.confirmLabel(
                clear: clear(surface: .clearSign, confirm: .confirmIntent(intent)), loc: zh
            )
            #expect(label == zh.t("componentsUi.signing.\(key)"), "\(intent) → \(label)")
            #expect(!label.contains(intent), "the id leaked into the verb: \(label)")
        }
        // An intent nobody has drawn a verb for falls back to the neutral one
        // rather than printing itself.
        let unknown = SigningLive.confirmLabel(
            clear: clear(surface: .clearSign, confirm: .confirmIntent("teleport")), loc: zh
        )
        #expect(unknown == zh.t("componentsUi.signing.confirmLabel"))
        #expect(!unknown.contains("teleport"))

        #expect(SigningLive.confirmLabel(clear: clear(surface: .messageSign, confirm: .sign), loc: zh)
                == zh.t("componentsUi.signing.signLabel"))
    }

    /// The confirm is three machines ANDed — and an **off-chain** signature has
    /// no fee to be ready about.
    // MARK: - Spec 096 (part B): what you see is what you sign

    /// The REAL core reading one transaction, every question it asks answered
    /// as a chain with no descriptor service and no selector database would.
    private func readByTheCore(to: String, data: String, value: String) throws -> ClearSigningViewWire {
        let core = ClearSigningCore()
        var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction", "to": to, "data": data, "value": value,
            "chain_id": 56, "locale": SigningController.defaultLocale,
        ])))
        var queue = result["effects"] as? [[String: Any]] ?? []
        var asked = 0
        while !queue.isEmpty, asked < 100 {
            let effect = queue.removeFirst()
            asked += 1
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let op = effect["operation"] as? [String: Any] else { continue }
            let answer: [String: Any]
            switch op["type"] as? String {
            case "now": answer = ["type": "clock", "now_ms": 1_790_947_000_000.0]
            case "http_get": answer = ["type": "descriptor_fetched", "path": op["path"] ?? "", "json": NSNull()]
            case "rpc_eth_call":
                answer = ["type": "rpc_answer", "probe": op["probe"] ?? "", "chain_id": op["chain_id"] ?? 56,
                          "to": op["to"] ?? "", "result": NSNull(), "rpc_error": true]
            case "selector_db_lookup": answer = ["type": "selector_candidates", "sigs": [String]()]
            case "timer": answer = ["type": "timed_out", "token": op["token"] ?? 0]
            default: continue
            }
            result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(answer)))
            queue += result["effects"] as? [[String: Any]] ?? []
        }
        return try CoreJSON.decode(ClearSigningViewWire.self, from: result["view"] as? [String: Any] ?? [:])
    }

    private func bnbContext() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "BNB Chain", chainDot: .yellow, nativeSymbol: "BNB",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894",
            origin: "https://app.aave.com"
        )
    }

    private func rows(_ blocks: [SigningBlock]) -> [SigningRow] {
        blocks.flatMap { block -> [SigningRow] in
            if case .rows(let rows) = block { return rows }
            return []
        }
    }

    /// F4, end to end on the real core: Aave's `depositETH` of 0.003 BNB (the
    /// 096 pass's own request) reads "Supply · 0.003 BNB" — not "Value 0" — and
    /// a call nobody could read still says the coin it sends.
    @Test func theCoinACallSendsIsInTheReadablePart() throws {
        let deposit = try readByTheCore(
            to: "0x0c2c95b24529664fe55d4437d7a31175cfe6c4f7",
            data: "0x474cf53d0000000000000000000000006807dc923806fe8fd134338eabca509979a7e0cb00000000000000000000000088cca0eedbf2c4426110bbfc998f0486892668940000000000000000000000000000000000000000000000000000000000000000",
            value: "0xaa87bee538000"
        )
        #expect(deposit.surface == .clearSign)
        let drawn = SigningLive.blocks(clear: deposit, to: nil, valueHex: nil, dataBytes: 100, context: bnbContext())
        let supply = rows(drawn).map { "\($0.label) \($0.value)" }
        #expect(supply.first == "\(loc.t("componentsUi.signing.intentSupply")) 0.003 BNB", "\(supply)")
        #expect(!supply.contains { $0.contains("Value") || $0.contains("Number") }, "\(supply)")
        #expect(deposit.nativeValue == nil, "the reading's own row says it — once")

        let blind = try readByTheCore(
            to: "0x7777777777777777777777777777777777777777", data: "0xdeadbeef", value: "0xaa87bee538000"
        )
        #expect(blind.surface == .blindTransaction)
        #expect(blind.nativeValue?.amount == "0.003")
        let coin = rows(SigningLive.blocks(clear: blind, to: nil, valueHex: nil, dataBytes: 4, context: bnbContext()))
        #expect(coin.first?.label == loc.t("componentsUi.signing.labelAmount"))
        #expect(coin.first?.value == "\u{2212}0.003 BNB")
    }

    /// F5: an order whose terms are off chain says so; a known contract is
    /// named in its batch card.
    @Test func anOrderSaysItsTermsAreNotShownAndAKnownTargetIsNamed() throws {
        let order = try CoreJSON.decode(ClearSignResultWire.self, from: [
            "intent": "Swap", "fields": [
                ["label": "Order", "value": "0x09e95d9c...6abfb9fe", "format": "raw", "warning": false,
                 "unverified": false, "role": "generic", "detail": false, "expired": false],
            ],
            "risk": "caution", "verified": true, "provenance": "built_in", "sign_type": "transaction",
            "partial": false, "best_effort": false, "to_own_token": false, "terms_off_chain": true,
        ])
        let warning = loc.t("componentsUi.signing.warnOrderTerms")
        #expect(!warning.contains("warnOrderTerms"), "the corpus has the sentence")
        let lone = SigningLive.blocks(clear: clear(surface: .clearSign, result: order), to: nil, valueHex: nil,
                                      dataBytes: 164, context: bnbContext())
        #expect(lone.contains { if case .warning(.caution, let text) = $0 { return text == warning } else { return false } })

        var batch = try resolvedBatch(#"[{"chainId":"0x64","calls":[{"to":"0x7687c0bc1dd2b9d7e9a5b1b4e1b0cbd8e0c3d141","value":"0x1"},{"to":"0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c","value":"0x1"}]}]"#)
        var call = try #require(batch.batch?.calls[1])
        call = ClearBatchCallWire(index: call.index, surface: .clearSign, result: order, plainSend: nil,
                                  to: "0x9008D19f58AAbD9eD0D60971565AA8510560ab41", toName: "CoW Protocol",
                                  dataBytes: 164, valueWei: "0", amount: "0", risk: .caution)
        batch.batch?.calls[1] = call
        let drawn = SigningLive.blocks(clear: batch, to: nil, valueHex: nil, dataBytes: 0, context: bnbContext())
        #expect(drawn.contains { if case .warning(.caution, let text) = $0 { return text == warning } else { return false } })
        let target = cards(drawn)[1].rows.first { $0.label == loc.t("componentsUi.signing.interactingLabel") }
        #expect(target?.value == "CoW Protocol")
        #expect(target?.mono == false, "a name is not monospace")
    }

    // MARK: - Spec 097 (part A): nothing false or unknown stated as certain

    /// The 1inch order the 097 pass signed (USDC → BNB), verbatim.
    private static let oneInchOrder = #"{"types":{"Order":[{"name":"salt","type":"uint256"},{"name":"maker","type":"address"},{"name":"receiver","type":"address"},{"name":"makerAsset","type":"address"},{"name":"takerAsset","type":"address"},{"name":"makingAmount","type":"uint256"},{"name":"takingAmount","type":"uint256"},{"name":"makerTraits","type":"uint256"}],"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"chainId","type":"uint256"},{"name":"verifyingContract","type":"address"}]},"domain":{"name":"1inch Aggregation Router","version":"6","chainId":56,"verifyingContract":"0x111111125421ca6dc452d289314280a0f8842a65"},"primaryType":"Order","message":{"salt":"33701748006248133072184610391074722838607073826819837756152271511067157162131","maker":"0x88cca0eedbf2c4426110bbfc998f048689266894","receiver":"0x0000000000000000000000000000000000000000","makerAsset":"0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d","takerAsset":"0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c","makingAmount":"1160000000000000000","takingAmount":"1430509396956033","makerTraits":"62645329528782394789705396186445553993653435352530577117757868129563663400960"}}"#

    /// UTC, ISO dates, a 24-hour clock: a date read against the chain's clock.
    private static let utcLocale: [String: Any] = [
        "number_format": "comma_dot", "date_format": "iso", "time_format": "h24", "tz_offset_minutes": 0,
    ]

    /// The REAL core reading one request, the 097 pass's shell scripted: the
    /// chain names WBNB and USDC on BNB Chain (unless it is down), nothing else
    /// answers.
    private func readPassRequest(_ event: [String: Any], chainDown: Bool = false) throws -> ClearSigningViewWire {
        let symbols = [
            "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c": "WBNB",
            "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d": "USDC",
        ]
        let word = { (n: Int) -> String in
            let hex = String(n, radix: 16)
            return "0x" + String(repeating: "0", count: 64 - hex.count) + hex
        }
        let core = ClearSigningCore()
        var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
        var queue = result["effects"] as? [[String: Any]] ?? []
        var asked = 0
        while !queue.isEmpty, asked < 200 {
            let effect = queue.removeFirst()
            asked += 1
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let op = effect["operation"] as? [String: Any] else { continue }
            let answer: [String: Any]
            switch op["type"] as? String {
            case "now": answer = ["type": "clock", "now_ms": 1_790_957_000_000.0]
            case "http_get": answer = ["type": "descriptor_fetched", "path": op["path"] ?? "", "json": NSNull()]
            case "rpc_eth_call":
                let symbol = chainDown ? nil : symbols[((op["to"] as? String) ?? "").lowercased()]
                let value: Any = symbol.map { symbol -> String in
                    if (op["probe"] as? String) == "decimals" { return word(18) }
                    let body = symbol.utf8.map { String(format: "%02x", $0) }.joined()
                    return word(32) + String(word(symbol.utf8.count).dropFirst(2))
                        + body.padding(toLength: 64, withPad: "0", startingAt: 0)
                } ?? NSNull()
                answer = ["type": "rpc_answer", "probe": op["probe"] ?? "", "chain_id": op["chain_id"] ?? 56,
                          "to": op["to"] ?? "", "result": value, "rpc_error": symbol == nil]
            case "selector_db_lookup": answer = ["type": "selector_candidates", "sigs": [String]()]
            case "timer": answer = ["type": "timed_out", "token": op["token"] ?? 0]
            default: continue
            }
            result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(answer)))
            queue += result["effects"] as? [[String: Any]] ?? []
        }
        return try CoreJSON.decode(ClearSigningViewWire.self, from: result["view"] as? [String: Any] ?? [:])
    }

    /// N6 (and N3 on this shell — the minimum is said by its row's label): the
    /// pass's 1inch order reads as the swap it signs. The zero receiver is the
    /// account itself, BNB arrives (the unwrap flag), the minimum says so, and
    /// the expiry `makerTraits` encodes is said — 16:42 UTC on 2026-10-02.
    @Test func aOneInchOrderSaysWhereItsProceedsGoAndUntilWhen() throws {
        let view = try readPassRequest([
            "type": "resolve_typed_data", "typed_data_json": Self.oneInchOrder,
            "chain_id": 56, "locale": Self.utcLocale,
        ])
        #expect(view.surface == .clearSign)
        let drawn = rows(SigningLive.blocks(clear: view, to: nil, valueHex: nil, dataBytes: 0, context: bnbContext()))
        let value = { (key: String) in drawn.first { $0.label == loc.t("componentsUi.signing.\(key)") }?.value }
        #expect(value("labelYouPay") == "1.16 USDC", "\(drawn)")
        #expect(value("labelYouReceiveMin") == "0.001430509396956033 BNB", "\(drawn)")
        #expect(value("labelRecipient") == "0x88cca0...266894", "\(drawn)")
        #expect(value("labelValidUntil") == "2026-10-02, 16:42", "\(drawn)")
        #expect(!drawn.contains { $0.value.contains("0x00000000") })
    }

    /// N1 + N8: an amount the chain never scaled is no figure and the reading
    /// says it is incomplete (a borrow of a token the wallet does not know on
    /// BNB Chain — Ethereum's USDC address; BNB Chain's own USDC is the
    /// registry's, 097 D); a batch call on USDC names the token.
    @Test func anUnscaledAmountIsIncompleteAndATokenTargetIsNamed() throws {
        let borrow = try readPassRequest([
            "type": "resolve_transaction", "to": "0x6807dc923806fe8fd134338eabca509979a7e0cb",
            "data": "0xa415bcad000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480000000000000000000000000000000000000000000000000429d069189e00000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000088cca0eedbf2c4426110bbfc998f048689266894",
            "value": "0x0", "chain_id": 56, "locale": Self.utcLocale,
        ], chainDown: true)
        let drawn = SigningLive.blocks(clear: borrow, to: nil, valueHex: nil, dataBytes: 164, context: bnbContext())
        let amount = rows(drawn).first { $0.label == loc.t("componentsUi.signing.intentBorrow") }
        #expect(amount?.value.hasPrefix("\u{2014}") == true, "no figure: \(String(describing: amount))")
        #expect(amount?.valueTone == .caution)
        let partial = loc.t("componentsUi.signing.partialWarning")
        #expect(drawn.contains { if case .warning(.caution, let text) = $0 { return text == partial } else { return false } })

        let usdc = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"
        let approve = "0x095ea7b300000000000000000000000031c2f6fcff4f8759b3bd5bf0e1084a055615c7680000000000000000000000000000000000000000000000001018f0e6fdc80000"
        let batch = try readPassRequest([
            "type": "resolve_batch", "chain_id": 56, "locale": Self.utcLocale,
            "params_json": #"[{"chainId":"0x38","calls":[{"to":"\#(usdc)","data":"\#(approve)"},{"to":"0x7777777777777777777777777777777777777777","value":"0x1"}]}]"#,
        ])
        let target = cards(SigningLive.blocks(clear: batch, to: nil, valueHex: nil, dataBytes: 0, context: bnbContext()))[0]
            .rows.first { $0.label == loc.t("componentsUi.signing.interactingLabel") }
        #expect(target?.value == "USDC")
    }

    /// F7: the request is still being read — the sheet says "Loading…". (The
    /// confirm staying shut under it is the core's gate since spec 099 R7:
    /// `ConfirmGateTests`.)
    @Test func theConfirmWaitsForTheReading() {
        let reading = ClearSigningViewWire(
            resolving: true, resolved: false, result: nil, message: nil,
            surface: .loading, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
        let drawn = SigningLive.blocks(clear: reading, to: nil, valueHex: nil, dataBytes: 0, context: bnbContext())
        #expect(drawn.count == 1)
        if case .sentence(let text, _)? = drawn.first {
            #expect(text == loc.t("componentsUi.signing.loading"))
        } else {
            Issue.record("loading is one neutral sentence")
        }
    }

    /// Spec 069: the sheet's fee card carries the send form's speed control,
    /// drawn by the same builder — and a fee left from the speed just walked
    /// away from does not show under the new one's name (issue 681). That it
    /// does not open the confirm either is the core's gate (spec 099 R7,
    /// `ConfirmGateTests`).
    @Test func theFeeCardCarriesTheSpeedControlAndNeverSignsAnotherSpeed() {
        let openGate = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        func estimate(_ tier: String, _ wei: String) -> FeeEstimateWire {
            FeeEstimateWire(
                chainId: 100, totalWei: wei, maxFeePerGas: "2000000000", totalGas: "0", deployed: true,
                quoted: true, feeAsset: .native, feeRecipient: "0xfee", tier: tier,
                effectiveGasPrice: "1000000000", maxGasPrice: "2000000000"
            )
        }
        func speed(_ tier: String, picked: Bool, options: [FeeSpeedOptionWire]) -> SendLive.SpeedInputs {
            SendLive.SpeedInputs(
                view: FeeSpeedViewWire(
                    tier: tier, preferred: "fast", previews: [], open: !options.isEmpty, picked: picked,
                    free: false, freeNote: false, single: false, gasPriceLine: true, options: options
                ),
                feeView: { _ in nil }
            )
        }
        let fast = estimate("fast", "2100000000000000")
        let feeAtFast = FeeViewWire(
            busy: false, failed: nil, fee: fast, stale: false, feeToken: nil, options: [], confirmFeeReady: true
        )
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        func model(_ fee: FeeViewWire, _ speed: SendLive.SpeedInputs) -> SigningModel {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: openGate,
                clear: clear(surface: .none, confirm: .confirmIntent("send")), guard: .empty,
                fee: fee, context: context(), speed: speed
            )
        }
        func feeValue(_ model: SigningModel) -> String? {
            if case .onchain(_, let value, _, _, _) = model.fee { return value }
            return nil
        }

        let open = model(feeAtFast, speed("fast", picked: false, options: [
            FeeSpeedOptionWire(tier: "fast", selected: true, fee: fast, measuring: false, gasPrice: "1 ~ 2 gwei"),
            FeeSpeedOptionWire(tier: "standard", selected: false, fee: nil, measuring: true, gasPrice: nil),
            FeeSpeedOptionWire(tier: "slow", selected: false, fee: estimate("slow", "1000000000000000"),
                               measuring: false, gasPrice: nil),
        ]))
        let control = open.feeSpeed
        #expect(control != nil, "the sheet draws the speed control")
        #expect(control?.label == loc.t("send.feeSpeedLabel"))
        #expect(control?.value == loc.t("send.gasTier.fast"))
        #expect(control?.options.map(\.id) == ["fast", "standard", "slow"])
        // Each option in the words its row would use, minus the row's "~".
        #expect(feeValue(open) == "~" + (control?.options.first?.value ?? ""))
        #expect(control?.options[1].value == "…")
        #expect(control?.options.first?.gasPrice == "1 ~ 2 gwei")

        let picked = model(feeAtFast, speed("slow", picked: true, options: []))
        #expect(feeValue(picked) == loc.t("componentsUi.gas.estimating"))

        let feeAtSlow = FeeViewWire(
            busy: false, failed: nil, fee: estimate("slow", "1000000000000000"), stale: false,
            feeToken: nil, options: [], confirmFeeReady: true
        )
        let landed = model(feeAtSlow, speed("slow", picked: true, options: []))
        #expect(feeValue(landed)?.hasPrefix("~") == true)
    }

    // -- Issue #262: the coin that pays -------------------------------------

    private func option(
        _ symbol: String, contract: String?, decimals: Int, balance: String, amount: String,
        insufficient: Bool, selected: Bool
    ) -> FeeOptionWire {
        FeeOptionWire(
            symbol: symbol, contract: contract, decimals: decimals, balance: balance,
            recipient: "0xrelay", usdBalance: "0", usdPrice: nil, amount: amount,
            insufficient: insufficient, selected: selected
        )
    }

    /// The report's own wallet: 0 ETH and 2 USDT on Ethereum, quoted in ETH.
    @Test func aFeeInACoinTheAccountDoesNotHoldSaysSoAndTheListOffersTheOneItHas() {
        let eth = option("ETH", contract: nil, decimals: 18, balance: "0",
                         amount: "400000000000000", insufficient: true, selected: true)
        let usdt = option("USDT", contract: "0xdac17f958d2ee523a2206206994597c13d831ec7",
                          decimals: 6, balance: "2000000", amount: "1020000",
                          insufficient: false, selected: false)
        let fee = FeeViewWire(
            busy: false, failed: nil,
            fee: FeeEstimateWire(
                chainId: 1, totalWei: "400000000000000", maxFeePerGas: "1", totalGas: "300000",
                deployed: false, quoted: true, feeAsset: .native, feeRecipient: "0xrelay"
            ),
            stale: false, feeToken: nil, options: [eth, usdt], confirmFeeReady: false
        )
        let closed = SigningLive.feeModel(clear: clear(surface: .clearSign), fee: fee, context: context())
        guard case .onchain(_, _, let shut, let warning, _) = closed else {
            Issue.record("a transaction's fee row is on-chain")
            return
        }
        #expect(warning == loc.t("send.warnInsufficientGas", vars: ["sym": "ETH"]))
        #expect(shut == nil, "the list is closed until the row is tapped")

        var ctx = context()
        ctx.feeOpen = true
        guard case .onchain(_, _, let open?, _, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: ctx
        ) else {
            Issue.record("an open list with two coins is drawn")
            return
        }
        #expect(open.title == loc.t("componentsUi.signing.feeTokenTitle"))
        #expect(open.options.map(\.id) == [SigningLive.nativeFeeId, usdt.contract])
        #expect(open.options[0].disabled, "ETH cannot pay: shown, not pickable")
        #expect(!open.options[1].disabled)
        #expect(open.options[1].balance == "2 USDT")
        #expect(open.options[1].fee == "~1.02 USDT")

        // A coin that pays is no warning; nor is a quote still in flight.
        let paid = FeeViewWire(
            busy: false, failed: nil, fee: fee.fee, stale: false, feeToken: nil,
            options: [option("ETH", contract: nil, decimals: 18, balance: "1000000000000000000",
                             amount: "400000000000000", insufficient: false, selected: true)],
            confirmFeeReady: true
        )
        let busy = FeeViewWire(
            busy: true, failed: nil, fee: fee.fee, stale: false, feeToken: nil,
            options: [eth, usdt], confirmFeeReady: false
        )
        for view in [paid, busy] {
            if case .onchain(_, _, _, let none, _) = SigningLive.feeModel(
                clear: clear(surface: .clearSign), fee: view, context: ctx
            ) {
                #expect(none == nil)
            }
        }
    }

    /// Spec 096 F2: the person chose a coin the transaction itself spends —
    /// the PancakeSwap USDC swap, its fee in USDC. The core flags it
    /// (`spent_by_operation`); the sheet says so under the fee while that
    /// coin pays, and says nothing while another coin does.
    @Test func aFeeCoinTheTransactionSpendsIsWarned() {
        let usdcAddress = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"
        var usdc = option("USDC", contract: usdcAddress, decimals: 18,
                          balance: "2341700000000000000", amount: "280000000000000000",
                          insufficient: false, selected: true)
        usdc.spentByOperation = true
        let bnb = option("BNB", contract: nil, decimals: 18, balance: "0",
                         amount: "460000000000000", insufficient: true, selected: false)
        let estimate = FeeEstimateWire(
            chainId: 56, totalWei: "0", maxFeePerGas: "0", totalGas: "450000",
            deployed: true, quoted: true,
            feeAsset: .erc20(token: usdcAddress, decimals: 18, amount: "280000000000000000", symbol: "USDC"),
            feeRecipient: "0xrelay"
        )
        let paying = FeeViewWire(
            busy: false, failed: nil, fee: estimate, stale: false, feeToken: usdcAddress,
            options: [bnb, usdc], confirmFeeReady: true
        )
        guard case .onchain(_, _, _, let warning, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: paying, context: context()
        ) else {
            Issue.record("a transaction's fee row is on-chain")
            return
        }
        #expect(warning == loc.t("componentsUi.gas.feeCoinSpent", vars: ["sym": "USDC"]))
        #expect(warning?.contains("{{") == false)

        // Listed but not paying: nothing to say.
        let listed = FeeOptionWire(
            symbol: usdc.symbol, contract: usdc.contract, decimals: usdc.decimals,
            balance: usdc.balance, recipient: usdc.recipient, usdBalance: usdc.usdBalance,
            usdPrice: usdc.usdPrice, amount: usdc.amount, insufficient: false, selected: false,
            spentByOperation: true
        )
        let other = option("USDT", contract: "0x55d398326f99059ff775485246999027b3197955",
                           decimals: 18, balance: "5000000000000000000",
                           amount: "280000000000000000", insufficient: false, selected: true)
        let inUsdt = FeeViewWire(
            busy: false, failed: nil, fee: estimate, stale: false, feeToken: other.contract,
            options: [bnb, listed, other], confirmFeeReady: true
        )
        if case .onchain(_, _, _, let none, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: inUsdt, context: context()
        ) {
            #expect(none == nil)
        }
    }

    // -- Issue #408: a coin that cannot pay says why --------------------------

    /// The report itself (Android v0.9.6, "Back up public keys" on Ethereum):
    /// 0 ETH and 0.754189 USDT against ~0.001334 ETH or ~3.58361 USDT. Each
    /// greyed coin now says need and have in its own unit — the core's words
    /// for the numbers — and the line under the fee says no coin can pay,
    /// where it used to say "Insufficient ETH for gas fees".
    @Test func issue408EveryGreyedCoinSaysWhyAndTheLineSaysNoCoinCanPay() {
        var eth = option("ETH", contract: nil, decimals: 18, balance: "0",
                         amount: "1333800000000000", insufficient: true, selected: true)
        eth.short = FeeShortfallWire(need: "0.001334 ETH", have: "0 ETH")
        var usdt = option("USDT", contract: "0xdac17f958d2ee523a2206206994597c13d831ec7",
                          decimals: 6, balance: "754189", amount: "3583610",
                          insufficient: true, selected: false)
        usdt.short = FeeShortfallWire(need: "3.58361 USDT", have: "0.754189 USDT")
        var fee = FeeViewWire(
            busy: false, failed: nil,
            fee: FeeEstimateWire(
                chainId: 1, totalWei: "1333800000000000", maxFeePerGas: "0", totalGas: "450000",
                deployed: true, quoted: true, feeAsset: .native, feeRecipient: "0xrelay"
            ),
            stale: false, feeToken: nil, options: [eth, usdt], confirmFeeReady: false
        )
        fee.noCoinPays = true

        guard case .onchain(_, _, _, let closed, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: context()
        ) else {
            Issue.record("a transaction's fee row is on-chain")
            return
        }
        #expect(closed == loc.t("componentsUi.gas.noCoinPays"))
        #expect(closed == "No token can pay this fee")

        var ctx = context()
        ctx.feeOpen = true
        guard case .onchain(_, _, let open?, let warning, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: ctx
        ) else {
            Issue.record("an open list with two coins is drawn")
            return
        }
        #expect(warning == closed, "the line does not change with the list open")
        let everyCoinGreyed = open.options.allSatisfy { $0.disabled }
        #expect(everyCoinGreyed)
        #expect(open.options[0].reason == "Need ~0.001334 ETH, have 0 ETH")
        #expect(open.options[1].reason == "Need ~3.58361 USDT, have 0.754189 USDT")
    }

    /// The coin list draws each coin's real logo — the send form's fee-coin
    /// sheet's own mark, on the REQUEST's chain: ETH on Ethereum wears the
    /// Ethereum logo and no badge; USDC and USDT their own contract's logos
    /// (checksummed path first), so they are no longer both a "U". With the
    /// quote still out there is no estimate to take a chain from, and the
    /// marks still name chain 1, never 0.
    @Test func theCoinListDrawsEachCoinsOwnLogoOnTheRequestsChain() {
        let eth = option("ETH", contract: nil, decimals: 18, balance: "0",
                         amount: "1000", insufficient: false, selected: true)
        let usdc = option("USDC", contract: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                          decimals: 6, balance: "0", amount: "1000", insufficient: false, selected: false)
        let usdt = option("USDT", contract: "0xdac17f958d2ee523a2206206994597c13d831ec7",
                          decimals: 6, balance: "0", amount: "1000", insufficient: false, selected: false)
        // Busy: no estimate, so nothing on the fee says which chain.
        let fee = FeeViewWire(
            busy: true, failed: nil, fee: nil, stale: false, feeToken: nil,
            options: [eth, usdc, usdt], confirmFeeReady: false
        )
        var ctx = context()
        ctx.feeOpen = true
        ctx.chainId = 1
        guard case .onchain(_, _, let list?, _, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: ctx
        ) else {
            Issue.record("an open list with three coins is drawn")
            return
        }
        // Whichever endpoint this host adopted (the built-in one when none
        // is stored), the paths are the rule's.
        #expect(list.options.map(\.mark.ticker) == ["ETH", "USDC", "USDT"])
        #expect(list.options[0].mark.logoURLs == [Marks.chainLogoURL(1)].compactMap { $0 })
        #expect(list.options[0].mark.logoURLs.first?.hasSuffix("/chainlogos/eip155-1.png") == true)
        #expect(list.options[0].mark.badgeHidden, "ETH on Ethereum: the badge would repeat the coin")
        #expect(list.options[1].mark.logoURLs.first?
            .hasSuffix("/assets/eip155-1/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48/logo.png") == true)
        #expect(list.options[2].mark.logoURLs.first?
            .hasSuffix("/assets/eip155-1/0xdAC17F958D2ee523a2206206994597C13D831ec7/logo.png") == true)
        #expect(list.options.allSatisfy { mark in
            !mark.mark.logoURLs.contains { $0.contains("eip155-0") }
        })

        // A hand-built context names no chain: the drawn tickers, no guess.
        ctx.chainId = nil
        guard case .onchain(_, _, let glyphs?, _, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: ctx
        ) else {
            Issue.record("an open list with three coins is drawn")
            return
        }
        #expect(glyphs.options.allSatisfy { $0.mark.logoURLs.isEmpty })
    }

    /// One coin short, one able: only the short one says why, and the line is
    /// still about the coin in force (#262) — picking the other one fixes it.
    @Test func issue408ACoinThatCanPayKeepsTheLineAboutTheCoinInForce() {
        var eth = option("ETH", contract: nil, decimals: 18, balance: "0",
                         amount: "400000000000000", insufficient: true, selected: true)
        eth.short = FeeShortfallWire(need: "0.0004 ETH", have: "0 ETH")
        let usdt = option("USDT", contract: "0xdac17f958d2ee523a2206206994597c13d831ec7",
                          decimals: 6, balance: "2000000", amount: "1020000",
                          insufficient: false, selected: false)
        let fee = FeeViewWire(
            busy: false, failed: nil,
            fee: FeeEstimateWire(
                chainId: 1, totalWei: "400000000000000", maxFeePerGas: "0", totalGas: "300000",
                deployed: true, quoted: true, feeAsset: .native, feeRecipient: "0xrelay"
            ),
            stale: false, feeToken: nil, options: [eth, usdt], confirmFeeReady: false
        )
        var ctx = context()
        ctx.feeOpen = true
        guard case .onchain(_, _, let open?, let warning, _) = SigningLive.feeModel(
            clear: clear(surface: .clearSign), fee: fee, context: ctx
        ) else {
            Issue.record("an open list with two coins is drawn")
            return
        }
        #expect(warning == loc.t("send.warnInsufficientGas", vars: ["sym": "ETH"]))
        #expect(open.options[0].reason == "Need ~0.0004 ETH, have 0 ETH")
        #expect(open.options[1].reason == nil, "USDT can pay: nothing to explain")
    }

    /// The core's words decode as it writes them, and a view from before them
    /// still decodes — the fields are additive, so neither fails a fee view.
    @Test func issue408TheShortfallDecodesAndItsAbsenceIsTolerated() throws {
        let row = """
        {"symbol":"USDT","contract":"0xdac1","decimals":6,"balance":"754189","recipient":"0x3e59",\
        "usd_balance":"0.75","usd_price":"1","amount":"3583610","insufficient":true,"selected":false,\
        "spent_by_operation":false
        """
        let decoder = CoreJSON.decoder
        let now = try decoder.decode(FeeViewWire.self, from: Data("""
        {"busy":false,"failed":null,"fee":null,"stale":false,"fee_token":null,\
        "options":[\(row),"short":{"need":"3.58361 USDT","have":"0.754189 USDT"}}],\
        "confirm_fee_ready":false,"no_coin_pays":true}
        """.utf8))
        #expect(now.noCoinPays)
        #expect(now.options.first?.short == FeeShortfallWire(need: "3.58361 USDT", have: "0.754189 USDT"))
        let before = try decoder.decode(FeeViewWire.self, from: Data("""
        {"busy":false,"failed":null,"fee":null,"stale":false,"fee_token":null,\
        "options":[\(row)}],"confirm_fee_ready":false}
        """.utf8))
        #expect(!before.noCoinPays)
        #expect(before.options.first?.short == nil)
    }

    /// An unlimited approval opens on its own "as requested" chip — kept as
    /// the site asked (2026-09-26) — reads as the danger it is, says so, and
    /// its consent rides into the approve opts.
    @Test func anUnlimitedApprovalIsKeptAsRequestedAndSaid() {
        let editor = GuardEditorViewWire(
            mode: .requested, customText: "", error: nil, choice: .unlimited, displayAmountRaw: nil,
            requestedFinite: false, requestedUnlimited: true, hasBalanceCap: false, revokeOffered: true, balanceRaw: nil
        )
        let guardView = GuardViewWire(
            surface: .approvalEditor,
            detected: GuardDetectedApprovalWire(
                kind: .erc20Approve, tokenAddress: "0xtoken", spender: "0xspender",
                amountRaw: nil, amountBits: 256, isUnbounded: true, isBooleanGrant: false,
                isReducing: false, editable: true, blockReason: nil, deadline: nil,
                locus: .calldataWord(index: 1)
            ),
            meta: GuardTokenMetaViewWire(symbol: "USDC", decimals: 6, verified: true, loading: false),
            editor: editor, confirmAllowed: true, rewrittenParamsJson: nil, unlimitedConsented: true,
            unlimitedWarning: true,
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )
        let blocks = SigningLive.guardBlocks(guardView, loc: loc)
        guard case .allowance(_, let value, let tone, let chips, let note, _, _, _)? = blocks.first else {
            Issue.record("the editor block is missing")
            return
        }
        #expect(tone == .danger)
        #expect(value == loc.t("componentsUi.signingApprove.unlimitedValue"))
        #expect(chips.first { $0.id == "requested" }?.state == .selected)
        #expect(chips.first { $0.id == "custom" }?.state == .idle)
        #expect(note == nil, "no 'unlimited is disabled' note — it is not")
        #expect(blocks.contains { block in
            if case .warning(.danger, let text) = block {
                return text == loc.t("componentsUi.signing.unlimitedWarning")
            }
            return false
        })
        let opts = SigningController.approveOpts(fee: nil, clear: .empty, guard: guardView)
        #expect(opts["unlimited_approved"] as? Bool == true)
        #expect(opts["params_override_json"] is NSNull, "the site's own bytes")
        let untouched = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect(untouched["unlimited_approved"] as? Bool == false)
    }

    /// Once a cap is chosen, the decode's "Unlimited" reads the cap and stops
    /// being the danger; kept as asked, the decode is left alone.
    @Test func aCappedUnlimitedApprovalReadsTheCap() throws {
        let result = try CoreJSON.decode(ClearSignResultWire.self, from: [
            "intent": "Approve", "fields": [
                ["label": "Amount", "value": "Unlimited", "format": "tokenAmount", "warning": true,
                 "unverified": false, "role": "send_amount", "detail": false, "expired": false],
                ["label": "Spender", "value": "0x1111", "format": "addressName", "warning": false,
                 "unverified": false, "role": "spender", "detail": false, "expired": false],
            ],
            "risk": "danger", "verified": false, "provenance": "fetched", "sign_type": "transaction",
            "partial": false, "best_effort": false, "to_own_token": false,
        ])
        var clear = ClearSigningViewWire.empty
        clear.result = result
        let editor = GuardEditorViewWire(
            mode: .balance, customText: "", error: nil, choice: .amount(raw: "250000000"),
            displayAmountRaw: "250000000", requestedFinite: false, requestedUnlimited: true,
            hasBalanceCap: true, revokeOffered: true, balanceRaw: "250000000"
        )
        let capped = GuardViewWire(
            surface: .approvalEditor,
            detected: GuardDetectedApprovalWire(
                kind: .erc20Approve, tokenAddress: "0xtoken", spender: "0x1111",
                amountRaw: nil, amountBits: 256, isUnbounded: true, isBooleanGrant: false,
                isReducing: false, editable: true, blockReason: nil, deadline: nil,
                locus: .calldataWord(index: 1)
            ),
            meta: GuardTokenMetaViewWire(symbol: "USDC", decimals: 6, verified: true, loading: false),
            editor: editor, confirmAllowed: true, rewrittenParamsJson: "[]", unlimitedConsented: false,
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )
        let shown = try #require(SigningLive.cappedApproval(clear, guard: capped).result)
        #expect(shown.fields[0].value == "250 USDC")
        #expect(!shown.fields[0].warning)
        #expect(shown.fields[1].value == "0x1111")
        #expect(shown.risk == .caution, "an approve is caution once its only warning is gone")
        #expect(SigningLive.cappedApproval(clear, guard: .empty).result == result, "untouched without a cap")

        // A bundle decodes from its first leg, so a capped first leg is what
        // that decode reads too.
        let batch = GuardViewWire(
            surface: .batch, detected: nil, meta: GuardViewWire.empty.meta, editor: nil,
            confirmAllowed: true, rewrittenParamsJson: "[]", unlimitedConsented: false,
            increaseTotal: nil, decimalsUnverified: false, expired: false,
            batch: GuardBatchViewWire(
                legs: [GuardLegViewWire(
                    to: "0xtoken", approval: capped.detected, meta: capped.meta, editor: editor,
                    choice: editor.choice, needsEditor: true, needsChoice: false, grantsBroad: false
                )],
                anyUncapped: false, anyToOwnToken: false, allSettled: true
            )
        )
        #expect(SigningLive.cappedApproval(clear, guard: batch).result?.fields[0].value == "250 USDC")
    }

    /// An off-chain permit says plainly that the wallet cannot cap it, and
    /// why. Rewriting one would desync the signature and revert the dApp's own
    /// transaction.
    /// Spec 094 S8: an UNLIMITED off-chain permit (Permit2's, Uniswap's) is
    /// also said in the danger tone, from the core's flag — and still gets no
    /// cap editor, which would only revert the dApp's transaction.
    @Test func anUnlimitedPermitIsWarnedAboutLikeAnUnlimitedApproval() {
        let guardView = GuardViewWire(
            surface: .permitSign,
            detected: GuardDetectedApprovalWire(
                kind: .permit2Single, tokenAddress: "0xtoken", spender: "0xspender",
                amountRaw: nil, amountBits: 160, isUnbounded: true, isBooleanGrant: false,
                isReducing: false, editable: false, blockReason: .offChainPermit,
                deadline: nil, locus: .typedPath("details.amount")
            ),
            meta: GuardViewWire.empty.meta, editor: nil, confirmAllowed: true,
            rewrittenParamsJson: nil, unlimitedConsented: false, unlimitedWarning: true,
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )
        let blocks = SigningLive.guardBlocks(guardView, loc: loc)
        let warnings = blocks.compactMap { block -> String? in
            if case .warning(.danger, let text) = block { return text }
            return nil
        }
        #expect(warnings.contains(loc.t("componentsUi.signing.unlimitedWarning")))
        #expect(warnings.contains(loc.t("componentsUi.signingApprove.permitCantCap")))
        #expect(!blocks.contains { if case .allowance = $0 { return true } else { return false } })
    }

    @Test func anOffChainPermitSaysItCannotBeCapped() {
        let guardView = GuardViewWire(
            surface: .permitSign,
            detected: GuardDetectedApprovalWire(
                kind: .erc2612Permit, tokenAddress: "0xtoken", spender: "0xspender",
                amountRaw: nil, amountBits: 256, isUnbounded: true, isBooleanGrant: false,
                isReducing: false, editable: false, blockReason: .offChainPermit,
                deadline: nil, locus: .typedPath(".message.value")
            ),
            meta: GuardViewWire.empty.meta, editor: nil, confirmAllowed: true,
            rewrittenParamsJson: nil, unlimitedConsented: false, increaseTotal: nil, decimalsUnverified: false,
            expired: false, batch: nil
        )
        let blocks = SigningLive.guardBlocks(guardView, loc: loc)
        #expect(blocks.contains { block in
            if case .warning(.danger, let text) = block {
                return text == loc.t("componentsUi.signingApprove.permitCantCap")
            }
            return false
        })
    }

    /// A signature has no network fee, and the sheet says so instead of
    /// showing a blank row.
    @Test func anOffChainSignatureShowsNoFeeRow() {
        if case .offchain = SigningLive.feeModel(
            clear: clear(surface: .messageSign), fee: nil, context: context()
        ) {} else {
            Issue.record("a message signature must not draw a network-fee row")
        }
    }

    /// The dApp's identity on the sheet is its **host**, twice. A name the
    /// page supplies is a claim.
    @Test func theSheetLeadsWithTheHostAndNotWithAName() {
        let model = SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc),
            request: SigningController.Incoming(
                id: "r", method: "eth_sendTransaction",
                paramsJson: #"[{"to":"0xabc","value":"0x1"}]"#,
                origin: "https://app.uniswap.org", transportId: "t", chainId: 100
            ),
            sign: .empty, clear: clear(surface: .none, resolved: false),
            guard: .empty, fee: nil, context: context()
        )
        #expect(model.dapp.host == "app.uniswap.org")
        #expect(model.dapp.name == "app.uniswap.org")
        #expect(model.network.name == "Gnosis")
        #expect(model.signer.seed == "0x88cca0eedbf2c4426110bbfc998f048689266894")
    }
}
