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
/// security key — the same set creating a wallet offers per key. Three places,
/// no fourth (spec 102).
///
/// Below them, apart: "Use my own signing page". It is not a place a key
/// lives; it says where this sign-in runs and where the account will review
/// and sign — a page on the person's own domain runs the ceremony itself (its
/// keys answer nowhere else), a `getvela.app` page signs in here and becomes
/// the account's venue. The chosen page heads the list, with the domain its
/// keys live on and this phone's check of it.
struct SignInMethodSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let onPick: (KeyMethod) -> Void
    /// Spec 102: Vela's own and every page this device trusts, the chosen one
    /// marked. Empty hides the entry.
    var pageChoices: [SigningPageChoiceModel] = []
    /// The page this sign-in will run on; `nil` signs in in the app.
    var chosenPage: String?
    var onChoosePage: ((String?) -> Void)?
    var onAddPage: ((String) -> Void)?
    var pageAddError: String?
    /// The page list is on screen: check the pages it lists.
    var onPagesShown: () -> Void = {}

    @State private var picking = false
    @State private var detent: PresentationDetent = .medium

    private var chosen: SigningPageChoiceModel? {
        guard let page = chosenPage else { return nil }
        return pageChoices.first { $0.url.map(SignerPageChecks.key) == SignerPageChecks.key(page) }
    }

    var body: some View {
        Group {
            if picking {
                SigningPagePicker(
                    loc: loc,
                    choices: pageChoices,
                    onPick: { url in
                        onChoosePage?(url)
                        picking = false
                        detent = url == nil ? .medium : .large
                    },
                    onAdd: onAddPage,
                    addError: pageAddError,
                    onClose: { picking = false }
                )
                .task { onPagesShown() }
            } else {
                methods
            }
        }
        .presentationDetents([.medium, .large], selection: $detent)
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }

    private var methods: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(loc.t(I18nKeys.Login.header))
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                    .padding(.bottom, Tokens.Space.s16)

                if let chosen {
                    ChosenSigningPageCard(loc: loc, choice: chosen) {
                        picking = true
                        detent = .large
                    }
                    .padding(.bottom, Tokens.Space.s12)
                }

                ForEach(KeyMethod.allCases, id: \.self) { method in
                    let copy = methodCopy(method, chooser: .signIn, loc: loc)
                    Button { onPick(method) } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                                Text(copy.title)
                                    .typeRole(Typography.rowTitle)
                                    .foregroundStyle(theme.fgBase)
                                Text(copy.body)
                                    .typeRole(Typography.flowCaption)
                                    .foregroundStyle(theme.fgMuted)
                                    .multilineTextAlignment(.leading)
                            }
                            Spacer()
                            Image(systemName: "chevron.right").foregroundStyle(theme.fgSubtle)
                        }
                        .frame(minHeight: Tokens.Layout.hitTarget)
                        .padding(.vertical, Tokens.Space.s8)
                    }
                }

                if chosen == nil, onChoosePage != nil, !pageChoices.isEmpty {
                    Divider().overlay(theme.borderBase).padding(.vertical, Tokens.Space.s8)
                    OwnSigningPageEntry(loc: loc) {
                        picking = true
                        detent = .large
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.vertical, Tokens.Space.s32)
        }
        .scrollBounceBehavior(.basedOnSize)
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
