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
//               deformation + haptic)
//  - `detent`   a step crossed: a slider stop, a picker snapping, the signing
//               slider's threshold
//  - `select`   a selection that TAKES EFFECT: a switch, a filter, a network /
//               fee-token / account pick, a favourite, a copy
//  - `success` / `reject`  an outcome the core decided
//
//  **Never** for scrolling, a row tap that navigates, a tab switch, Back, a
//  sheet opening or closing, typing, animation, or a value the app changed by
//  itself. One per gesture.
//
//  Before 058 iOS had four ungoverned call sites and no vocabulary; the policy
//  existed only in the Kotlin file's own doc comment, which is why 057 recorded
//  haptics as "absent — no policy written".
//

import UIKit

enum VelaHaptic {
    case press, detent, select, success, reject

    /// Perform it. Silent on any platform refusal: a wallet that crashed
    /// because a phone would not buzz has its priorities wrong.
    func play() {
        switch self {
        case .press:
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
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

/// Copy something, and say so with the one haptic a copy is allowed.
///
/// Every copy in this app goes through here, which is what keeps a copy button
/// that shows a checkmark and puts nothing on the clipboard from existing —
/// the exact defect 058 found on the receive code's two copy buttons.
@MainActor
func velaCopy(_ value: String) {
    guard !value.isEmpty else { return }
    UIPasteboard.general.string = value
    VelaHaptic.select.play()
}
