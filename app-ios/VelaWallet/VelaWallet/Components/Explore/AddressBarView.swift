//
//  AddressBarView.swift
//  VelaWallet
//
//  The browsing top bar — the ONE bar the browser draws on a phone (DESIGN N,
//  web board E4). The app's tab bar stays under the page, so the wallet is
//  one tap away from any dApp, and everything the old bottom toolbar held
//  moved up here or into the site menu:
//
//      ‹  [ 🔒 host ]  (account)  [n]  ⋯
//
//  - ‹ walks the page's history; with none left it returns to the Explore
//    home, tab kept alive. Never greyed: there is always somewhere to go back
//    to. (The edge swipe stays the page's own back.)
//  - The pill shows the DOMAIN, never the full URL, and when it must be cut
//    it loses its START: the end of a host is the registrable domain, the
//    part that decides who you are talking to — `app.uniswap.org.evil.xyz`
//    must never read as `app.uniswap.or…`.
//  - Tapping the pill edits the address (spec 070 US3): the full URL,
//    selected, ready to change — read by the core's `dappBrowserInput` like
//    the home's field. While it is edited the account, the count and ⋯ step
//    aside, and ‹ only puts the field away.
//  - The account's green dot IS the connection state; no dot is "not
//    connected", never a dimmed avatar (dimmed means disabled).
//  - The boxed count opens the tab switcher.
//

import SwiftUI
import UIKit

struct AddressBarView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let host: String
    let secure: Bool
    /// `explore.back`.
    let backLabel: String
    let menuLabel: String
    /// What the page is said to be when it is NOT secure. The padlock is
    /// drawn only for a secure origin; an insecure one is named, not merely
    /// left unpraised.
    var insecureLabel: String = ""
    /// Spec 082 RE1: whether any lock is drawn. A failure panel, and a load
    /// that has not committed in an empty tab, show NO lock — nothing from
    /// that host is on screen to vouch for (the core's `browserAddressBar`).
    var showsLock: Bool = true
    /// The full URL, for editing.
    var url: String = ""
    var addressLabel: String = ""
    /// 0…1 while a page loads, `nil` when nothing is loading.
    var progress: Double?
    /// The account the page sees, its connection, and the strip's size.
    var accountSeed: String = ""
    var connected: Bool = false
    /// `explore.account`, and `explore.connectedTag` added after it while
    /// the site is connected.
    var accountLabel: String = ""
    var connectedLabel: String = ""
    var tabCount: Int = 1
    /// `explore.tabs`.
    var tabsLabel: String = ""
    var onBack: () -> Void = {}
    var onAccount: () -> Void = {}
    var onTabs: () -> Void = {}
    var onMenu: () -> Void = {}
    /// Present = the pill can be edited, and a submitted address opens here.
    var onSubmit: ((String) -> Void)?

    @State private var editing = false
    @State private var draft = ""
    @FocusState private var focused: Bool

    /// The core caps the strip at 24, so two digits is the most it shows;
    /// past 99 the box says so rather than growing.
    static func countText(_ count: Int) -> String {
        count > 99 ? "99+" : String(count)
    }

    /// The account's spoken name: the connection is said, not only drawn.
    static func accountName(_ account: String, connected: Bool, tag: String) -> String {
        connected && !tag.isEmpty ? "\(account), \(tag)" : account
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            Button(action: back) {
                LucideIcon(.chevronLeft, size: LucideIconSize.browserBarGlyph)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(backLabel)
            .accessibilityIdentifier("explore.bar.back")

            if editing {
                field
                    // Ends a gutter in from the edge, as ‹'s glyph starts one
                    // in from the other.
                    .padding(.trailing, Tokens.Space.s12)
            } else {
                // A tap, not a `Button`: a button would fold the host and the
                // padlock into one element, and the host is what a person
                // (and the device suite) reads the page by.
                pill
                    .frame(maxWidth: .infinity)
                    .frame(height: ExploreGeometry.addressPill)
                    .background(theme.bgRaised, in: Capsule())
                    .onTapGesture(perform: startEditing)
                cluster
            }
        }
        .padding(.horizontal, Tokens.Space.s4)
        .padding(.vertical, Tokens.Space.s8)
        .background(theme.bgBase)
        .overlay(alignment: .bottom) { progressBar }
        .onChange(of: focused) { _, isFocused in
            if !isFocused { editing = false }
        }
    }

    /// ‹ — while the address is edited it only puts the field away; the page
    /// under it is untouched.
    private func back() {
        if editing {
            focused = false
            editing = false
            return
        }
        onBack()
    }

    private func startEditing() {
        guard onSubmit != nil else { return }
        draft = url
        editing = true
        focused = true
        // The whole address, selected on arrival — ready to be replaced. A
        // `TextField` selection binding needs iOS 18; the responder's own
        // select-all reaches the field the moment it is first responder.
        DispatchQueue.main.async {
            UIApplication.shared.sendAction(#selector(UIResponder.selectAll(_:)), to: nil, from: nil, for: nil)
        }
    }

    private var field: some View {
        TextField(addressLabel, text: $draft)
            .font(Typography.body.scaled(textScale).font)
            .foregroundStyle(theme.fgBase)
            .tint(theme.accentBase)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.URL)
            .submitLabel(.go)
            .focused($focused)
            .onSubmit {
                editing = false
                let typed = draft.trimmingCharacters(in: .whitespaces)
                if !typed.isEmpty { onSubmit?(draft) }
            }
            .padding(.horizontal, Tokens.Space.s12)
            .frame(maxWidth: .infinity)
            .frame(height: ExploreGeometry.addressPill)
            .background(theme.bgRaised, in: Capsule())
            .accessibilityLabel(addressLabel)
            .accessibilityIdentifier("explore.addressField")
    }

    private var pill: some View {
        HStack(spacing: Tokens.Space.s8) {
            // Spec 079 (owner): a lock, and only a lock. Closed and quiet for
            // https — which says the line is encrypted, not that the site is
            // honest, so it is decorative to a screen reader too (the host is
            // read out); open and in the warning colour for plain http.
            if !showsLock {
                // Nothing on screen from this host: no lock either way.
            } else if secure {
                LucideIcon(.lock, size: LucideIconSize.addressLock)
                    .foregroundStyle(theme.fgMuted)
                    .accessibilityHidden(true)
                    .accessibilityIdentifier("explore.lock")
            } else if !host.isEmpty, !insecureLabel.isEmpty {
                LucideIcon(.lockOpen, size: LucideIconSize.addressLock)
                    .foregroundStyle(theme.warningBase)
                    .accessibilityLabel(insecureLabel)
                    .accessibilityIdentifier("explore.insecure")
            }
            Text(verbatim: host)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
                // Cut from the START (see the file's doc): the registrable
                // domain at the end always shows.
                .truncationMode(.head)
                .accessibilityHint(onSubmit == nil ? "" : addressLabel)
                .accessibilityAction(named: Text(verbatim: addressLabel), startEditing)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Capsule())
    }

    /// The account, the count and ⋯ read as one cluster: their 44s touch.
    private var cluster: some View {
        HStack(spacing: Tokens.Space.s0) {
            Button(action: onAccount) {
                IdenticonAvatar(seed: accountSeed, size: ExploreGeometry.barAvatar, tappable: false)
                    .overlay(alignment: .bottomTrailing) {
                        if connected { connectedDot }
                    }
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Self.accountName(accountLabel, connected: connected, tag: connectedLabel))
            .accessibilityIdentifier("explore.bar.account")

            Button(action: onTabs) {
                Text(verbatim: Self.countText(tabCount))
                    .typeRole(Typography.tabCount.scaled(textScale))
                    .monospacedDigit()
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                    .fixedSize()
                    .padding(.horizontal, Tokens.Space.s4)
                    .frame(minWidth: ExploreGeometry.tabCount, minHeight: ExploreGeometry.tabCount)
                    .overlay(
                        RoundedRectangle(cornerRadius: Tokens.Radius.r4)
                            .stroke(theme.fgBase, lineWidth: Tokens.BorderWidth.emphasis)
                    )
                    .frame(minWidth: Tokens.Layout.hitTarget, minHeight: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(tabsLabel)
            .accessibilityValue(Self.countText(tabCount))
            .accessibilityIdentifier("explore.bar.tabs")

            Button(action: onMenu) {
                LucideIcon(.ellipsis, size: LucideIconSize.browserBarGlyph)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(menuLabel)
            .accessibilityIdentifier("explore.bar.menu")
        }
        .fixedSize()
    }

    /// The connection, as one green dot on the account's edge — ringed in the
    /// bar's own colour so it reads on any artwork.
    private var connectedDot: some View {
        Circle()
            .fill(theme.successBase)
            .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
            .padding(Tokens.BorderWidth.emphasis)
            .background(Circle().fill(theme.bgBase))
            .offset(x: Tokens.BorderWidth.emphasis, y: Tokens.BorderWidth.emphasis)
            .accessibilityHidden(true)
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
