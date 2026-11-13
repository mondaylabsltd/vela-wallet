//
//  QrModulesView.swift
//  VelaWallet
//
//  A QR code drawn on whole pixels (real-device S3).
//
//  The codes were drawn in a `Canvas` at `side / count` points a module:
//  29 modules into 260 points on a 2× phone is 17.93 pixels a module, so
//  modules came out 17 and 18 pixels wide, blurred at their edges, and the
//  receive card rounded each one UP so neighbours overlapped. A camera on a
//  lit phone forgives that; a standard decoder reading a screenshot did not,
//  and a screenshot is exactly what a person forwards.
//
//  Here the module is a whole number of DEVICE pixels — the largest that
//  fits — and the code is a bitmap of that size, shown at the screen's own
//  scale with no interpolation. Every module is the same size, its edges
//  are hard, and what is left over is quiet zone, never a fraction of a
//  module. One renderer for every code the app shows: receive, the
//  multisig's invite and proposal links, the outside-wallet link, and the
//  caBLE code a phone scans.
//

import SwiftUI
import UIKit

enum QrRaster {

    /// The largest whole module, in pixels, at which `count` modules and a
    /// quiet zone of `quiet` modules on each side fit `sidePx`; at least 1.
    static func modulePx(count: Int, quiet: Int, sidePx: CGFloat) -> Int {
        let units = count + quiet * 2
        guard units > 0, sidePx.isFinite, sidePx > 0 else { return 1 }
        return max(1, Int((sidePx / CGFloat(units)).rounded(.down)))
    }

    /// The code as a bitmap: each module `modulePx` pixels square, `quiet`
    /// modules of paper around it — `(count + 2·quiet) · modulePx` pixels a
    /// side, nothing anti-aliased. `nil` for an empty or ragged grid.
    static func image(
        _ modules: [[Bool]], modulePx: Int, quiet: Int,
        ink: UIColor = .black, paper: UIColor = .white
    ) -> CGImage? {
        let count = modules.count
        guard count > 0, modules.allSatisfy({ $0.count == count }), modulePx > 0, quiet >= 0 else { return nil }
        let side = (count + quiet * 2) * modulePx
        guard let context = CGContext(
            data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        context.setShouldAntialias(false)
        context.setAllowsAntialiasing(false)
        context.interpolationQuality = .none
        context.setFillColor(paper.cgColor)
        context.fill(CGRect(x: 0, y: 0, width: side, height: side))
        context.setFillColor(ink.cgColor)
        // Core Graphics counts rows from the bottom: row 0 is the top line.
        for (row, line) in modules.enumerated() {
            var col = 0
            while col < count {
                guard line[col] else { col += 1; continue }
                // Runs, not single modules: fewer rectangles, the same picture.
                var end = col
                while end + 1 < count, line[end + 1] { end += 1 }
                context.fill(CGRect(
                    x: (col + quiet) * modulePx,
                    y: side - (row + quiet + 1) * modulePx,
                    width: (end - col + 1) * modulePx,
                    height: modulePx
                ))
                col = end + 1
            }
        }
        return context.makeImage()
    }
}

/// A code's modules on paper, crisp at the screen's scale. It takes the
/// square it is offered, draws the largest whole-pixel code that fits and
/// centres it; the rest is paper.
struct QrModulesView: View {
    @Environment(\.displayScale) private var displayScale

    let modules: [[Bool]]
    /// Quiet modules on each side, inside the square.
    var quiet: Int = 2
    var ink: Color = .black
    var paper: Color = .white

    var body: some View {
        GeometryReader { geometry in
            let side = min(geometry.size.width, geometry.size.height)
            let scale = max(1, displayScale)
            let px = QrRaster.modulePx(count: modules.count, quiet: quiet, sidePx: side * scale)
            ZStack {
                paper
                if let image = QrRaster.image(
                    modules, modulePx: px, quiet: quiet, ink: UIColor(ink), paper: UIColor(paper)
                ) {
                    Image(decorative: image, scale: scale)
                        .interpolation(.none)
                        .antialiased(false)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        .aspectRatio(1, contentMode: .fit)
        // One element, an image: a caller's identifier and label land on it.
        .accessibilityElement(children: .ignore)
        .accessibilityAddTraits(.isImage)
    }
}
