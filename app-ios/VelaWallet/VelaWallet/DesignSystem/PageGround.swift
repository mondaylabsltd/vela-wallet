//
//  PageGround.swift
//  VelaWallet
//
//  The ground a page stands on — and the status bar's.
//
//  Every page of this app is a scroll view under no bar of its own: the
//  title, the back arrow and the rows all ride in the scroll. SwiftUI lets a
//  scroll view draw its content on into the safe area above it, which under a
//  navigation bar is what makes the bar's blur worth having — and with no bar
//  is a page scrolling up behind the clock. On an iPhone the Settings page
//  showed a paragraph and a row's value through the status bar (device walk
//  2026-10-10), and every other page did the same.
//
//  `.background(colour.ignoresSafeArea())` painted the ground BEHIND the
//  page, status bar included. `pageGround` paints the same ground and, over
//  whatever scrolls up, the strip above the safe area in the same colour: at
//  rest nothing looks different, and scrolled, the page passes under the
//  status bar instead of through it.
//
//  One modifier, so a new page cannot forget half of it — and `PageGround
//  Tests` (`StatusBarGroundTests`) reads the strip's pixels on each page.
//

import SwiftUI

extension View {
    /// The page's ground, and the status bar's over it.
    func pageGround(_ colour: Color) -> some View {
        modifier(PageGround(colour: colour))
    }
}

private struct PageGround: ViewModifier {
    let colour: Color

    func body(content: Content) -> some View {
        content
            .background(colour.ignoresSafeArea())
            .overlay(alignment: .top) {
                // No height of its own: only the safe area above the page,
                // which the ground reaches up into. Where the page does not
                // touch the top of the screen (a sheet, a board under the
                // gallery's chips) there is none, and this draws nothing.
                Color.clear
                    .frame(maxWidth: .infinity)
                    .frame(height: 0)
                    .background(colour, ignoresSafeAreaEdges: .top)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
    }
}
