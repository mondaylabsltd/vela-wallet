//
//  WelcomeScreen.swift
//  VelaWallet
//
//  The onboarding welcome screen — composition only (FR-009).
//
//  The v2 design (docs/design/onboarding-new, founder direction 2026-08-25), which
//  the web and the desktop already draw: brand row, a two-line headline with
//  one supporting sentence, and the two ways in at the bottom. The six-card
//  carousel is gone — the design is one column that says what the wallet IS
//  before it says what to do about it, and a deck of feature cards nobody
//  swipes past the first of was the opposite of that.
//

import SwiftUI

struct WelcomeScreen: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    @Bindable var model: WelcomeModel
    /// The login machine's `busy`. Signing in has no screen of its own — the
    /// system passkey sheet is the next thing the person sees, and it does not
    /// arrive in the same frame as the press — so this button IS the progress
    /// indicator for that wait. It stays at full emphasis with a spinner in
    /// place of its label; a control that dimmed instead would read as the app
    /// having gone unavailable rather than gone to work.
    var signingIn: Bool = false

    private var heroRole: TypeRole { model.content.heroTitleFit.role }

    var body: some View {
        // Two blocks, not one centred stack: brand and copy ride the top edge,
        // the CTAs ride the bottom, and the space between them is whatever the
        // phone has left over.
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: WelcomeGeometry.brandHeroGap) {
                BrandRow()

                VStack(alignment: .leading, spacing: WelcomeGeometry.heroSubGap) {
                    // The copy carries its own line break: every locale breaks
                    // where its own sentence wants to, not where 390pt runs out.
                    // Its SIZE comes from the same place for the same reason —
                    // a line that is 10.9em wide in Russian and 6.9em in Chinese
                    // cannot be set at one size and still fit 342pt.
                    Text(model.content.heroTitle)
                        .typeRole(heroRole)
                        .tracking(heroRole.size * WelcomeGeometry.heroTracking)
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)

                    Text(model.content.heroSubtitle)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            Spacer(minLength: WelcomeGeometry.heroCtaMinGap)

            VStack(spacing: WelcomeGeometry.ctaGap) {
                VelaButton(title: model.content.createWallet, kind: .primary, enabled: !signingIn) {
                    model.send(.createWallet)
                }
                // Signing in offers the SAME three authenticators creating does
                // — this device, a nearby device by scan, a hardware key — so a
                // wallet that lives on a security key is reachable even when a
                // platform passkey is also present. The picker opens on tap.
                VelaButton(title: model.content.alreadyHaveWallet, kind: .secondary, loading: signingIn) {
                    model.send(.openSignIn)
                }
            }
        }
        .padding(.top, Tokens.Space.s32)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.bottom, Tokens.Space.s8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(theme.bgBase.ignoresSafeArea())
    }
}

/// The three ways to sign in — this device, a nearby device by scan, a hardware
/// security key — the same set creating a wallet offers per key. `hybrid` (the
/// scan) is present-but-unavailable until the caBLE client lands (feature 020's
/// remnant in T174), exactly as it is on the create key screen.
struct SignInMethodSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let onPick: (KeyMethod) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(loc.t(I18nKeys.Login.header))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .padding(.bottom, Tokens.Space.s16)

            ForEach(KeyMethod.allCases, id: \.self) { method in
                // All three routes are live now: platform, scan (our caBLE
                // initiator, BLE-only capable), and a security key.
                let available = true
                let copy = methodCopy(method)
                Button { if available { onPick(method) } } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            Text(loc.t(copy.title))
                                .typeRole(Typography.rowTitle)
                                .foregroundStyle(theme.fgBase)
                            Text(loc.t(available ? copy.body : I18nKeys.Create.methodHybridUnavailable))
                                .typeRole(Typography.flowCaption)
                                .foregroundStyle(theme.fgMuted)
                                .multilineTextAlignment(.leading)
                        }
                        Spacer()
                        if available {
                            Image(systemName: "chevron.right").foregroundStyle(theme.fgSubtle)
                        }
                    }
                    .frame(minHeight: Tokens.Layout.hitTarget)
                    .padding(.vertical, Tokens.Space.s8)
                }
                .disabled(!available)
                .opacity(available ? 1 : Tokens.Opacity.disabled)
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

#Preview("Welcome") {
    WelcomeScreen(
        loc: Loc(),
        model: WelcomeModel(
            content: WelcomeContent(
                heroTitle: "An Ethereum wallet\nyou actually own",
                heroTitleFit: .regular,
                heroSubtitle: "Signing is done on your device. Your passkey’s private key never goes to Vela.",
                createWallet: "Create Wallet",
                alreadyHaveWallet: "I already have a wallet"
            ),
            onIntent: { _ in }
        )
    )
    .themed(.light)
}
