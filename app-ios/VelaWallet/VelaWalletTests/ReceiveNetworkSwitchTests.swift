//
//  ReceiveNetworkSwitchTests.swift
//  VelaWalletTests
//
//  Spec 090: the receive code's "include network" switch.
//
//  What the code says is the core's (`payment_request`), so these drive the
//  real machine the way `PaymentRequestStore` does and check that the sheet,
//  the saved card and Vela's own scanner all agree with it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ReceiveNetworkSwitchTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private var baseQr: ReceiveQrModel {
        guard case .receiveQr(let model)? = WalletFlowFixtures.build(.r2, loc: loc).sheet
        else { fatalError("the R2 fixture lost its QR sheet") }
        return model
    }

    private var baseCard: ShareCardModel {
        guard case .share(let card) = WalletFlowFixtures.build(.r4, loc: loc).base
        else { fatalError("the R4 fixture lost its card") }
        return card
    }

    /// The real machine, one event at a time.
    private final class Machine {
        let core = PaymentRequestCore()
        func send(_ event: [String: Any]) throws -> PaymentRequestViewWire {
            let result = try core.dispatch(eventJson: CoreJSON.string(event))
            let view = try CoreJSON.object(result)["view"] as? [String: Any] ?? [:]
            return try CoreJSON.decode(PaymentRequestViewWire.self, from: view)
        }
    }

    /// Started for `me`, showing Gnosis — what a tapped Gnosis row does.
    private func onGnosis(_ machine: Machine) throws -> PaymentRequestViewWire {
        _ = try machine.send([
            "type": "start", "account": me, "recipient": me,
            "base_url": "https://getvela.app/pay",
        ])
        return try machine.send([
            "type": "asset_picked", "chain_id": 100, "token_address": NSNull(),
            "symbol": "XDAI", "decimals": 18, "network_name": "Gnosis",
        ])
    }

    @Test func theGalleryDrawsTheSwitchOff() {
        #expect(baseQr.network == NetworkSwitchModel(
            label: loc.t("receive.includeNetwork"), isOn: false, hint: nil
        ))
    }

    /// Off: the bare address and no hint. On: the code, and the saved card,
    /// name the network on screen; the hint rides under the switch; copy is
    /// still the bare address; and Vela's scanner reads it back to the same
    /// address and chain.
    @Test func offIsTheBareAddressAndOnNamesTheNetworkEverywhere() throws {
        let machine = Machine()
        let chain = ChainCatalog.meta(100)
        let off = try onGnosis(machine)
        let offQr = FlowsLive.receiveQr(me, name: "我", chain: chain, pay: off, on: baseQr, loc: loc)
        #expect(offQr.modules == QrCode.modules(me))
        #expect(offQr.network == NetworkSwitchModel(
            label: loc.t("receive.includeNetwork"), isOn: false, hint: nil
        ))

        let on = try machine.send(["type": "include_network_changed", "include": true])
        let uri = "ethereum:\(me)@100"
        #expect(on.qrValue == uri)
        #expect(on.copyPayload == me)
        let onQr = FlowsLive.receiveQr(me, name: "我", chain: chain, pay: on, on: baseQr, loc: loc)
        #expect(onQr.modules == QrCode.modules(uri))
        #expect(onQr.network == NetworkSwitchModel(
            label: loc.t("receive.includeNetwork"), isOn: true,
            hint: loc.t("receive.includeNetworkHint")
        ))

        let card = FlowsLive.shareCard(me, name: "我", chain: chain, pay: on, on: baseCard, loc: loc)
        #expect(card.modules == QrCode.shareModules(uri))

        let scanned = try #require(Eip681.parse(on.qrValue))
        #expect(scanned.recipient == me)
        #expect(scanned.chainId == 100)
        #expect(scanned.tokenAddress == nil)
        #expect(scanned.amountBaseUnits == nil)
    }

    /// An answer about another network is not drawn: the bare address, which
    /// every wallet reads, and no switch beside a code it does not describe.
    @Test func anAnswerAboutAnotherNetworkIsNotDrawn() throws {
        let machine = Machine()
        _ = try onGnosis(machine)
        let on = try machine.send(["type": "include_network_changed", "include": true])
        let base = FlowsLive.receiveQr(
            me, name: "我", chain: ChainCatalog.meta(8453), pay: on, on: baseQr, loc: loc
        )
        #expect(base.modules == QrCode.modules(me))
        #expect(base.network == nil)
    }

    /// A press is one `select` haptic and a request for the other position.
    @Test func aFlipIsOneHapticAndAsksForTheOtherPosition() {
        var asked: Bool?
        let played = VelaHaptic.recording {
            VelaSwitchRow(label: "x", isOn: false, onChange: { asked = $0 }).flip()
        }
        #expect(played == [.select])
        #expect(asked == true)
        // The gallery's picture neither buzzes nor asks.
        #expect(VelaHaptic.recording { VelaSwitchRow(label: "x", isOn: true).flip() }.isEmpty)
    }
}
