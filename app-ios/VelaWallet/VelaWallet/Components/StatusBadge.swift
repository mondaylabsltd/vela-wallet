//
//  StatusBadge.swift
//  VelaWallet
//
//  The single authoritative outcome status badge (spec 014): a 56 pt
//  circle in the variant's soft tint with the variant's base-color glyph.
//  Decorative — the outcome headline carries the meaning for a11y.
//
//  Issue #460: an error is a "!" like every other non-success, in the error
//  colour. It was an × — and the sheet's column is leading-aligned, so a red
//  × in a disc sat top-left, exactly where a close button goes, and did
//  nothing when tapped.
//

import SwiftUI

struct StatusBadge: View {
    @Environment(\.theme) private var theme
    let variant: BadgeVariant

    var body: some View {
        ZStack {
            Circle().fill(background)
            Image(systemName: Self.glyphName(variant))
                .font(GlyphFont.badge)
                .foregroundStyle(foreground)
        }
        .frame(width: FlowGeometry.badgeSize, height: FlowGeometry.badgeSize)
        .accessibilityHidden(true)
    }

    /// The colour says how bad; the glyph never says "close".
    static func glyphName(_ variant: BadgeVariant) -> String {
        switch variant {
        case .success: "checkmark"
        case .warning, .neutral, .info, .error: "exclamationmark"
        case .timeout: "clock.fill"
        }
    }

    private var background: Color {
        switch variant {
        case .success: theme.successSoft
        case .warning, .timeout: theme.warningSoft
        case .neutral: theme.bgSunken
        case .error: theme.errorSoft
        case .info: theme.infoSoft
        }
    }

    private var foreground: Color {
        switch variant {
        case .success: theme.successBase
        case .warning, .timeout: theme.warningBase
        case .neutral: theme.fgBase
        case .error: theme.errorBase
        case .info: theme.infoBase
        }
    }
}

#Preview("Badges") {
    HStack(spacing: Tokens.Space.s12) {
        StatusBadge(variant: .success)
        StatusBadge(variant: .warning)
        StatusBadge(variant: .neutral)
        StatusBadge(variant: .error)
        StatusBadge(variant: .timeout)
        StatusBadge(variant: .info)
    }
    .padding(Tokens.Space.s24)
    .themed(.light)
}

#Preview("Badges dark") {
    HStack(spacing: Tokens.Space.s12) {
        StatusBadge(variant: .success)
        StatusBadge(variant: .warning)
        StatusBadge(variant: .neutral)
        StatusBadge(variant: .error)
        StatusBadge(variant: .timeout)
        StatusBadge(variant: .info)
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgRaised.color)
    .themed(.dark)
}
