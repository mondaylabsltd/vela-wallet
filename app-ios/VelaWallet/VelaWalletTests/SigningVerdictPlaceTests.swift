//
//  SigningVerdictPlaceTests.swift
//  VelaWalletTests
//
//  The simulation verdict's place on the signing sheet, and the confirm
//  under it (PR 3 final note F2; the device round's item 1).
//
//  A site's transaction keeps a place for the verdict from its first frame —
//  "Checking…" in the balance card's outline — at least as tall as the usual
//  verdict, so the usual verdict moves nothing when it lands.
//
//  The verdict is the one part of the sheet a site cannot write, so nothing
//  of it is ever under a fold. The place is a MINIMUM: a taller verdict — a
//  third and a fourth balance row, the warning under an unverified token —
//  is shown whole, the place as tall as what it holds. (It used to be one
//  height with a scroll, a fade and a clip inside it.) The confirm is not in
//  the scroll at all: it is pinned to the bottom of the sheet, at one
//  position whatever the body holds, and when the sheet outgrows the screen
//  the body scrolls under it and brings the verdict into view as it lands.
//  The header is pinned too: its ✕ is the one way to refuse, and a body that
//  scrolls by itself must not scroll the refusal out of sight.
//
//  The real `SigningSheet`, hosted in a window the size of a phone's sheet,
//  stepped through every verdict kind; positions are read from the tree an
//  assistive client gets and from the sheet's own scroll view, and printed
//  as `MEASURE` lines.
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
        case out, send, swap, three, unverified, tall, nothing, caution, danger
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
            "type": "erc20_unverified", "token": "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a", "direction": "in",
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
        // Four balance rows and the unverified-token warning; and a check
        // under which nothing moves — both the core's own views.
        case .tall: return try context(core: TrustCoreScene.tall(chainId: 100))
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

    // MARK: - The real sheet, hosted

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

    /// One node of the tree an assistive client gets: something it reads
    /// (`element`), or a place with a name of its own (the verdict's).
    private struct Element {
        let label: String
        let id: String
        let frame: CGRect
        let element: Bool
    }

    private static func identifier(of object: NSObject) -> String {
        let getter = #selector(getter: UIAccessibilityIdentification.accessibilityIdentifier)
        guard object.responds(to: getter) else { return "" }
        return object.perform(getter)?.takeUnretainedValue() as? String ?? ""
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        let id = Self.identifier(of: object)
        if object.isAccessibilityElement || !id.isEmpty {
            found.append(Element(label: object.accessibilityLabel ?? "", id: id,
                                 frame: object.accessibilityFrame, element: object.isAccessibilityElement))
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

    /// Every scroll view under `view` — the sheet has ONE, its body.
    private func scrollViews(in view: UIView) -> [UIScrollView] {
        var found: [UIScrollView] = []
        if let scroll = view as? UIScrollView { found.append(scroll) }
        for sub in view.subviews { found += scrollViews(in: sub) }
        return found
    }

    private func shown(_ view: UIView) async throws {
        view.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(200))
        _ = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in
            view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        try await Task.sleep(for: .milliseconds(100))
    }

    /// The sheet as it stands, once two reads agree on where everything is.
    private struct Seen {
        let verdict: Verdict
        let tree: [Element]
        /// The fee row's refresh control: the first thing under the place.
        let fee: CGRect
        let confirm: CGRect
        /// The header's ✕: the one way to refuse.
        let close: CGRect
        /// What stands in the verdict's place — the verdict's own frame;
        /// the room a short one leaves under it is not in it.
        let place: CGRect
        /// The body's viewport on screen — what of the body can be seen,
        /// clear of the status bar — and how far it is scrolled.
        let viewport: CGRect
        let offset: CGFloat
        let contentHeight: CGFloat
        let scrollViews: Int
        /// The sheet's own bottom edge, above the home indicator.
        let bottom: CGFloat

        /// Where `label` is said — in the verdict's place when it is said
        /// there (the drawn send has a figure and a coin of its own above).
        func frame(_ label: String) throws -> CGRect {
            let said = tree.filter { $0.element && $0.label == label }
            let inPlace = said.first { $0.frame.midY >= place.minY && $0.frame.midY <= place.maxY }
            return try #require(inPlace ?? said.first, "\(verdict): \"\(label)\" is in the tree").frame
        }

        var scrolls: Bool { contentHeight > viewport.height + 0.5 }

        /// Where the fee row and the verdict stand in the BODY, however far
        /// it is scrolled.
        var feeInBody: CGFloat { fee.minY + offset }
        var placeInBody: CGFloat { place.minY + offset }

        /// `frame` can be read whole: inside the body's viewport (to the
        /// point: the tree's frames are rounded to the screen's pixels).
        func inView(_ frame: CGRect) -> Bool {
            frame.minY >= viewport.minY - 1 && frame.maxY <= viewport.maxY + 1
        }
    }

    private func read(_ verdict: Verdict, _ view: UIView) async throws -> Seen {
        var last: [CGRect]?
        var seen: Seen?
        for _ in 0..<40 {
            var tree: [Element] = []
            collect(view as Any, depth: 0, into: &tree)
            let scrolls = scrollViews(in: view)
            if let fee = tree.first(where: { $0.id == "signing.fee.refresh" }),
               let confirm = tree.first(where: { $0.id == "signing.confirm" }),
               let close = tree.first(where: { $0.id == "signing.close" }),
               let place = tree.first(where: { $0.id == SigningVerdictRoom<EmptyView>.testId && !$0.element }),
               let body = scrolls.first {
                let frames = [fee.frame, confirm.frame, close.frame, place.frame,
                              CGRect(origin: body.contentOffset, size: body.contentSize)]
                seen = Seen(
                    verdict: verdict, tree: tree, fee: fee.frame, confirm: confirm.frame, close: close.frame,
                    place: place.frame,
                    viewport: body.convert(body.bounds, to: nil).inset(by: body.adjustedContentInset),
                    offset: body.contentOffset.y,
                    contentHeight: body.contentSize.height, scrollViews: scrolls.count,
                    bottom: view.bounds.maxY - view.safeAreaInsets.bottom
                )
                if frames == last { break }
                last = frames
            }
            try await shown(view)
            try await Task.sleep(for: .milliseconds(50))
        }
        return try #require(seen, "\(verdict): the fee row, the confirm, the ✕, the verdict's place and the body are in the tree")
    }

    /// The drawn send (cs1) through the production renderer, with `verdict`
    /// in the place the sheet keeps.
    private func model(_ verdict: Verdict) throws -> SigningModel {
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
        model.verdictPlace = SigningVerdictPlace(
            at: base.blocks.count, count: landed.count, pending: SigningLive.pendingVerdict(loc)
        )
        return model
    }

    /// A phone's sheet and a short phone's (an iPhone SE's), in points.
    private static let normal = CGSize(width: 390, height: 844)
    private static let short = CGSize(width: 375, height: 560)

    /// One live sheet of `size`, stepped through `verdicts` as a simulation
    /// lands them; `each` may handle the sheet (scroll it) after a step.
    private func walk(
        _ verdicts: [Verdict], size: CGSize,
        each: @MainActor (Seen, UIScrollView, UIView) async throws -> Void = { _, _, _ in }
    ) async throws -> [Seen] {
        _ = Self.automation
        let box = Box(try model(verdicts[0]))
        let host = UIHostingController(rootView: Host(box: box))
        let window = UIWindow(frame: CGRect(origin: .zero, size: size))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        var out: [Seen] = []
        for verdict in verdicts {
            box.model = try model(verdict)
            try await shown(host.view)
            // A landing glides into view: let it come to rest.
            try await Task.sleep(for: .milliseconds(400))
            let seen = try await read(verdict, host.view)
            out.append(seen)
            try await each(seen, try #require(scrollViews(in: host.view).first), host.view)
        }
        return out
    }

    private func measure(_ name: String, _ size: CGSize, _ step: Seen) {
        print("MEASURE \(name) \(Int(size.width))x\(Int(size.height)) \(step.verdict.rawValue):"
            + " confirm.y \(step.confirm.minY)…\(step.confirm.maxY) close.y \(step.close.minY)…\(step.close.maxY)"
            + " fee.y \(step.fee.minY)"
            + " place \(step.place.minY)…\(step.place.maxY) h=\(step.place.height)"
            + " viewport \(step.viewport.minY)…\(step.viewport.maxY) content=\(step.contentHeight) offset=\(step.offset)")
    }

    // MARK: - The confirm has one position

    /// The confirm is pinned: ONE frame — with no verdict yet, under each
    /// kind, under the tall one — whole, on the sheet, on a phone and on a
    /// short one. It used to be the last thing in the scroll, so whatever
    /// grew above it pushed it down.
    @Test func theConfirmHasOnePositionUnderEveryVerdict() async throws {
        let order: [Verdict] = [.out, .send, .swap, .three, .unverified, .tall, .nothing, .caution, .danger, .out]
        for size in [Self.normal, Self.short] {
            let walked = try await walk(order, size: size)
            let first = try #require(walked.first)
            for step in walked {
                measure("signing-confirm", size, step)
                #expect(step.confirm == first.confirm,
                        "\(Int(size.height)) \(step.verdict): confirm at \(step.confirm), was \(first.confirm)")
                #expect(step.confirm.minY >= step.viewport.maxY - 0.5,
                        "\(Int(size.height)) \(step.verdict): the confirm is not under the body")
                #expect(step.confirm.maxY <= step.bottom + 0.5 && step.confirm.height >= 44,
                        "\(Int(size.height)) \(step.verdict): the confirm is not whole on the sheet: \(step.confirm)")
                #expect(step.scrollViews == 1, "\(Int(size.height)) \(step.verdict): \(step.scrollViews) scroll views")
                // The ✕ — the one way to refuse — is pinned above the body:
                // one frame, whole, whatever the body scrolled to.
                #expect(step.close == first.close,
                        "\(Int(size.height)) \(step.verdict): the ✕ at \(step.close), was \(first.close)")
                #expect(step.close.minY >= 0 && step.close.maxY <= step.viewport.minY + 0.5 && step.close.height >= 40,
                        "\(Int(size.height)) \(step.verdict): the ✕ \(step.close) is not whole above the body \(step.viewport)")
                // A landed verdict is in view, whole — on the short screen too.
                if step.verdict != .out, step.place.height <= step.viewport.height {
                    #expect(step.inView(step.place),
                            "\(Int(size.height)) \(step.verdict): the verdict \(step.place) is not whole in \(step.viewport)")
                }
            }
        }
    }

    /// The usual verdicts move nothing: under "Checking…", "No asset
    /// changes", a one-row and a two-row card, the could-not-check line and
    /// the longest revert line, the fee row under the place — and so the
    /// account, and the confirm — sits where it sat, and the verdict starts
    /// where "Checking…" did. On a phone and at 375 pt.
    @Test func theUsualVerdictsMoveNothing() async throws {
        let usual: [Verdict] = [.out, .nothing, .send, .swap, .caution, .danger, .out]
        for size in [Self.normal, CGSize(width: 375, height: 844)] {
            let walked = try await walk(usual, size: size)
            let kept = try #require(walked.first)
            for step in walked {
                measure("signing-usual", size, step)
                #expect(step.fee == kept.fee, "\(Int(size.width)) \(step.verdict): the fee row at \(step.fee), was \(kept.fee)")
                #expect(step.place.minY == kept.place.minY,
                        "\(Int(size.width)) \(step.verdict): the verdict starts at \(step.place.minY), was \(kept.place.minY)")
                #expect(step.confirm == kept.confirm)
                #expect(step.offset == kept.offset, "\(Int(size.width)) \(step.verdict): the body scrolled")
                #expect(!step.scrolls, "\(Int(size.width)) \(step.verdict): the usual sheet does not fit a phone")
            }
        }
    }

    /// The place is never blank, and what stands in it is said aloud: the
    /// quiet "Checking…" while the simulation is out, then the verdict's own
    /// words — "No asset changes" in the core's.
    @Test func thePlaceIsNeverBlankAndSaysWhatItHolds() async throws {
        let walked = try await walk([.out, .send, .nothing, .caution], size: Self.normal)
        let said = walked.map { step in step.tree.filter(\.element).map(\.label) }
        #expect(said[0].contains("Balance changes") && said[0].contains("Checking…"))
        #expect(said[1].contains("Balance changes") && said[1].contains("xDAI") && !said[1].contains("Checking…"))
        #expect(said[2].contains("Balance changes") && said[2].contains("No asset changes"))
        #expect(said[2].contains(loc.t("componentsUi.signing.simResultNoChange")))
        #expect(said[3].contains(loc.t("componentsUi.signing.simUnavailableWarning")))
        for step in walked {
            #expect(step.tree.allSatisfy { !$0.label.contains("leave your wallet") })
        }
    }

    // MARK: - A tall verdict is shown whole

    private static let tallRows = ["xDAI", "WETH", "USDC", "Unverified token"]

    /// The verdict's own card, alone, as wide as the place: how tall it asks
    /// to be when nothing limits it.
    private func ownHeight(_ verdict: Verdict, width: CGFloat) throws -> CGFloat {
        guard case .balances(let title, let rows, let note, let tone)? = try verdictBlocks(verdict).first else {
            Issue.record("\(verdict) is not a balance card")
            return 0
        }
        let card = UIHostingController(
            rootView: SigningBalances(title: title, rows: rows, note: note, noteTone: tone).themed(.light)
        )
        return card.sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude)).height
    }

    /// Four balance rows and the unverified-token warning, on a phone: the
    /// place is exactly as tall as the verdict — taller than the place that
    /// was kept — every row and the warning stand whole and in view, there
    /// is no scroll view but the body's, and the confirm is where it was
    /// with one row and with none.
    @Test func aTallVerdictIsShownWholeAndTheConfirmDoesNotMove() async throws {
        let walked = try await walk([.out, .send, .swap, .three, .tall], size: Self.normal)
        let (out, send, swap, three, tall) = (walked[0], walked[1], walked[2], walked[3], walked[4])
        for step in walked { measure("signing-tall", Self.normal, step) }
        let warning = loc.t("componentsUi.signing.unverifiedWarning")

        // The verdict is laid out at the height it asks for, alone: nothing
        // squeezed it into the place.
        let own = try ownHeight(.tall, width: tall.place.width)
        #expect(abs(tall.place.height - own) <= 1.5, "the verdict is \(tall.place.height) tall, and asks \(own)")

        // The place. What it holds pushes the fee row down by what the place
        // GREW — so the place's height is the kept one plus that. Under the
        // usual verdicts it grew nothing; under three rows and under the
        // tall one it is the verdict's own height, to the point.
        #expect(send.feeInBody == out.feeInBody && swap.feeInBody == out.feeInBody)
        let threePlace = three.place.height - (three.feeInBody - out.feeInBody)
        let tallPlace = tall.place.height - (tall.feeInBody - out.feeInBody)
        // Two rows and half a third: the height kept before this round.
        let kept = ((swap.place.height + three.place.height) / 2).rounded()
        print("MEASURE signing-tall place: kept=\(kept) three=\(three.place.height) (+\(three.feeInBody - out.feeInBody))"
            + " tall=\(tall.place.height) (+\(tall.feeInBody - out.feeInBody)) content=\(own)")
        #expect(abs(threePlace - kept) <= 0.5, "three rows: the place is not the verdict's height (\(threePlace) vs \(kept))")
        #expect(abs(tallPlace - kept) <= 0.5, "the tall verdict: the place is not its height (\(tallPlace) vs \(kept))")
        #expect(tall.place.height > kept + 60, "the tall verdict did not outgrow the kept place")
        #expect(tall.placeInBody == out.placeInBody, "the verdict does not start where \"Checking…\" did")

        // Every row and the warning: a whole row each, one under the other,
        // inside the verdict, in view.
        let row = try send.frame("xDAI").height
        var under = tall.place.minY
        for label in Self.tallRows + [warning] {
            let frame = try tall.frame(label)
            if label != warning {
                #expect(abs(frame.height - row) <= 0.5, "\(label) is \(frame.height) tall, a row is \(row)")
            }
            #expect(frame.minY >= under - 0.5, "\(label) \(frame) overlaps the line above it")
            #expect(frame.maxY <= tall.place.maxY + 0.5, "\(label) \(frame) runs out of the verdict \(tall.place)")
            #expect(tall.inView(frame), "\(label) \(frame) is outside the body's viewport \(tall.viewport)")
            under = frame.maxY
        }
        // The four figures too, each on its coin's row.
        for (label, delta) in zip(Self.tallRows, ["\u{2212}1.5", "\u{2212}0.04", "\u{2212}25", "+"]) {
            let at = try tall.frame(delta)
            #expect(abs(at.midY - (try tall.frame(label)).midY) <= 1, "\(delta) is not on \(label)'s row")
        }
        #expect(try tall.frame(warning).height > row, "the warning is cut to a line")

        // Nothing scrolls but the body.
        #expect(tall.scrollViews == 1, "a scroll view inside the sheet's own: \(tall.scrollViews)")

        // The confirm: whole, and where it was.
        #expect(tall.confirm == send.confirm && tall.confirm == out.confirm,
                "the confirm moved: \(out.confirm) → \(send.confirm) → \(tall.confirm)")
        #expect(tall.confirm.maxY <= tall.bottom + 0.5)
    }

    /// On a screen too short for the sheet the BODY scrolls: the verdict is
    /// brought into view as it lands, every row and the warning can be
    /// brought fully into view, and the confirm has not moved — not when the
    /// verdict landed, and not while the body scrolled.
    @Test func onAShortScreenTheBodyScrollsAndTheConfirmStays() async throws {
        let warning = loc.t("componentsUi.signing.unverifiedWarning")
        var reached: [String: CGRect] = [:]
        var confirmWhileScrolling: [CGRect] = []
        var closeWhileScrolling: [CGRect] = []
        let walked = try await walk([.out, .send, .tall], size: Self.short) { seen, body, view in
            guard seen.verdict == .tall else { return }
            // Bring each row, then the warning, then the first row again,
            // fully into the viewport — by scrolling the body and nothing else.
            for label in Self.tallRows + [warning, Self.tallRows[0]] {
                var at = try await self.read(.tall, view)
                let frame = try at.frame(label)
                let below = frame.maxY - at.viewport.maxY
                let above = at.viewport.minY - frame.minY
                if below > 0 || above > 0 {
                    body.setContentOffset(
                        CGPoint(x: 0, y: body.contentOffset.y + (below > 0 ? below : -above)), animated: false)
                    try await self.shown(view)
                    at = try await self.read(.tall, view)
                }
                let now = try at.frame(label)
                #expect(at.inView(now), "\(label) cannot be brought into view: \(now) in \(at.viewport)")
                reached[label] = now
                confirmWhileScrolling.append(at.confirm)
                closeWhileScrolling.append(at.close)
            }
            // …and to its very end: the last row of the sheet.
            body.setContentOffset(
                CGPoint(x: 0, y: max(0, body.contentSize.height - body.bounds.height)), animated: false)
            try await self.shown(view)
            let end = try await self.read(.tall, view)
            confirmWhileScrolling.append(end.confirm)
            closeWhileScrolling.append(end.close)
        }
        let (out, send, tall) = (walked[0], walked[1], walked[2])
        for step in walked { measure("signing-short", Self.short, step) }

        // The sheet is taller than this screen, and what scrolls is its body.
        #expect(tall.scrolls, "the sheet fits \(Self.short): content \(tall.contentHeight), viewport \(tall.viewport.height)")
        #expect(tall.scrollViews == 1)
        // The verdict is still its own height: the short screen cut nothing
        // off it, and the body grew by it.
        #expect(abs(tall.place.height - (try ownHeight(.tall, width: tall.place.width))) <= 1.5)
        #expect(tall.contentHeight > out.contentHeight + 60)

        // It opened at its top, on "Checking…". Each verdict was brought
        // into view as it landed: whole where the viewport can hold it —
        // by the least scroll that shows all of it — else from its top.
        #expect(out.placeInBody == send.placeInBody && out.placeInBody == tall.placeInBody)
        #expect(send.inView(send.place), "the one-row verdict \(send.place) is not whole in \(send.viewport)")
        #expect(tall.offset > out.offset, "the tall verdict landed under the fold and the body did not move")
        if tall.place.height <= tall.viewport.height {
            #expect(tall.inView(tall.place), "the landed verdict \(tall.place) is not whole in the viewport \(tall.viewport)")
            // …by the least scroll that shows all of it: it ends at the fold.
            #expect(abs(tall.place.maxY - tall.viewport.maxY) <= 1, "the body scrolled further than the verdict needed")
        } else {
            #expect(abs(tall.place.minY - tall.viewport.minY) <= 1, "the landed verdict does not start at the viewport's top")
        }

        // Every row and the warning could be read, whole.
        #expect(reached.count == Self.tallRows.count + 1)

        // The confirm: one frame, whole, under the body — before the
        // verdict, under one row, under the tall one, and at every scroll.
        for frame in [send.confirm, tall.confirm] + confirmWhileScrolling {
            #expect(frame == out.confirm, "the confirm moved: \(frame) vs \(out.confirm)")
        }
        #expect(out.confirm.maxY <= out.bottom + 0.5 && out.confirm.minY >= tall.viewport.maxY - 0.5)

        // And the ✕, the one way to refuse: the body scrolled by itself when
        // the verdict landed, and to its end by hand, and the ✕ is where it
        // was — whole, above the body.
        #expect(tall.offset > 0, "the body did not scroll: nothing was shown about the ✕")
        for frame in [send.close, tall.close] + closeWhileScrolling {
            #expect(frame == out.close, "the ✕ moved: \(frame) vs \(out.close)")
        }
        #expect(out.close.minY >= 0 && out.close.maxY <= tall.viewport.minY + 0.5)
    }

    /// The placeholder is not a verdict: a sheet that opens on "Checking…"
    /// on a short screen opens at its top. One that opens with its verdict
    /// already there shows it.
    @Test func aSheetOpensAtItsTopAndOnItsVerdictWhenItHasOne() async throws {
        let pending = try await walk([.out], size: Self.short)[0]
        let landed = try await walk([.tall], size: Self.short)[0]
        measure("signing-open", Self.short, pending)
        measure("signing-open", Self.short, landed)
        #expect(pending.offset <= 0, "the sheet did not open at its top: \(pending.offset)")
        #expect(landed.offset > pending.offset, "a sheet opened with its verdict under the fold")
        #expect(landed.inView(landed.place) || abs(landed.place.minY - landed.viewport.minY) <= 1)
        #expect(landed.confirm == pending.confirm)
    }
}
