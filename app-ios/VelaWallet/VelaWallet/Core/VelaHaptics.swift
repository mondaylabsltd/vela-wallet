//
//  VelaHaptics.swift
//  VelaWallet
//
//  The five haptics this app performs, and the policy that says where.
//
//  Ported from Android's `core/platform/VelaHaptic.kt` (spec 048) — the same
//  five names, so "what does a Select feel like" has one answer across both
//  phones and a device pass can be scripted against the same vocabulary.
//
//  - `press`    a button under the finger (the founder's rule: press =
//               deformation + haptic), and a tab that switches the
//               destination (founder, 2026-09-26)
//  - `detent`   a step crossed: a slider stop, a picker snapping, the signing
//               slider's threshold
//  - `select`   a selection that TAKES EFFECT: a switch, a filter, a network /
//               fee-token / account pick, a favourite, a copy
//  - `success` / `reject`  an outcome the core decided
//
//  **Never** for scrolling, a row tap that navigates, re-tapping the tab in
//  force, Back, a sheet opening or closing, typing, animation, or a value the
//  app changed by itself. One per gesture.
//
//  Before 058 iOS had four ungoverned call sites and no vocabulary; the policy
//  existed only in the Kotlin file's own doc comment, which is why 057 recorded
//  haptics as "absent — no policy written". Until 074 the vocabulary had three
//  callers against Android's ~25 (the founder: 很多地方没有震动感觉). Now every
//  Android site has its iOS counterpart, and no feedback generator and no
//  pasteboard write exists outside this file.
//

import UIKit

enum VelaHaptic {
    case press, detent, select, success, reject

    /// A confirmation, not a jolt: a CTA is a tap, not a transaction.
    private static let pressIntensity: CGFloat = 0.7

    /// The send machine's `haptic { kind }` (spec 043), in this vocabulary:
    /// money left is a success, a refusal is a reject. Android plays the same
    /// two from `Haptics.success` / `Haptics.error`.
    init(sendKind: String) {
        self = sendKind == "error" ? .reject : .success
    }

    /// Perform it. Silent on any platform refusal: a wallet that crashed
    /// because a phone would not buzz has its priorities wrong.
    func play() {
        #if DEBUG
        Self.recorded?.append(self)
        #endif
        switch self {
        case .press:
            UIImpactFeedbackGenerator(style: .light).impactOccurred(intensity: Self.pressIntensity)
        case .detent:
            UISelectionFeedbackGenerator().selectionChanged()
        case .select:
            // UIKit's own "a choice landed" tick. The same event Android maps
            // to `CONFIRM`.
            UISelectionFeedbackGenerator().selectionChanged()
        case .success:
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        case .reject:
            UINotificationFeedbackGenerator().notificationOccurred(.error)
        }
    }
}

#if DEBUG
extension VelaHaptic {
    /// The test seam. A simulator cannot vibrate, so what a unit test can pin
    /// is the CALL: which haptics a gesture played, in order. `nil` outside a
    /// `recording` block, so a debug build does not keep a growing list.
    private(set) static var recorded: [VelaHaptic]?

    /// Runs `body` and returns every haptic it played, in order.
    ///
    /// `VelaHaptic` is main-actor isolated and `body` is synchronous, so no
    /// other test's haptic can land between the start and the read — Swift
    /// Testing runs suites in parallel, and a free-standing log would collect
    /// their buzzes too.
    static func recording(_ body: () throws -> Void) rethrows -> [VelaHaptic] {
        recorded = []
        defer { recorded = nil }
        try body()
        return recorded ?? []
    }
}
#endif

/// Copy something, and say so with the one haptic a copy is allowed.
///
/// Every copy in this app goes through here, which is what keeps a copy button
/// that shows a checkmark and puts nothing on the clipboard from existing —
/// the exact defect 058 found on the receive code's two copy buttons, and 074
/// on the receive list's rows and the receipt's hash.
///
/// `haptic: false` only where the control has already answered the finger —
/// a `VelaButton`'s press — so one gesture stays one haptic.
@MainActor
func velaCopy(_ value: String, haptic: Bool = true) {
    guard !value.isEmpty else { return }
    UIPasteboard.general.string = value
    if haptic { VelaHaptic.select.play() }
}
