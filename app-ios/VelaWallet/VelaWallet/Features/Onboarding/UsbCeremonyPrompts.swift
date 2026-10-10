//
//  UsbCeremonyPrompts.swift
//  VelaWallet
//
//  The dialogs the app-owned CTAP-over-CCID path draws itself.
//
//  Every OTHER passkey route on iOS hands the ceremony to the system sheet,
//  which draws its own PIN entry, touch prompt and account picker. The
//  app-owned CCID path has no such sheet — it IS the client — so it says these
//  three things on its own behalf, the way the desktop and Android do. The copy
//  is the shared corpus (spec 019 §5); nothing here is hard-coded.
//

import SwiftUI
import VelaCore

/// The security key's PIN. Dismissal (or Cancel) is a cancellation.
///
/// **An in-app numeric keypad, deliberately — not a `SecureField`.** A hardware
/// key that also does OTP enumerates on the iPhone as a USB KEYBOARD, and iOS
/// then suppresses the on-screen keyboard because it thinks one is attached.
/// The person would have no way to type: the software keyboard is gone, and the
/// key itself only emits an OTP on a touch, never a PIN. So the PIN is entered
/// on this app's own keypad, which owes nothing to the system keyboard
/// (device-found on iPhone, 2026-08-27). FIDO2 PINs are numeric in the
/// overwhelming majority of cases; a key with an alphanumeric PIN is the one
/// case this does not cover, and is noted for a follow-up.
struct UsbPinSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let pending: OnboardingModel.PendingPin
    let onSubmit: (String?) -> Void

    @State private var pin = ""

    /// FIDO2 requires a PIN of at least 4 UTF-8 bytes.
    private var canSubmit: Bool { pin.count >= 4 }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            Text(loc.t(I18nKeys.Create.pinTitle))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)

            Text(loc.t(I18nKeys.Create.pinBody, vars: ["product": pending.product]))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)

            // The masked PIN — a row of dots, one per digit, centred, with the
            // label shown until the first digit lands.
            ZStack {
                if pin.isEmpty {
                    Text(loc.t(I18nKeys.Create.pinLabel))
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgSubtle)
                } else {
                    HStack(spacing: Tokens.Space.s12) {
                        ForEach(0..<pin.count, id: \.self) { _ in
                            Circle()
                                .fill(theme.fgBase)
                                .frame(width: 12, height: 12)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, minHeight: Tokens.Space.s24, alignment: .center)
            .padding(.vertical, Tokens.Space.s8)

            if pending.isRetry {
                Text(loc.t(I18nKeys.Create.pinRejected))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.errorBase)
                    .frame(maxWidth: .infinity, alignment: .center)
            }
            if pending.retries >= 0 {
                Text(loc.t(I18nKeys.Create.pinAttemptsLeft, vars: ["attempts": String(pending.retries)]))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .frame(maxWidth: .infinity, alignment: .center)
            }

            PinKeypad(
                onDigit: { digit in if pin.count < 63 { pin.append(digit) } },
                onDelete: { if !pin.isEmpty { pin.removeLast() } }
            )
            .padding(.top, Tokens.Space.s8)

            VelaButton(title: loc.t(I18nKeys.Create.confirmKeyBtn), kind: .primary) {
                onSubmit(pin)
            }
            .disabled(!canSubmit)
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }
}

/// A 3×4 numeric keypad, owing nothing to the system keyboard. Square-ish keys
/// on a tight grid — a passcode pad, not a form field.
private struct PinKeypad: View {
    @Environment(\.theme) private var theme
    let onDigit: (Character) -> Void
    let onDelete: () -> Void

    private let rows: [[String]] = [
        ["1", "2", "3"],
        ["4", "5", "6"],
        ["7", "8", "9"],
        ["", "0", "⌫"],
    ]

    private let keyHeight: CGFloat = 56

    var body: some View {
        // A tight, even grid — the same 8pt rhythm the Android pad uses, so the
        // "0 / delete" row sits as close to "7 8 9" as every other row.
        VStack(spacing: Tokens.Space.s8) {
            ForEach(rows.indices, id: \.self) { r in
                HStack(spacing: Tokens.Space.s8) {
                    ForEach(rows[r], id: \.self) { label in
                        key(label)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func key(_ label: String) -> some View {
        if label.isEmpty {
            Color.clear.frame(maxWidth: .infinity, minHeight: keyHeight)
        } else {
            Button {
                if label == "⌫" {
                    onDelete()
                } else if let digit = label.first {
                    onDigit(digit)
                }
            } label: {
                Text(label)
                    .typeRole(label == "⌫" ? Typography.title : Typography.display)
                    .foregroundStyle(theme.fgBase)
                    .frame(maxWidth: .infinity)
                    .frame(height: keyHeight)
                    .background(theme.bgSunken)
                    .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r16))
            }
            .buttonStyle(.plain)
        }
    }
}

/// Which of several wallets on one key. Dismissal is a cancellation.
struct UsbWalletPickerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let pending: OnboardingModel.PendingWalletPick
    let onPick: (Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(I18nKeys.Login.pickTitle))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)

            Text(loc.t(I18nKeys.Login.pickBody, vars: ["product": pending.choices.first?.product ?? ""]))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)

            ForEach(Array(pending.choices.enumerated()), id: \.offset) { index, choice in
                if index > 0 { Divider().overlay(theme.borderBase) }
                Button {
                    onPick(index)
                } label: {
                    Text(choice.name.isEmpty ? loc.t(I18nKeys.Login.pickUnnamed) : choice.name)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgBase)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, Tokens.Space.s12)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDetents([.medium])
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }
}

/// The key is blinking — "touch it now". Sheet content (not a tap target): it
/// clears when the ceremony's next step arrives. Because every app-owned prompt
/// shares ONE bottom sheet, this swaps in over the PIN with no second sheet to
/// conflict with.
struct UsbTouchSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let touch: OnboardingModel.UsbTouch
    /// The phone's sheet only (issue #459): Cancel, a swipe or a tap outside
    /// ends that ceremony, as the desktop's phone card does. A security key's
    /// touch keeps no exit — the key itself is the answer.
    var onCancel: () -> Void = {}

    /// Over caBLE the "authenticator" is the person's phone, and the approval
    /// happens THERE — "touch your security key" would send them hunting for
    /// hardware they never owned (device-found 2026-08-28). Same corpus keys
    /// the desktop and Android use.
    private var remote: Bool { touch.remote }

    private var title: String {
        remote ? loc.t(I18nKeys.Flow.touchRemoteTitle) : loc.t(I18nKeys.Create.touchTitle)
    }

    private var message: String {
        if remote { return loc.t(I18nKeys.Flow.touchRemoteBody) }
        switch touch.kind {
        case "fingerprint":
            return loc.t(I18nKeys.Create.touchFingerprintBody, vars: ["product": touch.product])
        case "select":
            return loc.t(I18nKeys.Create.touchSelectBody)
        default:
            return loc.t(I18nKeys.Create.touchBody, vars: ["product": touch.product])
        }
    }

    var body: some View {
            VStack(spacing: Tokens.Space.s16) {
                Image(systemName: remote ? "iphone.radiowaves.left.and.right" : "key.radiowaves.forward")
                    .font(.system(size: 44, weight: .regular))
                    .foregroundStyle(theme.accentBase)
                    .symbolEffect(.pulse, options: .repeating)

                Text(title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                    .multilineTextAlignment(.center)

                Text(message)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)

                if remote {
                    VelaButton(title: loc.t(I18nKeys.Flow.cancel), kind: .secondary) { onCancel() }
                        .padding(.top, Tokens.Space.s8)
                        .accessibilityIdentifier("cable.touch.cancel")
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.vertical, Tokens.Space.s32)
            .presentationDetents([.medium])
            .presentationDragIndicator(remote ? .visible : .hidden)
            .presentationBackground(theme.bgRaised)
            .interactiveDismissDisabled(!remote)
    }
}

/// "Insert your security key" — up while the ceremony polls for one (issue
/// #450). A missing key is a waitable state, not an error: the sheet closes
/// ITSELF the moment the key answers, so plugging it in is the whole gesture.
/// Close is a cancel.
///
/// And the way out for a key this route can never see: "Use Apple's
/// security-key sheet", for an NFC key, a Lightning key or one on older
/// firmware. The app's own USB route reaches a USB-C key that offers FIDO over
/// its smart-card interface and nothing else, and before this option a person
/// holding any other key waited here for ever. It is offered, never timed:
/// a key not plugged in yet is not a key that cannot answer.
struct UsbInsertKeySheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.dynamicTypeSize) private var typeSize
    let loc: Loc
    /// Hand this ceremony to Apple's security-key sheet instead.
    var onUseSystemSheet: () -> Void = {}
    let onCancel: () -> Void

    /// What the content comes to at a default text size: the height the
    /// sheet opens at (`contentSizedSheet`).
    static let expectedHeight: CGFloat = 440

    var body: some View {
        VStack(spacing: Tokens.Space.s16) {
            Image(systemName: "key.horizontal")
                .font(.system(size: 44, weight: .regular))
                .foregroundStyle(theme.accentBase)
                .symbolEffect(.pulse, options: .repeating)

            Text(loc.t(I18nKeys.Flow.insertKeyTitle))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)

            Text(loc.t(I18nKeys.Flow.insertKeyBody))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)

            VStack(spacing: Tokens.Space.s8) {
                VelaButton(title: loc.t(I18nKeys.Flow.insertKeyAppleSheet), kind: .secondary) {
                    onUseSystemSheet()
                }
                .accessibilityIdentifier("insertKey.appleSheet")
                // Which keys that is for — the reason to press it.
                Text(loc.t(I18nKeys.Flow.insertKeyAppleSheetHint))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
                    .oneLineSubtitle(typeSize)
            }
            .padding(.top, Tokens.Space.s8)

            // Close stays the last thing, at the bottom, as on every
            // ceremony sheet.
            VelaButton(title: loc.t(I18nKeys.Flow.close), kind: .secondary) { onCancel() }
                .accessibilityIdentifier("insertKey.close")
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.top, Tokens.Space.s32)
        .padding(.bottom, Tokens.Space.s16)
        .contentSizedSheet(expected: Self.expectedHeight)
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }
}

/// The brief "connecting…" state that keeps the single bottom sheet up between
/// the person picking a method and the first ceremony prompt arriving — so the
/// sheet never dismisses and re-presents (the nesting bug), it only swaps
/// content. It ALSO covers the gaps between ceremony steps (the registry
/// queries after a signature), so its words must match the route the person
/// picked: it used to hardcode the USB copy, and a person mid-scan sat staring
/// at "USB security key — plug it in and touch it" (device-found 2026-08-28).
struct UsbConnectingSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let method: KeyMethod

    private var title: String {
        switch method {
        case .hybrid: loc.t(I18nKeys.Flow.touchRemoteTitle)
        case .securityKey: loc.t(I18nKeys.Create.methodSecurityKeyTitle)
        case .platform: loc.t(I18nKeys.Create.methodPlatformTitle)
        }
    }

    private var body_: String {
        switch method {
        // Same words as the remote touch prompt, from the shared corpus.
        case .hybrid: loc.t(I18nKeys.Flow.touchRemoteBody)
        case .securityKey: loc.t(I18nKeys.Create.methodSecurityKeyBody)
        // What unlocks a passkey HERE (087 F01) — the core names it.
        case .platform: methodCopy(.platform, chooser: .create, loc: loc).body
        }
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s16) {
            ProgressView()
            Text(title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
            Text(body_)
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDetents([.medium])
        .presentationDragIndicator(.hidden)
        .presentationBackground(theme.bgRaised)
        .interactiveDismissDisabled(true)
    }
}
