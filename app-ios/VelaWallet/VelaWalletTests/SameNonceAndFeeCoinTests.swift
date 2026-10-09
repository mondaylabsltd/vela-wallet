//
//  SameNonceAndFeeCoinTests.swift
//  VelaWalletTests
//
//  PR 2 §3 and §4, where this shell has logic of its own.
//
//  §3 — one transaction in flight per account and network:
//    - the tracker's `in_flight_ops` is computed from the core's OWN view
//      JSON (the re-encoded `TrackEntryWire` once dropped `sender` and
//      `stalled`: with them gone the hold is empty, or never times out);
//    - the stored record's `from` is the tracker's `sender` after a restart;
//    - forwarded, it holds the dApp sheet's confirm with the one line, on
//      that account and network only — and never a signature;
//    - the relay's nonce refusal is `previous_pending`, not `other`;
//    - a refusal is told by its reason, on Send and on the sheet.
//  §4 — a figure switched to another coin is measured again before it can be
//    confirmed: the shell decodes `provisional` and keeps the measuring sign.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SameNonceAndFeeCoinTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let op = "0x" + String(repeating: "a1", count: 32)
    private let dapp = "https://app.example"

    private func store() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: "vela.tests.nonce.\(UUID().uuidString)")!)
    }

    // MARK: - §3 the hold

    /// After a restart the tracker knows who signed a pending op (the stored
    /// row's `from`), and `in_flight_ops` over the core's own view names it.
    @Test func theTrackerNamesTheOpHoldingTheNonceFromItsOwnView() async throws {
        let store = store()
        let row = SignExecutor.recordRow([
            "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xbb","value":"0x1"}]"#, "result": "", "from": golden,
            "user_op_hash": op, "chain_id": 100,
            "now_ms": Date().timeIntervalSince1970 * 1000 - 20_000, "status": "pending",
        ], nativeSymbol: "XDAI")
        #expect(TrackerExecutor.pendingWire(row)?["sender"] as? String == golden)
        TxRecords.writeRecords([row], store: store)

        let tracker = TrackerStore(executor: TrackerExecutor(
            store: store, relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        ))
        tracker.boot()
        await Wait.until { tracker.view?.entry(userOpHash: op) != nil }
        let entry = try #require(tracker.view?.entry(userOpHash: op))
        #expect(entry.sender == golden.lowercased())

        let ops = try #require(
            (try JSONSerialization.jsonObject(with: Data(tracker.inFlightOpsJson.utf8))) as? [[String: Any]]
        )
        #expect(ops.count == 1, "\(tracker.inFlightOpsJson)")
        #expect(ops.first?["sender"] as? String == golden.lowercased())
        #expect((ops.first?["chain_id"] as? NSNumber)?.intValue == 100)
        #expect((ops.first?["user_op_hash"] as? String)?.lowercased() == op)
    }

    /// The core's `stalled` is honoured only when the view it reads still
    /// carries it — which is why the tracker forwards the core's own JSON.
    @Test func aStalledOpHoldsNothing() {
        func view(stalled: Bool) -> String {
            var entry: [String: Any] = [
                "user_op_hash": op, "chain_id": 100, "record_ids": ["r"], "status": "pending",
                "tx_hash": NSNull(), "polling": true, "submitted_at_ms": 1, "outcome": "landing",
                "sender": golden.lowercased(),
            ]
            if stalled { entry["stalled"] = true }
            return CoreJSON.string(["entries": [entry]])
        }
        #expect(inFlightOps(trackViewJson: view(stalled: false)).contains(golden.lowercased()))
        #expect(inFlightOps(trackViewJson: view(stalled: true)) == "[]")
        // The shell's own entry keeps the two fields when it is re-encoded.
        let decoded = try? CoreJSON.decode(TrackViewWire.self, from: try CoreJSON.object(view(stalled: true)))
        let reencoded = decoded?.entries.first?.coreJSON ?? ""
        #expect(reencoded.contains("\"stalled\":true") && reencoded.contains("\"sender\""))
    }

    private func controller(_ port: ScriptedRelayPort) -> SigningController {
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let store = store()
        return SigningController(
            wallet: (address: golden, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore(), offline: true),
            ports: SigningController.Ports(knownChains: { [100, 137] })
        )
    }

    /// The tracker's `in_flight_ops` as it forwards it: a JSON array.
    private func inFlight(chainId: Int) -> String {
        let ops: [[String: Any]] = [[
            "sender": golden.lowercased(), "chain_id": chainId,
            "user_op_hash": "0x" + String(repeating: "b2", count: 32),
        ]]
        return String(decoding: (try? JSONSerialization.data(withJSONObject: ops)) ?? Data(), as: UTF8.self)
    }

    /// A dApp transaction while this account's last one on the network is in
    /// flight: the confirm holds with the one line, ahead of any fee block —
    /// and lets go when the tracker says so. Another network does not hold.
    @Test func aDappTransactionWaitsForTheLastOneOnItsNetwork() async throws {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080")
        let controller = controller(port)
        controller.open(SigningController.Incoming(
            id: "t1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            origin: dapp, transportId: "tab-1", chainId: 100, grantedAddress: golden
        ))
        controller.inFlightChanged(inFlight(chainId: 100))
        await Wait.until { controller.confirmState.block == "previous_pending" }
        let held = controller.confirmState
        #expect(!held.enabled)
        #expect(held.block == "previous_pending")
        #expect(held.key == "componentsUi.signing.confirmBlock.previousPending")
        #expect(loc.t(held.key ?? "") == "Waiting for your last transaction on this network…")

        // Final (or stalled): the tracker's list drops it, and the hold goes.
        controller.inFlightChanged("[]")
        await Wait.until { controller.confirmState.block != "previous_pending" }
        #expect(controller.confirmState.block != "previous_pending")

        // An op on another network holds nothing here.
        controller.inFlightChanged(inFlight(chainId: 137))
        try? await Task.sleep(nanoseconds: 100_000_000)
        #expect(controller.confirmState.block != "previous_pending")
        controller.swipeDismissed()
    }

    /// A signature never waits for a transaction.
    @Test func aSignatureNeverWaits() async {
        let controller = controller(ScriptedRelayPort())
        controller.open(SigningController.Incoming(
            id: "m1", method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(golden)"]"#,
            origin: dapp, transportId: "tab-1", chainId: 100, grantedAddress: golden
        ))
        controller.inFlightChanged(inFlight(chainId: 100))
        await Wait.until { controller.confirmState.enabled }
        #expect(controller.confirmState.enabled)
        controller.swipeDismissed()
    }

    // MARK: - §3 refusals

    /// The relay's nonce refusal (its `nonce_in_flight`, or an older relay's
    /// `[existingHash:…]` read as held) is its own failure: Send says the
    /// previous transaction is pending, with Try again.
    @Test func theRelaysNonceRefusalIsPreviousPendingOnSend() throws {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "confirm"
        object["tx_error"] = "previous_pending"
        let view = try CoreJSON.decode(SendViewWire.self, from: object)
        #expect(SendLive.confirmNotice(view, loc: loc)
                == loc.t("componentsUi.signing.confirmBlock.previousPending"))
        object["tx_error"] = NSNull()
        object["previous_pending"] = [
            "chain_id": 100, "user_op_hash": op, "key": "componentsUi.signing.confirmBlock.previousPending",
        ]
        let holding = try CoreJSON.decode(SendViewWire.self, from: object)
        #expect(holding.previousPending?.key == "componentsUi.signing.confirmBlock.previousPending")
    }

    /// The relay's `rejection_reason` reaches the core's status result —
    /// verbatim, the core words it.
    @Test func theRelaysReasonReachesTheTracker() async throws {
        let port = ScriptedRelayPort()
        port.rpc[userOpStatusMethod()] = .ok([
            "status": "rejected", "rejection_reason": "nonce_used", "last_executor_stage": "rejected",
        ] as [String: Any])
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let reply = try CoreJSON.object(await TrackerExecutor(store: store(), relay: relay).perform([
            "type": "poll_status", "user_op_hash": op, "chain_id": 100,
        ]))
        #expect(reply["type"] as? String == "status")
        #expect(reply["rejection_reason"] as? String == "nonce_used")
        // An older relay says none: null, and the core reads the stage.
        port.rpc[userOpStatusMethod()] = .ok(["status": "rejected"] as [String: Any])
        let old = try CoreJSON.object(await TrackerExecutor(store: store(), relay: relay).perform([
            "type": "poll_status", "user_op_hash": op, "chain_id": 100,
        ]))
        #expect(old["rejection_reason"] is NSNull)
        #expect(RelayClient.rejectionReason(#"{"result":{"status":"failed","rejection_reason":"fee_below_market"}}"#)
                == "fee_below_market")
    }

    /// The sheet's ending names the reason the tracker's entry carries.
    @Test func theSheetsRefusalIsToldByItsReason() throws {
        let aftercare = try #require(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "ok", "result": op], submittedUserOp: op
        ))
        func entry(_ key: String?) -> TrackEntryWire {
            var entry = TrackEntryWire(
                userOpHash: op, chainId: 100, recordIds: ["r"], status: "rejected", txHash: nil,
                polling: false, submittedAtMs: 1, outcome: "final"
            )
            entry.refusal = key == nil ? nil : "nonce_used"
            entry.refusalKey = key
            return entry
        }
        func captions(_ track: TrackEntryWire) -> [String] {
            var context = SigningLive.Context(
                loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
                walletName: "Me", walletAddress: golden
            )
            context.track = track
            return SigningLive.aftercareReceipt(aftercare, summary: nil, context: context).captions
        }
        #expect(aftercare.state(track: entry("componentsUi.signing.wentFirst")) == .refused)
        #expect(captions(entry("componentsUi.signing.wentFirst")) == [loc.t("componentsUi.signing.wentFirst")])
        #expect(captions(entry(nil)) == [loc.t("componentsUi.signing.refused")])
    }

    /// Send's receipt draws the refusal's key in place of the hold's words.
    @Test func theSendReceiptIsToldByItsReason() throws {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "receipt"
        object["user_op_hash"] = op
        object["tx_status"] = "submitted"
        object["selected_token"] = [
            "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        object["receipt"] = [
            "status": "failed", "hold_reason": NSNull(), "kind": NSNull(), "transfers": [],
            "amount": "0.001", "usd_value": 0.001, "typical_inclusion_s": 5, "submitted_at_ms": NSNull(),
            "refusal_key": "componentsUi.signing.wentFirst",
        ] as [String: Any]
        let view = try CoreJSON.decode(SendViewWire.self, from: object)
        guard case .sendReceipt(let drawn) = WalletFlowFixtures.build(.sd4b, loc: loc).base else {
            Issue.record("SD4b does not draw the receipt")
            return
        }
        let receipt = SendLive.receipt(view, display: .usd, on: drawn, loc: loc)
        #expect(receipt.stage == .failed)
        #expect(receipt.captions == [loc.t("componentsUi.signing.wentFirst")])
        #expect(!receipt.captions.contains(loc.t("send.txRejectedFees")), "never the blanket fee sentence")
    }

    // MARK: - §4 a switched fee coin

    /// While the figure is provisional the confirm holds and the row keeps
    /// its measuring sign — the switched figure, never a confirmable one.
    @Test func aSwitchedCoinIsMeasuredAgainBeforeItCanBeConfirmed() throws {
        let view = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": true, "failed": NSNull(), "fee": NSNull(), "stale": false, "fee_token": "0xusdc",
            "options": [], "confirm_fee_ready": false, "provisional": true,
        ])
        #expect(view.provisional)
        #expect(!view.confirmFeeReady)
        let clear = ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil,
            surface: .clearSign, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
        #expect(SigningLive.feeRefresh(clear: clear, fee: view, loc: loc)?.refreshing == true)
        // Absent on the wire: not provisional.
        let plain = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": false, "failed": NSNull(), "fee": NSNull(), "stale": false, "fee_token": NSNull(),
            "options": [], "confirm_fee_ready": false,
        ])
        #expect(!plain.provisional)
    }

    /// Issue #483 on Send: a fee that failed says why under its row, in the
    /// fee machine's words — the chain by name for a chain read, Vela's own
    /// fault for an internal one — in the line the row already keeps.
    @Test func theSendFormSaysWhyThereIsNoFee() throws {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("SD2 does not draw the form")
            return
        }
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "enter_details"
        object["selected_token"] = [
            "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        let view = try CoreJSON.decode(SendViewWire.self, from: object)
        func fee(_ failed: Any) throws -> FeeViewWire {
            try CoreJSON.decode(FeeViewWire.self, from: [
                "busy": false, "failed": failed, "fee": NSNull(), "stale": false, "fee_token": NSNull(),
                "options": [], "confirm_fee_ready": false,
            ])
        }
        let speed = try CoreJSON.decoder.decode(FeeSpeedViewWire.self, from: Data(
            #"{"tier":"standard","preferred":"standard","previews":[],"open":false,"picked":false,"free":false,"free_note":false,"single":false,"gas_price_line":false,"options":[]}"#.utf8
        ))
        func note(_ failed: Any) throws -> String? {
            let feeView = try fee(failed)
            return SendLive.form(
                view, fee: feeView, display: .usd, on: drawn, loc: loc,
                speed: SendLive.SpeedInputs(view: speed, feeView: { _ in feeView })
            ).fee.failNote
        }
        #expect(try note(["chain_read": ["rate_limited": false]])
                == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(try note("internal") == loc.t("componentsUi.gas.reasonInternal"))
        #expect(try note("internal")?.contains("Gnosis") == false)
        #expect(try note(NSNull()) == nil)
    }

    /// The relay's published minimum reaches the core verbatim (§5).
    @Test func theRelaysMinimumIsPassedVerbatim() {
        #expect(RelayClient.minimumAmount("0x2386f26fc10000") == "0x2386f26fc10000")
        #expect(RelayClient.minimumAmount("10000") == "10000")
        #expect(RelayClient.minimumAmount(NSNumber(value: 10_000)) == "10000")
        #expect(RelayClient.minimumAmount(nil) == nil)
        #expect(RelayClient.minimumAmount("") == nil)
    }
}
