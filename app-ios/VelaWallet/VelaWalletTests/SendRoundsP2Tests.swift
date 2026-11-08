//
//  SendRoundsP2Tests.swift
//  VelaWalletTests
//
//  Native parity Phase 2, P2.2: the send flow's rounds 3–6, as the web's
//  `flows/live-send.ts` words them — the held line's order (another copy
//  sending first, the slow reads, the previous op, the fee, the risk check),
//  the confirm's notices (nonce used, the network's failure by name, the
//  signing page's), Max moved, the passkey note, the recipient's caution and
//  its "Check again", the frozen fee, "Send again", and the lock's "Add this
//  network". Each branch from the core's own view, patched; the events go to
//  the REAL send machine so a misspelled one fails here.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendRoundsP2Tests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    private let xdai: [String: Any] = [
        "symbol": "xDAI", "network": "Gnosis", "chain_id": 100,
        "token_address": NSNull(), "decimals": 18, "balance": "10",
        "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    private func view(_ patch: [String: Any]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        object["selected_token"] = xdai
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private struct Missing: Error {}

    private func confirm(_ view: SendViewWire, loc: Loc? = nil, fee: FeeViewWire? = nil) throws -> SendConfirmModel {
        let loc = loc ?? en
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else { throw Missing() }
        return SendLive.confirm(
            view, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawn, loc: loc, fee: fee
        )
    }

    // MARK: - The held line, in the web's order

    @Test func theHeldLineSaysWhatTheConfirmWaitsFor() throws {
        let queued = try view(["queued_behind_tab": true, "submit_slow": ["seconds": 7, "busy": false]])
        #expect(SendLive.heldNote(queued, fee: nil, loc: en) == en.t("send.queuedBehindTab"))

        let slow = try view(["submit_slow": ["seconds": 7, "busy": false]])
        #expect(SendLive.heldNote(slow, fee: nil, loc: en)
                == "Waiting for Gnosis to answer — 7 s so far. Nothing is signed yet.")
        let busy = try view(["submit_slow": ["seconds": 12, "busy": true]])
        #expect(SendLive.heldNote(busy, fee: nil, loc: en)
                == "Gnosis's nodes are busy — still trying (12 s). Nothing is signed yet.")
        #expect(SendLive.heldNote(busy, fee: nil, loc: zh)?.contains("Gnosis") == true)

        let checking = try view(["risk_checking": true])
        #expect(SendLive.heldNote(checking, fee: nil, loc: en) == "Checking this address…")

        // A submit that did not go is the notice; the line says nothing beside it.
        let failed = try view(["tx_status": "error", "tx_error": "chain_unreachable",
                               "submit_slow": ["seconds": 7, "busy": false]])
        #expect(SendLive.heldNote(failed, fee: nil, loc: en) == nil)
    }

    // MARK: - The confirm's notices

    @Test func aSpentNonceIsNotSentCalmlyWithReviewAgain() throws {
        let model = try confirm(try view(["tx_status": "error", "tx_error": "nonce_used"]))
        #expect(model.noticeTitle == en.t("componentsTx.receipt.statusNotSent"))
        #expect(model.notice == en.t("componentsUi.signing.wentFirst"))
        #expect(model.noticeAction == en.t("send.txReviewAgain"))
    }

    /// §35: an operation that expired before it was submitted is drawn as a
    /// spent nonce is — "Not sent", calmly (the notice's title, so no
    /// failure styling), its own sentence, and Review again.
    @Test func anExpiredOperationIsNotSentCalmlyWithReviewAgain() throws {
        let model = try confirm(try view(["tx_status": "error", "tx_error": "expired"]))
        #expect(model.noticeTitle == en.t("componentsTx.receipt.statusNotSent"))
        #expect(model.notice == en.t("componentsUi.signing.expired"))
        #expect(model.noticeAction == en.t("send.txReviewAgain"))
        #expect(try confirm(try view(["tx_status": "error", "tx_error": "expired"]), loc: zh).notice
                == zh.t("componentsUi.signing.expired"))
    }

    @Test func theNetworksFailureIsSaidByName() throws {
        let busy = try confirm(try view(["tx_status": "error", "tx_error": "chain_busy"]))
        #expect(busy.notice == "Gnosis's nodes are busy and didn't answer in time, so nothing was signed or sent. Try again in a moment.")
        #expect(busy.noticeAction == en.t("send.txRetryBtn"))
        let down = try confirm(try view(["tx_status": "error", "tx_error": "chain_unreachable"]))
        #expect(down.notice == "Couldn't reach Gnosis, so nothing was signed or sent. Try again in a moment.")
    }

    /// Round 7: the signing page's failures are said as what happened there;
    /// only the page's own refusal offers no retry.
    @Test func theSigningPagesFailuresAreTheirOwnWords() throws {
        let cases: [(String, String, Bool)] = [
            ("page_unanswered", "componentsUi.signing.trustedSignerUnanswered", true),
            ("page_mismatch", "componentsUi.signing.trustedSignerMismatch", true),
            ("page_not_opened", "componentsUi.signing.integrity.couldNotCheck", true),
            ("page_refused", "componentsUi.signing.trustedSignerRefused", false),
        ]
        for (error, key, retries) in cases {
            let model = try confirm(try view(["tx_status": "error", "tx_error": error]))
            #expect(model.notice == en.t(key), "\(error)")
            #expect((model.noticeAction != nil) == retries, "\(error)")
        }
    }

    @Test func thePasskeyNoteIsCalm() throws {
        let closed = try confirm(try view(["passkey_note": "cancelled"]))
        #expect(closed.passkeyNote == en.t("componentsUi.signing.passkeyClosed"))
        let timedOut = try confirm(try view(["passkey_note": "timed_out"]))
        #expect(timedOut.passkeyNote == en.t("componentsUi.signing.passkeyTimedOut"))
        #expect(try confirm(try view([:])).passkeyNote == nil)
    }

    @Test func maxMovedIsSaidUnderTheAmount() throws {
        let model = try confirm(try view(["max_moved": ["from": "0.156387", "to": "0.156382"]]))
        #expect(model.maxNote?.hasPrefix("Max is now 0.156382 xDAI (it was 0.156387)") == true)
        #expect(try confirm(try view([:])).maxNote == nil)
    }

    // MARK: - The recipient

    @Test func anAddressThatIsProbablyNotTheOneMeantIsCautioned() throws {
        #expect(try confirm(try view(["recipient_checksum_mismatch": true])).recipientTag
                == en.t("send.recipientChecksum"))
        #expect(try confirm(try view(["recipient_warning": "zero_address"])).recipientTag
                == en.t("send.recipientZeroAddress"))
        #expect(try confirm(try view(["recipient_warning": "own_address"])).recipientTag
                == en.t("send.recipientSelf"))
        // The token's own contract still comes first (spec 096 F12).
        #expect(try confirm(try view(["recipient_is_token_contract": true, "recipient_warning": "zero_address"])).recipientTag
                == en.t("send.recipientTokenContract"))
    }

    @Test func anUnfinishedCheckSaysSoWithCheckAgain() throws {
        let unfinished = try confirm(try view(["risk_unfinished": true]))
        #expect(unfinished.recipientCheck == RecipientCheckModel(
            text: en.t("send.riskUnfinished"), retry: "Check again"
        ))
        let unread = try confirm(try view(["risk_unfinished": true, "risk_history_unread": true]))
        #expect(unread.recipientCheck?.text == en.t("send.riskHistoryUnread"))
        #expect(try confirm(try view([:])).recipientCheck == nil)
    }

    // MARK: - Send again

    @Test func aReceiptOfNothingSentOffersTheWayOn() throws {
        guard case .sendReceipt(let drawn) = WalletFlowFixtures.build(.sd4a, loc: en).base else { throw Missing() }
        func receipt(_ refusal: Any) throws -> SendReceiptModel {
            let wire = try view(["receipt": [
                "status": "not_sent", "hold_reason": NSNull(), "refusal_key": refusal,
                "can_send_again": true, "still_unknown": false, "kind": NSNull(), "transfers": [],
                "coins": [], "amount": "1", "usd_value": 1.0, "submitted_at_ms": NSNull(),
                "typical_inclusion_s": NSNull(),
            ]])
            return SendLive.receipt(wire, display: .usd, on: drawn, loc: en)
        }
        #expect(try receipt(NSNull()).retry == en.t("send.txSendAgain"))
        #expect(try receipt("componentsUi.signing.wentFirst").retry == en.t("send.txReviewAgain"))
        #expect(try receipt("componentsUi.signing.expired").retry == en.t("send.txReviewAgain"), "§35")
        let landed = try view([:])
        #expect(SendLive.receipt(landed, display: .usd, on: drawn, loc: en).retry == nil)
    }

    // MARK: - The lock's "Add this network"

    @Test func theLockOffersToAddTheNetwork() throws {
        let locked = try view(["lock_error": ["type": "network", "chain_id": 7_777_777]])
        let notice = SendLive.lockNotice(locked, loc: en)
        #expect(notice?.action == en.t("send.lock.addNetwork"))
        #expect(notice?.actionChainId == 7_777_777)
        #expect(notice?.busy == false)
        let adding = try view(["lock_error": ["type": "network", "chain_id": 7_777_777], "adding_network": true])
        #expect(SendLive.lockNotice(adding, loc: en)?.busy == true)
        // A token lock has no network to add.
        #expect(SendLive.lockNotice(try view(["lock_error": ["type": "token"]]), loc: en)?.action == nil)
    }

    // MARK: - The events reach the real machine

    /// Every round 3–6 event this shell sends is one the core reads — a
    /// misspelled field is refused by serde and changes nothing, so each is
    /// sent through the store's own spelling and must not be refused.
    @Test func everyNewEventIsOneTheCoreReads() throws {
        let core = SendCore()
        let events: [[String: Any]] = [
            ["type": "submit_slow", "elapsed_ms": 7_000.0, "busy": false],
            ["type": "chain_answered", "chain_id": 100],
            ["type": "send_again"],
            ["type": "retry_risk_check"],
            ["type": "add_network_tapped", "chain_id": 7_777_777],
        ]
        for event in events {
            #expect(throws: Never.self) { _ = try core.dispatch(eventJson: CoreJSON.string(event)) }
        }
    }

    /// The executor's answer to `add_network`: whatever the networks machine
    /// said, as the send machine's `SendAddNetworkOutcome`; unwired, an add
    /// that did not happen.
    @Test func theAddNetworkAnswerIsTheNetworksMachines() {
        #expect(SendExecutor.addNetworkAnswer().contains(#""type":"error""#))
    }

    // MARK: - The page's ending

    @Test func aTrustedPageEndingIsTheCoresOutcome() {
        #expect(TrustedSignerNotice.closed.signOutcome["type"] as? String == "page_declined")
        #expect(TrustedSignerNotice.closed.sendFailure["type"] as? String == "passkey_cancelled")
        let cases: [(TrustedSignerNotice, String)] = [
            (.timeout, "unanswered"), (.mismatch, "mismatch"), (.refused, "refused"), (.unavailable, "not_opened"),
        ]
        for (notice, failure) in cases {
            #expect(notice.signOutcome["type"] as? String == "page_failed")
            #expect(notice.signOutcome["failure"] as? String == failure)
            #expect(notice.sendFailure["failure"] as? String == failure)
        }
        #expect(SignErrorKind.pageUnanswered.signerReasonKey == "componentsUi.signing.trustedSignerUnanswered")
        #expect(SignErrorKind.pageNotOpened.signerReasonKey == "componentsUi.signing.integrity.couldNotCheck")
    }
}
