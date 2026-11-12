//
//  HoldConfirmButton.swift
//  VelaWallet
//
//  R22 (review 1-3, docs/multisig-machines.md §36): the confirm the core
//  asks to be held. Every "… anyway" and every danger is held for the
//  core's `hold_to_confirm_ms()` (1.5 s) before it acts: a tap, a double tap
//  or a press made while the sheet is still arriving cannot sign.
//
//  The accessible equivalent is not optional (WCAG 2.5.1 / 2.5.7: no held
//  gesture may be the only way). With VoiceOver or Switch Control running —
//  or on any accessibility activation (VoiceOver's double tap, Switch
//  Control's Tap, Full Keyboard Access) — the first activation ARMS the
//  button and says "Press again to confirm"; a second one within the core's
//  `confirm_again_window_ms()` (5 s) confirms; past it, it disarms.
//
//  ONE component, used for the dApp signing sheet's confirm (and the
//  hand-off card's Open, which is that confirm) and every multisig sign /
//  propose / approve / execute button whose door asks for a hold. `holdMs ==
//  0` is a plain `VelaButton`, exactly as before.
//
//  The rule of when to hold is the core's (`sign_confirm::posture`,
//  `SignDoor.hold_ms`); what a press does with it is `HoldConfirmState`,
//  pure, so the tests drive it without a finger.
//

import SwiftUI
import UIKit
import VelaCore

/// How a confirm is pressed — the core's posture, and the words that say it.
struct ConfirmPress: Equatable {
    /// Drawn in the danger style (`anyway || danger`).
    var danger: Bool
    /// Held this long before it acts; `0`: a plain press.
    var holdMs: Int
    /// "Hold to confirm" — beside the button, and its accessibility hint.
    var holdHint: String
    /// "Press again to confirm" — the armed state of the accessible path.
    var confirmAgain: String

    static let plain = ConfirmPress(danger: false, holdMs: 0, holdHint: "", confirmAgain: "")

    var held: Bool { holdMs > 0 }

    /// A multisig door's press (`SignDoor.hold_ms`, its "… anyway" or danger
    /// review), in the signing sheet's words.
    @MainActor
    static func door(holdMs: Int, danger: Bool, loc: Loc) -> ConfirmPress {
        ConfirmPress(
            danger: danger, holdMs: holdMs,
            holdHint: loc.t("componentsUi.signing.holdToConfirm"),
            confirmAgain: loc.t("componentsUi.signing.confirmAgain")
        )
    }
}

/// What a held confirm does with each touch and activation. Pure: the clock
/// is handed in, so a test drives it exactly.
struct HoldConfirmState: Equatable {
    /// What one step came to.
    enum Outcome: Equatable {
        case nothing
        /// The accessible path's first activation: armed, "Press again".
        case armed
        /// The confirm fires.
        case confirmed
    }

    /// The accessible path is armed since this instant (epoch ms).
    private(set) var armedAtMs: Double?
    /// A finger is holding the button since this instant.
    private(set) var holdingSinceMs: Double?

    /// Is the accessible path armed at `nowMs`?
    func armed(nowMs: Double, windowMs: Double) -> Bool {
        guard let armedAtMs else { return false }
        return nowMs - armedAtMs <= windowMs
    }

    /// An explicit activation (assistive technology, a keyboard): the first
    /// arms; a second within `windowMs` confirms; one after the window arms
    /// again.
    mutating func activate(nowMs: Double, windowMs: Double) -> Outcome {
        if armed(nowMs: nowMs, windowMs: windowMs) {
            armedAtMs = nil
            return .confirmed
        }
        armedAtMs = nowMs
        return .armed
    }

    /// The window ran out with no second activation: disarmed.
    mutating func expire(nowMs: Double, windowMs: Double) {
        if !armed(nowMs: nowMs, windowMs: windowMs) { armedAtMs = nil }
    }

    /// A finger came down on the button.
    mutating func pressBegan(nowMs: Double) {
        holdingSinceMs = nowMs
    }

    /// The hold's timer reached `holdMs` with the finger still down: the
    /// confirm fires — once, and only for a press that lasted that long.
    mutating func holdReached(nowMs: Double, holdMs: Double) -> Outcome {
        guard let since = holdingSinceMs, nowMs - since >= holdMs else { return .nothing }
        holdingSinceMs = nil
        return .confirmed
    }

    /// The finger lifted. Early, it does nothing — the hold starts over.
    mutating func pressEnded() -> Outcome {
        holdingSinceMs = nil
        return .nothing
    }
}

/// The confirm: a plain `VelaButton` when `press.holdMs == 0`; otherwise a
/// held one, with its accessible two-press path.
struct HoldConfirmButton: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String
    var kind: VelaButton.Kind = .primary
    var enabled: Bool = true
    /// The action is under way (`VelaButton.loading`): the busy button,
    /// never a hold over work already started.
    var loading: Bool = false
    /// Draw "Hold to confirm" (or, armed, "Press again") under the
    /// button. `false` where the host draws it in a room of its own — the
    /// signing sheet's footer, whose note line keeps the confirm at one y.
    var showsHint: Bool = true
    let press: ConfirmPress
    var haptic: VelaHaptic = .confirm
    /// §39 (R22 follow-up, A1): the confirm began (`true`: a hold started,
    /// the assistive path armed) or ended without firing (`false`: lifted
    /// early, disarmed, shut). The dApp sheet takes the fee it draws at the
    /// beginning and holds its fee row still until the end.
    var onEngage: (Bool) -> Void = { _ in }
    let action: () -> Void

    @State private var state = HoldConfirmState()
    @State private var progress: CGFloat = 0
    @State private var armed = false
    @State private var assistive = HoldConfirmButton.assistiveRunning
    @State private var disarm: Task<Void, Never>?
    @State private var holdTimer: Task<Void, Never>?

    /// VoiceOver or Switch Control is running: taps take the two-press path.
    static var assistiveRunning: Bool {
        UIAccessibility.isVoiceOverRunning || UIAccessibility.isSwitchControlRunning
    }

    private static var nowMs: Double { Date().timeIntervalSince1970 * 1000 }
    private var windowMs: Double { Double(confirmAgainWindowMs()) }

    var body: some View {
        if press.held, !loading {
            held
        } else {
            VelaButton(title: title, kind: kind, enabled: enabled, loading: loading, haptic: haptic, action: action)
        }
    }

    private var labelColor: Color {
        switch kind {
        case .secondary: theme.fgBase
        case .ink: theme.fgInverse
        case .primary, .danger: theme.onAccent
        }
    }

    private var fill: Color {
        switch kind {
        case .primary: theme.accentBase
        case .danger: theme.errorBase
        case .ink: theme.fgBase
        case .secondary: .clear
        }
    }

    /// The held confirm: a real `Button` (so a shut one is shut for
    /// assistive technology too), whose press — not its tap — is the hold.
    private var held: some View {
        Button {
            // A tap is no confirm. With assistive technology it is an
            // activation: the two-press path.
            if assistive, enabled { activate() }
        } label: {
            VStack(spacing: Tokens.Space.s4) {
                capsule
                // "Hold to confirm" under the button — or, armed, "Press again".
                if showsHint {
                    Text(prose: armed ? press.confirmAgain : press.holdHint)
                        .typeRole(Typography.flowCaption.scaled(textScale))
                        .foregroundStyle(press.danger ? theme.errorBase : theme.fgMuted)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityHidden(true)
                }
            }
        }
        .buttonStyle(HoldPressStyle { pressing in
            guard enabled, !assistive else { return }
            if pressing { begin() } else { end() }
        })
        .disabled(!enabled)
        .accessibilityLabel(title)
        .accessibilityHint(armed ? press.confirmAgain : press.holdHint)
        // Any accessibility activation — VoiceOver's double tap, Switch
        // Control's Tap, Full Keyboard Access — takes the two-press path.
        .accessibilityAction { if enabled { activate() } }
        .onReceive(NotificationCenter.default.publisher(for: UIAccessibility.voiceOverStatusDidChangeNotification)) { _ in
            assistive = Self.assistiveRunning
        }
        .onReceive(NotificationCenter.default.publisher(for: UIAccessibility.switchControlStatusDidChangeNotification)) { _ in
            assistive = Self.assistiveRunning
        }
        .onChange(of: enabled) { _, open in if !open { reset() } }
        .onDisappear { reset() }
    }

    private var capsule: some View {
        Text(title)
            .typeRole(Typography.button)
            .foregroundStyle(labelColor)
            .multilineTextAlignment(.center)
            .padding(.horizontal, Tokens.Space.s24)
            .padding(.vertical, Tokens.Space.s8)
            .frame(maxWidth: .infinity)
            .frame(minHeight: Tokens.Control.lg)
            .background {
                ZStack(alignment: .leading) {
                    if kind == .secondary {
                        Capsule().strokeBorder(theme.borderStrong, lineWidth: Tokens.BorderWidth.hairline)
                    } else {
                        Capsule().fill(fill)
                    }
                    // The hold's progress: a fill that grows over the core's
                    // `hold_ms`, from the leading edge.
                    GeometryReader { box in
                        Capsule()
                            .fill(labelColor.opacity(0.22))
                            .frame(width: box.size.width * progress)
                    }
                    .clipShape(Capsule())
                }
            }
            .contentShape(Capsule())
            .overlay {
                if armed {
                    Capsule().strokeBorder(labelColor.opacity(0.9), lineWidth: Tokens.BorderWidth.hairline * 2)
                        .padding(2)
                }
            }
            .opacity(enabled ? 1 : Tokens.Opacity.disabled)
    }

    /// A finger came down: the fill grows over `holdMs`.
    private func begin() {
        guard state.holdingSinceMs == nil else { return }
        state.pressBegan(nowMs: Self.nowMs)
        onEngage(true)
        VelaHaptic.press.play()
        withAnimation(.linear(duration: Double(press.holdMs) / 1000)) { progress = 1 }
        holdTimer?.cancel()
        let ms = press.holdMs
        holdTimer = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(ms) * 1_000_000)
            guard !Task.isCancelled else { return }
            reached()
        }
    }

    /// The press lasted `holdMs`: the confirm fires (the gesture's clock and
    /// this one agree to within a frame or two).
    private func reached() {
        let tolerance = 100.0
        if state.holdReached(nowMs: Self.nowMs, holdMs: Double(press.holdMs) - tolerance) == .confirmed {
            fire()
        }
    }

    /// The finger lifted — early, nothing happens and the hold starts over.
    private func end() {
        holdTimer?.cancel()
        holdTimer = nil
        _ = state.pressEnded()
        // Lifted early, the confirm is over (after it fired, the action has
        // already taken the figure, and this ends nothing).
        if !armed { onEngage(false) }
        withAnimation(.easeOut(duration: 0.2)) { progress = 0 }
    }

    /// The accessible path: arm, then confirm within the window.
    private func activate() {
        switch state.activate(nowMs: Self.nowMs, windowMs: windowMs) {
        case .armed:
            armed = true
            onEngage(true)
            VelaHaptic.press.play()
            UIAccessibility.post(notification: .announcement, argument: press.confirmAgain)
            disarm?.cancel()
            let window = windowMs
            disarm = Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(window * 1_000_000))
                guard !Task.isCancelled else { return }
                state.expire(nowMs: Self.nowMs, windowMs: window)
                armed = state.armed(nowMs: Self.nowMs, windowMs: window)
                if !armed { onEngage(false) }
            }
        case .confirmed:
            fire()
        case .nothing:
            break
        }
    }

    private func fire() {
        disarm?.cancel()
        armed = false
        withAnimation(.easeOut(duration: 0.2)) { progress = 0 }
        VelaButton.pressed(action, haptic: haptic)
    }

    private func reset() {
        holdTimer?.cancel()
        disarm?.cancel()
        _ = state.pressEnded()
        state.expire(nowMs: .infinity, windowMs: 0)
        armed = false
        progress = 0
        onEngage(false)
    }
}

/// The held confirm's press, as the button reports it: down, and up (a
/// release, or a finger that moved away — a scroll). Nothing is drawn here.
private struct HoldPressStyle: ButtonStyle {
    let onPressing: (Bool) -> Void

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .onChange(of: configuration.isPressed) { _, pressed in onPressing(pressed) }
    }
}
