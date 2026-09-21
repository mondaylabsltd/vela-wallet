//
//  AddressBarView.swift
//  VelaWallet
//
//  The browsing top bar (mock E4): close, the domain in a pill with its
//  padlock, and the site menu. The pill shows the DOMAIN, never the full
//  URL — the part of an address that decides who you are talking to must
//  not be pushed off the end by a long path.
//
//  Tapping the pill edits the address (spec 070 US3): the full URL, ready to
//  change — a search or another site, read by the core's `dappBrowserInput`
//  like the start page's field.
//

import SwiftUI

struct AddressBarView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let host: String
    let secure: Bool
    let secureLabel: String
    let closeLabel: String
    let menuLabel: String
    /// What the page is said to be when it is NOT secure. The padlock is
    /// drawn only for a secure origin; an insecure one is named, not merely
    /// left unpraised.
    var insecureLabel: String = ""
    /// The full URL, for editing.
    var url: String = ""
    var addressLabel: String = ""
    /// 0…1 while a page loads, `nil` when nothing is loading.
    var progress: Double?
    var onClose: () -> Void = {}
    var onMenu: () -> Void = {}
    /// Present = the pill can be edited, and a submitted address opens here.
    var onSubmit: ((String) -> Void)?

    @State private var editing = false
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            Button(action: onClose) {
                LucideIcon(.close, size: LucideIconSize.browserBarGlyph)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(closeLabel)

            Group {
                if editing {
                    TextField(addressLabel, text: $draft)
                        .font(Typography.body.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .submitLabel(.go)
                        .focused($focused)
                        .onSubmit {
                            editing = false
                            onSubmit?(draft)
                        }
                        .padding(.horizontal, Tokens.Space.s16)
                        .accessibilityIdentifier("explore.addressField")
                } else {
                    // A tap, not a `Button`: a button would fold the host and
                    // the padlock into one element, and the host is what a
                    // person (and the device suite) reads the page by.
                    pill.onTapGesture(perform: startEditing)
                }
            }
            .frame(maxWidth: .infinity)
            .frame(height: ExploreGeometry.addressPill)
            .background(theme.bgRaised, in: Capsule())

            Button(action: onMenu) {
                LucideIcon(.ellipsis, size: LucideIconSize.browserBarGlyph)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(menuLabel)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .padding(.vertical, Tokens.Space.s8)
        .overlay(alignment: .bottom) { progressBar }
        .onChange(of: focused) { _, isFocused in
            if !isFocused { editing = false }
        }
    }

    private func startEditing() {
        guard onSubmit != nil else { return }
        draft = url
        editing = true
        focused = true
    }

    private var pill: some View {
        HStack(spacing: Tokens.Space.s8) {
            if secure {
                LucideIcon(.lock, size: LucideIconSize.addressLock)
                    .foregroundStyle(theme.fgMuted)
                    .accessibilityLabel(secureLabel)
                    .accessibilityIdentifier("explore.lock")
            } else if !host.isEmpty, !insecureLabel.isEmpty {
                LucideIcon(.triangleAlert, size: LucideIconSize.addressLock)
                    .foregroundStyle(theme.warningBase)
                    .accessibilityLabel(insecureLabel)
                    .accessibilityIdentifier("explore.insecure")
            }
            Text(verbatim: host)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
                .accessibilityHint(onSubmit == nil ? "" : addressLabel)
                .accessibilityAction(named: Text(verbatim: addressLabel), startEditing)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Capsule())
    }

    /// A hairline that fills as the page loads — the only sign, on a slow
    /// network, that a tap did something.
    @ViewBuilder private var progressBar: some View {
        if let progress, progress < 1 {
            GeometryReader { proxy in
                Rectangle()
                    .fill(theme.accentBase)
                    .frame(width: proxy.size.width * max(progress, 0.05))
                    .animation(.easeOut(duration: Tokens.Motion.fast), value: progress)
            }
            .frame(height: Tokens.BorderWidth.emphasis)
            .accessibilityHidden(true)
        }
    }
}
