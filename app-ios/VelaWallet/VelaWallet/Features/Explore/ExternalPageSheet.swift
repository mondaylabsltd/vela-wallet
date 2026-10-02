//
//  ExternalPageSheet.swift
//  VelaWallet
//
//  Spec 088 FR-004: a page another app or website asked to open
//  (`velawallet://open?url=…`). It does not load until the person has seen
//  where it comes from — the host, large, as the core named it — and said
//  "Open in Vela Wallet". Dismissing is "no": nothing loads, nothing is kept.
//
//  Words reused, not new: the host and the URL are data, the button is the
//  /pay page's own "Open in Vela Wallet", the other is Cancel. Android draws
//  the same sheet (`ExternalPageSheet.kt`).
//

import SwiftUI

struct ExternalPageSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let page: BrowserController.ExternalPage
    let onAnswer: (Bool) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(page.host)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .accessibilityIdentifier("external-page-host")

            Text(page.url)
                .typeRole(Typography.monoSmall)
                .foregroundStyle(theme.fgSubtle)
                .lineLimit(3)
                .truncationMode(.middle)

            VStack(spacing: Tokens.Space.s12) {
                VelaButton(title: loc.t(I18nKeys.ExternalPage.open), kind: .primary) { onAnswer(true) }
                VelaButton(title: loc.t(I18nKeys.ExternalPage.cancel), kind: .secondary) { onAnswer(false) }
            }
            .padding(.top, Tokens.Space.s16)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDetents([.height(320)])
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }
}
