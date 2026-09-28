//
//  SiteRowView.swift
//  VelaWallet
//
//  A site inside a group (spec 022): mark, name, blurb or host, and the
//  trailing "刚刚 / 昨天" the recent group carries.
//

import SwiftUI

struct SiteRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let site: SiteModel
    var onOpen: (String) -> Void = { _ in }

    var body: some View {
        Button {
            onOpen(site.id)
        } label: {
            HStack(spacing: Tokens.Space.s12) {
                SiteAvatarView(site: site)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: site.name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    Text(verbatim: site.subtitle ?? site.host)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: Tokens.Space.s12)
                if let meta = site.meta, !meta.isEmpty {
                    Text(verbatim: meta)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.accentBase)
                }
            }
            .padding(.vertical, Tokens.Space.s12)
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
    }
}
