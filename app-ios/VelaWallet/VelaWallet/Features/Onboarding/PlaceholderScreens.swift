//
//  PlaceholderScreens.swift
//  VelaWallet
//
//  Minimal intent destinations (US3) — wallet creation/import are later
//  features; these prove the navigation layer with localized titles.
//

import SwiftUI

struct IntentPlaceholderScreen: View {
    @Environment(\.theme) private var theme
    let title: String

    var body: some View {
        VStack(spacing: Tokens.Space.s16) {
            VelaMark(size: WelcomeGeometry.markSize)
            Text(title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .pageGround(theme.bgBase)
    }
}
