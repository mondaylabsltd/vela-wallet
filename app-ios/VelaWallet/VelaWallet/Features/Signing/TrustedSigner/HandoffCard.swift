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

/// One integrity line: a spinner while the page is checked, a shield when it
/// passed, a warning when it will not open. The version is set in the mono
/// face so it reads as the hash it is; a refusal is said in the line's own
/// colour too, and leads with its glyph — a line that is red only by colour is
/// one some people cannot see is red. (The web's `IntegrityLine`.)
struct IntegrityLineView: View {
    @Environment(\.theme) private var theme
    let line: SignerIntegrityLine
    /// The line in words (`SignerIntegrityLine.text`).
    let text: String

    init(line: SignerIntegrityLine, text: String) {
        self.line = line
        self.text = text
    }

    init(loc: Loc, line: SignerIntegrityLine) {
        self.init(line: line, text: line.text(loc))
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
            glyph
                .frame(width: LucideIconSize.rowGlyph)
                .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + Tokens.Space.s4 }
            words
                .typeRole(Typography.flowCaption)
                .foregroundStyle(line.tone == .refused ? theme.errorBase : theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("integrity.\(Self.id(line.state))")
        }
    }

    /// The version — eight hex characters — in the mono face.
    private var words: Text {
        guard !line.version.isEmpty, let range = text.range(of: line.version) else { return Text(text) }
        let version = Text(String(text[range]))
            .font(Typography.monoSmall.font)
            .foregroundColor(line.tone == .refused ? theme.errorBase : theme.fgBase)
        return Text(String(text[..<range.lowerBound])) + version + Text(String(text[range.upperBound...]))
    }

    @ViewBuilder private var glyph: some View {
        switch line.tone {
        case .checking:
            ProgressView().controlSize(.mini)
        case .ok:
            LucideIcon(.shieldCheck, size: LucideIconSize.rowGlyph)
                .foregroundStyle(theme.successBase)
                .accessibilityHidden(true)
        case .caution, .refused:
            LucideIcon(.triangleAlert, size: LucideIconSize.rowGlyph)
                .foregroundStyle(line.tone == .refused ? theme.errorBase : theme.warningBase)
                .accessibilityHidden(true)
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
    /// The page by name — "Official", the person's label, or its host…
    let pageName: String
    /// …and by host, drawn beside the name when they differ.
    let page: String
    /// "Confirm with {{key}}" — `nil` when the page is told no single key.
    let key: String?
    let line: SignerIntegrityLine
    /// The line in words.
    let lineText: String
    /// The page could not be opened at all (spec 079): the mark warns,
    /// whatever the check said about the bytes.
    var down = false

    /// May the page be opened? The core's answer, never the shell's.
    var opens: Bool { line.opens }

    /// The same card once the page has the request (the web's HO3): the wait
    /// as its title, where to look as its line — the page and its check stay.
    func waiting(title: String, hint: String?, down: Bool) -> HandoffCardModel {
        HandoffCardModel(
            title: title, pageName: pageName, page: page, key: hint,
            line: line, lineText: lineText, down: down
        )
    }

    static func build(
        page: String, keyLabel: String?, line: SignerIntegrityLine, loc: Loc,
        name: String? = nil
    ) -> HandoffCardModel {
        let host = SigningPageNames.host(page)
        let official = signingPageDomain(url: page) == SigningPlanWire.appDomain
            && SignerPageChecks.key(page) == SignerPageChecks.key(trustedSignerDefaultUrl())
        return HandoffCardModel(
            title: loc.t("componentsUi.signing.handoffTitle"),
            pageName: name ?? (official ? loc.t("settings.signing.pageOfficial") : host),
            page: host,
            key: keyLabel.map { loc.t("componentsUi.signing.handoffKey", vars: ["key": $0]) },
            line: line,
            lineText: line.text(loc)
        )
    }
}

/// The card's body (the web's `HandoffCard`): a mark that says what the line
/// says — a shield only for a page that passed — the title, which key will
/// confirm, and one quiet box holding the page and what was checked about it.
/// Buttons are the host's.
struct HandoffCardView: View {
    @Environment(\.theme) private var theme
    let model: HandoffCardModel

    var body: some View {
        VStack(spacing: Tokens.Space.s12) {
            mark
                .padding(.bottom, Tokens.Space.s4)
            Text(model.title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("handoff.title")
            if let key = model.key {
                Text(key)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("handoff.key")
            }

            // What is trusted, in one place: the page, and what was checked.
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                    Text(model.pageName)
                        .typeRole(Typography.bodyStrong)
                        .foregroundStyle(theme.fgBase)
                    if model.pageName != model.page {
                        Text(model.page)
                            .typeRole(Typography.monoSmall)
                            .foregroundStyle(theme.fgSubtle)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
                IntegrityLineView(line: model.line, text: model.lineText)
            }
            .padding(.horizontal, Tokens.Space.s20)
            .padding(.vertical, Tokens.Space.s16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r16))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                    .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
            )
            .padding(.top, Tokens.Space.s12)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, Tokens.Space.s16)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("handoff.card")
    }

    private var mark: some View {
        let tone: SignerIntegrityTone = model.down ? .caution : model.line.tone
        let (glyph, fg, bg): (LucideGlyph, Color, Color) = switch tone {
        case .ok: (.shieldCheck, theme.successBase, theme.successSoft)
        case .refused: (.triangleAlert, theme.errorBase, theme.errorSoft)
        case .caution: (.triangleAlert, theme.warningBase, theme.warningSoft)
        // Not yet a verdict: calm, and no glyph that says either way.
        case .checking: (.triangleAlert, theme.fgMuted, theme.bgSunken)
        }
        return ZStack {
            Circle().fill(bg)
            if tone == .checking {
                ProgressView().tint(fg)
            } else {
                LucideIcon(glyph, size: LucideIconSize.tabBar).foregroundStyle(fg)
            }
        }
        .frame(width: FlowGeometry.badgeSize, height: FlowGeometry.badgeSize)
        .accessibilityHidden(true)
    }
}
