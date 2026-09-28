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
        error: SignErrorKind? = nil, kind: SignMethodKind? = nil
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
            trackerHandoff: nil, notice: nil, globalChainId: 100, blocked: nil
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
        let signing = SigningLive.receipt(sign: sign(signing: true), blocks: blocks, context: context())
        #expect(signing?.stage == .submitting)
        #expect(signing?.title == loc.t("send.txSigning"))

        let submitting = SigningLive.receipt(sign: sign(submitting: true), blocks: blocks, context: context())
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

    @Test func aMessageIsSigningThenSignedNeverSubmittedToANetwork() {
        let submitting = SigningLive.receipt(
            sign: sign(submitting: true, kind: .personalSign), blocks: blocks, context: context()
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
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign(submitting: true),
            clear: .empty, guard: .empty, fee: nil, context: context()
        )
        #expect(model.closeLabel == loc.t("onboarding.common.close"))
        #expect(model.receipt?.title == loc.t("send.txSubmitting"))
    }

    // MARK: - The ending, after the core closed the sheet

    @Test func anAnswerBecomesTheEndingTheAftercareShows() {
        #expect(SigningAftercare.of(
            method: "personal_sign", chainId: 100, payload: ["type": "ok", "result": "0xsig"], submittedUserOp: nil
        ) == .signed(chainId: 100))
        #expect(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100, payload: ["type": "ok", "result": tx], submittedUserOp: nil
        ) == .landed(chainId: 100, txHash: tx))
        // The wait ran out: the page got the operation hash.
        #expect(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "ok", "result": op.uppercased().replacingOccurrences(of: "0X", with: "0x")],
            submittedUserOp: op
        ) == .stillConfirming(chainId: 100, userOpHash: op))
        #expect(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "err", "code": 4001, "kind": "user_rejected"], submittedUserOp: op
        ) == nil, "a refusal has no ending to show")
        #expect(SigningAftercare.signed(chainId: 1).tickSeconds < SigningAftercare.landed(chainId: 1, txHash: tx).tickSeconds)
    }

    @Test func theAftercareTicksForALandingAndFollowsTheTrackerForAnOpStillOnItsWay() {
        let landed = SigningLive.aftercareReceipt(
            .landed(chainId: 100, txHash: tx), summary: "Send · −0.001 XDAI",
            context: context(explorer: "https://gnosisscan.io")
        )
        #expect(landed.stage == .confirmed)
        #expect(landed.hash?.copyValue == tx, "the copy is the whole hash")
        #expect(landed.hash?.value == "\(tx.prefix(10))…\(tx.suffix(8))", "the line is its short form")
        #expect(landed.viewOnExplorer == loc.t("history.viewOnExplorer"))
        #expect(SigningLive.aftercareReceipt(.landed(chainId: 100, txHash: tx), summary: nil, context: context())
            .viewOnExplorer == nil, "no explorer, no link")

        let still = SigningAftercare.stillConfirming(chainId: 100, userOpHash: op)
        let waiting = SigningLive.aftercareReceipt(
            still, summary: nil, context: context(track: entry("accepted_not_landed", "still_confirming"))
        )
        #expect(waiting.stage == .submitted)
        #expect(waiting.captions.contains(loc.t("componentsUi.signing.stillConfirming")))
        #expect(waiting.cta == loc.t("send.txCloseBackground"))

        let landedLater = SigningLive.aftercareReceipt(
            still, summary: nil, context: context(track: entry("confirmed", "final", txHash: tx))
        )
        #expect(landedLater.stage == .confirmed)
        #expect(landedLater.hash?.copyValue == tx)

        let unknown = SigningLive.aftercareReceipt(
            still, summary: nil, context: context(track: entry("accepted_not_landed", "unknown"))
        )
        #expect(unknown.captions.contains(loc.t("componentsUi.signing.unknownOutcome")))
        #expect(unknown.stage == .submitted, "24 hours is not a failure either")

        let dropped = SigningLive.aftercareReceipt(
            still, summary: nil, context: context(track: entry("dropped", "final"))
        )
        #expect(dropped.stage == .failed)

        let signed = SigningLive.aftercareReceipt(.signed(chainId: 100), summary: nil, context: context())
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
