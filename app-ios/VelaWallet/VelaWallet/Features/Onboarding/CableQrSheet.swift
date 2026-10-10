//
//  CableQrSheet.swift
//  VelaWallet
//
//  "Sign in with your phone": the caBLE QR the OTHER device scans. Shown while
//  a Hybrid ceremony is finding and talking to that phone; replaced by the
//  touch sheet the moment the phone connects, and cleared however the ceremony
//  ends. The matrix comes from the core (cableQrMatrix, the same encoder every
//  platform draws with), so this view owns only pixels.
//
//  Issue #459: a person who changes their mind can leave — Cancel, a swipe or
//  a tap outside — and that ends the ceremony as a cancel, quietly, as the
//  desktop's card always has. It used to hold them for the 90 s scan and then
//  blame the link.
//
//  Issue #480: the sheet is as tall as what it holds. It was `.large` — a
//  near-full-screen sheet with a title, a code and a button floating in the
//  middle of it (on an iPad, the iPhone layout scaled up, most of a screen of
//  nothing). The content is measured and the detent is that height; Cancel is
//  the last thing in it, at the bottom, as on every ceremony sheet. Not
//  `.medium`: on a 4.7" phone that crops the code (#447).
//

import SwiftUI
import VelaCore

struct CableQrSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let payload: String
    /// Creating a key on the phone, or finding one there (087 F02): a sign-in
    /// creates nothing, so its line is the scan itself.
    let chooser: KeyChooser
    /// Cancel, a swipe or a tap outside: the ceremony ends as a cancel.
    var onCancel: () -> Void = {}

    /// What the content measures. It starts at what a default text size
    /// comes to (title, line, a 260pt code, Cancel and the padding), so the
    /// sheet opens near its height instead of growing from nothing.
    @State private var contentHeight: CGFloat = Self.expectedHeight
    static let expectedHeight: CGFloat = 480

    var body: some View {
        let copy = methodCopy(.hybrid, chooser: chooser, loc: loc)
        // A scroll view only for the sizes the content outgrows the screen at
        // (the accessibility text sizes): everywhere else it fits and stays.
        ScrollView {
            VStack(spacing: Tokens.Space.s16) {
                // The hybrid row's own words, as the chooser that opened this
                // card drew them ("Phone or tablet" over the core's line).
                Text(copy.title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                    .multilineTextAlignment(.center)

                Text(copy.body)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)

                if let matrix = cableQrMatrix(text: payload) {
                    QrView(matrix: matrix)
                        .aspectRatio(1, contentMode: .fit)
                        .frame(maxWidth: 260)
                        .padding(.top, Tokens.Space.s8)
                }

                // The desktop card's Cancel (`common.cancel`): a lone code with
                // nothing to press reads as stuck.
                VelaButton(title: loc.t(I18nKeys.Flow.cancel), kind: .secondary) { onCancel() }
                    .padding(.top, Tokens.Space.s8)
                    .accessibilityIdentifier("cable.cancel")
            }
            .frame(maxWidth: .infinity)
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.top, Tokens.Space.s32)
            .padding(.bottom, Tokens.Space.s16)
            .fixedSize(horizontal: false, vertical: true)
            .onGeometryChange(for: CGFloat.self) { proxy in
                proxy.size.height
            } action: { height in
                contentHeight = height
            }
        }
        .scrollBounceBehavior(.basedOnSize)
        .presentationDetents([.height(max(contentHeight, 1))])
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
        // A swipe or a tap outside is a cancel too: the shared sheet's
        // dismissal reaches `OnboardingModel.dismissOnboardingSheet`.
    }
}

/// The module matrix as pixels: a light card with a quiet zone, dark modules
/// only. Always black-on-white regardless of theme — scanners want contrast,
/// not palette.
private struct QrView: View {
    let matrix: QrMatrix

    var body: some View {
        Canvas { context, size in
            let width = Int(matrix.width)
            let quiet = 2
            let units = CGFloat(width + quiet * 2)
            let cell = min(size.width, size.height) / units
            context.fill(
                Path(CGRect(origin: .zero, size: size)),
                with: .color(.white)
            )
            for row in 0..<width {
                for col in 0..<width {
                    guard matrix.modules[row * width + col] else { continue }
                    let rect = CGRect(
                        x: (CGFloat(col) + CGFloat(quiet)) * cell,
                        y: (CGFloat(row) + CGFloat(quiet)) * cell,
                        width: cell,
                        height: cell
                    )
                    context.fill(Path(rect), with: .color(.black))
                }
            }
        }
    }
}
