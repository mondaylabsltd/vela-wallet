//
//  FlowNav.swift
//  VelaWallet
//
//  Flow navigation (spec 021 SC-002) — the iOS port of the web's
//  `nav.svelte.ts`, with the same entries and the same steps.
//
//  A stack, not a current-screen field. The mocks stack: Receive opens a
//  network list and a network opens its QR; Send runs picker → form →
//  confirm → receipt. Back has to unwind one level, which a single field
//  cannot express.
//
//  It lives inside the wallet screen rather than as a NavigationStack path
//  because these screens are still fixtures — and because the flows push
//  full-bleed surfaces (the scanner, the share card) that a navigation bar
//  would frame wrongly.
//

import SwiftUI

/// Where a flow can be entered from the wallet home.
enum WalletFlowEntry {
    case receive, send, scan, activity, assets, addToken, tokenDetail, txDetail
}

@Observable
final class FlowNav {
    /// Deepest state last. Empty means the wallet home is showing.
    private(set) var stack: [FlowStateId] = []

    var top: FlowStateId? { stack.last }
    var isOpen: Bool { !stack.isEmpty }

    /// The stack an entry opens, deepest last.
    ///
    /// `addToken` opens two: the assets screen and the sheet over it. That is
    /// what makes the back chevron in the T3 mock mean something — it goes to
    /// the list you were adding to, not out of the flow entirely.
    private static let entries: [WalletFlowEntry: [FlowStateId]] = [
        .receive: [.r1],
        .send: [.sd1],
        .scan: [.s1],
        .activity: [.a1],
        .assets: [.t1],
        .addToken: [.t1, .t3],
        .tokenDetail: [.t1, .t2],
        .txDetail: [.a1, .a2],
    ]

    /// Pushes a step deeper within a flow that is already open.
    private static let steps: [FlowStep: FlowStateId] = [
        .receiveQr: .r2,
        .receiveQrAsset: .r3,
        .txDetail: .a2,
        .tokenDetail: .t2,
        .addToken: .t3,
        .sendForm: .sd2,
        .sendConfirm: .sd3,
        .sendReceipt: .sd4b,
        .contactPick: .sd2e,
        .feeToken: .sd2f,
        .batchImport: .sd2c,
        .sendMulti: .sd1b,
        .addRecipient: .sd2b,
        .scan: .s1,
        .receive: .r1,
    ]

    func enter(_ entry: WalletFlowEntry) {
        stack = Self.entries[entry] ?? []
    }

    /// Step deeper. Unknown steps are ignored rather than trapping: screens
    /// emit navigation intents generously (`done`, `chains`, …) and a flow
    /// that has nowhere to put one should do nothing, not crash a wallet.
    func push(_ step: FlowStep) {
        if step == .done {
            close()
            return
        }
        guard let next = Self.steps[step], top != next else { return }
        stack.append(next)
    }

    /// One level up. At the root this leaves the flow and shows the wallet.
    func back() {
        _ = stack.popLast()
    }

    /// A sheet the person closed (× or a drag) takes its own level with it —
    /// only when that level is the top, so a sheet the send machine derives
    /// (not pushed) never pops the form under it. Left on the stack, the
    /// closed sheet's state stayed on top: the next row tapped resolved to the
    /// same state and was suppressed, `push` ignored the same step, and the
    /// first ‹ only closed the invisible sheet (082 iPhone pass, X-HISTORY).
    func sheetClosed(_ state: FlowStateId) {
        if top == state { _ = stack.popLast() }
    }

    func close() {
        stack.removeAll()
    }

    /// A machine closed its journey: the flow goes with it, but only while
    /// one of that machine's screens is what is showing — a close that lands
    /// after the person already went elsewhere must not shut that (087 F27,
    /// Android's `if (sendClosed && flows.top in SEND_STATES) flows.close()`).
    func close(ifShowing states: Set<FlowStateId>) {
        if let top, states.contains(top) { close() }
    }
}

extension FlowStep: Hashable {}
