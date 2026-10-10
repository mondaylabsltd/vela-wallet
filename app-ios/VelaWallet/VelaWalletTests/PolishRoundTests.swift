//
//  PolishRoundTests.swift
//  VelaWalletTests
//
//  PR 2's polish round — what the core now says and this shell draws:
//
//  - a held nonce at submit is "Not sent yet", calmly — the signing sheet's
//    `failure_not_sent` and Send's `tx_error` `previous_pending` — never
//    "Failed" in the failure's colour, the held line said once;
//  - a failed fee row does exactly its `failure.tap` — asks again ("Tap to
//    retry"), opens the coins ("Pay with another coin") or, with no coin
//    left, nothing — on the Send form, its confirm and the signing sheet;
//  - a failure that answered another chain's question is dropped on the
//    Send form, the confirm and the bridge to the send machine;
//  - a balance that read nothing has no figure (`display_total_usd` null).
//
//  Hermetic: the fee and balance views come from the real cores
//  (`FeeCoreScene`, `BalanceCoreScene`). The registry's three outcomes and
//  Tempo's not-read rule are `RegistryDocumentTests`.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct PolishRoundTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"

    private func feeView(_ json: String) throws -> FeeViewWire {
        try CoreJSON.decoder.decode(FeeViewWire.self, from: Data(json.utf8))
    }

    /// A send of 25 USDC on Base (8453) — or of xDAI on Gnosis (100).
    private func sendView(stage: String, chainId: Int = 8453, patch: [String: Any] = [:]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = stage
        object["selected_token"] = chainId == 8453
            ? [
                "network": "base", "chain_id": 8453, "symbol": "USDC", "balance": "250",
                "decimals": 6, "token_address": usdc, "price_usd": 1.0, "logo_urls": [], "spam": false,
            ] as [String: Any]
            : [
                "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
                "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "logo_urls": [], "spam": false,
            ] as [String: Any]
        object["recipient"] = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
        object["amount"] = "1"
        object["token_amount"] = "1"
        object["confirm_amount"] = "1"
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private var speed: SendLive.SpeedInputs? {
        HandoffFeeFixtures.speedView.map { SendLive.SpeedInputs(view: $0, feeView: { _ in nil }) }
    }

    private func form(_ view: SendViewWire, fee: FeeViewWire?) throws -> FeeRowModel {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            throw Missing.drawing
        }
        return SendLive.form(view, fee: fee, display: .usd, on: drawn, loc: loc, speed: speed).fee
    }

    private func confirm(_ view: SendViewWire, fee: FeeViewWire?) throws -> SendConfirmModel {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            throw Missing.drawing
        }
        return SendLive.confirm(view, from: (golden, nil), display: .usd, on: drawn, loc: loc, fee: fee)
    }

    private enum Missing: Error { case drawing }

    private func context(feeOpen: Bool = false) -> SigningLive.Context {
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: .blue, nativeSymbol: "ETH",
            walletName: "Me", walletAddress: golden.lowercased()
        )
        context.chainId = 8453
        context.feeOpen = feeOpen
        return context
    }

    private let clearSign = ClearSigningViewWire(
        resolving: false, resolved: true, result: nil, message: nil,
        surface: .clearSign, confirm: .confirm, blindTyped: nil, dangerHaptic: false
    )

    private func row(_ model: FeeModel) -> (value: String, tappable: Bool, listed: Int?)? {
        if case .onchain(_, let value, let selector, _, let tappable) = model {
            return (value, tappable, selector?.options.count)
        }
        return nil
    }

    // MARK: - The wire

    /// The new fields decode; absent, they read as an older core meant.
    @Test func theNewFieldsDecodeAndDefault() throws {
        let failure: [String: Any] = [
            "failure": "would_fail", "reason_key": NSNull(), "auto_retry": false, "retrying": false,
            "figure_key": "componentsUi.gas.payWithAnotherCoin",
            "footer_key": "componentsUi.signing.confirmBlock.feeWouldFail",
            "tap": "choose_coin", "chain_id": 8453, "fee_token": usdc,
        ]
        let said = try CoreJSON.decode(FeeFailureViewWire.self, from: failure)
        #expect(said.tap == .chooseCoin)
        #expect(said.chainId == 8453)
        #expect(said.feeToken == usdc)
        var older = failure
        older["tap"] = nil
        older["chain_id"] = nil
        older["fee_token"] = nil
        let old = try CoreJSON.decode(FeeFailureViewWire.self, from: older)
        #expect(old.tap == .retry, "a reader that predates it reads retry")
        #expect(old.chainId == nil && old.feeToken == nil)
        #expect(old.isFor(chain: 1), "a failure built without a run is taken as it is")
        var unknown = failure
        unknown["tap"] = "dance"
        #expect(try CoreJSON.decode(FeeFailureViewWire.self, from: unknown).tap == .nothing,
                "a tap this build does not know promises nothing")

        // The sheet: `failure_not_sent`, from the real core's view — and absent.
        var sign = try CoreJSON.object(SignRequestCore().view())
        #expect(sign["failure_not_sent"] as? Bool == false, "the core always says it")
        sign["failure_not_sent"] = true
        #expect(try CoreJSON.decode(SignViewWire.self, from: sign).notSent)
        sign["failure_not_sent"] = nil
        #expect(try !CoreJSON.decode(SignViewWire.self, from: sign).notSent)
    }

    // MARK: - Item 1: "Not sent yet"

    /// The sheet: its own calm state — the title, the core's sentence, the
    /// still clock on the sunken disc, Try again — never "Failed".
    @Test func theSheetSaysNotSentYetCalmly() throws {
        var sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: SignErrorNoticeWire(kind: .submitFailed, detail: nil), funding: nil,
            confirmGateOpen: true, reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
        sign.failureNotSent = true
        sign.failureRetryable = true
        sign.failureRefusalKey = I18nKeys.CoreRound.notSentBody
        let summary: [SigningBlock] = [
            .intent(text: "Send", tone: .neutral),
            .amount(line: AmountLine(sign: "−", value: "0.001", symbol: "ETH")),
        ]
        let receipt = try #require(SigningLive.receipt(sign: sign, blocks: summary, context: context()))
        #expect(receipt.stage == .notSent)
        #expect(receipt.title == loc.t("componentsUi.signing.notSentTitle"))
        #expect(receipt.title != loc.t("componentsTx.receipt.statusFailed"))
        #expect(receipt.captions.last == loc.t("componentsUi.signing.notSentBody"))
        #expect(receipt.retry == loc.t("send.txRetryBtn"))
        #expect(StatusHeroView.markGlyph(.notSent) == .clock, "a wait, never the failure's mark")

        // Its line on the form is a calm sentence, never the danger card.
        let blocks = SigningLive.statusBlocks(sign: sign, loc: loc)
        let danger = blocks.contains {
            if case .warning(.danger, _) = $0 { return true }
            return false
        }
        #expect(!danger, "\(blocks.map(\.id))")
        let calm = blocks.contains {
            if case .sentence(let text, .neutral) = $0 { return text == loc.t("componentsUi.signing.notSentBody") }
            return false
        }
        #expect(calm)

        // A refusal that is not this one is still "Failed".
        sign.failureNotSent = false
        sign.failureRefusalKey = "componentsUi.signing.refused"
        sign.failureRefused = true
        #expect(SigningLive.receipt(sign: sign, blocks: summary, context: context())?.stage == .failed)
    }

    /// Send's confirm: "Not sent yet" over the sentence, calm, Try again —
    /// and the previous transaction's held line is not said a second time.
    @Test func theSendConfirmSaysNotSentYetOnce() throws {
        let view = try sendView(stage: "confirm", chainId: 100, patch: [
            "tx_status": "error", "tx_error": "previous_pending",
        ])
        let page = try confirm(view, fee: nil)
        #expect(page.noticeTitle == loc.t("componentsUi.signing.notSentTitle"))
        #expect(page.notice == loc.t("componentsUi.signing.notSentBody"))
        #expect(page.notice != loc.t("componentsUi.signing.confirmBlock.previousPending"))
        #expect(page.noticeAction == loc.t("send.txRetryBtn"))
        #expect(page.heldNote == nil)

        // The previous transaction tracked too: the notice says it; the line
        // under the button does not repeat it.
        let holding = try sendView(stage: "confirm", chainId: 100, patch: [
            "tx_status": "error", "tx_error": "previous_pending",
            "previous_pending": [
                "chain_id": 100, "user_op_hash": "0x" + String(repeating: "c3", count: 32),
                "key": "componentsUi.signing.confirmBlock.previousPending",
            ],
        ])
        #expect(SendLive.heldNote(holding, fee: nil, loc: loc) == nil, "said once")
        // Held, with nothing refused: the line under the button, no title.
        let held = try sendView(stage: "confirm", chainId: 100, patch: [
            "previous_pending": [
                "chain_id": 100, "user_op_hash": "0x" + String(repeating: "c3", count: 32),
                "key": "componentsUi.signing.confirmBlock.previousPending",
            ],
        ])
        let heldPage = try confirm(held, fee: nil)
        #expect(heldPage.heldNote == loc.t("componentsUi.signing.confirmBlock.previousPending"))
        #expect(heldPage.noticeTitle == nil)
        // Any other submit failure keeps its one line and no title.
        let generic = try confirm(
            try sendView(stage: "confirm", patch: ["tx_status": "error", "tx_error": "generic"]), fee: nil
        )
        #expect(generic.noticeTitle == nil)
        #expect(generic.notice == loc.t("send.txErrorGeneric"))
    }

    // MARK: - Item 3/4: the row does exactly its `tap`

    /// The real core after the relay answered "would fail": a tap opens the
    /// coins while another is on offer, does nothing when none is.
    @Test func theCoreSaysWhatATapOnAWouldFailRowDoes() throws {
        let coin = try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: true)))
        let failure = try #require(coin.failure, "the core said no failure: \(coin)")
        #expect(failure.failure == "would_fail")
        #expect(failure.tap == .chooseCoin)
        #expect(failure.figureKey == "componentsUi.gas.payWithAnotherCoin")
        #expect(failure.footerKey == "componentsUi.signing.confirmBlock.feeWouldFail")
        #expect(failure.chainId == 8453)
        #expect(failure.feeToken?.lowercased() == usdc)

        let none = try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: false)))
        #expect(none.failure?.tap == FeeFailureTapWire.nothing)
        #expect(none.failure?.figureKey == nil)
        #expect(none.failure?.footerKey == "componentsUi.signing.confirmBlock.feeWouldFail")
    }

    /// The Send form's row: the figure and the tap from `failure.tap`.
    @Test func theSendFormsRowDoesWhatItSays() throws {
        let view = try sendView(stage: "enter_details")
        let coin = try form(view, fee: try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: true))))
        #expect(coin.value == loc.t("componentsUi.gas.payWithAnotherCoin"))
        #expect(coin.tap == .chooseCoin, "a tap opens the coins")
        #expect(!coin.tapRetries)

        let none = try form(view, fee: try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: false))))
        #expect(none.value == "—")
        #expect(none.tap == .nothing, "no control with nothing left to try")

        let tapOnly = try form(view, fee: try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 8453)).failed))
        #expect(tapOnly.value == loc.t("componentsUi.gas.estimateFailed"))
        #expect(tapOnly.tap == .retry)

        #expect(try form(view, fee: nil).tap == .open, "no failure: the row opens the coins")
    }

    /// The Send form has no held confirm to carry the failure's line, so a
    /// fee that would fail says it on the row's own line — "This would fail
    /// if sent as it is." — with or without a coin left; the confirm and the
    /// sheet keep saying it under their held button, never twice.
    @Test func aFeeThatWouldFailSaysSoOnTheSendForm() throws {
        let line = loc.t("componentsUi.signing.confirmBlock.feeWouldFail")
        #expect(line == "This would fail if sent as it is.")
        let form = try sendView(stage: "enter_details")
        for anotherCoin in [true, false] {
            let fee = try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: anotherCoin)))
            #expect(try self.form(form, fee: fee).failNote == line, "anotherCoin: \(anotherCoin)")
            // The confirm: its held line says it; the fee row's note does not.
            let page = try confirm(try sendView(stage: "confirm"), fee: fee)
            #expect(page.heldNote == line)
            #expect(page.facts.allSatisfy { $0.note != line }, "said twice on the confirm")
        }
        // Every other failure keeps its own reason (or none) — never this line.
        let down = try feeView(try #require(FeeCoreScene.chainDown.views(chainId: 8453)).failed)
        #expect(try self.form(form, fee: down).failNote
                == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Base"]))
        let tap = try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 8453)).failed)
        #expect(try self.form(form, fee: tap).failNote == nil)
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(zh.t("componentsUi.signing.confirmBlock.feeWouldFail") == "按现在的方式发送会失败。")
    }

    /// The confirm: the fee line does what it says, and the footer agrees —
    /// "Tap it to retry" only over a line a tap retries.
    @Test func theConfirmsFeeLineDoesWhatItsFooterSays() throws {
        let view = try sendView(stage: "confirm")
        func feeFact(_ page: SendConfirmModel) -> FactRowModel? { page.facts.first { $0.feeTap != nil } }

        let tap = try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 8453)).failed)
        let tapPage = try confirm(view, fee: tap)
        let tapFact = try #require(feeFact(tapPage))
        #expect(tapFact.value == loc.t("componentsUi.gas.estimateFailed"))
        #expect(tapFact.feeTap == .retry, "the footer asks for a tap: the line takes it")
        #expect(tapPage.heldNote == loc.t("componentsUi.signing.confirmBlock.feeFailed"))

        let coin = try confirm(view, fee: try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: true))))
        let coinFact = try #require(feeFact(coin))
        #expect(coinFact.value == loc.t("componentsUi.gas.payWithAnotherCoin"))
        #expect(coinFact.feeTap == .chooseCoin)
        #expect(coin.heldNote == loc.t("componentsUi.signing.confirmBlock.feeWouldFail"))
        #expect(coin.heldNote != loc.t("componentsUi.signing.confirmBlock.feeFailed"), "never \"Tap it to retry\"")

        let none = try confirm(view, fee: try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: false))))
        #expect(feeFact(none)?.value == "—")
        #expect(feeFact(none)?.feeTap == FeeRowTap.nothing)

        // A settled fee's line is a fact on this page.
        #expect(try confirm(view, fee: nil).facts.allSatisfy { $0.feeTap == nil })
    }

    /// The signing sheet: the row's figure, whether it is a control, its
    /// chevron and what the controller does with the tap — one mapping.
    @Test func theSheetsRowDoesWhatItSays() throws {
        let coinFee = try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: true)))
        let coin = try #require(row(SigningLive.feeModel(clear: clearSign, fee: coinFee, context: context())))
        #expect(coin.value == loc.t("componentsUi.gas.payWithAnotherCoin"))
        #expect(coin.tappable)
        #expect(SigningController.feeTap(coinFee) == .chooseCoin)
        // The tap, taken: the coin list is open, both coins on it.
        let open = try #require(row(SigningLive.feeModel(clear: clearSign, fee: coinFee, context: context(feeOpen: true))))
        #expect(open.listed == 2)

        let noneFee = try feeView(try #require(FeeCoreScene.wouldFail(anotherCoin: false)))
        let none = try #require(row(SigningLive.feeModel(clear: clearSign, fee: noneFee, context: context())))
        #expect(none.value == "—")
        #expect(!none.tappable, "no control with nothing left to try")
        #expect(SigningController.feeTap(noneFee) == .nothing)

        let tapFee = try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 8453)).failed)
        #expect(SigningController.feeTap(tapFee) == .retry)
        #expect(try #require(row(SigningLive.feeModel(clear: clearSign, fee: tapFee, context: context()))).tappable)

        // The chevron promises the coin list: there for "Pay with another
        // coin", gone for a retry and for nothing.
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 8453
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
        func chevron(_ fee: FeeViewWire) -> Bool {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: .empty, guard: .empty, fee: fee, context: context()
            ).feeChevron
        }
        #expect(chevron(coinFee))
        #expect(!chevron(noneFee))
        #expect(!chevron(tapFee))
    }

    /// The figure is one of the keys this build draws, else the dash —
    /// never a dotted key path.
    @Test func anUnknownFigureIsTheDash() {
        func figure(_ key: String?) -> String {
            FeeFailureViewWire(
                failure: "would_fail", reasonKey: nil, autoRetry: false, retrying: false,
                figureKey: key, footerKey: "componentsUi.signing.confirmBlock.feeWouldFail"
            ).figure(loc)
        }
        #expect(figure("componentsUi.gas.payWithAnotherCoin") == loc.t("componentsUi.gas.payWithAnotherCoin"))
        #expect(figure("componentsUi.gas.estimateFailed") == loc.t("componentsUi.gas.estimateFailed"))
        #expect(figure("componentsUi.gas.somethingNew") == "—")
        #expect(figure(nil) == "—")
    }

    // MARK: - Item 5: another chain's failure is dropped

    /// The form moved to Gnosis while the fee machine's failure is still
    /// Base's: no figure, no reason, no footer, and the bridge does not count
    /// it as failed. Its own chain's failure is drawn.
    @Test func aFailureForAnotherChainIsNotDrawnOrTold() throws {
        let baseFailure = try feeView(try #require(FeeCoreScene.chainDown.views(chainId: 8453)).failed)
        #expect(baseFailure.failure?.chainId == 8453)
        let onGnosis = try sendView(stage: "enter_details", chainId: 100)
        #expect(SendLive.formFailure(baseFailure, view: onGnosis) == nil)
        let row = try form(onGnosis, fee: baseFailure)
        #expect(row.failNote == nil, "the old chain's reason flashed on the new chain")
        #expect(row.tap == .open)
        #expect(row.value != loc.t("componentsUi.gas.estimateFailed"))

        let confirmOnGnosis = try sendView(stage: "confirm", chainId: 100)
        let page = try confirm(confirmOnGnosis, fee: baseFailure)
        #expect(page.facts.allSatisfy { $0.feeTap == nil })
        let feeLine = try #require(page.facts.count > 3 ? page.facts[3] : nil)
        #expect(feeLine.note == nil, "the old chain's reason under the new chain's fee")
        #expect(feeLine.value != loc.t("componentsUi.gas.estimateFailed"))
        #expect(page.heldNote == nil)

        // On Base it is this form's own failure.
        let onBase = try sendView(stage: "enter_details")
        #expect(SendLive.formFailure(baseFailure, view: onBase) != nil)
        #expect(try form(onBase, fee: baseFailure).failNote
                == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Base"]))
        // A sweep's form chain is its `multi_chain_id`.
        let sweep = try sendView(stage: "enter_details", patch: [
            "selected_token": NSNull(), "multi_select_mode": true, "multi_chain_id": 100,
        ])
        #expect(SendLive.formChain(sweep) == 100)
        #expect(SendLive.formFailure(baseFailure, view: sweep) == nil)
    }

    /// The send machine hears only its own chain's failure: the bridge's
    /// input is `formFailure`, and the store tells each change once.
    @Test func theBridgeTellsTheSendMachineOnlyItsOwnChainsFailure() throws {
        let baseFailure = try feeView(try #require(FeeCoreScene.chainDown.views(chainId: 8453)).failed)
        let onGnosis = try sendView(stage: "confirm", chainId: 100)
        let onBase = try sendView(stage: "confirm")
        #expect((SendLive.formFailure(baseFailure, view: onGnosis) != nil) == false)
        #expect((SendLive.formFailure(baseFailure, view: onBase) != nil) == true)
        #expect(SendLive.formFailure(nil, view: onBase) == nil)
    }

    // MARK: - Item 7: a balance that read nothing has no figure

    @Test func aBalanceThatReadNothingHasNoFigure() throws {
        let home = WalletFixtures.buildMobileState(.h1, loc: loc)
        for internalFault in [true, false] {
            let view = try #require(BalanceCoreScene.view(internalFault: internalFault, everyChain: true))
            #expect(view.unreachable == true)
            #expect(view.displayTotalUsd == nil, "the core still says a total for nothing read")
            let hero = WalletLive.balance(view, fallback: home.balance, loc: loc)
            #expect(hero.state == .loading)
            #expect(hero.integer == nil && hero.decimals == nil)
            // The balance breakdown names no total either.
            let detail = SettingsLive.withBalanceDetail(
                view, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
            ).balanceDetail
            #expect(!detail.summary.contains("$0.00"), "\(detail.summary)")
        }
    }

    // MARK: - The corpus

    @Test func thePolishKeysResolve() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        for key in [
            I18nKeys.CoreRound.notSentTitle, I18nKeys.CoreRound.notSentBody,
            I18nKeys.CoreRound.payWithAnotherCoin, I18nKeys.CoreRound.feeWouldFail,
        ] {
            #expect(I18nKeys.all.contains(key))
            #expect(loc.t(key) != key, "\(key) en")
            #expect(zh.t(key) != key, "\(key) zh")
        }
        #expect(loc.t(I18nKeys.CoreRound.notSentTitle) == "Not sent yet")
        #expect(zh.t(I18nKeys.CoreRound.notSentTitle) == "暂未发送")
    }
}
