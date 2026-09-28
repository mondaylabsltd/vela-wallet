//
//  SigningReceiptTests.swift
//  VelaWalletTests
//
//  Spec 079 US1: after the approval the signing sheet is a receipt (the
//  send's own words), once the core has answered the page the ending stays on
//  screen, the ✕ is the one close and answers the page once, the tracker's
//  clock keeps ticking past the wait window, and a receipt poll never outlives
//  what is left of the wait. The Android twin is `SigningReceiptTest.kt`.
//  Hermetic: no network, no ceremony, no device.
//
//  Spec 082 (T105): the words come from the core's `phase`, the ending from
//  its `signEndingOf` / `signEndingState` — a reverted dApp transaction never
//  reads 已确认, and a lost reply reads "may have been sent", never "try
//  again".
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SigningReceiptTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let op = "0xace642c724834405c56919fb198d6a6389cc2867ff3f3e3585b59d1921fe681d"
    private let tx = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f"

    private func context(track: TrackEntryWire? = nil, explorer: String? = nil) -> SigningLive.Context {
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
        context.typicalS = 5
        context.track = track
        context.explorerBase = explorer
        return context
    }

    private let blocks: [SigningBlock] = [
        .intent(text: "Send", tone: .neutral),
        .amount(line: AmountLine(sign: "−", value: "0.001", symbol: "XDAI")),
    ]

    private func sign(
        signing: Bool = false, submitting: Bool = false, op: String? = nil,
        error: SignErrorKind? = nil, kind: SignMethodKind? = nil,
        phase: SignPhaseWire = .idle, maybeSent: Bool = false
    ) -> SignViewWire {
        SignViewWire(
            surface: .sheet,
            request: kind.map {
                SignRequestViewWire(
                    id: "r1", method: $0 == .personalSign ? "personal_sign" : "eth_sendTransaction",
                    kind: $0, paramsJson: "[]", origin: "http://127.0.0.1:8137", dapp: nil,
                    chainId: 100, signerAddress: nil
                )
            },
            isSigning: signing, isSubmitting: submitting, pendingOpHash: op,
            error: error.map { SignErrorNoticeWire(kind: $0, detail: nil) },
            funding: nil, confirmGateOpen: true, reconcilePending: false, swipeAction: .reject,
            trackerHandoff: nil, notice: nil, globalChainId: 100, blocked: nil,
            phase: phase, pendingOpMaybeSent: maybeSent
        )
    }

    private func entry(_ status: String, _ outcome: String, txHash: String? = nil) -> TrackEntryWire {
        TrackEntryWire(
            userOpHash: op, chainId: 100, recordIds: ["rec-1"], status: status, txHash: txHash,
            polling: outcome != "final" && outcome != "unknown", submittedAtMs: 1, outcome: outcome
        )
    }

    // MARK: - The receipt while the sheet is up

    @Test func aRequestNotYetApprovedHasNoReceipt() {
        #expect(SigningLive.receipt(sign: sign(), blocks: blocks, context: context()) == nil)
    }

    @Test func thePasskeyTheSubmissionAndTheWaitAreEachNamedInTheSendsWords() {
        let signing = SigningLive.receipt(
            sign: sign(signing: true, phase: .awaitingSignature), blocks: blocks, context: context()
        )
        #expect(signing?.stage == .submitting)
        #expect(signing?.title == loc.t("send.txSigning"))

        let submitting = SigningLive.receipt(
            sign: sign(submitting: true, phase: .submitting), blocks: blocks, context: context()
        )
        #expect(submitting?.title == loc.t("send.txSubmitting"))
        #expect(submitting?.captions.contains(loc.t("send.txBackgroundHint")) == true)
        #expect(submitting?.cta == loc.t("send.txCloseBackground"))

        let waiting = SigningLive.receipt(
            sign: sign(op: op), blocks: blocks, context: context(track: entry("pending", "landing"))
        )
        #expect(waiting?.stage == .submitted)
        #expect(waiting?.title == loc.t("send.txSubmittedTitle"))
        #expect(waiting?.captions.first == "Send · −0.001 XDAI")
        #expect(waiting?.eta?.typicalS == 5, "the chain's clock drives the ring")
        #expect(waiting?.eta?.submittedAtMs == 1, "counted from the tracker's own hand-over time")
    }

    @Test func pastTheWaitWindowTheWordsSayItIsStillOnItsWayNeverFailed() {
        let still = SigningLive.receipt(
            sign: sign(op: op), blocks: blocks,
            context: context(track: entry("accepted_not_landed", "still_confirming"))
        )
        #expect(still?.stage == .submitted)
        #expect(still?.captions.contains(loc.t("componentsUi.signing.stillConfirming")) == true)
        #expect(still?.eta == nil, "no countdown for an op past its window")
    }

    /// G22: the network work before the prompt is "preparing" — never
    /// "awaiting your biometric" for forty seconds of precheck (RA9). The
    /// words follow the core's phase, not `is_signing`, which is true from
    /// the approval on.
    @Test func theWordsAreThePhasesNotTheSigningFlag() {
        let preparing = SigningLive.receipt(
            sign: sign(signing: true, submitting: true, phase: .preparing), blocks: blocks, context: context()
        )
        #expect(preparing?.title == loc.t("send.txPreparing"))
        #expect(preparing?.title != loc.t("send.txSigning"))
        let awaiting = SigningLive.receipt(
            sign: sign(signing: true, submitting: true, phase: .awaitingSignature), blocks: blocks,
            context: context()
        )
        #expect(awaiting?.title == loc.t("send.txSigning"))
        // A message's prompt is the signing sentence, not the send's.
        let message = SigningLive.receipt(
            sign: sign(signing: true, kind: .personalSign, phase: .awaitingSignature),
            blocks: blocks, context: context()
        )
        #expect(message?.title == loc.t("componentsUi.signing.signing"))
        // Idle is no receipt, whatever the flags say.
        #expect(SigningLive.receipt(sign: sign(phase: .idle), blocks: blocks, context: context()) == nil)
        // And the phase this build does not know reads as preparing.
        let unknown = try? CoreJSON.decoder.decode(SignPhaseWire.self, from: Data(#""thinking""#.utf8))
        #expect(unknown == .preparing)
    }

    /// The Trusted Signer's card speaks while its page holds the signature —
    /// the core's `awaiting_signature` — and not through the preparing work.
    @Test func theSignerCardIsUpOnlyWhileTheSignatureIsAwaited() {
        var ctx = context()
        ctx.trustedSignerRoute = true
        #expect(ctx.signerPageOpen(sign(signing: true, phase: .awaitingSignature)))
        #expect(!ctx.signerPageOpen(sign(signing: true, phase: .preparing)))
    }

    @Test func aMessageIsSigningThenSignedNeverSubmittedToANetwork() {
        let submitting = SigningLive.receipt(
            sign: sign(submitting: true, kind: .personalSign, phase: .submitting), blocks: blocks, context: context()
        )
        #expect(submitting?.title == loc.t("componentsUi.signing.signing"))
        #expect(submitting?.captions.contains(loc.t("send.txBackgroundHint")) == false)
    }

    @Test func aSubmissionThatFailedIsAFailedReceiptWithTheSheetsOwnSentence() {
        let failed = SigningLive.receipt(sign: sign(error: .submitFailed), blocks: blocks, context: context())
        #expect(failed?.stage == .failed)
        #expect(failed?.captions.contains(loc.t("send.txErrorGeneric")) == true)
        // The person's own no is not a failure to show.
        #expect(SigningLive.receipt(sign: sign(error: .userRejected), blocks: blocks, context: context()) == nil)
    }

    /// The sheet's model carries the ✕'s label, and a live approval's
    /// receipt — never one for a request the wallet refused outright.
    @Test func theSheetCarriesItsCloseAndItsReceipt() {
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        let model = SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request,
            sign: sign(submitting: true, phase: .submitting),
            clear: .empty, guard: .empty, fee: nil, context: context()
        )
        #expect(model.closeLabel == loc.t("onboarding.common.close"))
        #expect(model.receipt?.title == loc.t("send.txSubmitting"))
    }

    // MARK: - May have been sent (spec 082 RA10)

    /// A lost reply on the live sheet: the submitting title, the caption, the
    /// op hash to look it up by, a close that leaves it running — and no
    /// Retry, nowhere, ever.
    @Test func aLostReplyReadsMayHaveBeenSentWithTheOpHashAndNoRetry() {
        let receipt = SigningLive.receipt(
            sign: sign(op: op, phase: .submitting, maybeSent: true), blocks: blocks, context: context()
        )
        #expect(receipt?.stage == .submitted)
        #expect(receipt?.title == loc.t("send.txSubmitting"))
        #expect(receipt?.captions.contains(loc.t("componentsUi.signing.maybeSent")) == true)
        #expect(receipt?.hash?.copyValue == op)
        #expect(receipt?.cta == loc.t("send.txCloseBackground"))
        #expect(receipt?.captions.contains(loc.t("send.txErrorGeneric")) == false)
        // Once the relay has shown it holds the op, the ordinary words.
        let acknowledged = SigningLive.receipt(
            sign: sign(op: op, phase: .submitting, maybeSent: true), blocks: blocks,
            context: context(track: entry("pending", "landing"))
        )
        #expect(acknowledged?.title == loc.t("send.txSubmittedTitle"))
    }

    /// RI3's reader test: the new sentence resolves in zh and en and never
    /// echoes its own key.
    @Test func theMaybeSentSentenceResolvesInZhAndEn() {
        let key = "componentsUi.signing.maybeSent"
        for tag in ["zh", "en"] {
            let text = Loc(overrideTag: tag, preferredLanguages: []).t(key)
            #expect(!text.isEmpty)
            #expect(text != key, "\(tag) echoed the key")
            #expect(!text.contains("maybeSent"))
        }
        #expect(Loc(overrideTag: "zh", preferredLanguages: []).t(key) != loc.t(key))
    }

    // MARK: - The ending, after the core closed the sheet

    @Test func anAnswerBecomesTheEndingTheAftercareShows() {
        #expect(SigningAftercare.of(
            method: "personal_sign", chainId: 100, payload: ["type": "ok", "result": "0xsig"], submittedUserOp: nil
        )?.ending == .signed)
        #expect(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100, payload: ["type": "ok", "result": tx], submittedUserOp: op
        )?.ending == .landed(txHash: tx, userOpHash: op))
        // The wait ran out: the page got the operation hash.
        let still = SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "ok", "result": op.uppercased().replacingOccurrences(of: "0X", with: "0x")],
            submittedUserOp: op
        )
        #expect(still?.ending == .stillConfirming(userOpHash: op))
        #expect(still?.chainId == 100)
        #expect(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "err", "code": 4001, "kind": "user_rejected"], submittedUserOp: op
        ) == nil, "a refusal has no ending to show")
        let signed = SigningAftercare.of(
            method: "personal_sign", chainId: 1, payload: ["type": "ok", "result": "0xsig"], submittedUserOp: nil
        )
        let landed = SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 1, payload: ["type": "ok", "result": tx], submittedUserOp: op
        )
        #expect((signed?.tickSeconds ?? 0) < (landed?.tickSeconds ?? 0))
    }

    private func landed() -> SigningAftercare {
        SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100, payload: ["type": "ok", "result": tx], submittedUserOp: op
        )!
    }

    private func stillConfirming() -> SigningAftercare {
        SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100, payload: ["type": "ok", "result": op], submittedUserOp: op
        )!
    }

    /// Every `SignEndingState` the core can name, as the core names it.
    @Test func everyEndingStateIsTheCores() {
        #expect(landed().state(track: nil) == .following(userOpHash: op, outcome: "landing", feeHeld: false))
        #expect(landed().state(track: entry("confirmed", "final", txHash: tx)) == .confirmed(txHash: tx))
        #expect(landed().state(track: entry("dropped", "final", txHash: tx)) == .reverted(txHash: tx))
        #expect(stillConfirming().state(track: entry("not_sent", "final")) == .notSent)
        #expect(stillConfirming().state(track: entry("rejected", "final")) == .notSent)
        #expect(stillConfirming().state(track: entry("pending", "maybe_sent"))
            == .following(userOpHash: op, outcome: "maybe_sent", feeHeld: false))
        #expect(stillConfirming().state(track: entry("fee_held", "landing"))
            == .following(userOpHash: op, outcome: "landing", feeHeld: true))
        let signed = SigningAftercare.of(
            method: "personal_sign", chainId: 100, payload: ["type": "ok", "result": "0xsig"], submittedUserOp: nil
        )
        #expect(signed?.state(track: nil) == .signed)
    }

    /// W3: a dApp transaction that landed and reverted is a failure with its
    /// hash and the explorer — never 已确认, in any language.
    @Test func aRevertedDappTransactionNeverReadsConfirmed() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        for language in [loc, zh] {
            var ctx = SigningLive.Context(
                loc: language, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
                walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
            )
            ctx.track = entry("dropped", "final", txHash: tx)
            ctx.explorerBase = "https://gnosisscan.io"
            let reverted = SigningLive.aftercareReceipt(landed(), summary: nil, context: ctx)
            #expect(reverted.stage == .failed)
            #expect(reverted.title == language.t("componentsTx.receipt.statusFailed"))
            #expect(reverted.title != language.t("componentsTx.receipt.statusConfirmed"))
            #expect(reverted.captions.contains(language.t("componentsTx.receipt.failedHint")))
            #expect(!reverted.captions.contains(language.t("send.txErrorGeneric")))
            #expect(reverted.hash?.copyValue == tx)
            #expect(reverted.viewOnExplorer == language.t("history.viewOnExplorer"))
        }
        // And before the tracker has seen it, a landing is on its way — not a tick.
        let early = SigningLive.aftercareReceipt(landed(), summary: nil, context: context())
        #expect(early.stage == .submitted)
    }

    @Test func theAftercareTicksOnlyForWhatTheTrackerConfirmed() {
        let confirmed = SigningLive.aftercareReceipt(
            landed(), summary: "Send · −0.001 XDAI",
            context: context(track: entry("confirmed", "final", txHash: tx), explorer: "https://gnosisscan.io")
        )
        #expect(confirmed.stage == .confirmed)
        #expect(confirmed.hash?.copyValue == tx, "the copy is the whole hash")
        #expect(confirmed.hash?.value == "\(tx.prefix(10))…\(tx.suffix(8))", "the line is its short form")
        #expect(confirmed.viewOnExplorer == loc.t("history.viewOnExplorer"))
        #expect(SigningLive.aftercareReceipt(
            landed(), summary: nil, context: context(track: entry("confirmed", "final", txHash: tx))
        ).viewOnExplorer == nil, "no explorer, no link")

        let waiting = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("accepted_not_landed", "still_confirming"))
        )
        #expect(waiting.stage == .submitted)
        #expect(waiting.captions.contains(loc.t("componentsUi.signing.stillConfirming")))
        #expect(waiting.cta == loc.t("send.txCloseBackground"))

        let landedLater = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("confirmed", "final", txHash: tx))
        )
        #expect(landedLater.stage == .confirmed)
        #expect(landedLater.hash?.copyValue == tx)

        let unknown = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("accepted_not_landed", "unknown"))
        )
        #expect(unknown.captions.contains(loc.t("componentsUi.signing.unknownOutcome")))
        #expect(unknown.stage == .submitted, "24 hours is not a failure either")

        let maybe = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("pending", "maybe_sent"))
        )
        #expect(maybe.stage == .submitted)
        #expect(maybe.captions.contains(loc.t("componentsUi.signing.maybeSent")))
        #expect(maybe.hash?.copyValue == op)

        let notSent = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("not_sent", "final"))
        )
        #expect(notSent.stage == .failed)
        #expect(notSent.captions.contains(loc.t("send.txErrorGeneric")))

        let held = SigningLive.aftercareReceipt(
            stillConfirming(), summary: nil, context: context(track: entry("fee_held", "landing"))
        )
        #expect(held.captions.contains(loc.t("send.txHeldFees")))

        let signed = SigningLive.aftercareReceipt(
            SigningAftercare.of(
                method: "personal_sign", chainId: 100, payload: ["type": "ok", "result": "0xsig"], submittedUserOp: nil
            )!, summary: nil, context: context()
        )
        #expect(signed.title == loc.t("clearSigning.alertSignedTitle"))
        #expect(signed.stage == .confirmed)
    }

    @Test func theSummaryIsTheIntentAndItsFigure() {
        #expect(SigningLive.summaryOf(blocks) == "Send · −0.001 XDAI")
        #expect(SigningLive.summaryOf([
            .intent(text: "Swap", tone: .neutral),
            .swap(pay: AmountLine(sign: "", value: "1", symbol: "USDC"),
                  receive: AmountLine(sign: "", value: "0.9", symbol: "XDAI")),
        ]) == "Swap · 1 USDC → 0.9 XDAI")
        #expect(SigningLive.summaryOf([.sentence(text: "x", tone: .neutral)]) == nil)
    }

    // MARK: - The tracker's clock (T023a)

    /// An op past its wait window is `accepted_not_landed` — not `pending` —
    /// and still followed: the clock must keep running for it.
    @Test func theClockRunsWhileAnythingIsFollowedNotOnlyWhilePending() {
        let past = TrackViewWire(entries: [entry("accepted_not_landed", "still_confirming")])
        #expect(!past.hasPending)
        #expect(past.isFollowing, "an op past its window is still asked about")
        #expect(past.entry(userOpHash: op.uppercased().replacingOccurrences(of: "0X", with: "0x")) != nil)

        let done = TrackViewWire(entries: [entry("confirmed", "final", txHash: tx), entry("accepted_not_landed", "unknown")])
        #expect(!done.isFollowing, "a final or abandoned op stops the clock")
    }

    // MARK: - The receipt wait (T023b)

    @Test func aPollThatAnswersInTimeIsTheAnswer() async {
        let answer = await SignExecutor.within(seconds: 60) { "0xabc" }
        #expect(answer == "0xabc")
    }

    /// A poll that never answers — the relay black-holed — gives way at the
    /// deadline, and the poll itself is cancelled.
    @Test func aPollThatHangsGivesWayAtTheDeadline() async {
        let cancelled = Flag()
        let answer: String? = await SignExecutor.within(seconds: 0.05) {
            while !Task.isCancelled { try? await Task.sleep(nanoseconds: 5_000_000) }
            cancelled.set()
            return "late"
        }
        #expect(answer == nil)
        await Wait.until { cancelled.isSet }
        #expect(cancelled.isSet, "the hanging poll was cancelled, not left running")
    }
}

// MARK: - The ✕ (T021, T023)

@MainActor
struct SigningCloseTests {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// The ✕ before the approval is the refusal: the page gets exactly one
    /// 4001, however many times it is tapped.
    @Test func theCloseBeforeTheApprovalAnswersThePageOnce() async {
        let suite = "vela.tests.signing.close.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        var answers: [[String: Any]] = []
        let controller = SigningController(
            wallet: (address: golden, credentialId: "cred-1"),
            relay: relay,
            accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore()),
            ports: SigningController.Ports(
                respond: { _, _, payload, _ in answers.append(payload) },
                knownChains: { [100] }
            )
        )
        controller.open(SigningController.Incoming(
            id: "req-x", method: "personal_sign",
            paramsJson: #"["0x48656c6c6f","\#(golden)"]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        ))
        #expect(controller.sign.isVisible)
        #expect(!controller.closedByPerson)

        controller.swipeDismissed()
        await Wait.until { !answers.isEmpty }
        controller.swipeDismissed()

        #expect(answers.count == 1)
        #expect(answers.first?["type"] as? String == "err")
        #expect((answers.first?["code"] as? NSNumber)?.intValue == 4001)
        #expect(controller.closedByPerson)
        #expect(!controller.sign.isVisible, "the sheet is gone")
    }
}
