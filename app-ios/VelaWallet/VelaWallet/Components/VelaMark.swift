//
//  VelaMark.swift
//  VelaWallet
//
//  The Vela sailboat mark drawn from the design geometry of
//  docs/design/onboarding/logo-{light,dark}.svg (viewBox 258×260) — FR-008.
//  Sails are mode-invariant; the hull is themed (Brand constants).
//

import SwiftUI

struct VelaMark: View {
    @Environment(\.theme) private var theme
    var size: CGFloat = WelcomeGeometry.markSize

    var body: some View {
        ZStack {
            MainSail().fill(Brand.sailMain.color)
            SecondarySail().fill(Brand.sailSecondary.color)
            Hull().fill(theme.markHull)
        }
        .frame(width: size, height: size * 260 / 258)
        .accessibilityHidden(true) // decorative; the wordmark carries the name
    }
}

// SVG viewBox 258×260 → unit-scaled paths.

private struct MainSail: Shape {
    func path(in rect: CGRect) -> Path {
        let s = rect.width / 258
        var p = Path()
        p.move(to: CGPoint(x: 122 * s, y: 0))
        p.addCurve(to: CGPoint(x: 18 * s, y: 187 * s),
                   control1: CGPoint(x: 70 * s, y: 53 * s),
                   control2: CGPoint(x: 38 * s, y: 118 * s))
        p.addLine(to: CGPoint(x: 122 * s, y: 187 * s))
        p.closeSubpath()
        return p
    }
}

private struct SecondarySail: Shape {
    func path(in rect: CGRect) -> Path {
        let s = rect.width / 258
        var p = Path()
        p.move(to: CGPoint(x: 142 * s, y: 42 * s))
        p.addCurve(to: CGPoint(x: 240 * s, y: 187 * s),
                   control1: CGPoint(x: 193 * s, y: 75 * s),
                   control2: CGPoint(x: 225 * s, y: 128 * s))
        p.addLine(to: CGPoint(x: 142 * s, y: 187 * s))
        p.closeSubpath()
        return p
    }
}

private struct Hull: Shape {
    func path(in rect: CGRect) -> Path {
        let s = rect.width / 258
        var p = Path()
        p.move(to: CGPoint(x: 0, y: 207 * s))
        p.addLine(to: CGPoint(x: 258 * s, y: 207 * s))
        p.addCurve(to: CGPoint(x: 165 * s, y: 260 * s),
                   control1: CGPoint(x: 243 * s, y: 240 * s),
                   control2: CGPoint(x: 211 * s, y: 260 * s))
        p.addLine(to: CGPoint(x: 92 * s, y: 260 * s))
        p.addCurve(to: CGPoint(x: 0, y: 207 * s),
                   control1: CGPoint(x: 49 * s, y: 260 * s),
                   control2: CGPoint(x: 16 * s, y: 240 * s))
        p.closeSubpath()
        return p
    }
}

#Preview("VelaMark light") {
    VelaMark(size: 120).padding().themed(.light)
}

#Preview("VelaMark dark") {
    VelaMark(size: 120).padding().background(Color.black).themed(.dark)
}
