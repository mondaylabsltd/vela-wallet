//
//  SiteRowView.swift
//  VelaWallet
//
//  A site inside a group (spec 022): mark, name, blurb or host, and the
//  trailing "刚刚 / 昨天" the recent group carries.
//

import SwiftUI
import VelaCore

struct SiteRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let site: SiteModel
    var onOpen: (String) -> Void = { _ in }

    /// The row's two lines — the core's `browserSiteLabel` (spec 082 RE7,
    /// G8): a page whose title IS its host is named once, with no second
    /// line saying it again. A blurb the group carries still shows.
    static func lines(_ site: SiteModel) -> (name: String, second: String?) {
        let label = browserSiteLabel(title: site.name, host: site.host)
        let blurb = site.subtitle.flatMap { text -> String? in
            guard !text.isEmpty, text.caseInsensitiveCompare(label.name) != .orderedSame,
                  text.caseInsensitiveCompare(site.host) != .orderedSame
            else { return nil }
            return text
        }
        return (label.name, blurb ?? label.hostLine)
    }

    var body: some View {
        let lines = Self.lines(site)
        return Button {
            onOpen(site.id)
        } label: {
            HStack(spacing: Tokens.Space.s12) {
                SiteAvatarView(site: site)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: lines.name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    if let second = lines.second {
                        Text(verbatim: second)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                            .lineLimit(1)
                    }
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
