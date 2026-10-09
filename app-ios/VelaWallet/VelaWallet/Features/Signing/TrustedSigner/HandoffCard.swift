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
//      Review and sign on a trusted signing page
//      Confirm with                     Phone or tablet
//      ┌──────────────────────────────────────────────┐
//      │ Vela's official signing page  sign.getvela.app│
//      │ ✓ Version 0ba8ee8c · matches Vela's published │
//      │   build list · checked 14:32                  │
//      └──────────────────────────────────────────────┘
//      [ Continue to signing page ]
//
//  The key is a row, drawn as the signing sheet's "Signing account | name"
//  row is: the core's label ("Confirm with", or "New key on" while a ceremony
//  makes the key) and its value (the key's name, else its place).
//
//  The integrity line is what backs the word "trusted": it is the core's
//  ruling on the bytes this phone fetched (`SignerPageChecks`), never a claim
//  of "verified" or "untampered". A refusal takes the line's place, in the
//  warning colour, and the button stays disabled.
//
//  The fee is said once per screen. The screens that hand off — the signing
//  sheet and the send confirm — keep their own fee rows (and the speed, which
//  is chosen before the hand-off, D-18), so the card there draws none. The
//  card's compact fee row (core round 5, `handoffFeeRow`) is for a screen with
//  no fee of its own.
//
//  The send confirm draws the card `compact`, pinned in its footer over the
//  button that goes to the page: its figure and facts already fill a phone,
//  so the block is a small mark beside the title, then the key and the page
//  as the confirm's own raised rows — and the integrity line is on screen,
//  never under the button, whatever the figures above it hold:
//
//      ✓ Review and sign on a trusted signing page
//      ┌──────────────────────────────────────────────┐
//      │ Confirm with                     This device │
//      │──────────────────────────────────────────────│
//      │ Vela's official signing page  sign.getvela.app│
//      │ ✓ Version 0ba8ee8c · matches … · checked 14:32│
//      └──────────────────────────────────────────────┘
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
            ZStack(alignment: .topLeading) {
                // Two lines' room — what a verdict usually takes — so the
                // check landing ("checking" → "… checked 14:32") does not
                // move what is under it. Each reserved line carries a run in
                // the version's mono face too: a line holding the version is
                // a third of a point taller than one without, which moved
                // Open by a pixel when the check landed.
                reserved
                    .typeRole(Typography.flowCaption)
                    .hidden()
                    .accessibilityHidden(true)
                words
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(line.tone == .refused ? theme.errorBase : theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("integrity.\(Self.id(line.state))")
            }
        }
    }

    /// Two empty lines, each as tall as a line holding the version.
    private var reserved: Text {
        let mono = Text(verbatim: "\u{00A0}").font(Typography.monoSmall.font)
        return Text(verbatim: "\u{00A0}") + mono + Text(verbatim: "\n\u{00A0}") + mono
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

/// "Trust this version" — the answer to an `AskToTrust` line, drawn under the
/// line and aligned with its words, on every surface that asks it (D-15):
/// Signing pages, the account's venue rows, the choosers' list and chosen
/// page, and the hand-off card.
struct SigningPageTrustButton: View {
    @Environment(\.theme) private var theme
    let title: String
    /// The UI tests' hook, per surface.
    let id: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typeRole(Typography.actionLabel)
                .foregroundStyle(theme.accentBase)
                .frame(minHeight: Tokens.Layout.hitTarget)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.leading, LucideIconSize.rowGlyph + Tokens.Space.s8)
        .accessibilityIdentifier(id)
    }

    /// The app's one checker stores `version` on the page and checks it
    /// again — what a surface with no host of its own does.
    static func trust(_ url: String, version: String) {
        Task { @MainActor in await SignerPageChecks.shared.trust(url, version: version) }
    }
}

/// The card's fee row: the fee in force, drawn as the sheet's folded fee row
/// draws it, and the speed it was priced at.
struct HandoffFeeModel: Equatable {
    /// "Network fee".
    let label: String
    /// "~$0.01" — or the coin's own amount, as the fee row says it.
    let value: String
    /// "Standard" — `nil` where there is nothing to restate (one speed).
    let tier: String?

    /// `HandoffFee` as the core wrote it (`handoffFeeRow`).
    private struct Wire: Decodable {
        let fee: FeeEstimateWire
        let tier: String?
        let tierKey: String?
    }

    /// The row, from the fee session in force and the speed control as last
    /// rendered — `nil` (no row) when the core says there is none: a message,
    /// or a fee not settled for the speed in force.
    static func of(
        feeJson: String?, speedJson: String?, fee: FeeViewWire?,
        display: WalletLive.Display, networks: WalletNetworks, loc: Loc
    ) -> HandoffFeeModel? {
        guard let json = handoffFeeRow(feeJson: feeJson, speedJson: speedJson),
              let wire = try? CoreJSON.decoder.decode(Wire.self, from: Data(json.utf8))
        else { return nil }
        return HandoffFeeModel(
            label: loc.t("componentsUi.gas.networkFee"),
            value: "~" + SendLive.feeLine(wire.fee, view: nil, fee: fee, display: display, networks: networks),
            tier: wire.tierKey.map { loc.t($0) }
        )
    }
}

/// The key row in words: 「确认方式 | 手机或平板」, 「新钥匙存在 | 这台设备」.
struct HandoffKeyRow: Equatable {
    let label: String
    let value: String

    init(label: String, value: String) {
        self.label = label
        self.value = value
    }

    init(_ key: KeyLabelWire, loc: Loc) {
        self.init(label: key.label(loc), value: key.text(loc))
    }
}

/// What the card says, already in words.
struct HandoffCardModel: Equatable {
    /// `componentsUi.signing.handoffTitle` — or a ceremony's own title.
    let title: String
    /// The page by name — "Vela's official signing page", the person's
    /// label, or "Self-hosted · {{domain}}"…
    let pageName: String
    /// …and by host, drawn beside the name when they differ.
    let page: String
    /// The key row — `nil` when the page is told no single key, and on the
    /// waiting card.
    let key: HandoffKeyRow?
    /// The waiting card's one line under its title: where to look.
    var hint: String? = nil
    let line: SignerIntegrityLine
    /// The line in words.
    let lineText: String
    /// The fee and speed the page will sign — only on a screen that shows no
    /// fee of its own; `nil`: no row.
    var fee: HandoffFeeModel? = nil
    /// The page's address, for "Trust this version".
    var pageUrl: String = ""
    /// "Trust this version" — set while the line asks (`AskToTrust`, a
    /// self-hosted page's own build); `nil` otherwise.
    var trust: String? = nil
    /// The page could not be opened at all (spec 079): the mark warns,
    /// whatever the check said about the bytes.
    var down = false

    /// May the page be opened? The core's answer, never the shell's.
    var opens: Bool { line.opens }

    /// The same card once the page has the request (the web's HO3): the wait
    /// as its title, where to look as its line — the page and its check stay.
    func waiting(title: String, hint: String?, down: Bool) -> HandoffCardModel {
        HandoffCardModel(
            title: title, pageName: pageName, page: page, key: nil, hint: hint,
            line: line, lineText: lineText, fee: nil, pageUrl: pageUrl, trust: nil, down: down
        )
    }

    static func build(
        page: String, keyLabel: KeyLabelWire?, line: SignerIntegrityLine, loc: Loc,
        name: String? = nil, title: String? = nil, fee: HandoffFeeModel? = nil
    ) -> HandoffCardModel {
        HandoffCardModel(
            title: title ?? loc.t("componentsUi.signing.handoffTitle"),
            pageName: SigningPageNames.name(
                url: page, label: name ?? "", official: SigningPageNames.isOfficial(page), loc: loc
            ),
            page: SigningPageNames.host(page),
            key: keyLabel.map { HandoffKeyRow($0, loc: loc) },
            line: line,
            lineText: line.text(loc),
            fee: fee,
            pageUrl: page,
            trust: line.state == .askToTrust ? loc.t("settings.signing.pageTrust") : nil
        )
    }
}

/// The card's body (the web's `HandoffCard`): a mark that says what the line
/// says — a shield only for a page that passed — the title, which key will
/// confirm, and one quiet box holding the page and what was checked about it.
/// Buttons are the host's.
struct HandoffCardView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let model: HandoffCardModel
    /// "Trust this version". `nil`: the app's one checker stores it on the
    /// page and checks again (`SignerPageChecks.trustAsked`).
    var onTrust: (() -> Void)? = nil
    /// The send confirm's form (102 integration polish): the screen already
    /// holds the figure, its facts and a pinned button, so the block is a
    /// section of that screen, not a page of its own — a small mark beside
    /// the title on one line, then the key and the page as the confirm's own
    /// quiet rows. The big mark and title cost a 6.1" phone the integrity
    /// line, which sat half under the pinned button.
    var compact = false

    var body: some View {
        if compact { compactBody } else { fullBody }
    }

    private var compactBody: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                inlineMark
                    .frame(width: LucideIconSize.rowGlyph, height: LucideIconSize.rowGlyph)
                    .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + Tokens.Space.s4 }
                Text(model.title)
                    .typeRole(Typography.bodyStrong.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("handoff.title")
            }
            .padding(.horizontal, Tokens.Space.s4)

            // The key and the page, as the confirm's facts are drawn — one
            // raised card of label | value rows above the pinned button.
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                if let key = model.key {
                    HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                        Text(verbatim: key.label)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                            .lineLimit(1)
                        Spacer(minLength: Tokens.Space.s8)
                        Text(verbatim: key.value)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .multilineTextAlignment(.trailing)
                    }
                    .padding(.vertical, Tokens.Space.s12)
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("handoff.key")
                    FlowDivider()
                }
                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    ViewThatFits(in: .horizontal) {
                        HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                            pageName.fixedSize()
                            pageHost.fixedSize()
                        }
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            pageName
                            pageHost
                        }
                    }
                    IntegrityLineView(line: model.line, text: model.lineText)
                    if let trust = model.trust { trustButton(trust) }
                }
                .padding(.vertical, Tokens.Space.s12)
            }
            .padding(.horizontal, Tokens.Space.s12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("handoff.card")
    }

    /// The compact form's mark: the big mark's verdict at a row glyph's size
    /// — a shield only for a page that passed — in a fixed box, so the check
    /// landing swaps the glyph without moving the title.
    @ViewBuilder private var inlineMark: some View {
        let tone: SignerIntegrityTone = model.down ? .caution : model.line.tone
        switch tone {
        case .checking:
            ProgressView().controlSize(.mini)
        case .ok:
            LucideIcon(.shieldCheck, size: LucideIconSize.rowGlyph)
                .foregroundStyle(theme.successBase)
                .accessibilityHidden(true)
        case .caution, .refused:
            LucideIcon(.triangleAlert, size: LucideIconSize.rowGlyph)
                .foregroundStyle(tone == .refused ? theme.errorBase : theme.warningBase)
                .accessibilityHidden(true)
        }
    }

    private func trustButton(_ title: String) -> some View {
        SigningPageTrustButton(title: title, id: "handoff.trust") {
            if let onTrust {
                onTrust()
            } else {
                let page = model.pageUrl
                Task { @MainActor in await SignerPageChecks.shared.trustAsked(page) }
            }
        }
    }

    private var fullBody: some View {
        VStack(spacing: Tokens.Space.s12) {
            mark
                .padding(.bottom, Tokens.Space.s4)
            Text(model.title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("handoff.title")
            if let hint = model.hint {
                Text(hint)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("handoff.hint")
            }
            // Which key confirms, as a row — the sheet's "Signing account |
            // name" row's roles, colours and alignment (`SigningSignerRow`).
            if let key = model.key {
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                    Text(verbatim: key.label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                    Spacer(minLength: Tokens.Space.s8)
                    Text(verbatim: key.value)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .multilineTextAlignment(.trailing)
                }
                .padding(.top, Tokens.Space.s4)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("handoff.key")
            }

            // What is trusted, in one place: the page, and what was checked.
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                // The name, and the host beside it — under it when the two do
                // not fit on one line; never the host twice.
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                        pageName.fixedSize()
                        pageHost.fixedSize()
                    }
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        pageName
                        pageHost
                    }
                }
                if let fee = model.fee {
                    HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                        Text(fee.label)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgMuted)
                        Spacer(minLength: Tokens.Space.s8)
                        Text(verbatim: fee.tier.map { "\(fee.value) · \($0)" } ?? fee.value)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgBase)
                            .multilineTextAlignment(.trailing)
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("handoff.fee")
                }
                IntegrityLineView(line: model.line, text: model.lineText)
                // The answer to "Trust it on this device?", beside the question.
                if let trust = model.trust { trustButton(trust) }
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

    private var pageName: some View {
        Text(model.pageName)
            .typeRole(Typography.bodyStrong)
            .foregroundStyle(theme.fgBase)
    }

    @ViewBuilder private var pageHost: some View {
        if !model.pageName.contains(model.page) {
            Text(model.page)
                .typeRole(Typography.monoSmall)
                .foregroundStyle(theme.fgSubtle)
                .lineLimit(1)
                .truncationMode(.middle)
        }
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
