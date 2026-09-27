//
//  FeedbackOutcomeToast.swift
//  VelaWallet
//
//  A report always ends in something the person can see (founder, 2026-09-27:
//  "反馈成功或失败都要有提示吧，而不是生硬的退出到设置页面吧"). The sheet shows
//  its own success state and fallback block; this is for the one case the
//  sheet cannot cover — it was closed while the report was still sending. The
//  send carries on (the settings page owns the sender), and when it lands the
//  page says how it went, with the one action that follows from it:
//
//  - filed:    `successTitle`, and `viewIssue` opens the issue;
//  - fallback: `fallbackTitle`, and `openGithub` opens the prefilled form —
//              the only road left for that report, so this toast stays until
//              the person acts on it or closes it.
//

import SwiftUI

struct FeedbackOutcomeToast: View {

    /// What the toast says, from the sender's outcome and the sheet's words.
    struct Model: Equatable {
        let success: Bool
        let title: String
        let actionTitle: String
        let url: String

        /// `nil` for anything that is not an outcome (idle, sending).
        static func from(_ state: FeedbackSender.State, words: FeedbackModel) -> Model? {
            switch state {
            case .filed(_, let url, _, _):
                Model(success: true, title: words.successTitle, actionTitle: words.viewIssue, url: url)
            case .fallback(let url, _):
                Model(success: false, title: words.fallbackTitle, actionTitle: words.openGithub, url: url)
            case .idle, .sending:
                nil
            }
        }
    }

    @Environment(\.theme) private var theme
    @ScaledMetric(relativeTo: .body) private var glyph = LucideIconSize.rowGlyph
    @Environment(\.walletTextScale) private var textScale

    let model: Model
    let closeLabel: String
    let onAction: () -> Void
    let onClose: () -> Void

    var body: some View {
        // Two tiers (device, 375 pt): the title on its own line with the ✕,
        // the action under it. Side by side, "Couldn't send from the app"
        // broke into three lines beside "Open GitHub form".
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(model.success ? .check : .triangleAlert, size: glyph * textScale)
                .foregroundStyle(model.success ? theme.successBase : theme.warningBase)
                .padding(.top, Tokens.Space.s12)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(model.title)
                    .typeRole(Typography.label)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, Tokens.Space.s12)
                Button(action: onAction) {
                    Text(model.actionTitle)
                        .typeRole(Typography.label)
                        .fontWeight(.semibold)
                        .foregroundStyle(theme.infoBase)
                        .fixedSize(horizontal: false, vertical: true)
                        // A hit area taller than the words.
                        .padding(.vertical, Tokens.Space.s8)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("feedback.toastAction")
                .padding(.bottom, Tokens.Space.s4)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button(action: onClose) {
                LucideIcon(.close, size: glyph * textScale)
                    .foregroundStyle(theme.fgMuted)
                    .frame(minWidth: Tokens.Control.md, minHeight: Tokens.Control.md)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(closeLabel)
            .accessibilityIdentifier("feedback.toastClose")
        }
        .padding(.leading, Tokens.Space.s16)
        .padding(.trailing, Tokens.Space.s4)
        .padding(.vertical, Tokens.Space.s4)
        .background(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .fill(theme.bgRaised)
                .shadow(color: .black.opacity(0.12), radius: 12, y: 4)
        )
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
        )
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("feedback.toast")
    }
}
