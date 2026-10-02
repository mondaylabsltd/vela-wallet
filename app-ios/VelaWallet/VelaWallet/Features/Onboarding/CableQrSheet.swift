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

import SwiftUI
import VelaCore

struct CableQrSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let payload: String
    /// Creating a key on the phone, or finding one there (087 F02): a sign-in
    /// creates nothing, so its line is the scan itself.
    let chooser: KeyChooser

    var body: some View {
        let copy = methodCopy(.hybrid, chooser: chooser, loc: loc)
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
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDetents([.large])
        .presentationDragIndicator(.hidden)
        .presentationBackground(theme.bgRaised)
        // Dismissing would strand a ceremony blocked on the scan; it times out
        // on its own instead, exactly like the touch sheet.
        .interactiveDismissDisabled(true)
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
