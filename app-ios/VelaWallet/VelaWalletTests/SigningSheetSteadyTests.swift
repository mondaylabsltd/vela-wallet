//
//  SigningSheetSteadyTests.swift
//  VelaWalletTests
//
//  Nothing on the signing sheet moves while its fee is measured.
//
//  On the Android device the whole sheet moved ~33 px on every fee refresh,
//  speed change and 30 s re-quote; the web measured 29 px. Two lines came and
//  went with the measurement: the confirm's note under the button ("正在计算
//  网络费用…") and the shortfall line under the fee card. On iOS the sheet is
//  a top-aligned scroll at the large detent, so the shortfall line moved the
//  confirm under the finger, and on a sheet scrolled to its end every line
//  that went shrank the content and pulled everything down. The web's rule,
//  here too: once said, the confirm's note keeps its line (invisible and
//  silent while the gate is open); the shortfall line keeps its height while
//  the fee is measured again and goes only when a fee lands with nothing to
//  say.
//
//  The real `SigningSheet`, hosted in a window, stepped through the models a
//  quote landing and being asked again hands it; positions read from the tree
//  an assistive client gets.
//

import Foundation
import Observation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct SigningSheetSteadyTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// The sheet's model, swapped under one live view — so the view's own
    /// state carries from step to step, as it does on a phone.
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

    // MARK: - The models a measurement hands the sheet

    private enum Step {
        /// A fee landed: the figure, the gate open.
        case landed
        /// The 30 s re-quote or a refresh: the figure stays, the gate shuts
        /// and says why.
        case requote
        /// A new speed: no figure of its own yet.
        case newSpeed
        /// A fee landed that no coin can pay: the shortfall under the card,
        /// the gate shut with nothing more to say.
        case short
        /// The first figure measured, and the core already knows no coin has
        /// anything to pay from (`FeeView.nothing_to_pay_from`).
        case firstNothingToPay
    }

    private var measuringNote: String { loc.t("componentsUi.signing.confirmBlock.feeMeasuring") }
    private var noCoin: String { loc.t("componentsUi.gas.noCoinPays") }

    private func model(_ step: Step) -> SigningModel {
        let base = SigningFixtures.build(.cs36, loc: loc)
        let label = loc.t("componentsUi.gas.networkFee")
        let figure = "~0.0021 ETH · ≈$6.30"
        let fee: FeeModel
        let enabled: Bool
        let note: String?
        let measuring: Bool
        switch step {
        case .landed:
            fee = .onchain(label: label, value: figure, selector: nil, tappable: false)
            (enabled, note, measuring) = (true, nil, false)
        case .requote:
            fee = .onchain(label: label, value: figure, selector: nil, tappable: false)
            (enabled, note, measuring) = (false, measuringNote, true)
        case .newSpeed:
            fee = .onchain(label: label, value: loc.t("componentsUi.gas.estimating"), selector: nil,
                           tappable: false)
            (enabled, note, measuring) = (false, measuringNote, true)
        case .short:
            fee = .onchain(label: label, value: figure, selector: nil, warning: noCoin, tappable: false)
            (enabled, note, measuring) = (false, nil, false)
        case .firstNothingToPay:
            fee = .onchain(label: label, value: loc.t("componentsUi.gas.estimating"), selector: nil,
                           tappable: false)
            (enabled, note, measuring) = (false, measuringNote, true)
        }
        var model = SigningModel(
            id: base.id, dapp: base.dapp, network: base.network, blocks: base.blocks,
            tech: base.tech, techOpen: base.techOpen, fee: fee, signer: base.signer,
            confirm: (action: base.confirm?.action ?? "", enabled: enabled),
            confirmBlockLine: note, panelTitle: base.panelTitle
        )
        model.dappOwn = base.dappOwn
        model.closeLabel = loc.t("common.close")
        model.feeRefresh = FeeRefreshModel(label: loc.t("send.feeRefresh"), refreshing: measuring)
        if step == .firstNothingToPay { model.feeReserve = noCoin }
        return model
    }

    // MARK: - Where things sit

    private struct Frame: Equatable, CustomStringConvertible {
        let fee: CGFloat
        let confirm: CGFloat
        var description: String { "fee \(fee) confirm \(confirm)" }
    }

    private struct Element {
        let label: String
        let hint: String
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
            found.append(Element(label: object.accessibilityLabel ?? "", hint: object.accessibilityHint ?? "",
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

    private func scrollView(in view: UIView) -> UIScrollView? {
        if let scroll = view as? UIScrollView { return scroll }
        for sub in view.subviews { if let found = scrollView(in: sub) { return found } }
        return nil
    }

    /// The frame as it is on screen: laid out, committed (a drawing waits for
    /// the screen's update, which is also what places the tree's nodes —
    /// read before it, the first step's sat 8 points off), then a moment for
    /// the tree to follow.
    private func shown(_ view: UIView) async throws {
        view.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(200))
        _ = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in
            view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        try await Task.sleep(for: .milliseconds(100))
    }

    /// Where the fee row and the confirm sit, once two reads 150 ms apart
    /// agree (up to ~6 s): beside a thousand other tests on one main actor,
    /// one read can come before the tree has followed the last commit.
    private func settled(_ view: UIView) async throws -> (Frame, [Element]) {
        var last: Frame?
        var tree: [Element] = []
        for _ in 0..<40 {
            tree = []
            collect(view as Any, depth: 0, into: &tree)
            if let fee = tree.first(where: { $0.id == "signing.fee.refresh" }),
               let confirm = tree.first(where: { $0.id == "signing.confirm" }) {
                let frame = Frame(fee: fee.frame.minY, confirm: confirm.frame.minY)
                if frame == last { return (frame, tree) }
                last = frame
            }
            try await shown(view)
            try await Task.sleep(for: .milliseconds(50))
        }
        let frame = try #require(last, "the fee row's refresh and the confirm are in the tree")
        return (frame, tree)
    }

    /// Walks `steps` on one live sheet `height` points tall — scrolled to its
    /// end after the first step when `atEnd` — and reads where the fee row and
    /// the confirm sit after each, with every element said aloud.
    private func walk(_ steps: [Step], height: CGFloat = 844, atEnd: Bool = false)
        async throws -> (frames: [Frame], trees: [[Element]]) {
        _ = Self.automation
        let box = Box(model(steps[0]))
        let host = UIHostingController(rootView: Host(box: box))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: height))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        var frames: [Frame] = []
        var trees: [[Element]] = []
        for (index, step) in steps.enumerated() {
            box.model = model(step)
            try await shown(host.view)
            if index == 0, atEnd, let scroll = scrollView(in: host.view) {
                let end = scroll.contentSize.height - scroll.bounds.height + scroll.adjustedContentInset.bottom
                scroll.setContentOffset(CGPoint(x: 0, y: max(0, end)), animated: false)
                try await shown(host.view)
            }
            let (frame, tree) = try await settled(host.view)
            frames.append(frame)
            trees.append(tree)
        }
        return (frames, trees)
    }

    // MARK: - The sequences

    /// A funded wallet: measured at open, landed, re-quoted, a new speed,
    /// landed — on a sheet scrolled to its end, where a note that went
    /// shrank the content and pulled the whole sheet down by its line.
    @Test func aFundedWalletsSheetHoldsStillAtItsEnd() async throws {
        let walked = try await walk([.newSpeed, .landed, .requote, .landed, .newSpeed, .landed],
                                    height: 480, atEnd: true)
        for (index, frame) in walked.frames.enumerated() {
            #expect(frame == walked.frames[0], "step \(index): \(frame) vs \(walked.frames[0])")
        }
    }

    /// A wallet no coin of which can pay: the shortfall line keeps its height
    /// while the fee is measured again, so the confirm does not rise under
    /// the finger and fall back when the quote lands.
    @Test func aWalletThatCannotPayKeepsTheShortfallsLine() async throws {
        let walked = try await walk([.newSpeed, .short, .requote, .short, .newSpeed, .short])
        // From the first landing on — the first measurement had no shortfall
        // to hold yet — not a point.
        for (index, frame) in walked.frames.enumerated().dropFirst(2) {
            #expect(frame == walked.frames[1], "step \(index): \(frame) vs \(walked.frames[1])")
        }
    }

    /// The first figure, on an account with nothing to pay from (the backup
    /// sheet on Ethereum, iPhone pass 2026-10-09): the core says so while
    /// the figure is measured, the line's room is held from then, and the
    /// figure's landing does not move the confirm — 26 pt on the phone.
    @Test func aFirstFigureNoCoinCanPayLandsWithoutMovingTheConfirm() async throws {
        let walked = try await walk([.firstNothingToPay, .short, .requote, .short])
        for (index, frame) in walked.frames.enumerated() {
            #expect(frame == walked.frames[0], "step \(index): \(frame) vs \(walked.frames[0])")
        }
        // Held, not said: nothing is claimed before the figure.
        #expect(!walked.trees[0].contains { $0.label == noCoin })
        #expect(!walked.trees[0].contains { $0.id == "signing.fee.reason" })
        // Without the core's word the first landing still moves it — the
        // held line covers only what was said.
        let unknown = try await walk([.newSpeed, .short])
        #expect(unknown.frames[1].confirm > unknown.frames[0].confirm)
    }

    /// A held line is space and nothing else: not read aloud, not a hook a
    /// test can find — and the shortfall's goes when a fee lands that the
    /// coin can pay.
    @Test func aHeldLineIsSilentAndTheShortfallGoesWhenAFeeLandsClean() async throws {
        let walked = try await walk([.short, .requote, .landed])
        let measuring = walked.trees[1]
        #expect(measuring.contains { $0.label == measuringNote }, "the note is said while the gate is shut")
        #expect(!measuring.contains { $0.label == noCoin }, "the last verdict is not said while measuring")
        #expect(!measuring.contains { $0.id == "signing.fee.reason" })
        let landed = walked.trees[2]
        #expect(!landed.contains { $0.label == measuringNote }, "a held note is silent")
        #expect(!landed.contains { $0.id == "signing.confirmBlock" })
        #expect(!landed.contains { $0.label == noCoin })
        // The shortfall's line went with a clean landing: the confirm sits a
        // line higher than it did under the shortfall, and only then.
        #expect(walked.frames[2].confirm < walked.frames[0].confirm)
        #expect(walked.frames[1].confirm == walked.frames[0].confirm)
    }
}
