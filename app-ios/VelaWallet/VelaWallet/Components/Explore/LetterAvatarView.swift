//
//  LetterAvatarView.swift
//  VelaWallet
//
//  A site or token's mark (spec 022): its first letter on a wash of its own
//  brand colour. Since spec 079 (and the 2026-09 site-icons ruling: https
//  only, no referrer) a site is drawn with its own icon when one loads —
//  `SiteAvatarView` — and this letter is the fallback until then and when
//  none does.
//

import SwiftUI

struct LetterAvatarView: View {
    @Environment(\.theme) private var theme

    let letter: String
    let tint: Color
    var size: CGFloat = ExploreGeometry.rowAvatar
    /// Muted rendering for the unknown-site case.
    var muted = false

    var body: some View {
        Text(verbatim: letter)
            .font(.custom(FontName.bold, size: size * 0.42))
            .foregroundStyle(muted ? theme.fgMuted : tint)
            .frame(width: size, height: size)
            .background(muted ? AnyShapeStyle(theme.bgSunken) : AnyShapeStyle(tint.opacity(0.16)),
                        in: Circle())
            .accessibilityHidden(true)
    }
}

/// A site's avatar (spec 079): its own icon when one loads — https only — and
/// its letter until then, and when none does.
struct SiteAvatarView: View {
    let site: SiteModel
    var size: CGFloat = ExploreGeometry.rowAvatar

    var body: some View {
        RemoteLogoView(urls: site.iconUrls, size: size) {
            LetterAvatarView(letter: site.letter, tint: site.tint, size: size)
        }
        .accessibilityHidden(true)
    }
}
