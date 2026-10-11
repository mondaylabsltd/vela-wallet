//
//  ContentSizedSheet.swift
//  VelaWallet
//
//  A sheet as tall as what it holds (issue #480).
//
//  `.medium` crops a tall content on a 4.7" phone (#447) and `.large` leaves a
//  short one floating in a near-full-screen sheet. The content is measured
//  and the detent is that height — `ContactFormSheet`'s pattern, here once.
//  A scroll view takes over only where the content outgrows the screen (the
//  accessibility text sizes); everywhere else it fits and does not move.
//

import SwiftUI

private struct ContentSizedSheet: ViewModifier {
    /// What the content measures, starting from what it is expected to come
    /// to at a default text size — so the sheet opens near its height instead
    /// of growing from nothing.
    @State private var height: CGFloat

    init(expected: CGFloat) {
        _height = State(initialValue: expected)
    }

    func body(content: Content) -> some View {
        ScrollView {
            content
                .fixedSize(horizontal: false, vertical: true)
                .onGeometryChange(for: CGFloat.self) { proxy in
                    proxy.size.height
                } action: { measured in
                    height = measured
                }
        }
        .scrollBounceBehavior(.basedOnSize)
        .presentationDetents([.height(max(height, 1))])
    }
}

extension View {
    /// Present this sheet at its content's height. `expected` is the height
    /// it opens at before the first measurement.
    func contentSizedSheet(expected: CGFloat) -> some View {
        modifier(ContentSizedSheet(expected: expected))
    }
}
