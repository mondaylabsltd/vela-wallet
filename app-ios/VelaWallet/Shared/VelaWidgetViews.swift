//
//  VelaWidgetViews.swift
//  VelaWallet + VelaWidgets
//
//  The widgets' faces (native parity N-I2), drawn from a `WidgetSnapshot`
//  and nothing else. Shared with the app so the unit suite can render every
//  face, in both appearances and in every language, without a Home Screen.
//
//  The design language holds here as on the home: the hero is open on the
//  page (no card inside the widget's own), the figure is the hero and its
//  decimals are quieter, the label is small and muted, the doors are soft
//  `bg.sunken` chips, and nothing wears the accent — opening Send moves no
//  money. Tokens only.
//

import SwiftUI
import WidgetKit

/// The face's type, in the app's typeface. Sizes are the token scale, and
/// each is relative to a text style, so the person's text size moves them.
enum WidgetType {
    static let regular = "PlusJakartaSans-Regular"
    static let medium = "PlusJakartaSans-Medium"
    static let semiBold = "PlusJakartaSans-SemiBold"
    static let bold = "PlusJakartaSans-Bold"

    static func font(_ name: String, _ size: CGFloat, _ style: Font.TextStyle) -> Font {
        .custom(name, size: size, relativeTo: style)
    }
}

/// One appearance's colours, from the generated tokens.
struct WidgetPalette {
    let page: Color
    let sunken: Color
    let ink: Color
    let muted: Color
    let subtle: Color
    let warning: Color

    init(_ scheme: ColorScheme) {
        let p = scheme == .dark ? Tokens.dark : Tokens.light
        page = p.bgBase.color
        sunken = p.bgSunken.color
        ink = p.fgBase.color
        muted = p.fgMuted.color
        subtle = p.fgSubtle.color
        warning = p.warningBase.color
    }
}

/// The figure, as the hero draws it: the glyph and the whole part, then the
/// decimals quieter — or the mask, or nothing at all.
struct WidgetFigure: View {
    let snapshot: WidgetSnapshot
    let size: CGFloat
    var ink: Color
    var quiet: Color

    var body: some View {
        switch snapshot.figure {
        case .amount:
            (Text(verbatim: snapshot.integer ?? "")
                .font(WidgetType.font(WidgetType.bold, size, .title))
                .foregroundColor(ink)
                + Text(verbatim: snapshot.decimals.map { ".\($0)" } ?? "")
                .font(WidgetType.font(WidgetType.bold, size * WidgetFace.decimalsScale, .title3))
                .foregroundColor(quiet))
                // A figure is never cut: it is set smaller instead, as the
                // hero does at its widest.
                .lineLimit(1)
                .minimumScaleFactor(WidgetFace.figureMinScale)
                .privacySensitive()
                .widgetAccentable()
                .accessibilityLabel(Text(verbatim: snapshot.spoken))
        case .hidden:
            Text(verbatim: WidgetFace.mask)
                .font(WidgetType.font(WidgetType.bold, size, .title))
                .foregroundColor(ink)
                .lineLimit(1)
                .minimumScaleFactor(WidgetFace.figureMinScale)
                .accessibilityLabel(Text(verbatim: snapshot.spoken))
        case .none:
            EmptyView()
        }
    }
}

/// Geometry and constants the faces share.
enum WidgetFace {
    /// The decimals at the hero's proportion (t26 under t40).
    static let decimalsScale = Tokens.TextSize.t26 / Tokens.TextSize.t40
    /// How small a long figure may be set before anything else gives.
    static let figureMinScale: CGFloat = 0.5
    /// The hidden balance: six dots, as the hero draws them.
    static let mask = "••••••"
    /// The figure's size on the Home Screen faces.
    static let smallFigure = Tokens.TextSize.t26
    static let mediumFigure = Tokens.TextSize.t32

    /// "Updated 14:32" today, "Updated 3 Nov, 14:32" before — when the
    /// figure was read, never how long ago (a widget is not redrawn every
    /// minute, and "2m ago" would go on saying 2m).
    static func stamp(_ snapshot: WidgetSnapshot, now: Date = Date()) -> String? {
        guard let at = snapshot.asOf else { return nil }
        let locale = Locale(identifier: snapshot.language)
        var calendar = Calendar(identifier: .gregorian)
        calendar.locale = locale
        let style: Date.FormatStyle = calendar.isDate(at, inSameDayAs: now)
            ? Date.FormatStyle(date: .omitted, time: .shortened)
            : Date.FormatStyle(date: .abbreviated, time: .shortened)
        return "\(snapshot.updatedLabel) \(at.formatted(style.locale(locale)))"
    }
}

/// The small Home Screen face: the label, the figure, when it was read.
struct BalanceSmallFace: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.widgetRenderingMode) private var mode
    let snapshot: WidgetSnapshot?
    var now = Date()

    var body: some View {
        let palette = WidgetPalette(scheme)
        let full = mode == .fullColor
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            VelaGlyph(color: full ? palette.ink : .primary, fullColor: full)
            Spacer(minLength: Tokens.Space.s0)
            if let snapshot {
                Text(verbatim: snapshot.title)
                    .font(WidgetType.font(WidgetType.medium, Tokens.TextSize.t11, .caption))
                    .foregroundColor(full ? palette.muted : .secondary)
                    .lineLimit(1)
                WidgetFigure(snapshot: snapshot, size: WidgetFace.smallFigure,
                             ink: full ? palette.ink : .primary, quiet: full ? palette.muted : .secondary)
                WidgetFootnote(snapshot: snapshot, palette: palette, full: full, now: now)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// The medium face: the small one's hero, and the three doors beside it.
struct BalanceMediumFace: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.widgetRenderingMode) private var mode
    let snapshot: WidgetSnapshot?
    var now = Date()

    var body: some View {
        let palette = WidgetPalette(scheme)
        let full = mode == .fullColor
        HStack(alignment: .top, spacing: Tokens.Space.s16) {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                VelaGlyph(color: full ? palette.ink : .primary, fullColor: full)
                Spacer(minLength: Tokens.Space.s0)
                if let snapshot {
                    Text(verbatim: snapshot.accountName.map { "\(snapshot.title) · \($0)" } ?? snapshot.title)
                        .font(WidgetType.font(WidgetType.medium, Tokens.TextSize.t11, .caption))
                        .foregroundColor(full ? palette.muted : .secondary)
                        .lineLimit(1)
                    WidgetFigure(snapshot: snapshot, size: WidgetFace.mediumFigure,
                                 ink: full ? palette.ink : .primary, quiet: full ? palette.muted : .secondary)
                    WidgetFootnote(snapshot: snapshot, palette: palette, full: full, now: now)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .accessibilityElement(children: .combine)
            if let snapshot {
                VStack(spacing: Tokens.Space.s8) {
                    ForEach(VelaEntry.allCases, id: \.self) { entry in
                        Link(destination: entry.url) {
                            HStack(spacing: Tokens.Space.s8) {
                                Image(systemName: entry.systemImage)
                                    .font(WidgetType.font(WidgetType.semiBold, Tokens.TextSize.t13, .footnote))
                                    // One column for the three glyphs, so the
                                    // words start on one line.
                                    .frame(width: Tokens.TextSize.t17)
                                Text(verbatim: snapshot.label(for: entry))
                                    .font(WidgetType.font(WidgetType.semiBold, Tokens.TextSize.t13, .footnote))
                                    .lineLimit(1)
                                    .minimumScaleFactor(WidgetFace.figureMinScale)
                                Spacer(minLength: Tokens.Space.s0)
                            }
                            .foregroundColor(full ? palette.ink : .primary)
                            .padding(.horizontal, Tokens.Space.s12)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .background(Capsule().fill(full ? palette.sunken : Color.primary.opacity(Tokens.Opacity.dim / 4)))
                        }
                        .accessibilityLabel(Text(verbatim: snapshot.label(for: entry)))
                    }
                }
                .frame(width: WidgetGeometry.doorsWidth)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

enum WidgetGeometry {
    /// The medium face's door column: room for "Receive" in every language
    /// the corpus has at the default size, set smaller before it is cut.
    static let doorsWidth: CGFloat = 112
    /// The Vela mark's size at the top of a face.
    static let glyph: CGFloat = 18
}

/// The line under the figure: the hero's own note when it has one (a
/// partial total, "Checking…"), else when the figure was read.
struct WidgetFootnote: View {
    let snapshot: WidgetSnapshot
    let palette: WidgetPalette
    let full: Bool
    var now = Date()

    var body: some View {
        if let note = snapshot.note {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s4) {
                if snapshot.partial {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(WidgetType.font(WidgetType.medium, Tokens.TextSize.t10, .caption2))
                        .foregroundColor(full ? palette.warning : .secondary)
                        .accessibilityHidden(true)
                }
                Text(verbatim: note)
                    .font(WidgetType.font(WidgetType.medium, Tokens.TextSize.t11, .caption2))
                    .foregroundColor(full ? (snapshot.partial ? palette.warning : palette.muted) : .secondary)
                    .lineLimit(2)
            }
        }
        if let stamp = WidgetFace.stamp(snapshot, now: now) {
            Text(verbatim: stamp)
                .font(WidgetType.font(WidgetType.regular, Tokens.TextSize.t11, .caption2))
                .foregroundColor(full ? palette.subtle : .secondary)
                .lineLimit(1)
                .minimumScaleFactor(WidgetFace.figureMinScale)
        }
    }
}

/// The Lock Screen's rectangle: label, figure, when.
struct BalanceRectangularFace: View {
    let snapshot: WidgetSnapshot?
    var now = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            if let snapshot {
                Text(verbatim: snapshot.title)
                    .font(WidgetType.font(WidgetType.semiBold, Tokens.TextSize.t11, .caption))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                WidgetFigure(snapshot: snapshot, size: Tokens.TextSize.t20, ink: .primary, quiet: .secondary)
                if let stamp = WidgetFace.stamp(snapshot, now: now) {
                    Text(verbatim: stamp)
                        .font(WidgetType.font(WidgetType.regular, Tokens.TextSize.t11, .caption2))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .minimumScaleFactor(WidgetFace.figureMinScale)
                }
            } else {
                VelaGlyph(color: .primary, fullColor: false)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// The Lock Screen's line over the clock: "Total balance $1,383.28".
struct BalanceInlineFace: View {
    let snapshot: WidgetSnapshot?

    var body: some View {
        if let snapshot {
            switch snapshot.figure {
            case .amount:
                Text(verbatim: "\(snapshot.title) \(snapshot.integer ?? "")\(snapshot.decimals.map { ".\($0)" } ?? "")")
                    .privacySensitive()
            case .hidden:
                Text(verbatim: "\(snapshot.title) \(WidgetFace.mask)")
            case .none:
                Text(verbatim: snapshot.title)
            }
        } else {
            Image(systemName: VelaEntry.scan.systemImage)
        }
    }
}

/// The Lock Screen's circle: the scanner, one tap away.
struct ScanCircularFace: View {
    let label: String?

    var body: some View {
        ZStack {
            AccessoryWidgetBackground()
            Image(systemName: VelaEntry.scan.systemImage)
                .font(WidgetType.font(WidgetType.semiBold, Tokens.TextSize.t20, .title3))
                .widgetAccentable()
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: label ?? ""))
        .widgetURL(VelaEntry.scan.url)
    }
}

/// Vela's mark, small: the page's signature at the top of a face — the
/// sailboat of `VelaMark` (docs/design/onboarding/logo-*.svg, viewBox
/// 258×260), drawn rather than an asset: the extension carries no image
/// catalog of the app's. In full colour the sails keep the brand's coral and
/// the hull the page's ink; in a tinted or Lock Screen rendering the whole
/// mark takes the one colour the system gives it.
struct VelaGlyph: View {
    @Environment(\.colorScheme) private var scheme
    var color: Color
    var fullColor = true
    var size: CGFloat = WidgetGeometry.glyph

    /// The brand's sails (`Brand.sailMain`, `Brand.sailSecondary`) and hulls.
    private static let sailMain = TokenColor(argb: 0xFF_FF6A45)
    private static let sailSecondary = TokenColor(argb: 0xFF_FFA98E)
    private static let hullLight = TokenColor(argb: 0xFF_554B46)
    private static let hullDark = TokenColor(argb: 0xFF_DED5CE)

    var body: some View {
        ZStack {
            GlyphShape(part: .mainSail).fill(fullColor ? Self.sailMain.color : color)
            GlyphShape(part: .secondarySail).fill(fullColor ? Self.sailSecondary.color : color.opacity(0.7))
            GlyphShape(part: .hull).fill(fullColor ? (scheme == .dark ? Self.hullDark : Self.hullLight).color : color)
        }
        .frame(width: size, height: size * 260 / 258)
        .accessibilityHidden(true)
    }
}

private struct GlyphShape: Shape {
    enum Part { case mainSail, secondarySail, hull }
    let part: Part

    func path(in rect: CGRect) -> Path {
        let s = rect.width / 258
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x * s, y: y * s) }
        var p = Path()
        switch part {
        case .mainSail:
            p.move(to: pt(122, 0))
            p.addCurve(to: pt(18, 187), control1: pt(70, 53), control2: pt(38, 118))
            p.addLine(to: pt(122, 187))
        case .secondarySail:
            p.move(to: pt(142, 42))
            p.addCurve(to: pt(240, 187), control1: pt(193, 75), control2: pt(225, 128))
            p.addLine(to: pt(142, 187))
        case .hull:
            p.move(to: pt(0, 207))
            p.addLine(to: pt(258, 207))
            p.addCurve(to: pt(165, 260), control1: pt(243, 240), control2: pt(211, 260))
            p.addLine(to: pt(92, 260))
            p.addCurve(to: pt(0, 207), control1: pt(49, 260), control2: pt(16, 240))
        }
        p.closeSubpath()
        return p
    }
}
