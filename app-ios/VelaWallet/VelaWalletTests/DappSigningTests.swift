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

    @Test func aTransactionRecordCarriesTheCallAndClipsALongRequest() {
        let long = String(repeating: "a", count: 9000)
        let row = SignExecutor.recordRow([
            "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e","value":"0x38d7ea4c68000","data":"0x\#(long)"}]"#,
            "result": "", "from": "0xme", "chain_id": 100, "now_ms": 1_757_000_000_000,
            "status": "pending", "user_op_hash": "0xcf9f", "dapp_origin": "https://x.test",
        ], nativeSymbol: "xDAI")
        #expect(row["type"] as? String == "dapp_tx")
        #expect(row["to"] as? String == "0x76875e")
        #expect(row["symbol"] as? String == "xDAI")
        #expect(row["requestTruncated"] as? Bool == true)
        #expect((row["signedRequest"] as? String)?.count == 4096)
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
            confirmAllowed: true, rewrittenParamsJson: rewritten, increaseTotal: nil,
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

        // A batch leads with its FIRST leg.
        let batch = SigningController.clearKickoff(
            method: "wallet_sendCalls",
            paramsJson: #"[{"calls":[{"to":"0xfirst","data":"0x01"},{"to":"0xsecond"}]}]"#,
            chainId: 100, origin: nil
        )
        #expect(batch?["to"] as? String == "0xfirst")

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

    /// Typed data is the **second** parameter.
    @Test func typedDataIsTheSecondParameter() {
        #expect(SigningController.typedDataOf(
            paramsJson: #"["0xme","{\"primaryType\":\"Permit\"}"]"#
        ) == #"{"primaryType":"Permit"}"#)
        // Some dApps send the object rather than a string.
        #expect(SigningController.typedDataOf(
            paramsJson: #"["0xme",{"primaryType":"Permit"}]"#
        ).contains("Permit"))
        #expect(SigningController.typedDataOf(paramsJson: "[]").isEmpty)
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
            gasFeeToken: nil, quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: golden)
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
            confirmAllowed: true, rewrittenParamsJson: capped, increaseTotal: nil,
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
            pool: RpcPool(store: store, accounts: AccountStore()),
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
        #expect(!controller.guardView.confirmAllowed, "an unlimited approval must hold the gate shut")

        controller.guardPreset("revoke")

        #expect(controller.guardView.editor?.choice == .revoke,
                "the chip's event never reached the guard through the controller")
        #expect(controller.guardView.confirmAllowed)
        #expect(controller.guardView.rewrittenParamsJson != nil,
                "a capped approval must produce params to sign")
    }
}

// MARK: - The sheet

@MainActor
struct SigningLiveTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func context() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894",
            origin: "https://x.test"
        )
    }

    private func clear(
        surface: ClearSurface, resolved: Bool = true,
        result: ClearSignResultWire? = nil, confirm: ClearConfirmWire = .confirm
    ) -> ClearSigningViewWire {
        ClearSigningViewWire(
            resolving: false, resolved: resolved, result: result, message: nil,
            surface: surface, confirm: confirm, blindTyped: nil, dangerHaptic: false
        )
    }

    /// **A plain native transfer is not a contract interaction.**
    ///
    /// The core resolves it without a descriptor — resolved, and no result —
    /// and the blind card's "cannot decode (0 bytes)" read a dust send as an
    /// unknown contract call on Android's first pass.
    @Test func aNoCalldataTransferIsDrawnAsASendRatherThanAsABlindCall() {
        let blocks = SigningLive.blocks(
            clear: clear(surface: .none),
            to: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            valueHex: "0x38d7ea4c68000", dataBytes: 0, context: context()
        )
        let intents = blocks.compactMap { block -> String? in
            if case .intent(let text, _) = block { return text }
            return nil
        }
        #expect(intents.count == 1)
        #expect(intents.first != loc.t("componentsUi.signing.intentContractCall"))
        #expect(blocks.contains { if case .amount = $0 { true } else { false } },
                "the amount is the first thing a person needs to see")
        #expect(blocks.contains { if case .party = $0 { true } else { false } })
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

    /// **The slide's verb is a corpus string, never a raw intent id.**
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

    /// The slide is three machines ANDed — and an **off-chain** signature has
    /// no fee to be ready about.
    @Test func theSlideOpensOnlyWhenAllThreeMachinesAgree() {
        let openGate = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        let readyFee = FeeViewWire(
            busy: false, failed: nil, fee: nil, stale: false, feeToken: nil,
            options: [], confirmFeeReady: true
        )
        let unreadyFee = FeeViewWire(
            busy: true, failed: nil, fee: nil, stale: false, feeToken: nil,
            options: [], confirmFeeReady: false
        )
        let blockingGuard = GuardViewWire(
            surface: .approvalEditor, detected: nil, meta: GuardViewWire.empty.meta,
            editor: nil, confirmAllowed: false, rewrittenParamsJson: nil,
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )

        #expect(SigningLive.confirmEnabled(
            sign: openGate, guard: .empty, fee: readyFee, clear: clear(surface: .clearSign)
        ))
        #expect(!SigningLive.confirmEnabled(
            sign: openGate, guard: blockingGuard, fee: readyFee, clear: clear(surface: .clearSign)
        ), "an unlimited approval with no cap chosen must hold the slide shut")
        #expect(!SigningLive.confirmEnabled(
            sign: openGate, guard: .empty, fee: unreadyFee, clear: clear(surface: .clearSign)
        ))
        // No fee, no waiting for one.
        #expect(SigningLive.confirmEnabled(
            sign: openGate, guard: .empty, fee: unreadyFee, clear: clear(surface: .messageSign)
        ), "a personal_sign has no network fee and must not wait for a quote")
    }

    /// Spec 069: the sheet's fee card carries the send form's speed control,
    /// drawn by the same builder — and a fee left from the speed just walked
    /// away from neither shows under the new one's name nor opens the slide,
    /// though the core's own gate is still open on it (issue 681).
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
        #expect(open.confirm?.enabled == true)

        let picked = model(feeAtFast, speed("slow", picked: true, options: []))
        #expect(feeValue(picked) == loc.t("componentsUi.gas.estimating"))
        #expect(picked.confirm?.enabled == false, "the slide never signs the speed walked away from")

        let feeAtSlow = FeeViewWire(
            busy: false, failed: nil, fee: estimate("slow", "1000000000000000"), stale: false,
            feeToken: nil, options: [], confirmFeeReady: true
        )
        let landed = model(feeAtSlow, speed("slow", picked: true, options: []))
        #expect(feeValue(landed)?.hasPrefix("~") == true)
        #expect(landed.confirm?.enabled == true)
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

    /// The "as requested" chip is **disabled** for an unlimited approval, not
    /// merely unselected — and the cap value reads as the danger it is.
    @Test func anUnlimitedApprovalOffersNoAsRequestedChip() {
        let editor = GuardEditorViewWire(
            mode: nil, customText: "", error: nil, choice: nil, displayAmountRaw: nil,
            requestedFinite: false, hasBalanceCap: false, balanceRaw: nil
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
            editor: editor, confirmAllowed: false, rewrittenParamsJson: nil,
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )
        let blocks = SigningLive.guardBlocks(guardView, loc: loc)
        guard case .allowance(_, let value, let tone, let chips, _, _, _)? = blocks.first else {
            Issue.record("the editor block is missing")
            return
        }
        #expect(tone == .danger)
        #expect(value == loc.t("componentsUi.signingApprove.unlimitedValue"))
        #expect(chips.first { $0.id == "requested" }?.state == .disabled)
        #expect(chips.first { $0.id == "custom" }?.state == .idle)
        #expect(blocks.contains { if case .warning(.danger, _) = $0 { true } else { false } })
    }

    /// An off-chain permit says plainly that the wallet cannot cap it, and
    /// why. Rewriting one would desync the signature and revert the dApp's own
    /// transaction.
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
            rewrittenParamsJson: nil, increaseTotal: nil, decimalsUnverified: false,
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
