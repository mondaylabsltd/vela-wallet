//
//  AgentAdvice.swift
//  VelaWallet
//
//  The person's AI agent's opinion on the dApp request on the signing sheet
//  (spec 103, `vela_advise`) — P2.3's card, the web's `AgentAdviceCard`.
//
//  **Every rule is the core's.** Whether an opinion is accepted, what of it
//  is kept (plain capped text, no bidi or zero-width character, source hosts
//  only), which request it belongs to, and whether it still stands against
//  the sheet now — the same review (`review_digest`) and a fee within the
//  band of the one it was given for — is `AgentCore::advice()`'s answer
//  (`{advice, standing, fee_when_given}`). This file reads that answer and
//  words it from the corpus (`componentsUi.signing.agentAdvice.*`); it decides
//  nothing.
//
//  The card is the agent's, never one of the sheet's words: it colours
//  nothing but its own chip, changes no button and none of the wallet's own
//  warnings, and nothing waits for it. Given for an earlier version of the
//  request, or at a fee that has since moved, it is greyed and says which.
//

import SwiftUI

/// `AgentCore::advice()`'s answer, read.
struct AgentAdvice: Equatable {
    /// `looks_fine`, `caution`, `danger` or `unsure`.
    let verdict: String
    let summary: String
    let points: [String]
    /// Hosts only — the core kept nothing else of the agent's URLs.
    let sourceHosts: [String]
    /// `current`, `earlier_version` or `fee_changed`.
    let standing: String
    /// The fee the opinion was given for, as the core states it.
    let feeWhenGiven: (amount: String, symbol: String)?

    static func == (a: AgentAdvice, b: AgentAdvice) -> Bool {
        a.verdict == b.verdict && a.summary == b.summary && a.points == b.points
            && a.sourceHosts == b.sourceHosts && a.standing == b.standing
            && a.feeWhenGiven?.amount == b.feeWhenGiven?.amount
            && a.feeWhenGiven?.symbol == b.feeWhenGiven?.symbol
    }

    /// `nil` for no answer, or one this build cannot read.
    init?(json: String?) {
        guard let json, let object = try? CoreJSON.object(json),
              let advice = object["advice"] as? [String: Any],
              let verdict = advice["verdict"] as? String,
              let summary = advice["summary"] as? String
        else { return nil }
        self.verdict = verdict
        self.summary = summary
        points = (advice["points"] as? [Any])?.compactMap { $0 as? String } ?? []
        sourceHosts = (advice["source_hosts"] as? [Any])?.compactMap { $0 as? String } ?? []
        // No standing read is never drawn as current (the web's default).
        standing = object["standing"] as? String ?? "earlier_version"
        if let fee = object["fee_when_given"] as? [String: Any],
           let amount = fee["amount"] as? String, !amount.isEmpty {
            feeWhenGiven = (amount, fee["symbol"] as? String ?? "")
        } else {
            feeWhenGiven = nil
        }
    }
}

/// The card's words (`componentsUi.signing.agentAdvice.*`), built apart from
/// the view so a test reads every branch.
struct AgentAdviceCardModel: Equatable {
    let title: String
    /// The verdict's wire name — the chip's tone.
    let verdict: String
    let verdictWord: String
    let summary: String
    let points: [String]
    /// "Sources: host, host" — `nil` with none.
    let sources: String?
    /// "Fee when given: 0.0021 xDAI" — `nil` when it was given for no fee.
    let feeLine: String?
    /// Why it is greyed — `nil` while it stands for the sheet.
    let staleLine: String?
    let footer: String

    var stale: Bool { staleLine != nil }

    /// The web's `adviceCard`, word for word.
    static func of(_ advice: AgentAdvice, loc: Loc) -> AgentAdviceCardModel {
        func t(_ key: String) -> String { loc.t("componentsUi.signing.agentAdvice.\(key)") }
        let word: String = switch advice.verdict {
        case "looks_fine": t("looksFine")
        case "caution": t("caution")
        case "danger": t("danger")
        default: t("unsure")
        }
        let fee = advice.feeWhenGiven.map {
            [$0.amount, $0.symbol].filter { !$0.isEmpty }.joined(separator: " ")
        }
        return AgentAdviceCardModel(
            title: t("title"),
            verdict: advice.verdict,
            verdictWord: word,
            summary: advice.summary,
            points: advice.points,
            sources: advice.sourceHosts.isEmpty
                ? nil : "\(t("sources")): \(advice.sourceHosts.joined(separator: ", "))",
            feeLine: fee.map { loc.t("componentsUi.signing.agentAdvice.feeWhenGiven", vars: ["fee": $0]) },
            staleLine: advice.standing == "current"
                ? nil : t(advice.standing == "fee_changed" ? "feeChanged" : "stale"),
            footer: t("footer")
        )
    }
}

/// The card: its own dashed outline on the sunken ground, the agent's title
/// and its verdict's chip, what it said, and the footer that says it is an
/// opinion that changes nothing. Plain text only — nothing here renders
/// markup or a link; the sources are hosts.
struct AgentAdviceCard: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let model: AgentAdviceCardModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            HStack(alignment: .center, spacing: Tokens.Space.s8) {
                Text(verbatim: model.title)
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: Tokens.Space.s0)
                chip
            }
            if let stale = model.staleLine {
                line(stale).accessibilityIdentifier("agentAdvice.stale")
            }
            if let fee = model.feeLine {
                line(fee).accessibilityIdentifier("agentAdvice.fee")
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(verbatim: model.summary)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                ForEach(Array(model.points.enumerated()), id: \.offset) { _, point in
                    HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                        Text(verbatim: "•")
                            .typeRole(Typography.rowSub.scaled(textScale))
                        Text(verbatim: point)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .foregroundStyle(theme.fgBase)
                }
                if let sources = model.sources {
                    line(sources).accessibilityIdentifier("agentAdvice.sources")
                }
            }
            // Given for an earlier version: greyed, never drawn as current.
            .opacity(model.stale ? Tokens.Opacity.disabled : 1)
            Text(verbatim: model.footer)
                .typeRole(Typography.flowCaption.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, Tokens.Space.s16)
        .padding(.vertical, Tokens.Space.s12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r16))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(theme.borderStrong,
                        style: StrokeStyle(lineWidth: Tokens.BorderWidth.emphasis,
                                           dash: [Tokens.Space.s4, Tokens.Space.s4]))
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel(model.title)
        .accessibilityIdentifier("agentAdvice")
    }

    /// The agent's verdict. Never green: a reassurance is the wallet's to give.
    private var chip: some View {
        let (ink, ground, edge): (Color, Color, Color) = switch model.verdict {
        case "caution": (theme.warningBase, theme.warningSoft, theme.warningBase)
        case "danger": (theme.errorBase, theme.errorSoft, theme.errorBase)
        case "unsure": (theme.fgMuted, .clear, theme.borderStrong)
        default: (theme.fgBase, .clear, theme.borderStrong)
        }
        return Text(verbatim: model.verdictWord)
            .typeRole(Typography.label.scaled(textScale))
            .foregroundStyle(ink)
            .padding(.horizontal, Tokens.Space.s12)
            .padding(.vertical, Tokens.Space.s4)
            .background(ground, in: Capsule())
            .overlay(Capsule().stroke(edge, lineWidth: Tokens.BorderWidth.hairline))
            .opacity(model.stale ? Tokens.Opacity.disabled : 1)
            .fixedSize()
            .accessibilityIdentifier("agentAdvice.verdict")
    }

    private func line(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.flowCaption.scaled(textScale))
            .foregroundStyle(theme.fgMuted)
            .fixedSize(horizontal: false, vertical: true)
    }
}
