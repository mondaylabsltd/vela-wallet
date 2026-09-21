//
//  SlideToConfirmView.swift
//  VelaWallet
//
//  The one way to confirm a signature (spec 022 §4, product contract).
//
//  There is no reject button beside it: dismissing the sheet IS the
//  rejection, so the only deliberate act on this screen is the affirmative
//  one. The gesture asks for 88% of the track — far more than a mis-tap, far
//  less than a fight — and VoiceOver gets the same power without the drag,
//  because a confirmation only a thumb can perform is one some people could
//  never give.
//

import SwiftUI

struct SlideToConfirmView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let hint: String
    let action: String
    let enabled: Bool
    var onConfirm: () -> Void = {}

    @State private var progress: CGFloat = 0
    @State private var dragging = false
    @State private var done = false

    private var label: String { "\(hint) · \(action)" }

    var body: some View {
        GeometryReader { geo in
            let travel = max(1, geo.size.width - ExploreGeometry.slideKnob - Tokens.Space.s8)
            ZStack(alignment: .leading) {
                Capsule().fill(theme.bgSunken)
                Capsule()
                    .fill(theme.accentSoft)
                    .frame(width: ExploreGeometry.slideKnob + progress * travel)
                Text(verbatim: label)
                    .typeRole(Typography.button.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .opacity(1 - progress)
                    .frame(maxWidth: .infinity)
                LucideIcon(.arrowRight, size: LucideIconSize.slideArrow)
                    .foregroundStyle(theme.onAccent)
                    .frame(width: ExploreGeometry.slideKnob, height: ExploreGeometry.slideKnob)
                    .background(theme.accentBase, in: Circle())
                    .offset(x: Tokens.Space.s4 + progress * travel)
                    .animation(dragging || reduceMotion ? nil : .spring(response: 0.25,
                                                                        dampingFraction: 0.7),
                               value: progress)
            }
            .contentShape(Capsule())
            .gesture(
                DragGesture(minimumDistance: 1)
                    .onChanged { value in
                        guard enabled, !done else { return }
                        dragging = true
                        progress = min(1, max(0, value.location.x / travel))
                    }
                    .onEnded { _ in
                        guard enabled, !done else { return }
                        dragging = false
                        if progress >= ExploreGeometry.slideCommit {
                            progress = 1
                            done = true
                            onConfirm()
                        } else {
                            progress = 0
                        }
                    }
            )
        }
        .frame(height: ExploreGeometry.slideTrack)
        // Opened again after a slide: the ceremony ended with nothing signed
        // (a cancelled passkey sheet, a closed Clear Signer page) and the
        // request stayed open, so the slide is back at rest — a knob left at
        // the far end is a request nobody can sign another way (spec 071).
        .onChange(of: enabled) { _, open in
            guard open, done else { return }
            done = false
            progress = 0
        }
        .opacity(enabled ? 1 : Tokens.Opacity.disabled)
        .allowsHitTesting(enabled)
        // VoiceOver and Switch Control confirm by activating, not dragging.
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityAddTraits(.isButton)
        // **`allowsHitTesting(false)` is invisible to assistive technology.**
        // Without this a VoiceOver user is told "slide to confirm, button" for
        // a control that does nothing — for instance while an unlimited
        // approval is waiting for a cap, which is exactly the moment the
        // wallet is refusing on their behalf and ought to say so. Device-found
        // in 053, by a test that could not tell the two states apart either.
        .disabled(!enabled)
        .accessibilityAction {
            guard enabled, !done else { return }
            progress = 1
            done = true
            onConfirm()
        }
    }
}
