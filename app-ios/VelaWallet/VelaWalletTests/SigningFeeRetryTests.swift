//
//  SigningFeeRetryTests.swift
//  VelaWalletTests
//
//  Spec 079 US2: the signing sheet's fee row carries the send form's refresh
//  control, says why a quote failed when the network is the reason, and asks
//  again by itself on the core's schedule while the sheet can still use it.
//  The Android twin is in `SigningLiveTest.kt`. Hermetic.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SigningFeeRetryTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func context() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
    }

    private func clear(_ surface: ClearSurface) -> ClearSigningViewWire {
        ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil,
            surface: surface, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
    }

    private func fee(busy: Bool = false, failed: String? = nil, options: Int = 1) -> FeeViewWire {
        let coin = FeeOptionWire(
            symbol: "xDAI", contract: nil, decimals: 18, balance: "1000000000000000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "1000",
            insufficient: false, selected: true
        )
        let other = FeeOptionWire(
            symbol: "USDC", contract: "0xusdc", decimals: 6, balance: "1000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "10",
            insufficient: false, selected: false
        )
        return FeeViewWire(
            busy: busy, failed: failed,
            fee: failed == nil && !busy
                ? FeeEstimateWire(
                    chainId: 100, totalWei: "1000", maxFeePerGas: "1", totalGas: "21000",
                    deployed: true, quoted: true, feeAsset: .native, feeRecipient: "0xrelay"
                )
                : nil,
            stale: false, feeToken: nil, options: options > 1 ? [coin, other] : [coin],
            confirmFeeReady: failed == nil && !busy
        )
    }

    private func warning(_ model: FeeModel) -> String? {
        if case .onchain(_, _, _, let warning, _) = model { return warning }
        return nil
    }

    private func tappable(_ model: FeeModel) -> Bool {
        if case .onchain(_, _, _, _, let tappable) = model { return tappable }
        return false
    }

    /// The fee can always be asked again, and a fee that could not be quoted
    /// says why — but only when the network is why.
    @Test func theFeeRowCarriesTheSendFormsRefreshAndSaysWhyAQuoteFailed() {
        let shown = SigningLive.feeRefresh(clear: clear(.clearSign), fee: fee(), loc: loc)
        #expect(shown?.label == loc.t("send.feeRefresh"))
        #expect(shown?.refreshing == false)
        #expect(SigningLive.feeRefresh(clear: clear(.clearSign), fee: fee(busy: true), loc: loc)?.refreshing == true,
                "dimmed while a measurement is out")
        #expect(SigningLive.feeRefresh(clear: clear(.messageSign), fee: nil, loc: loc) == nil,
                "a message has no network fee to refresh")

        let down = SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: "quote_unavailable"), context: context())
        #expect(warning(down) == loc.t("componentsUi.funding.denialNetworkError"))
        #expect(tappable(down), "a failed quote is tapped to ask again")
        for failure in ["fee_token_unavailable", "estimate_failed", "gas_quote_too_high"] {
            #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: failure), context: context()))
                    == loc.t("componentsUi.funding.denialNetworkError"), "\(failure)")
        }
        for failure in ["missing_public_key", "calculation_failed"] {
            #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: failure), context: context()))
                    == nil, "no network sentence for \(failure): the network did not cause it")
        }
        #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(), context: context())) == nil)
    }

    /// The sheet's model: a chevron only where a tap opens a coin list; a
    /// refresh wherever there is a network fee; none on a refused request.
    @Test func theSheetDrawsTheRefreshAndAChevronOnlyForACoinList() {
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        func model(_ fee: FeeViewWire) -> SigningModel {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: clear(.none), guard: .empty, fee: fee, context: context()
            )
        }
        let one = model(fee())
        #expect(one.feeRefresh?.label == loc.t("send.feeRefresh"))
        #expect(!one.feeChevron, "one coin: no list, so no chevron")
        #expect(model(fee(options: 2)).feeChevron)
        #expect(!model(fee(failed: "quote_unavailable")).feeChevron,
                "a failed quote is retried by the refresh, not a list")
    }

    /// The re-quote schedule is the core's — 3 s, 6 s, 12 s, then every
    /// 15 s — and only while the sheet can still use a fee.
    @Test func aRecoverableFailureIsAskedAgainOnTheCoresSchedule() {
        let down = fee(failed: "quote_unavailable")
        #expect(SigningController.requoteDelay(down, attempt: 1, allowed: true) == 3_000)
        #expect(SigningController.requoteDelay(down, attempt: 2, allowed: true) == 6_000)
        #expect(SigningController.requoteDelay(down, attempt: 3, allowed: true) == 12_000)
        #expect(SigningController.requoteDelay(down, attempt: 4, allowed: true) == 15_000)
        #expect(SigningController.requoteDelay(down, attempt: 9, allowed: true) == 15_000)

        #expect(SigningController.requoteDelay(down, attempt: 1, allowed: false) == nil,
                "approved, answered or closed: nothing more is asked")
        #expect(SigningController.requoteDelay(fee(busy: true, failed: "quote_unavailable"), attempt: 1, allowed: true) == nil,
                "a measurement already out is not doubled")
        #expect(SigningController.requoteDelay(fee(failed: "missing_public_key"), attempt: 1, allowed: true) == nil)
        #expect(SigningController.requoteDelay(fee(), attempt: 1, allowed: true) == nil, "a quote needs no retry")
    }
}
