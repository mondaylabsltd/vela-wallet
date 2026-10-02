//
//  NoticeCapsule.swift
//  VelaWallet
//
//  A brief notice that asks nothing of anyone — "Link copied", "Debug mode is
//  now available" (spec 091). One line in a capsule, inverted from the page;
//  the screen that shows it places it, fades it and takes it away.
//

import SwiftUI

struct NoticeCapsule: View {
    @Environment(\.theme) private var theme
    let text: String

    var body: some View {
        Text(verbatim: text)
            .typeRole(Typography.label)
            .foregroundStyle(theme.bgBase)
            .padding(.horizontal, Tokens.Space.s16)
            .padding(.vertical, Tokens.Space.s8)
            .background(theme.fgBase, in: Capsule())
    }
}
