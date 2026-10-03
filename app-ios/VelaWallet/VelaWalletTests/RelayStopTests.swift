//
//  RelayStopTests.swift — spec 098 §2, §4.
//
//  The core opens the relay's two stops at Continue, while the stage is still
//  the form. Until 098 this app drew them on the confirm page only, so pressing
//  Continue into a relay with no gas did nothing anyone could see — and a relay
//  that cannot serve the chain at all was never a stop: the send went on to the
//  passkey and failed after the person had signed.
//
//  Views are the core's own, patched — no field is invented.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct RelayStopTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private struct Missing: Error {}

    private func sendView(_ patch: [String: Any]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private let emptyFloat: [String: Any] = [
        "chain_id": 100, "address": "0x3e59292e18417f814112f731e7163534c6d2fe3c",
        "asset": "native", "balance": "0", "floor": "100000000000000",
        "bootstrap_needed": true, "operator_served": true,
    ]

    @Test func theTreasuryStopSaysWhatItHasAndThatItWatches() throws {
        let view = try sendView(["stage": "enter_details", "treasury_bootstrap": emptyFloat])
        let text = try #require(SendLive.stopNotice(view, loc: loc))
        #expect(text.hasPrefix(loc.t("componentsUi.treasuryBootstrap.title")))
        #expect(text.contains(loc.t("componentsUi.treasuryBootstrap.watching")))
        #expect(text.contains(loc.t("componentsUi.treasuryBootstrap.disclaimer")))
        // The form's button is the stop's retry.
        #expect(SendLive.stopRetry(view, loc: loc) == loc.t("componentsUi.treasuryBootstrap.retryBtn"))
    }

    @Test func aRelayThatCannotServeTheChainIsAStopOfItsOwn() throws {
        let view = try sendView([
            "stage": "enter_details",
            "relay_unreachable": ["chain_id": 1337, "operator_served": false],
        ])
        let text = try #require(SendLive.stopNotice(view, loc: loc))
        #expect(text.hasPrefix(loc.t("componentsUi.relayUnreachable.title")))
        // A network the person added: theirs to point elsewhere.
        #expect(text.contains(loc.t("componentsUi.relayUnreachable.customLead")))
        #expect(text.contains(loc.t("componentsUi.relayUnreachable.settingsHint")))
        #expect(SendLive.stopRetry(view, loc: loc) == loc.t("componentsUi.relayUnreachable.retryBtn"))
        // Not the funding stop.
        #expect(!text.contains(loc.t("componentsUi.treasuryBootstrap.title")))
    }

    @Test func onConfirmTheCantReachStopHasItsOwnWayOut() throws {
        let view = try sendView([
            "stage": "confirm",
            "relay_unreachable": ["chain_id": 1, "operator_served": true],
        ])
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            throw Missing()
        }
        let model = SendLive.confirm(
            view, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawn, loc: loc
        )
        #expect(model.noticeAction == loc.t("componentsUi.relayUnreachable.retryBtn"))
        #expect(model.noticeSecondary == loc.t("componentsUi.relayUnreachable.closeBtn"))
        #expect(model.notice?.hasPrefix(loc.t("componentsUi.relayUnreachable.title")) == true)
    }

    /// Until 098 the phone said "fund it" and never said where: the address
    /// was in the core's view and on no screen.
    @Test func theTreasuryStopSaysWhereTheGasGoesOnBothPages() throws {
        let form = try sendView(["stage": "enter_details", "treasury_bootstrap": emptyFloat])
        let fund = try #require(SendLive.fundAddress(form, loc: loc))
        #expect(fund.address == "0x3e59292e18417f814112f731e7163534c6d2fe3c")
        #expect(fund.copy == loc.t("componentsUi.treasuryBootstrap.copyBtn"))

        let confirm = try sendView(["stage": "confirm", "treasury_bootstrap": emptyFloat])
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            throw Missing()
        }
        let model = SendLive.confirm(
            confirm, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawn, loc: loc
        )
        #expect(model.noticeFund?.address == "0x3e59292e18417f814112f731e7163534c6d2fe3c")

        // Nothing to fund when the relay cannot reach the network at all.
        let unreachable = try sendView([
            "stage": "enter_details",
            "relay_unreachable": ["chain_id": 1337, "operator_served": false],
        ])
        #expect(SendLive.fundAddress(unreachable, loc: loc) == nil)
    }

    @Test func noStopNoRetry() throws {
        let view = try sendView(["stage": "enter_details"])
        #expect(SendLive.stopNotice(view, loc: loc) == nil)
        #expect(SendLive.stopRetry(view, loc: loc) == nil)
        #expect(SendLive.fundAddress(view, loc: loc) == nil)
    }
}
