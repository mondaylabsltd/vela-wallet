//
//  VelaButton.swift
//  VelaWallet
//
//  The single authoritative CTA control. Primary = accent fill (the only
//  accent surface on the welcome screen); secondary = outlined, label in
//  fg.base per DV-001 (the dark mock's muted label fails contrast).
//

import SwiftUI

struct VelaButton: View {
    enum Kind {
        case primary
        case secondary
        /// Spec 023: 退出登录 / 仍然退出 / 全部清除 are filled buttons in the
        /// error colour, not accent ones. Accent is reserved for the action
        /// that moves value (design review 2026-07), and signing out or
        /// erasing a device moves none — it destroys.
        case danger
    }

    let title: String
    let kind: Kind
    var enabled: Bool = true
    /// The button's action is under way and this control is what the person is
    /// waiting on. Deliberately NOT the same as `enabled: false`: a dimmed
    /// button reads as "unavailable", and the one state it must never be
    /// confused with is "working". Busy keeps full emphasis and turns a
    /// spinner where its label was (docs/design-system.md — "Loading state:
    /// ActivityIndicator replacing text").
    var loading: Bool = false
    let action: () -> Void

    /// Bumped on every accepted press purely to drive the haptic — a press
    /// whose visible result is a system sheet several hundred ms away needs an
    /// answer in the same instant the finger lands.
    @State private var presses = 0

    var body: some View {
        Button {
            presses &+= 1
            action()
        } label: {
            Text(title).typeRole(Typography.button)
        }
        .buttonStyle(VelaButtonStyle(kind: kind, loading: loading))
        .disabled(!enabled || loading)
        // Dimming follows `enabled` alone, never `loading`.
        .opacity(enabled ? 1 : Tokens.Opacity.disabled)
        // The label is hidden behind the spinner while busy; the button still
        // answers to its own name.
        .accessibilityLabel(title)
        .sensoryFeedback(.impact(weight: .light, intensity: Interaction.pressHapticIntensity), trigger: presses)
    }
}

private struct VelaButtonStyle: ButtonStyle {
    @Environment(\.theme) private var theme
    let kind: VelaButton.Kind
    var loading: Bool = false

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        let labelColor = kind == .secondary ? theme.fgBase : theme.onAccent
        // minHeight, not a fixed height: long-locale labels wrap centered and
        // grow the capsule instead of being clipped (spec 014 long-label fix).
        return configuration.label
            .foregroundStyle(labelColor)
            .multilineTextAlignment(.center)
            // Hidden rather than removed: the label keeps holding the button's
            // height and width, so nothing reflows when the spinner arrives.
            .opacity(loading ? 0 : 1)
            .overlay {
                if loading {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .controlSize(.small)
                        .tint(labelColor)
                }
            }
            .padding(.horizontal, Tokens.Space.s24)
            .padding(.vertical, Tokens.Space.s8)
            .frame(maxWidth: .infinity)
            .frame(minHeight: Tokens.Control.lg)
            .background {
                switch kind {
                case .primary:
                    Capsule().fill(theme.accentBase)
                case .danger:
                    Capsule().fill(theme.errorBase)
                case .secondary:
                    Capsule().strokeBorder(theme.borderStrong, lineWidth: Tokens.BorderWidth.hairline)
                }
            }
            // The whole pill takes the tap, not just what is painted.
            //
            // SwiftUI hit-tests DRAWN content: the primary's filled capsule
            // covers its own area, but `strokeBorder` paints a 1pt ring and
            // nothing else — so the secondary answered only on its border and
            // on the glyphs of its label, and the empty space either side of
            // the words was dead (device-found 2026-08-25). This is also what
            // makes the 52pt control height a real 52pt target.
            .contentShape(Capsule())
            .opacity(pressed ? Interaction.pressedOpacity : 1)
            // The scale the other three platforms already draw (docs/design-system.md
            // "spring scale 0.97 on press"); opacity alone was iOS reading the
            // rule as half of itself.
            .scaleEffect(pressed ? Interaction.pressScaleButton : 1)
            .animation(Interaction.pressSpring, value: pressed)
    }
}

/// Interaction-state constants the export does not name — licensed by
/// docs/design-system.md (`opacity.*` engineering tokens; web's opacity-hover
/// addition is the same move).
enum Interaction {
    static let pressedOpacity: Double = 0.8
    /// docs/design-system.md motion table: button press scales to 0.97.
    static let pressScaleButton: CGFloat = 0.97
    /// Never a timing curve for interactive feedback — always a spring
    /// (docs/design-system.md). The damping mirrors Android's `VelaMotion.pressSpring`.
    static let pressSpring: Animation = .interactiveSpring(response: 0.2, dampingFraction: 0.75)
    /// A confirmation, not a jolt: the CTA is a tap, not a transaction.
    static let pressHapticIntensity: Double = 0.7
    /// How long the address strip's 已复制 confirmation stays visible
    /// (spec 014 — copy feedback is the one sanctioned timed visual).
    static let copiedFeedbackSeconds: Double = 1.5
}

#Preview("Buttons") {
    VStack(spacing: Tokens.Space.s16) {
        VelaButton(title: "Create Wallet", kind: .primary) {}
        VelaButton(title: "I already have a wallet", kind: .secondary) {}
        VelaButton(title: "Signing in", kind: .secondary, loading: true) {}
        VelaButton(title: "Disabled", kind: .primary, enabled: false) {}
    }
    .padding(Tokens.Space.s24)
    .themed(.light)
}

#Preview("Buttons dark") {
    VStack(spacing: Tokens.Space.s16) {
        VelaButton(title: "创建钱包", kind: .primary) {}
        VelaButton(title: "我已有钱包", kind: .secondary) {}
        VelaButton(title: "我已有钱包", kind: .secondary, loading: true) {}
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}
