//
//  SafariExtensionRow.swift
//  VelaWallet
//
//  Settings → "Turn on the Safari extension" (native AUDIT §6.5). The app
//  embeds a Safari Web Extension (`VelaSafariExtension`), and iOS gives an
//  app no way to read whether it is on, nor a link into Safari's own
//  settings — so this row says, once, where the switch is, and opens the
//  Settings app on the way there. Nothing is read or changed: the
//  extension and the app share no state (no App Group), by design.
//
//  One isolated view and its own sheet, drawn under the settings sections
//  (`SettingsScreen.homeBody`); the words are the corpus's
//  (`settings.safariExtension.*`, all 15 locales).
//

import SwiftUI
import UIKit

struct SafariExtensionRow: View {
    let loc: Loc
    @State private var open = false

    var body: some View {
        SettingsRow(
            row: SettingsRowModel(
                id: "safari-extension",
                title: loc.t("settings.safariExtension.title"),
                icon: .globe,
                subtitle: loc.t("settings.safariExtension.subtitle")
            ),
            divider: false,
            onTap: { _ in open = true }
        )
        .accessibilityIdentifier("settings.safariExtension")
        .sheet(isPresented: $open) {
            SafariExtensionSheet(loc: loc) { open = false }
                .presentationDetents([.medium])
                .presentationDragIndicator(.visible)
        }
    }
}

/// Where Safari keeps the switch, and the way to Settings.
private struct SafariExtensionSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.openURL) private var openURL
    let loc: Loc
    let onDone: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(loc.t("settings.safariExtension.title"))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .padding(.bottom, Tokens.Space.s12)
            Text(loc.t("settings.safariExtension.body"))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.bottom, Tokens.Space.s24)
            // Neutral ink: it moves no value and signs nothing.
            VelaButton(title: loc.t("settings.safariExtension.open"), kind: .ink) {
                if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
                onDone()
            }
            .accessibilityIdentifier("settings.safariExtension.open")
            Spacer(minLength: 0)
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bgBase)
    }
}
