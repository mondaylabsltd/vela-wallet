//
//  TxDeleteActionTests.swift
//  VelaWalletTests
//
//  「删除记录」 is not the heaviest thing on a transaction's detail (device walk
//  2026-10-11: a full-width button filled with the error colour, under an
//  outlined "View on explorer").
//
//  The real `TxDetailBody` — the one view a transfer's detail and a dApp
//  row's detail both draw — hosted, read two ways: the tree for where the
//  delete and the explorer control stand and how wide they are, and the
//  window's pixels for what the delete is MADE of: words on the sheet's own
//  ground, not a filled block.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct TxDeleteActionTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    private func drawn(_ state: FlowStateId, _ loc: Loc) throws -> TxDetailModel {
        guard case .txDetail(var model)? = WalletFlowFixtures.build(state, loc: loc).sheet else {
            throw Unbuilt()
        }
        model.deleteLabel = loc.t("history.deleteRecord")
        return model
    }

    private struct Unbuilt: Error {}

    private struct Seen {
        let delete: CGRect
        let explorer: CGRect?
        /// Of the delete's own box: how much is the error colour's red, and
        /// whether its edges are the sheet's ground.
        let redShare: Double
        let edgesAreGround: Bool
    }

    private static let width: CGFloat = 390
    private static let margin = Tokens.Space.s24

    private func look(_ model: TxDetailModel) async throws -> Seen {
        let page = ScrollView {
            TxDetailBody(model: model, onExplorer: {}, onDelete: {})
                .padding(Self.margin)
        }
        .background(Theme(scheme: .light).bgBase)
        .themed(.light)
        let hosted = HostedScreen(page, size: CGSize(width: Self.width, height: 844))
        defer { hosted.close() }
        let tree = try await hosted.read { $0.element(TxDeleteAction.testId) != nil }
        let delete = try #require(tree.element(TxDeleteAction.testId), "the delete is on the page").frame
        let explorer = model.viewOnExplorer.flatMap { tree.labelled($0)?.frame }
        let picture = try await hosted.picture(settled: true)
        let ground = picture.pixel(4, Int(delete.midY))
        var red = 0, all = 0
        for y in Int(delete.minY) + 1..<Int(delete.maxY) - 1 {
            for x in Int(delete.minX) + 1..<Int(delete.maxX) - 1 {
                let p = picture.pixel(x, y)
                all += 1
                if p.r > 150, p.r - p.g > 60, p.r - p.b > 60 { red += 1 }
            }
        }
        // The box's left and right edges, inside it: ground, where a filled
        // or bordered button has its own colour.
        let left = CGRect(x: delete.minX + 1, y: delete.minY + 4, width: 3, height: delete.height - 8)
        let right = CGRect(x: delete.maxX - 4, y: delete.minY + 4, width: 3, height: delete.height - 8)
        let edges = picture.strangers(in: left, to: ground) + picture.strangers(in: right, to: ground)
        return Seen(delete: delete, explorer: explorer, redShare: Double(red) / Double(max(all, 1)),
                    edgesAreGround: edges == 0)
    }

    /// A settled transfer: the delete is words in the error colour, its own
    /// width, a finger tall, under the explorer control — which keeps the row.
    @Test(arguments: [FlowStateId.a2, .a3])
    func theDeleteIsAQuietTextActionUnderTheUsefulOne(state: FlowStateId) async throws {
        for loc in [zh, en] {
            let model = try drawn(state, loc)
            #expect(model.viewOnExplorer != nil, "the drawn detail has an explorer control")
            let seen = try await look(model)
            let row = Self.width - 2 * Self.margin
            let explorer = try #require(seen.explorer, "the explorer control is on the page")

            #expect(seen.delete.minY >= explorer.maxY, "the delete is not under the explorer control")
            #expect(seen.delete.width < row * 0.6, "the delete is \(seen.delete.width) of a \(row) row: a button, not words")
            #expect(explorer.width > row * 0.9, "the useful control lost the row: \(explorer.width)")
            #expect(seen.delete.height >= Tokens.Layout.hitTarget, "less than a finger tall: \(seen.delete.height)")
            #expect(abs(seen.delete.midX - Self.width / 2) < 1, "not centred under the row")
            // Words, not a block: a filled button is red nearly all over.
            #expect(seen.redShare > 0.005, "the words are not in the error colour (\(seen.redShare))")
            #expect(seen.redShare < 0.25, "\(Int(seen.redShare * 100)) % of the delete is red: it is filled")
            #expect(seen.edgesAreGround, "the delete has a fill or a border")
        }
    }

    /// Still pending — the "may have been sent" record: muted, as it was
    /// (spec 082 RJ18), and the same shape.
    @Test func aPendingRecordsDeleteIsTheSameShapeAndMuted() async throws {
        var model = try drawn(.a3, zh)
        model.deleteQuiet = true
        let seen = try await look(model)
        #expect(seen.redShare == 0, "a pending record's delete is red")
        #expect(seen.edgesAreGround)
        #expect(seen.delete.width < (Self.width - 2 * Self.margin) * 0.6)
        #expect(seen.delete.height >= Tokens.Layout.hitTarget)
    }

    /// A dApp row's detail draws the same control, from the same model field
    /// — and a record with nothing to open on an explorer still has it.
    @Test func aDappRowsDetailAndARecordWithNoExplorerHaveTheSameDelete() async throws {
        let (_, feed) = try Self.fixtureFeed()
        let items = FlowsLive.items(feed)
        let dapp = try #require(items.first { $0.dapp != nil }, "the shared fixture has a dApp row")
        let detail = FlowsLive.txDetail(
            dapp, record: feed.transactions.first { $0.id == dapp.id }, on: try drawn(.a2, en), loc: en
        )
        #expect(detail.deleteLabel == en.t("history.deleteRecord"), "a dApp row's detail offers the delete")
        let seen = try await look(detail)
        #expect(seen.delete.width < (Self.width - 2 * Self.margin) * 0.6)
        #expect(seen.redShare < 0.25 && seen.edgesAreGround)
        if let explorer = seen.explorer { #expect(seen.delete.minY >= explorer.maxY) }

        var bare = try drawn(.a2, en)
        bare = TxDetailModel(
            title: bare.title, status: bare.status, closeLabel: bare.closeLabel, amount: bare.amount,
            fiat: bare.fiat, positive: bare.positive, facts: bare.facts, viewOnExplorer: nil,
            deleteLabel: bare.deleteLabel
        )
        let alone = try await look(bare)
        #expect(alone.explorer == nil)
        #expect(alone.redShare < 0.25 && alone.edgesAreGround)
        #expect(alone.delete.width < (Self.width - 2 * Self.margin) * 0.6)
    }

    /// The shared privacy fixture's feed, shown — a real core's rows.
    private static func fixtureFeed() throws -> (BalanceViewWire, FeedViewWire) {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let url = repoRoot.appendingPathComponent("rust/crates/vela-core/tests/fixtures/privacy-hidden.json")
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any] ?? [:]
        let shown = json["shown"] as? [String: Any] ?? [:]
        return (
            try CoreJSON.decode(BalanceViewWire.self, from: shown["balance"] as? [String: Any] ?? [:]),
            try CoreJSON.decode(FeedViewWire.self, from: shown["feed"] as? [String: Any] ?? [:])
        )
    }
}
