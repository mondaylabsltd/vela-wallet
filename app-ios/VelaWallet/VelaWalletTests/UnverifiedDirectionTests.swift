//
//  UnverifiedDirectionTests.swift
//  VelaWalletTests
//
//  An unverified token is a direction, never a figure (PR 3, fix B).
//
//  Android's signing sheet printed 「未验证代币 +5,000,000,000,000,000,000,000.00」
//  for an unverified ERC-20 balance change: the simulation's raw delta,
//  which is whatever the site being signed for chose to emit. This sheet
//  drew the sign only — but its judgment still HELD the figure, "to read the
//  sign from". The core no longer hands it over, and this shell's wire type
//  can no longer hold it.
//
//  The real `token_trust` core judges every delta here; the sheet is the
//  real `SigningSheet`, hosted, and read through the tree an assistive
//  client gets.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct UnverifiedDirectionTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    /// A token nobody vouches for.
    private let token = TrustCoreScene.unknown
    /// 5,000 × 10¹⁸ — the figure the site chose.
    private let lure = "5000000000000000000000"
    /// Every way that figure could be written on a sheet.
    private let lureForms = ["5000", "5,000", "5 000", "5.000", "5\u{202F}000", "5\u{00A0}000", "5'000", "5e21"]

    private var label: String { loc.t("componentsUi.signing.balanceUnverifiedToken") }
    private var warning: String { loc.t("componentsUi.signing.unverifiedWarning") }

    /// The real core's judgment of `deltas` on Gnosis; nobody names `token`.
    private func judged(_ deltas: [[String: Any]]) throws -> TrustSimViewWire {
        try #require(TrustCoreScene.sim(deltas, chainId: 100), "the core projected no simulation view")
    }

    private func unverified(_ delta: String) -> [String: Any] {
        ["kind": "erc20", "token": token, "delta": delta]
    }

    private func context(_ sim: TrustSimViewWire) -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: me, origin: "https://app.example",
            sim: sim, simulation: .answered
        )
    }

    private func card(_ sim: TrustSimViewWire) throws -> (rows: [BalanceDeltaRow], note: String?, tone: SigningTone) {
        let blocks = SigningLive.balanceBlocks(isTransaction: true, context: context(sim))
        guard blocks.count == 1, case .balances(_, let rows, let note, let tone)? = blocks.first else {
            throw Missing(what: "a balance card: \(blocks.map(\.id))")
        }
        return (rows, note, tone)
    }

    private struct Missing: Error { let what: String }

    // MARK: - The judgment: a direction, and nothing to print

    /// The core hands over which way the token moves, and this shell's type
    /// has nowhere to keep the figure: not in the decoded judgment, and not
    /// in what the approve hands back for the record.
    @Test func theJudgmentIsADirectionAndHoldsNoFigure() throws {
        let cases: [(String, TrustSimDirectionWire)] = [
            (lure, .in), ("-" + lure, .out), ("0", .still), ("not-a-number", .unreadable),
        ]
        for (delta, direction) in cases {
            let sim = try judged([unverified(delta)])
            #expect(sim.ready)
            #expect(sim.judgments == [.erc20Unverified(token: token, direction: direction)], "\(delta): \(sim.judgments)")
            let held = String(reflecting: sim)
            for form in lureForms { #expect(!held.contains(form), "\(delta): the view holds the figure: \(held)") }
            let wire = try #require(sim.judgments.first?.wire)
            #expect(Set(wire.keys) == ["type", "token", "direction"], "\(wire)")
            #expect(wire["direction"] as? String == direction.rawValue)
            #expect(wire["delta"] == nil, "no figure goes back to the core for the record")
        }
    }

    /// A direction a newer core might add is one this build cannot state:
    /// the row with its caution and no sign, never a view that fails.
    @Test func aDirectionThisBuildHasNeverHeardOfIsUnreadable() throws {
        let judgments = try CoreJSON.decoder.decode([TrustSimJudgmentWire].self, from: Data(#"""
        [{"type":"erc20_unverified","token":null,"direction":"sideways"}]
        """#.utf8))
        #expect(judgments == [.erc20Unverified(token: nil, direction: .unreadable)])
    }

    /// A judgment in the shape from before the fix — a `delta`, no
    /// `direction` — still decodes: the view it sits in is not lost, its
    /// figure is not read, and it is drawn with its caution and no sign.
    /// (A stored record's lines never come this way — `TxRecords.toWire`
    /// hands them to the core, which reads both shapes:
    /// `DappActivityTests.aStoredUnverifiedLineWithAnOldFigureReadsAsItsDirection`.)
    @Test func anOldShapeJudgmentDecodesWithoutItsFigure() throws {
        let sim = try CoreJSON.decoder.decode(TrustSimViewWire.self, from: Data(#"""
        {"ready":true,"judgments":[
          {"type":"native","delta":"-1500000000000000000"},
          {"type":"erc20_unverified","token":"\#(token)","delta":"\#(lure)"}]}
        """#.utf8))
        #expect(sim.judgments.count == 2, "the old line did not cost the view")
        #expect(sim.judgments[1] == .erc20Unverified(token: token, direction: .unreadable))
        let held = String(reflecting: sim.judgments[1])
        for form in lureForms { #expect(!held.contains(form), "the figure was kept: \(held)") }
        #expect(sim.judgments[1].wire["delta"] == nil)

        let drawn = try card(sim)
        #expect(drawn.rows.map(\.symbol) == ["xDAI", label])
        #expect(drawn.rows[1].delta == "", "no direction was stated, so none is drawn")
        #expect(drawn.rows[1].tone == .caution)
        #expect(drawn.note == warning && drawn.tone == .caution)
    }

    // MARK: - The row: the label and a sign

    /// An inflow draws the label and "+", an outflow "−" (U+2212), a move of
    /// nothing no row, and a figure the core could not read the row with its
    /// caution and no sign — each from the direction alone.
    @Test func theRowIsItsLabelAndASignFromTheDirectionAlone() throws {
        let native: [String: Any] = ["kind": "native", "token": NSNull(), "delta": "-1500000000000000000"]

        let into = try card(judged([unverified(lure)]))
        #expect(into.rows.count == 1)
        #expect(into.rows[0].symbol == label && into.rows[0].delta == "+" && into.rows[0].tone == .caution)
        #expect(into.note == warning && into.tone == .caution)

        let out = try card(judged([unverified("-" + lure)]))
        #expect(out.rows.count == 1)
        #expect(out.rows[0].symbol == label && out.rows[0].delta == "\u{2212}" && out.rows[0].tone == .caution)
        #expect(out.note == warning)

        // A zero beside a real move: no row, and no caution about a row
        // that is not there.
        let still = try card(judged([native, unverified("0")]))
        #expect(still.rows.map(\.symbol) == ["xDAI"])
        #expect(still.note == nil && still.tone == .neutral)
        // A zero alone is the core's "No asset changes".
        let nothing = try card(judged([unverified("0")]))
        #expect(nothing.rows.isEmpty && nothing.note == loc.t("componentsUi.signing.simResultNoChange"))

        let unreadable = try card(judged([native, unverified("not-a-number")]))
        #expect(unreadable.rows.map(\.symbol) == ["xDAI", label])
        #expect(unreadable.rows[1].delta == "" && unreadable.rows[1].tone == .caution)
        #expect(unreadable.note == warning, "what cannot be read is never \"nothing moves\"")

        for row in into.rows + out.rows + unreadable.rows.suffix(1) {
            #expect(!row.delta.contains { $0.isNumber }, "a digit in the unverified row: \(row.delta)")
        }
    }

    // MARK: - The sheet

    /// A site's transaction whose simulation moved 5,000 × 10¹⁸ of a token
    /// nobody vouches for, through the production builder.
    private func sheet(_ sim: TrustSimViewWire) throws -> SigningModel {
        let params: [[String: Any]] = [[
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": "0x14d1120d7b160000",
        ]]
        let request = SigningController.Incoming(
            id: "r", method: "eth_sendTransaction",
            paramsJson: String(decoding: try JSONSerialization.data(withJSONObject: params), as: UTF8.self),
            origin: "https://app.example", transportId: "tab-1", chainId: 100
        )
        let read = try ClearSigningCore().dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": "0x14d1120d7b160000",
            "chain_id": 100, "locale": SigningController.defaultLocale,
        ]))
        let clear = try CoreJSON.decode(
            ClearSigningViewWire.self, from: CoreJSON.object(read)["view"] as? [String: Any] ?? [:]
        )
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request,
            sign: .empty, clear: clear, guard: .empty, fee: nil, context: context(sim),
            gate: SignConfirmStateWire(enabled: true, block: nil, key: nil)
        )
    }

    /// The drawn sheet says "Unverified token" and "+", and no word on it —
    /// the row, the card, anything else — holds a digit of the figure the
    /// simulation carried. An outflow says "−".
    @Test func theDrawnSheetSaysTheLabelAndThePlusAndNoDigitOfTheFigure() async throws {
        let into = try judged([unverified(lure)])
        let probe = SigningSheetProbe(try sheet(into))
        defer { probe.close() }
        let seen = try await probe.read()
        #expect(seen.said(label), "\(seen.words)")
        #expect(seen.said("+"), "the inflow's sign is on the sheet: \(seen.words)")
        #expect(seen.said(warning), "\(seen.words)")
        #expect(!seen.said("\u{2212}"))
        for word in seen.words {
            for form in lureForms {
                #expect(!word.contains(form), "the site's figure is on the sheet: \"\(word)\"")
            }
        }
        // The row itself: the sign stands on the label's line, and it is
        // the only thing there.
        let name = try #require(seen.tree.first { $0.label == label })
        let onItsLine = seen.tree.filter {
            $0.label != label && abs($0.frame.midY - name.frame.midY) < 4
        }
        #expect(onItsLine.map(\.label) == ["+"], "\(onItsLine.map(\.label))")
        print("MEASURE unverified-in words=\(seen.words)")

        let out = try await probe.show(try sheet(try judged([unverified("-" + lure)])))
        #expect(out.said(label) && out.said("\u{2212}") && !out.said("+"), "\(out.words)")
        for word in out.words {
            for form in lureForms {
                #expect(!word.contains(form), "the site's figure is on the sheet: \"\(word)\"")
            }
        }
        print("MEASURE unverified-out words=\(out.words)")
    }
}
