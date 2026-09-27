//
//  ShareCardArtwork.swift
//  VelaWallet
//
//  R4 — what "Save image" produces (spec 021; redrawn 2026-09-27 to the WeChat
//  Pay collection card the founder holds it against).
//
//  Not a screen. It is a render product that ends up in someone's photo
//  library and then in a chat, so its colours are mode-invariant, its sizes do
//  not follow the text-size settings, and it carries the app icon and the
//  wordmark: away from the app, the card has to say what it is on its own.
//
//  The composition, top to bottom: the app icon's orange field with the
//  headline and, under it, the one network this address may be paid on; a
//  white sheet holding the code — the NETWORK's logo in its centre, where a
//  payer's eye lands before it scans — and under the code the account itself:
//  its identicon on the left, the name and the whole address in two mono lines
//  beside it; then the field closes over a white foot in one downward curve,
//  and the app icon and the wordmark stand on the white.
//
//  The identicon is DERIVED from the address, so a card someone doctored to
//  swap the address carries artwork that no longer matches it — which is why
//  it sits beside the characters it is checked against.
//
//  Every position is the web's (`share-image.ts`, `composeShareSvg`), computed
//  in `ShareCardLayout` from `ShareCardGeometry` and drawn at absolute points,
//  so the four platforms' cards can be laid over one another. `ImageRenderer`
//  draws this view, so everything in it is synchronous: the network's logo is
//  fetched BEFORE the render (`ShareCardExport`) and handed in as pixels.
//

import SwiftUI

/// Every position on the card, computed once — the port of `composeShareSvg`.
/// Pure apart from what the fonts measure, so a test can ask where a line
/// landed without rendering anything.
struct ShareCardLayout {
    private typealias G = ShareCardGeometry

    /// A line of text and the baseline it sits on.
    struct Line: Equatable {
        let text: String
        let baseline: CGFloat
    }

    let headlineSize: CGFloat
    let headline: [Line]
    let noteSize: CGFloat
    let note: Line
    let sheet: CGRect
    let qr: CGRect
    /// The code's centre — the plate's, the logo's and the disc's.
    let centre: CGPoint
    let tickerBaseline: CGFloat
    /// The name as drawn: whole, or cut to its room with an ellipsis. The
    /// address is never cut.
    let name: Line
    let address: [Line]
    let identicon: CGRect
    /// Where the name and the address start, right of the identicon.
    let textX: CGFloat
    /// Where the field's curve leaves the card's edges; it dips
    /// `curveDepth` below this at the centre.
    let edge: CGFloat
    let height: CGFloat
    let icon: CGRect
    let wordmarkX: CGFloat
    let wordmarkBaseline: CGFloat

    init(_ model: ShareCardModel) {
        let width = G.width
        let cx = width / 2

        // The orange: headline, then the network it may be paid on.
        let fit = Self.fitHeadline(model.headline) { text, size in
            ShareCardType.width(text, ShareCardType.sans(size, .bold))
        }
        headlineSize = fit.size
        let headlineLine = fit.size * G.headlineLeading
        headline = fit.lines.enumerated().map { index, text in
            Line(text: text,
                 baseline: Self.baseline(G.top + headlineLine * (CGFloat(index) + 0.5), fit.size))
        }
        let noteTop = G.top + headlineLine * CGFloat(fit.lines.count) + G.noteGap
        let noteWidth = ShareCardType.width(model.networkNote, ShareCardType.sans(G.noteSize, .medium))
        noteSize = noteWidth <= G.textWidth
            ? G.noteSize
            : max(G.noteMinSize, (G.noteSize * G.textWidth / noteWidth).rounded(.down))
        note = Line(text: model.networkNote,
                    baseline: Self.baseline(noteTop + G.noteLine / 2, noteSize))

        // The sheet and its code.
        let sheetY = noteTop + G.noteLine + G.sheetGap
        let qrY = sheetY + G.sheetPad
        qr = CGRect(x: (width - G.qr) / 2, y: qrY, width: G.qr, height: G.qr)
        centre = CGPoint(x: cx, y: qrY + G.qr / 2)
        tickerBaseline = Self.baseline(centre.y, G.tickerSize)

        // The account: identicon left, name and the whole address beside it,
        // the pair centred on the card by its drawn width.
        let idTop = qrY + G.qr + G.identityGap
        let textHeight = G.nameLine + G.nameAddressGap + G.addressLine * 2
        let textRoom = G.qr - G.identicon - G.identityTextGap
        let nameFont = ShareCardType.sans(G.nameSize, .bold)
        let monoFont = ShareCardType.mono(G.addressSize)
        let drawnName = Self.truncate(model.name, width: textRoom) {
            ShareCardType.width($0, nameFont)
        }
        let widest = ([ShareCardType.width(drawnName, nameFont)]
            + model.lines.map { ShareCardType.width($0, monoFont) }).max() ?? 0
        let blockWidth = G.identicon + G.identityTextGap + min(textRoom, widest)
        let idX = Self.round(cx - blockWidth / 2)
        identicon = CGRect(x: idX, y: idTop + (textHeight - G.identicon) / 2,
                           width: G.identicon, height: G.identicon)
        textX = idX + G.identicon + G.identityTextGap
        name = Line(text: drawnName, baseline: Self.baseline(idTop + G.nameLine / 2, G.nameSize))
        let addressTop = idTop + G.nameLine + G.nameAddressGap
        address = model.lines.enumerated().map { index, line in
            Line(text: line,
                 baseline: Self.baseline(addressTop + G.addressLine * (CGFloat(index) + 0.5),
                                         G.addressSize))
        }
        let sheetBottom = idTop + textHeight + G.sheetPadBottom
        sheet = CGRect(x: (width - G.sheetWidth) / 2, y: sheetY,
                       width: G.sheetWidth, height: sheetBottom - sheetY)

        // The field closes over the foot in one curve that dips at the centre,
        // and the brand line stands on the white, centred in the foot.
        edge = sheetBottom + G.curveGap
        let lowest = edge + G.curveDepth
        height = lowest + G.foot
        let wordmarkWidth = ShareCardType.width(model.wordmark,
                                                ShareCardType.sans(G.wordmarkSize, .bold))
        let brandX = Self.round(cx - (G.icon + G.iconGap + wordmarkWidth) / 2)
        let brandCentre = lowest + G.foot / 2
        icon = CGRect(x: brandX, y: brandCentre - G.icon / 2, width: G.icon, height: G.icon)
        wordmarkX = brandX + G.icon + G.iconGap
        wordmarkBaseline = Self.baseline(brandCentre, G.wordmarkSize)
    }

    /// The headline set to the card: one line at the largest size from 32 down
    /// to 26 that fits, else two lines split where the halves come out closest
    /// in width (at a space when there is one, anywhere in CJK), shrunk until
    /// the longer half fits.
    static func fitHeadline(
        _ text: String,
        measure: (String, CGFloat) -> CGFloat
    ) -> (size: CGFloat, lines: [String]) {
        let largest = G.headlineSize, smallest = G.headlineMinSize, room = G.textWidth
        let whole = measure(text, largest)
        if whole <= room { return (largest, [text]) }
        let shrunk = (largest * room / whole).rounded(.down)
        if shrunk >= smallest { return (shrunk, [text]) }

        let chars = Array(text)
        let hasSpace = chars.contains(" ")
        var best: [String]?
        var bestWidth = CGFloat.infinity
        for index in chars.indices.dropFirst() {
            if hasSpace, chars[index] != " " { continue }
            let first = String(chars[..<index]).trimmingCharacters(in: .whitespaces)
            let second = String(chars[index...]).trimmingCharacters(in: .whitespaces)
            if first.isEmpty || second.isEmpty { continue }
            let width = max(measure(first, largest), measure(second, largest))
            if width < bestWidth {
                best = [first, second]
                bestWidth = width
            }
        }
        // Nowhere to break (a single glyph): the smallest size, on one line.
        guard let best else { return (smallest, [text]) }
        return (min(largest, (largest * room / bestWidth).rounded(.down)), best)
    }

    /// `text` cut to `width` with an ellipsis, or whole when it fits.
    static func truncate(_ text: String, width: CGFloat, measure: (String) -> CGFloat) -> String {
        if measure(text) <= width { return text }
        var chars = Array(text)
        while chars.count > 1, measure(String(chars) + "\u{2026}") > width {
            chars.removeLast()
        }
        while chars.last?.isWhitespace == true { chars.removeLast() }
        return String(chars) + "\u{2026}"
    }

    /// The baseline that centres a line of `size` text on `centre` — the
    /// web's rule, so every platform's lines land on the same points.
    static func baseline(_ centre: CGFloat, _ size: CGFloat) -> CGFloat {
        Self.round(centre + size * G.baselineDrop)
    }

    private static func round(_ value: CGFloat) -> CGFloat {
        (value * 100).rounded() / 100
    }
}

struct ShareCardArtwork: View {
    private typealias G = ShareCardGeometry

    /// The code snaps to this device's pixels, so its modules meet with no
    /// hairline between them and no grey fringe round them.
    @Environment(\.displayScale) private var displayScale

    let model: ShareCardModel
    /// The network's logo, fetched before the render. `nil` draws the
    /// lettered disc — the ticker on the network's colour.
    var logo: UIImage?

    var body: some View {
        let layout = ShareCardLayout(model)
        ZStack(alignment: .topLeading) {
            G.paper
            ShareCardField(edge: layout.edge).fill(G.field)

            ForEach(Array(layout.headline.enumerated()), id: \.offset) { _, line in
                text(line, ShareCardType.sans(layout.headlineSize, .bold), G.paper,
                     centredOn: layout.centre.x)
            }
            text(layout.note, ShareCardType.sans(layout.noteSize, .medium), G.paper,
                 centredOn: layout.centre.x)

            RoundedRectangle(cornerRadius: G.sheetRadius, style: .circular)
                .fill(G.paper)
                .frame(width: layout.sheet.width, height: layout.sheet.height)
                .at(layout.sheet.origin)
            // `nil` is the gallery's drawn card: the demo pattern, which
            // nobody could mistake for a live code beside a fixture address.
            ShareCardCode(modules: model.modules ?? QrPattern.cells, frame: layout.qr,
                          scale: displayScale)
                .fill(G.ink)
            RoundedRectangle(cornerRadius: G.plateRadius, style: .circular)
                .fill(G.paper)
                .frame(width: G.plate, height: G.plate)
                .at(CGPoint(x: layout.centre.x - G.plate / 2, y: layout.centre.y - G.plate / 2))
            mark(layout)

            // Drawn for a PICTURE — an image has nothing to tap.
            IdenticonAvatar(seed: model.identiconSeed, size: G.identicon, tappable: false)
                .at(layout.identicon.origin)
            text(layout.name, ShareCardType.sans(G.nameSize, .bold), G.ink, from: layout.textX)
            ForEach(Array(layout.address.enumerated()), id: \.offset) { _, line in
                text(line, ShareCardType.mono(G.addressSize), G.ink.opacity(G.addressOpacity),
                     from: layout.textX)
            }

            ShareCardAppIcon()
                .frame(width: G.icon, height: G.icon)
                .at(layout.icon.origin)
            text(ShareCardLayout.Line(text: model.wordmark, baseline: layout.wordmarkBaseline),
                 ShareCardType.sans(G.wordmarkSize, .bold), G.ink, from: layout.wordmarkX)
        }
        .frame(width: G.width, height: layout.height)
    }

    /// The network in the code's centre: its logo, clipped round and ringed so
    /// a white one does not dissolve into the plate — or the lettered disc.
    @ViewBuilder
    private func mark(_ layout: ShareCardLayout) -> some View {
        let origin = CGPoint(x: layout.centre.x - G.logo / 2, y: layout.centre.y - G.logo / 2)
        if let logo {
            Image(uiImage: logo)
                .resizable()
                .interpolation(.high)
                .scaledToFill()
                .frame(width: G.logo, height: G.logo)
                .clipShape(Circle())
                .overlay(
                    Circle()
                        .inset(by: G.logoRing / 2)
                        .stroke(G.ink.opacity(G.logoRingOpacity), lineWidth: G.logoRing)
                )
                .at(origin)
        } else {
            Circle()
                .fill(model.networkMark.badgeColor)
                .frame(width: G.logo, height: G.logo)
                .at(origin)
            text(ShareCardLayout.Line(text: model.networkMark.ticker, baseline: layout.tickerBaseline),
                 ShareCardType.sans(G.tickerSize, .bold), G.paper, centredOn: layout.centre.x)
        }
    }

    /// A line centred on `x`, its baseline on the line's.
    private func text(_ line: ShareCardLayout.Line, _ font: UIFont, _ colour: Color,
                      centredOn x: CGFloat) -> some View {
        ShareCardType.text(line.text, font)
            .foregroundStyle(colour)
            .lineLimit(1)
            .fixedSize()
            .alignmentGuide(.leading) { $0.width / 2 - x }
            .alignmentGuide(.top) { $0[.firstTextBaseline] - line.baseline }
    }

    /// A line starting at `x`, its baseline on the line's.
    private func text(_ line: ShareCardLayout.Line, _ font: UIFont, _ colour: Color,
                      from x: CGFloat) -> some View {
        ShareCardType.text(line.text, font)
            .foregroundStyle(colour)
            .lineLimit(1)
            .fixedSize()
            .alignmentGuide(.leading) { _ in -x }
            .alignmentGuide(.top) { $0[.firstTextBaseline] - line.baseline }
    }
}

private extension View {
    /// Top-left corner at `point` in the card's coordinates — the SVG's x/y.
    func at(_ point: CGPoint) -> some View {
        alignmentGuide(.leading) { _ in -point.x }
            .alignmentGuide(.top) { _ in -point.y }
    }
}

/// The orange field: the card's top down to `edge`, closed by one quadratic
/// curve that dips `curveDepth` into the white foot at the centre — the WeChat
/// card's direction, the web's `Q` with its control point twice the dip down.
private struct ShareCardField: Shape {
    let edge: CGFloat

    func path(in rect: CGRect) -> Path {
        let width = ShareCardGeometry.width
        var path = Path()
        path.move(to: .zero)
        path.addLine(to: CGPoint(x: width, y: 0))
        path.addLine(to: CGPoint(x: width, y: edge))
        path.addQuadCurve(to: CGPoint(x: 0, y: edge),
                          control: CGPoint(x: width / 2,
                                           y: edge + ShareCardGeometry.curveDepth * 2))
        path.closeSubpath()
        return path
    }
}

/// The code as ONE path in the card's coordinates, every module edge snapped
/// to a device pixel — the SVG's `crispEdges`.
///
/// 224 points over 37 modules is not a whole number of pixels, so unsnapped
/// modules land on fractions, and a module drawn on its own fringes grey where
/// it should meet its neighbour: a hairline a camera reads as a broken code.
/// Snapped edges and one fill leave nothing between them.
private struct ShareCardCode: Shape {
    let modules: [[Bool]]
    let frame: CGRect
    let scale: CGFloat

    func path(in rect: CGRect) -> Path {
        let count = modules.count
        guard count > 0, scale > 0 else { return Path() }
        let step = frame.width / CGFloat(count)
        func snap(_ value: CGFloat) -> CGFloat { (value * scale).rounded() / scale }
        let xs = (0...count).map { snap(frame.minX + CGFloat($0) * step) }
        let ys = (0...count).map { snap(frame.minY + CGFloat($0) * step) }
        var path = Path()
        for (row, cells) in modules.enumerated() {
            // Runs, not single modules: fewer rectangles, the same picture.
            var column = 0
            while column < cells.count {
                guard cells[column] else {
                    column += 1
                    continue
                }
                var end = column
                while end + 1 < cells.count, cells[end + 1] { end += 1 }
                path.addRect(CGRect(x: xs[column], y: ys[row],
                                    width: xs[end + 1] - xs[column], height: ys[row + 1] - ys[row]))
                column = end + 1
            }
        }
        return path
    }
}

/// The canonical app icon, `docs/design/icon/app-icon.svg`, drawn from its own
/// 68-unit geometry: the plate, the main sail, the jib and the hull. Not the
/// in-app sailboat `VelaMark` — a card that leaves the app wears the icon a
/// person would find on the phone.
private struct ShareCardAppIcon: View {
    private typealias Art = ShareCardGeometry.AppIconArt

    var body: some View {
        ZStack {
            IconPart(part: .plate).fill(Art.plate)
            IconPart(part: .mainSail).fill(Art.mainSail)
            IconPart(part: .jib).fill(Art.jib)
            IconPart(part: .hull).fill(Art.hull)
        }
    }

    private struct IconPart: Shape {
        enum Part { case plate, mainSail, jib, hull }
        let part: Part

        func path(in rect: CGRect) -> Path {
            let unit = rect.width / Art.viewBox
            func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
                CGPoint(x: rect.minX + x * unit, y: rect.minY + y * unit)
            }
            var path = Path()
            switch part {
            case .plate:
                path.addRoundedRect(
                    in: CGRect(origin: p(Art.plateInset, Art.plateInset),
                               size: CGSize(width: Art.plateSize * unit, height: Art.plateSize * unit)),
                    cornerSize: CGSize(width: Art.plateRadius * unit, height: Art.plateRadius * unit),
                    style: .circular
                )
            case .mainSail:
                // M33,12 C25,21 20,32 17,42 L33,42 L33,12 Z
                path.move(to: p(33, 12))
                path.addCurve(to: p(17, 42), control1: p(25, 21), control2: p(20, 32))
                path.addLine(to: p(33, 42))
                path.closeSubpath()
            case .jib:
                // M36,19 C45,25 50,34 52,42 L36,42 L36,19 Z
                path.move(to: p(36, 19))
                path.addCurve(to: p(52, 42), control1: p(45, 25), control2: p(50, 34))
                path.addLine(to: p(36, 42))
                path.closeSubpath()
            case .hull:
                // M13,46 L55,46 C52,52 47,55 40,55 L28,55 C21,55 16,52 13,46 Z
                path.move(to: p(13, 46))
                path.addLine(to: p(55, 46))
                path.addCurve(to: p(40, 55), control1: p(52, 52), control2: p(47, 55))
                path.addLine(to: p(28, 55))
                path.addCurve(to: p(13, 46), control1: p(21, 55), control2: p(16, 52))
                path.closeSubpath()
            }
            return path
        }
    }
}
