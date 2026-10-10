//
//  SigningVerdictPlaceTests.swift
//  VelaWalletTests
//
//  The simulation verdict's place on the signing sheet (PR 3 final note F2).
//
//  The iOS sheet kept no room for the verdict: it was appended to the form
//  when the simulation answered, a moment after the sheet opened, and the fee
//  row, the signing account and the confirm rode down by its height under a
//  thumb already on its way to the button.
//
//  Now a site's transaction keeps ONE place for it from the first frame — a
//  balance card of two rows and half a third — holding "Checking…" while the
//  simulation is out, then whatever it found; a verdict taller than the place
//  scrolls inside it.
//
//  The real `SigningSheet`, hosted in a window, stepped through every verdict
//  kind; the confirm's position is read from the tree an assistive client
//  gets. Each walk is made twice: as the sheet was (the verdict appended) and
//  as it is, and the numbers are printed as `MEASURE` lines.
//

import Foundation
import Observation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct SigningVerdictPlaceTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"

    // MARK: - The verdicts, from the production builder

    private func context(
        _ judgments: [[String: Any]]?, _ simulation: SigningController.Simulation
    ) throws -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: me, origin: "https://app.example",
            sim: try judgments.map {
                try CoreJSON.decode(TrustSimViewWire.self, from: ["ready": true, "judgments": $0])
            },
            simulation: simulation
        )
    }

    private enum Verdict: String, CaseIterable {
        case out, send, swap, three, unverified, nothing, caution, danger
    }

    /// The judged view as the real `token_trust` core writes it.
    private func context(core sim: TrustSimViewWire?) throws -> SigningLive.Context {
        let judged: TrustSimViewWire = try #require(sim, "the core projected no simulation view")
        return SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: me, origin: "https://app.example",
            sim: judged, simulation: .answered
        )
    }

    private static let usdc: [String: Any] = [
        "type": "erc20_trusted", "token": "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83",
        "delta": "2500000", "symbol": "USDC", "decimals": 6, "in_trusted_set": true,
    ]

    private func context(_ verdict: Verdict) throws -> SigningLive.Context {
        let native: [String: Any] = ["type": "native", "delta": "-1500000000000000000"]
        let unknown: [String: Any] = [
            "type": "erc20_unverified", "token": "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a", "delta": "123456789",
        ]
        let weth: [String: Any] = [
            "type": "erc20_trusted", "token": "0x6a023ccd1ff6f2045c3309768ead9e68f978f6e1",
            "delta": "-40000000000000000", "symbol": "WETH", "decimals": 18,
        ]
        switch verdict {
        case .out: return try context(nil, .pending)
        case .send: return try context([native], .answered)
        case .swap: return try context([native, Self.usdc], .answered)
        case .three: return try context([native, weth, Self.usdc], .answered)
        case .unverified: return try context([native, unknown], .answered)
        // A check under which nothing moves, as the core says it.
        case .nothing: return try context(core: TrustCoreScene.nothing(chainId: 100))
        case .caution:
            return try context(nil, .notice(
                risk: "caution", key: "componentsUi.signing.simUnavailableWarning", reason: nil))
        case .danger:
            // The longest reason the core prints: 64 characters.
            return try context(nil, .notice(
                risk: "danger", key: "componentsUi.signing.simWillFailReason",
                reason: String(repeating: "ERC20: transfer amount exceeds balance ", count: 2).prefix(64).description))
        }
    }

    private func verdictBlocks(_ verdict: Verdict) throws -> [SigningBlock] {
        SigningLive.balanceBlocks(isTransaction: true, context: try context(verdict))
    }

    // MARK: - The rule: who keeps a place, and what stands in it

    private func plainSend() throws -> ClearSigningViewWire {
        let params: [[String: Any]] = [[
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": "0x14d1120d7b160000",
        ]]
        let call = SigningController.firstCall(paramsJson: try paramsJson(params))
        let result = try ClearSigningCore().dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": call?.to as Any? ?? NSNull(), "data": call?.data as Any? ?? NSNull(),
            "value": call?.value as Any? ?? NSNull(),
            "chain_id": 100, "locale": SigningController.defaultLocale,
        ]))
        return try CoreJSON.decode(ClearSigningViewWire.self,
                                   from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:])
    }

    private func paramsJson(_ params: [[String: Any]]) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: params), as: UTF8.self)
    }

    private func sheet(
        _ verdict: Verdict, method: String = "eth_sendTransaction", own: Bool = false
    ) throws -> SigningModel {
        let params: [[String: Any]] = [[
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": "0x14d1120d7b160000",
        ]]
        var request = SigningController.Incoming(
            id: "r", method: method,
            paramsJson: method == "eth_sendTransaction" ? try paramsJson(params) : #"["0x68656c6c6f","0x88cca0eedbf2c4426110bbfc998f048689266894"]"#,
            origin: "https://app.example", transportId: own ? SigningLive.walletTransport : "tab-1", chainId: 100
        )
        request.firstParty = own
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request,
            sign: .empty, clear: try plainSend(), guard: .empty, fee: nil, context: try context(verdict)
        )
    }

    /// A site's transaction keeps the place from its first frame, and what
    /// stands in it is never nothing: "Checking…" in the balance card's own
    /// outline while the simulation is out, then exactly the blocks the
    /// verdict builder wrote.
    @Test func aSitesTransactionKeepsThePlaceFromItsFirstFrame() throws {
        for verdict in Verdict.allCases {
            let model = try sheet(verdict)
            let place = try #require(model.verdictPlace, "\(verdict): no place is kept")
            let inner = try #require(model.formItems.compactMap { item -> [SigningBlock]? in
                if case .verdict(let inner) = item { return inner }
                return nil
            }.first, "\(verdict): the form draws no place")
            #expect(model.formItems.filter { $0.id == SigningFormItem.verdictId }.count == 1)
            #expect(inner.count == 1, "\(verdict): one thing is said in it")
            if verdict == .out {
                #expect(place.count == 0)
                guard case .balances(let title, let rows, let note, let tone)? = inner.first else {
                    Issue.record("the placeholder is not the balance card's outline")
                    continue
                }
                #expect(title == "Balance changes" && rows.isEmpty && note == "Checking…" && tone == .neutral)
            } else {
                #expect(place.count == 1)
                #expect(inner.map(\.id) == (try verdictBlocks(verdict)).map(\.id))
            }
            // The place holds the verdict and nothing else: every other
            // block is drawn outside it, in its old order.
            let outside = model.formItems.compactMap { item -> String? in
                if case .block(let block) = item { return block.id }
                return nil
            }
            let flat = model.blocks.map(\.id)
            #expect(outside == Array(flat[..<place.at]) + Array(flat[(place.at + place.count)...]))
        }
        // In the reader's language, from the corpus: two existing keys.
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        guard case .balances(let title, _, let note, _) = SigningLive.pendingVerdict(zh) else {
            Issue.record("the placeholder is not a balance card")
            return
        }
        #expect(title == zh.t("componentsUi.signing.balanceChangesTitle") && note == "正在检查…")
    }

    /// A message moves nothing and has no verdict to wait for: no place. The
    /// wallet's own request keeps none either — its usual verdict is a
    /// folded technical row (issue #314), not a card.
    @Test func aMessageAndTheWalletsOwnRequestKeepNoPlace() throws {
        let message = try sheet(.out, method: "personal_sign")
        #expect(message.verdictPlace == nil)
        #expect(!message.formItems.contains { $0.id == SigningFormItem.verdictId })

        let own = try sheet(.nothing, own: true)
        #expect(own.dappOwn && own.verdictPlace == nil)
        #expect(!own.formItems.contains { $0.id == SigningFormItem.verdictId })
        // A drawn board keeps none: its blocks are a picture.
        #expect(SigningFixtures.build(.cs23, loc: loc).verdictPlace == nil)
    }

    /// "No asset changes" is the core's line, drawn exactly when the judged
    /// view carries its key (item 3) — for a check that moved nothing, and
    /// for one whose every move was a zero. It was this shell's own case
    /// ("no row came out") and its own sentence, "No assets leave your
    /// wallet", which the corpus no longer has.
    @Test func noAssetChangesIsTheCoresLineAndTheCoreSaysWhen() throws {
        let key = "componentsUi.signing.simResultNoChange"
        #expect(loc.t(key) == "No asset changes")
        #expect(!loc.t("componentsUi.signing.balanceNoAssetsMove").contains("leave your wallet"),
                "the retired sentence is still in the corpus")

        let nothing = try #require(TrustCoreScene.nothing(chainId: 100))
        #expect(nothing.ready && nothing.judgments.isEmpty && nothing.noChangeKey == key)
        let zero = try #require(TrustCoreScene.sim(
            [["kind": "native", "token": NSNull(), "delta": "0"]], chainId: 100))
        #expect(zero.noChangeKey == key, "a move of zero is not a move: \(zero)")
        for sim in [nothing, zero] {
            guard case .balances(let title, let rows, let note, let tone)? =
                SigningLive.balanceBlocks(isTransaction: true, context: try context(core: sim)).first
            else {
                Issue.record("no card")
                continue
            }
            #expect(title == "Balance changes" && rows.isEmpty && note == "No asset changes" && tone == .neutral)
        }
        // Something moves: no key, and no such line.
        let moving = try #require(TrustCoreScene.tall(chainId: 100))
        #expect(moving.noChangeKey == nil && moving.judgments.count == 4)
        guard case .balances(_, let rows, let note, let tone)? =
            SigningLive.balanceBlocks(isTransaction: true, context: try context(core: moving)).first
        else {
            Issue.record("no card")
            return
        }
        #expect(rows.map(\.symbol) == ["xDAI", "WETH", "USDC", "Unverified token"])
        #expect(note == loc.t("componentsUi.signing.unverifiedWarning") && tone == .caution)

        // The key decides, not this shell: the same answer in another
        // language reads that language's line.
        var zh = try context(core: nothing)
        zh = SigningLive.Context(
            loc: Loc(overrideTag: "zh", preferredLanguages: []), chainName: zh.chainName, chainDot: zh.chainDot,
            nativeSymbol: zh.nativeSymbol, walletName: zh.walletName, walletAddress: zh.walletAddress,
            sim: nothing, simulation: .answered
        )
        #expect(SigningLive.noChangeLine(zh) == zh.loc.t(key))
        // Not while the simulation is out, and not under a notice.
        #expect(SigningLive.noChangeLine(try context(nil, .pending)) == nil)
        var noticed = try context(core: nothing)
        noticed.simulation = .notice(risk: "caution", key: "componentsUi.signing.simUnavailableWarning", reason: nil)
        #expect(SigningLive.noChangeLine(noticed) == nil)
    }

    /// No row to draw and NO key is not "nothing moves": a move this sheet
    /// cannot write (or a view from before the key) gets the core's
    /// could-not-check line, in caution — never an empty card, and never
    /// the quiet line.
    @Test func noRowsAndNoKeyIsCouldNotCheckNeverAnEmptyCard() throws {
        let unreadable = try context([["type": "native", "delta": "not-a-number"]], .answered)
        let keyless = try context([], .answered)
        for (name, context) in [("a move nobody can write", unreadable), ("a view without the key", keyless)] {
            #expect(context.sim?.noChangeKey == nil)
            let blocks = SigningLive.balanceBlocks(isTransaction: true, context: context)
            guard blocks.count == 1, case .warning(let tone, let text)? = blocks.first else {
                Issue.record("\(name): \(blocks.map(\.id))")
                continue
            }
            #expect(tone == .caution, "\(name)")
            #expect(text == loc.t("componentsUi.signing.simUnavailableWarning"), "\(name)")
        }
    }

    // MARK: - Nothing moves

    @MainActor
    @Observable
    final class Box {
        var model: SigningModel
        init(_ model: SigningModel) { self.model = model }
    }

    struct Host: View {
        let box: Box
        var body: some View {
            SigningSheet(model: box.model, onClose: {}, onRefreshFee: {})
                .themed(.light)
        }
    }

    private static let automation: Void = {
        guard let library = dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW) else { return }
        for name in ["_AXSSetAutomationEnabled", "_AXSApplicationAccessibilitySetEnabled"] {
            if let symbol = dlsym(library, name) {
                typealias Switch = @convention(c) (Int32) -> Void
                unsafeBitCast(symbol, to: Switch.self)(1)
            }
        }
    }()

    private struct Element {
        let label: String
        let id: String
        let frame: CGRect
    }

    private static func identifier(of object: NSObject) -> String {
        let getter = #selector(getter: UIAccessibilityIdentification.accessibilityIdentifier)
        guard object.responds(to: getter) else { return "" }
        return object.perform(getter)?.takeUnretainedValue() as? String ?? ""
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        if object.isAccessibilityElement {
            found.append(Element(label: object.accessibilityLabel ?? "",
                                 id: Self.identifier(of: object), frame: object.accessibilityFrame))
        }
        if let children = object.accessibilityElements {
            for child in children { collect(child, depth: depth + 1, into: &found) }
        } else if case let count = object.accessibilityElementCount(), count != NSNotFound, count > 0 {
            for index in 0..<count {
                if let child = object.accessibilityElement(at: index) {
                    collect(child, depth: depth + 1, into: &found)
                }
            }
        } else if let view = object as? UIView {
            for sub in view.subviews { collect(sub, depth: depth + 1, into: &found) }
        }
    }

    private func shown(_ view: UIView) async throws {
        view.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(200))
        _ = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in
            view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        try await Task.sleep(for: .milliseconds(100))
    }

    /// Where the fee row's refresh and the confirm sit, once two reads agree.
    private func settled(_ view: UIView) async throws -> (fee: CGFloat, confirm: CGFloat, tree: [Element]) {
        var last: [CGFloat]?
        var tree: [Element] = []
        for _ in 0..<40 {
            tree = []
            collect(view as Any, depth: 0, into: &tree)
            if let fee = tree.first(where: { $0.id == "signing.fee.refresh" }),
               let confirm = tree.first(where: { $0.id == "signing.confirm" }) {
                let frame = [fee.frame.minY, confirm.frame.minY]
                if frame == last { return (frame[0], frame[1], tree) }
                last = frame
            }
            try await shown(view)
            try await Task.sleep(for: .milliseconds(50))
        }
        let frame = try #require(last, "the fee row's refresh and the confirm are in the tree")
        return (frame[0], frame[1], tree)
    }

    /// The drawn send (cs1) with `verdict` on it — in the place (`kept`), or
    /// appended to the form as the sheet used to do it.
    private func model(_ verdict: Verdict, kept: Bool) throws -> SigningModel {
        let base = SigningFixtures.build(.cs1, loc: loc)
        let landed = try verdictBlocks(verdict)
        var model = SigningModel(
            id: base.id, dapp: base.dapp, network: base.network, blocks: base.blocks + landed,
            tech: base.tech, techOpen: false,
            fee: .onchain(label: loc.t("componentsUi.gas.networkFee"), value: "~0.0021 xDAI", selector: nil,
                          tappable: false),
            signer: base.signer, confirm: (action: base.confirm?.action ?? "Confirm", enabled: true),
            confirmBlockLine: nil, panelTitle: base.panelTitle
        )
        model.closeLabel = loc.t("common.close")
        model.feeRefresh = FeeRefreshModel(label: loc.t("send.feeRefresh"), refreshing: false)
        if kept {
            model.verdictPlace = SigningVerdictPlace(
                at: base.blocks.count, count: landed.count, pending: SigningLive.pendingVerdict(loc)
            )
        }
        return model
    }

    /// One live sheet, `width` points wide, stepped through `verdicts`: where
    /// the confirm sits under each, and what is said.
    private func walk(
        _ verdicts: [Verdict], kept: Bool, width: CGFloat = 390, height: CGFloat = 1_400
    ) async throws -> [(verdict: Verdict, fee: CGFloat, confirm: CGFloat, tree: [Element])] {
        _ = Self.automation
        let box = Box(try model(verdicts[0], kept: kept))
        let host = UIHostingController(rootView: Host(box: box))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: width, height: height))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        var out: [(Verdict, CGFloat, CGFloat, [Element])] = []
        for verdict in verdicts {
            box.model = try model(verdict, kept: kept)
            try await shown(host.view)
            let at = try await settled(host.view)
            out.append((verdict, at.fee, at.confirm, at.tree))
        }
        return out
    }

    /// The measurement. As it was: the confirm under each verdict sits lower
    /// than it did while the simulation was out. As it is: the confirm, and
    /// the fee row above it, have ONE position — with no verdict yet, with
    /// each kind, and with the two that are taller than the place.
    @Test func theConfirmHasOnePositionUnderEveryVerdict() async throws {
        let order: [Verdict] = [.out, .send, .swap, .three, .unverified, .nothing, .caution, .danger, .out]
        for width in [390.0, 375.0] as [CGFloat] {
            let before = try await walk(order, kept: false, width: width)
            let after = try await walk(order, kept: true, width: width)
            let open = try #require(before.first).confirm
            for (was, now) in zip(before, after) {
                print("MEASURE signing-verdict w=\(Int(width)) \(was.verdict.rawValue): confirm.y before \(was.confirm)"
                    + " (\(was.confirm - open >= 0 ? "+" : "")\(was.confirm - open)) after \(now.confirm)"
                    + " | fee.y before \(was.fee) after \(now.fee)")
            }
            // As it was: every verdict moved the confirm.
            for step in before where step.verdict != .out {
                #expect(step.confirm > open + 20, "w\(Int(width)) \(step.verdict): the old sheet did not move (\(step.confirm) vs \(open))")
            }
            // As it is: not a point, under any of them.
            let kept = try #require(after.first)
            for step in after {
                #expect(step.confirm == kept.confirm,
                        "w\(Int(width)) \(step.verdict): confirm at \(step.confirm), was \(kept.confirm)")
                #expect(step.fee == kept.fee, "w\(Int(width)) \(step.verdict): fee row at \(step.fee), was \(kept.fee)")
            }
        }
    }

    /// The place is never blank, and what stands in it is said aloud: the
    /// quiet "Checking…" while the simulation is out, then the verdict's own
    /// words.
    @Test func thePlaceIsNeverBlankAndSaysWhatItHolds() async throws {
        let walked = try await walk([.out, .send, .nothing, .caution], kept: true)
        let said = walked.map { step in step.tree.map(\.label) }
        #expect(said[0].contains("Balance changes") && said[0].contains("Checking…"))
        #expect(said[1].contains("Balance changes") && said[1].contains("xDAI") && !said[1].contains("Checking…"))
        #expect(said[2].contains("No asset changes"))
        #expect(said[2].contains(loc.t("componentsUi.signing.simResultNoChange")))
        #expect(said[3].contains(loc.t("componentsUi.signing.simUnavailableWarning")))
    }

    /// The usual verdict lands whole — a send's row, a swap's two — and one
    /// taller than the place is cut at the fold and scrolls inside it: its
    /// last line starts below the place's bottom edge, not below a taller
    /// sheet.
    @Test func aTallVerdictScrollsInsideThePlace() async throws {
        let walked = try await walk([.swap, .three, .unverified], kept: true)
        func top(_ step: Int, _ label: String) throws -> CGFloat {
            try #require(walked[step].tree.first { $0.label == label }, "\(label) is in the tree").frame.minY
        }
        func bottom(_ step: Int, _ label: String) throws -> CGFloat {
            try #require(walked[step].tree.first { $0.label == label }, "\(label) is in the tree").frame.maxY
        }
        let fee = walked[0].fee
        // A swap: both rows above the fee row, whole.
        #expect(try bottom(0, "USDC") < fee)
        // Three coins: the third row begins inside the place (it is cut, not
        // hidden) and the fee row has not moved for it.
        let third = try top(1, "USDC")
        #expect(third < fee, "the third row is hidden entirely: starts at \(third), fee row at \(fee)")
        #expect(walked[1].fee == fee && walked[2].fee == fee)
        // The place scrolls: there is a scroll view inside the sheet's own.
        #expect(try bottom(1, "USDC") > third)
    }
}
