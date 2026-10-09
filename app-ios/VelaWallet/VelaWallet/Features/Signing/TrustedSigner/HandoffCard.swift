//
//  HandoffCard.swift
//  VelaWallet
//
//  The hand-off card (spec 102 D4) and the one integrity line every surface
//  draws about a signing page.
//
//  When an account's venue is a trusted page, the app does not preview the
//  transaction a second time — the page is the authority. What the app says
//  is short and checkable:
//
//      Review and sign on your trusted page
//      sign.getvela.app
//      ┌──────────────────────────────────────────────┐
//      │ 🔒 Confirm with This device                   │
//      │ ✓  Version 0ba8ee8c · matches Vela's          │
//      │    published build list · checked now         │
//      └──────────────────────────────────────────────┘
//      [ Continue to signing page ]
//
//  The integrity line is what backs the word "trusted": it is the core's
//  ruling on the bytes this phone fetched (`SignerPageChecks`), never a claim
//  of "verified" or "untampered". A refusal takes the line's place, in the
//  warning colour, and the button stays disabled.
//

import SwiftUI
import VelaCore

/// One integrity line: a spinner while the page is checked, a check when it
/// matches, a warning when it will not open.
struct IntegrityLineView: View {
    @Environment(\.theme) private var theme
    let line: SignerIntegrityLine
    /// The line in words (`SignerIntegrityLine.text`).
    let text: String

    init(line: SignerIntegrityLine, text: String) {
        self.line = line
        self.text = text
    }

    init(loc: Loc, line: SignerIntegrityLine, nowMs: Double = Date().timeIntervalSince1970 * 1000) {
        self.init(line: line, text: line.text(loc, nowMs: nowMs))
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
            glyph
                .frame(width: LucideIconSize.action, alignment: .center)
                .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + Tokens.Space.s4 }
            Text(text)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(color)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("integrity.\(Self.id(line.state))")
        }
    }

    @ViewBuilder private var glyph: some View {
        switch line.tone {
        case .checking:
            ProgressView().controlSize(.mini)
        case .ok:
            LucideIcon(.check, size: LucideIconSize.rowGlyph)
                .foregroundStyle(theme.successBase)
                .accessibilityHidden(true)
        case .caution, .refused:
            LucideIcon(.triangleAlert, size: LucideIconSize.rowGlyph)
                .foregroundStyle(line.tone == .refused ? theme.errorBase : theme.warningBase)
                .accessibilityHidden(true)
        }
    }

    private var color: Color {
        switch line.tone {
        case .checking: theme.fgMuted
        case .ok: theme.fgMuted
        case .caution: theme.warningBase
        case .refused: theme.errorBase
        }
    }

    /// A stable id per state, for the UI tests.
    static func id(_ state: SignerIntegrityState) -> String {
        switch state {
        case .checking: "checking"
        case .matches: "matches"
        case .trustedHere: "trusted"
        case .unchecked: "unchecked"
        case .mismatch: "mismatch"
        case .blocked: "blocked"
        case .askToTrust: "askTrust"
        case .couldNotCheck: "couldNotCheck"
        case .noVersion: "noVersion"
        case .allBlocked: "allBlocked"
        }
    }
}

/// What the card says, already in words.
struct HandoffCardModel: Equatable {
    /// `componentsUi.signing.handoffTitle`.
    let title: String
    /// The page, by its host — which page is about to open.
    let page: String
    /// "Confirm with {{key}}" — `nil` when the page is told no single key.
    let key: String?
    let line: SignerIntegrityLine
    /// The line in words.
    let lineText: String

    /// May the page be opened? The core's answer, never the shell's.
    var opens: Bool { line.opens }

    static func build(page: String, keyLabel: String?, line: SignerIntegrityLine, loc: Loc) -> HandoffCardModel {
        HandoffCardModel(
            title: loc.t("componentsUi.signing.handoffTitle"),
            page: SigningPageNames.host(page),
            key: keyLabel.map { loc.t("componentsUi.signing.handoffKey", vars: ["key": $0]) },
            line: line,
            lineText: line.text(loc)
        )
    }
}

/// The card's body: title, the page, and a quiet panel holding the key and
/// the integrity line. Buttons are the host's — the signing sheet and the
/// Trusted Signer's own sheet each have their own row of them.
struct HandoffCardView: View {
    @Environment(\.theme) private var theme
    let model: HandoffCardModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                ZStack {
                    Circle().fill(theme.accentSoft)
                    LucideIcon(.lock, size: LucideIconSize.action)
                        .foregroundStyle(theme.accentBase)
                        .accessibilityHidden(true)
                }
                .frame(width: FlowGeometry.badgeSize, height: FlowGeometry.badgeSize)
                .padding(.bottom, Tokens.Space.s4)

                Text(model.title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("handoff.title")
                HStack(spacing: Tokens.Space.s4) {
                    LucideIcon(.globe, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                        .accessibilityHidden(true)
                    Text(model.page)
                        .typeRole(Typography.monoSmall)
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                if let key = model.key {
                    HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                        LucideIcon(.userRound, size: LucideIconSize.rowGlyph)
                            .foregroundStyle(theme.fgMuted)
                            .frame(width: LucideIconSize.action)
                            .accessibilityHidden(true)
                        Text(key)
                            .typeRole(Typography.bodyStrong)
                            .foregroundStyle(theme.fgBase)
                            .fixedSize(horizontal: false, vertical: true)
                            .accessibilityIdentifier("handoff.key")
                    }
                    Divider().overlay(theme.borderBase)
                }
                IntegrityLineView(line: model.line, text: model.lineText)
            }
            .padding(Tokens.Space.s16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .stroke(model.line.tone == .refused ? theme.errorBase.opacity(Tokens.Opacity.dim) : theme.borderBase,
                            lineWidth: Tokens.BorderWidth.hairline)
            )
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("handoff.card")
    }
}
