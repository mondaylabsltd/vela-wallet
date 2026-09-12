//
//  WalletFlowFixturesTests.swift
//  VelaWalletTests
//
//  Spec 021: every state in the matrix builds, and the canon reproduces the
//  mock content verbatim once merged with the zh corpus (the mocks are zh
//  renderings), so visual diffing against docs/design/wallet-2/ stays meaningful.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct WalletFlowFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    @Test func everyStateBuilds() {
        #expect(FlowStateId.allCases.count == 30)
        for state in FlowStateId.allCases {
            #expect(WalletFlowFixtures.build(state, loc: loc).state == state)
        }
    }

    @Test func onlyR2xCarriesTheLargeTextScale() {
        for state in FlowStateId.allCases {
            let expected: CGFloat = state == .r2x ? 1.35 : 1
            #expect(WalletFlowFixtures.build(state, loc: loc).textScale == expected)
        }
    }

    @Test func r1ListsTheEightNetworksWithOneSharedAddress() throws {
        let model = WalletFlowFixtures.build(.r1, loc: loc)
        guard case .receive(let receive) = model.base else {
            Issue.record("expected the receive list")
            return
        }
        #expect(receive.subtitle == "同一地址，通用于全部 8 个网络")
        #expect(receive.rows.map(\.name) == [
            "Ethereum", "BNB Chain", "Polygon", "Arbitrum",
            "Optimism", "Base", "Avalanche", "Gnosis",
        ])
        // The point of the screen: every row is the SAME address.
        #expect(Set(receive.rows.map(\.addressDisplay)).count == 1)
    }

    @Test func r2TitlesTheNetworkAndR3TheAssetAndOnlyR3HasAContract() {
        guard case .receiveQr(let r2)? = WalletFlowFixtures.build(.r2, loc: loc).sheet,
              case .receiveQr(let r3)? = WalletFlowFixtures.build(.r3, loc: loc).sheet
        else {
            Issue.record("expected both QR sheets")
            return
        }
        #expect(r2.title == "使用这个地址接收 Ethereum 上的资产")
        #expect(r2.contract == nil)
        #expect(r3.title == "使用这个地址接收 Ethereum 上的 USDT")
        #expect(r3.contract?.label == "代币合约")
    }

    @Test func splitsTheAccountAddressIntoTwoEvenLines() {
        #expect(
            WalletFlowFixtures.addressLines("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c")
                == ["0x14fB1fB21751E29F7Ec", "48dC450017552E3D1eA5c"]
        )
    }

    @Test func a1GroupsTheHistoryByDayEndingOnALiteralDate() {
        guard case .history(let history) = WalletFlowFixtures.build(.a1, loc: loc).base else {
            Issue.record("expected the history")
            return
        }
        #expect(history.groups.map(\.label) == ["今天", "昨天", "8/12"])
        let first = history.groups[0].rows[0]
        #expect(first.title == "已发送")
        #expect(first.amount == "−2")
        #expect(first.unit == "POL")
    }

    @Test func a2CarriesAContractRowAndA3DoesNot() {
        guard case .txDetail(let a2)? = WalletFlowFixtures.build(.a2, loc: loc).sheet,
              case .txDetail(let a3)? = WalletFlowFixtures.build(.a3, loc: loc).sheet
        else {
            Issue.record("expected both detail sheets")
            return
        }
        #expect(a2.amount == "+120 USDT")
        #expect(a2.positive)
        #expect(a2.facts.contains { $0.label == "代币合约" })

        #expect(a3.amount == "−2 POL")
        #expect(!a3.positive)
        // A native coin has no contract — the row must be absent, not empty.
        #expect(!a3.facts.contains { $0.label == "代币合约" })
    }

    @Test func t1ListsTheSixMockAssetsAndT4ReplacesThemWithGuidance() {
        guard case .assets(let t1) = WalletFlowFixtures.build(.t1, loc: loc).base,
              case .assets(let t4) = WalletFlowFixtures.build(.t4, loc: loc).base
        else {
            Issue.record("expected both asset screens")
            return
        }
        #expect(t1.rows.count == 6)
        #expect(t1.rows[0].ticker == "BNB")
        #expect(t1.rows[0].balance == "0.8533")
        #expect(t1.empty == nil)

        #expect(t4.rows.isEmpty)
        #expect(t4.empty?.title == "存入您的第一笔资产")
        #expect(t4.empty?.hintTitle == "已经收到代币但没有显示？")
    }

    @Test func t3OffersTheRealContractAndT5RejectsATruncatedOne() {
        guard case .addToken(let t3)? = WalletFlowFixtures.build(.t3, loc: loc).sheet,
              case .addToken(let t5)? = WalletFlowFixtures.build(.t5, loc: loc).sheet
        else {
            Issue.record("expected both add-token sheets")
            return
        }
        #expect(t3.fieldValue == WalletFlowFixtures.usdtContract)
        #expect(t3.fieldError == nil)
        #expect(!t3.ctaDisabled)
        if case .token = t3.result {} else { Issue.record("expected a token result") }

        #expect(t5.fieldError == "无效的合约地址")
        // A rejected address must not leave a stale result card under it.
        if case .none = t5.result {} else { Issue.record("expected no result card") }
        #expect(t5.ctaDisabled)
    }

    @Test func t5bMarksAvalancheIncompatibleAndDisablesTheCta() {
        guard case .addToken(let model)? = WalletFlowFixtures.build(.t5b, loc: loc).sheet else {
            Issue.record("expected the add-token sheet")
            return
        }
        #expect(model.tab == .native)
        guard case .network(_, _, let chip, let facts, _) = model.result else {
            Issue.record("expected a network result")
            return
        }
        #expect(chip.text == "不兼容")
        #expect(chip.tone == .error)
        #expect(facts.contains { $0.value == "43114" })
        #expect(model.ctaDisabled)
    }

    @Test func sd1bGreysTheOffNetworkRowsAndSelectsOnlyTheOnNetworkOnes() {
        guard case .sendPick(let model) = WalletFlowFixtures.build(.sd1b, loc: loc).base,
              let selection = model.selection
        else {
            Issue.record("expected the multi-select picker")
            return
        }
        // Never both: a row that cannot be picked cannot be picked.
        for (index, on) in selection.selected.enumerated() {
            #expect(!(on && selection.dimmed[index]), "row \(index) is both selected and dimmed")
        }
        #expect(selection.selected.filter { $0 }.count == 3)
        #expect(model.notice?.text.contains("Ethereum") == true)
        #expect(model.cta.accent)
    }

    @Test func sd2bTotalsThreeRecipientsToTheAmountSd2SendsAlone() {
        guard case .sendForm(let model) = WalletFlowFixtures.build(.sd2b, loc: loc).base else {
            Issue.record("expected the send form")
            return
        }
        #expect(model.mode == .split)
        #expect(model.recipients.count == 3)
        #expect(model.recipients.compactMap { Double($0.amount) }.reduce(0, +) == 120)
        #expect(model.summary?.value == "120 USDT · ≈$120.00")
    }

    @Test func sd2dSweepsThreeTokensToOneAddressAndSaysSo() {
        guard case .sendForm(let model) = WalletFlowFixtures.build(.sd2d, loc: loc).base else {
            Issue.record("expected the send form")
            return
        }
        #expect(model.mode == .sweep)
        #expect(model.sweepRows.count == 3)
        #expect(model.recipient?.note == "多币发送时收款人为同一地址")
    }

    @Test func sd2cCountsOnlyTheRowsItCanActuallyImport() {
        guard case .batchImport(let model)? = WalletFlowFixtures.build(.sd2c, loc: loc).sheet else {
            Issue.record("expected the import sheet")
            return
        }
        #expect(model.rows.filter(\.ok).count == 2)
        // The CTA promises what it delivers — three parsed, two importable.
        #expect(model.parsedLabel.contains("3"))
        #expect(model.cta.contains("2"))
        #expect(model.rejectedText?.contains("1") == true)
    }

    @Test func sd2fOffersOneFeeTokenAsChosenAndTheRestNot() {
        guard case .feeToken(let model)? = WalletFlowFixtures.build(.sd2f, loc: loc).sheet else {
            Issue.record("expected the fee sheet")
            return
        }
        #expect(model.rows.filter(\.selected).count == 1)
        #expect(model.rows[0].symbol == "ETH")
    }

    @Test func sd3ShowsFourFactsAndSd3cAddsThePerAssetBreakdown() {
        guard case .sendConfirm(let sd3) = WalletFlowFixtures.build(.sd3, loc: loc).base,
              case .sendConfirm(let sd3c) = WalletFlowFixtures.build(.sd3c, loc: loc).base
        else {
            Issue.record("expected both confirmations")
            return
        }
        #expect(sd3.amount == "120 USDT")
        #expect(sd3.facts.count == 4)
        #expect(sd3.breakdown.isEmpty)

        // The mock drew 项资产; the corpus key the legacy receipt already shares
        // says 种资产. Not worth churning a shared string over.
        #expect(sd3c.amount == "3 种资产")
        #expect(sd3c.breakdown.count == 3)
        #expect(sd3c.subline.contains("$200.90"))
    }

    @Test func theReceiptKeepsOneAccentCtaAndOnlyForTheFinalState() {
        let states: [FlowStateId] = [.sd4a, .sd4b, .sd4c]
        let stages: [SendReceiptModel] = states.compactMap { state in
            guard case .sendReceipt(let m) = WalletFlowFixtures.build(state, loc: loc).base else {
                return nil
            }
            return m
        }
        #expect(stages.count == 3)
        #expect(stages.map(\.stage) == [.submitting, .submitted, .confirmed])
        #expect(stages.map(\.ctaAccent) == [false, false, true])
        // A transaction still in flight offers no hash and no explorer link.
        #expect(stages[0].hash == nil)
        #expect(stages[2].hash?.value == "0x8f3a…c21d")
        #expect(stages[0].captions.contains("关闭此页交易会在后台继续"))
    }

    @Test func pinsTheChainIdsTheAddNetworkCardPrints() {
        #expect(WalletFlowFixtures.networks.first { $0.name == "Avalanche" }?.chainId == "43114")
        #expect(WalletFlowFixtures.networks.first { $0.name == "Gnosis" }?.chainId == "100")
    }

    /// The navigation contract SC-002 rests on: a flow entered from the home
    /// opens at the right screen, steps deeper, and unwinds one level at a time.
    @Test func theFlowStackEntersStepsAndUnwinds() {
        let nav = FlowNav()
        #expect(!nav.isOpen)

        nav.enter(.send)
        #expect(nav.stack == [.sd1])

        nav.push(.sendForm)
        nav.push(.sendConfirm)
        #expect(nav.stack == [.sd1, .sd2, .sd3])

        nav.back()
        #expect(nav.stack == [.sd1, .sd2])

        // `addToken` opens two, so its back chevron leads to the list.
        nav.enter(.addToken)
        #expect(nav.stack == [.t1, .t3])

        // Unknown steps do nothing rather than trapping.
        nav.push(.chains)
        #expect(nav.stack == [.t1, .t3])

        nav.push(.done)
        #expect(!nav.isOpen)
    }
}
