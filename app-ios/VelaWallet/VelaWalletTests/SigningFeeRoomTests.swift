//
//  SigningFeeRoomTests.swift
//  VelaWalletTests
//
//  Nothing on the signing sheet moves for what the fee card comes to say
//  (device walks 2026-10-10 / 10-11). On the Android phone the whole sheet
//  stepped upward twice as the fee landed, the ✕ under a finger. This sheet
//  is always `.large`, its header and its confirm pinned, so those three
//  never moved here — but INSIDE the body the fee's worth arrived with the
//  speed's note and "no coin can pay", and the signing account under the
//  card was pushed to the fold.
//
//  The real `SigningSheet`, hosted at the 390 × 844 phone and at the SE's
//  375 × 667, walked through what a fee does after the sheet is up — priced;
//  priced and nobody can pay; failed and asked again; offline, then priced
//  — with the tree read at every step for the ✕, the header, the fee row,
//  the signing account and the confirm.
//
//  With the card's rooms named (`FeeRooms`, as the live builder names them)
//  every one of the five stands still at every step. With none — the sheet
//  as it was — the same walks move the signing account, by the numbers the
//  second test keeps.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SigningFeeRoomTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    /// What the fee is doing.
    enum Step: String {
        /// The first quote is out: "Estimating…", the gate shut.
        case estimating
        /// Priced, payable, nothing to say.
        case priced
        /// Priced: the speed's note in the card, "no coin can pay" under it.
        case cannotPay
        /// The relay refused the gas price: the dash and its sentence.
        case failed
        /// The chain cannot be reached: the dash and its sentence.
        case offline
    }

    static let walks: [(name: String, steps: [Step])] = [
        ("priced", [.estimating, .priced]),
        ("priced + can't pay", [.estimating, .cannotPay]),
        ("failed", [.estimating, .failed, .estimating, .failed]),
        ("offline", [.estimating, .offline, .cannotPay]),
    ]

    /// The rooms, as `SigningLive.feeRooms` names them for an on-chain fee.
    private func rooms(_ loc: Loc) -> FeeRooms {
        FeeRooms(
            speedNote: loc.t("send.feeSpeedFree"),
            reasons: [
                loc.t("componentsUi.gas.noCoinPays"),
                loc.t("send.warnInsufficientGas", vars: ["sym": "xDAI"]),
            ]
        )
    }

    private func model(_ step: Step, loc: Loc, rooms: FeeRooms?) -> SigningModel {
        let base = SigningFixtures.build(.cs36, loc: loc)
        let label = loc.t("componentsUi.gas.networkFee")
        let figure = "~0.01 xDAI · ≈¥0.07"
        let measuringNote = loc.t("componentsUi.signing.confirmBlock.feeMeasuring")
        let fee: FeeModel
        var enabled = false
        var note: String? = measuringNote
        var measuring = true
        var free = false
        switch step {
        case .estimating:
            fee = .onchain(label: label, value: loc.t("componentsUi.gas.estimating"), selector: nil,
                           tappable: false)
        case .priced:
            fee = .onchain(label: label, value: figure, selector: nil, tappable: false)
            (enabled, note, measuring) = (true, nil, false)
        case .cannotPay:
            fee = .onchain(label: label, value: figure, selector: nil,
                           warning: loc.t("componentsUi.gas.noCoinPays"), tappable: false)
            (note, measuring, free) = (nil, false, true)
        case .failed:
            fee = .onchain(label: label, value: "—", selector: nil,
                           warning: loc.t("componentsUi.gas.reasonQuoteHigh"), tappable: false)
        case .offline:
            fee = .onchain(label: label, value: "—", selector: nil,
                           warning: loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]),
                           tappable: false)
        }
        var model = SigningModel(
            id: base.id, dapp: base.dapp, network: base.network, blocks: base.blocks,
            tech: base.tech, techOpen: base.techOpen, fee: fee, signer: base.signer,
            confirm: (action: base.confirm?.action ?? "", enabled: enabled),
            confirmBlockLine: note, panelTitle: base.panelTitle
        )
        model.dappOwn = base.dappOwn
        model.closeLabel = loc.t("onboarding.common.close")
        model.feeRefresh = FeeRefreshModel(label: loc.t("send.feeRefresh"), refreshing: measuring)
        model.feeSpeed = FeeSpeedModel(
            label: loc.t("send.feeSpeedLabel"), value: free ? "Fast" : "Standard", open: false,
            onceNote: "", freeNote: free ? loc.t("send.feeSpeedFree") : nil,
            singleNote: nil, gasPriceLabel: "", gasPriceLine: false, options: []
        )
        model.feeRooms = rooms
        return model
    }

    /// Where the five stand.
    struct Places: Equatable, CustomStringConvertible {
        var close: CGFloat
        var header: CGFloat
        var fee: CGFloat
        var signer: CGFloat
        var confirm: CGFloat

        var description: String { "✕ \(close) header \(header) fee \(fee) signer \(signer) confirm \(confirm)" }
    }

    private func places(_ seen: SigningSheetProbe.Seen, _ model: SigningModel, loc: Loc) throws -> Places {
        let close = try #require(seen.tree.first { $0.label == model.closeLabel }, "the ✕")
        let header = try #require(seen.tree.first { !$0.label.isEmpty && $0.label != model.closeLabel }, "the header")
        let fee = try #require(seen.tree.first { $0.label == loc.t("componentsUi.gas.networkFee") }, "the fee row")
        let signer = try #require(seen.tree.first { $0.label == model.signer.label }, "the signing account")
        let confirm = try #require(seen.confirm, "the confirm")
        return Places(close: close.frame.minY, header: header.frame.minY, fee: fee.frame.minY,
                      signer: signer.frame.minY, confirm: confirm.frame.minY)
    }

    /// One walk on one live sheet: where everything stands at each step.
    private func walk(_ steps: [Step], size: CGSize, loc: Loc, rooms: FeeRooms?) async throws -> [Places] {
        let first = model(steps[0], loc: loc, rooms: rooms)
        let probe = SigningSheetProbe(first, size: size)
        defer { probe.close() }
        var out: [Places] = []
        for step in steps {
            let model = model(step, loc: loc, rooms: rooms)
            let seen = try await probe.show(model) { seen in
                // The tree has followed the model: this step's fee row is on it.
                switch step {
                case .estimating: seen.said(loc.t("componentsUi.gas.estimating"))
                case .priced: seen.says("≈¥0.07") && !seen.says(loc.t("componentsUi.gas.noCoinPays"))
                case .cannotPay: seen.said(loc.t("componentsUi.gas.noCoinPays"))
                case .failed: seen.said(loc.t("componentsUi.gas.reasonQuoteHigh"))
                case .offline: seen.says("Gnosis")
                }
            }
            out.append(try places(seen, model, loc: loc))
        }
        return out
    }

    static let sizes = [CGSize(width: 390, height: 844), CGSize(width: 375, height: 667)]

    /// With its rooms, the card is one height from the first settled frame:
    /// the ✕, the header, the fee row, the signing account and the confirm
    /// are where they were at every step of every walk.
    @Test(arguments: sizes)
    func nothingMovesForWhatTheFeeCardSays(size: CGSize) async throws {
        for loc in [zh, en] {
            for (name, steps) in Self.walks {
                let at = try await walk(steps, size: size, loc: loc, rooms: rooms(loc))
                print("MEASURE signing-fee after \(loc.resolvedLanguage) \(Int(size.width))x\(Int(size.height)) [\(name)] "
                    + zip(steps, at).map { "\($0.rawValue): \($1)" }.joined(separator: " → "))
                for (index, places) in at.enumerated() {
                    #expect(places == at[0],
                            "\(loc.resolvedLanguage) \(Int(size.width)) [\(name)] \(steps[index].rawValue): \(places) — was \(at[0])")
                }
            }
        }
    }

    /// For a person to look at (`VELA_TEST_SHOTS`): the sheet in each state.
    @Test func picturesOfTheFeeCardInEachState() async throws {
        guard ProcessInfo.processInfo.environment["VELA_TEST_SHOTS"]?.isEmpty == false else { return }
        for step in [Step.estimating, .priced, .cannotPay, .failed, .offline] {
            let sheet = SigningSheet(model: model(step, loc: zh, rooms: rooms(zh)), onClose: {}, onRefreshFee: {})
                .themed(.light)
            let hosted = HostedScreen(sheet)
            _ = try await hosted.read()
            try await hosted.save("signing-fee-\(step.rawValue)")
            hosted.close()
        }
    }

    /// What it was: with no room kept, the header and the confirm are still
    /// (they are pinned) and the signing account moves under the card.
    @Test(arguments: sizes)
    func withNoRoomsTheSigningAccountMovesUnderTheCard(size: CGSize) async throws {
        for (name, steps) in Self.walks {
            let at = try await walk(steps, size: size, loc: zh, rooms: nil)
            print("MEASURE signing-fee before zh \(Int(size.width))x\(Int(size.height)) [\(name)] "
                + zip(steps, at).map { "\($0.rawValue): \($1)" }.joined(separator: " → "))
            for places in at {
                #expect(places.close == at[0].close && places.header == at[0].header, "[\(name)] the pinned header moved")
                #expect(places.confirm == at[0].confirm, "[\(name)] the pinned confirm moved")
                #expect(places.fee == at[0].fee, "[\(name)] the fee row itself moved")
            }
            // Every walk but the clean one moves it: what the rooms are for.
            let moved = Set(at.map(\.signer)).count > 1
            #expect(moved == (name != "priced"), "[\(name)] the signing account at \(at.map(\.signer))")
        }
    }

    /// The live builder names the rooms on every on-chain fee, in every state
    /// of the fee — and the same ones, so the room never changes size.
    @Test func theLiveBuilderNamesTheSameRoomsInEveryStateOfTheFee() throws {
        let context = SigningLive.Context(
            loc: zh, chainName: "Gnosis", chainDot: .clear, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: "0x1111111111111111111111111111111111111111"
        )
        let transaction = try CoreJSON.decode(
            ClearSigningViewWire.self, from: CoreJSON.object(ClearSigningCore().view())
        )
        let expected = FeeRooms(
            speedNote: zh.t("send.feeSpeedFree"),
            reasons: [zh.t("componentsUi.gas.noCoinPays"), zh.t("send.warnInsufficientGas", vars: ["sym": "xDAI"])]
        )
        // No fee view yet, a failure, a retry: the same rooms.
        #expect(SigningLive.feeRooms(clear: transaction, fee: nil, speed: true, context: context) == expected)
        for scene in FeeCoreScene.allCases {
            for json in [scene.views()?.failed, scene.views()?.retrying].compactMap({ $0 }) {
                let fee = try CoreJSON.decode(FeeViewWire.self, from: CoreJSON.object(json))
                #expect(SigningLive.feeRooms(clear: transaction, fee: fee, speed: true, context: context) == expected,
                        "\(scene): the fee card's rooms changed")
            }
        }
        // No speed control, no note to keep room for.
        #expect(SigningLive.feeRooms(clear: transaction, fee: nil, speed: false, context: context)?.speedNote == nil)
        // Each room is a real sentence of the corpus.
        for line in expected.reasons + [expected.speedNote ?? ""] {
            #expect(!line.isEmpty && !line.contains("componentsUi.") && !line.contains("send."))
        }
    }
}
