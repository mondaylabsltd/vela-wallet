//
//  ScreenshotViewerGeometry.swift
//  VelaWallet
//
//  The feedback screenshot viewer's visual constants (spec C, 2026-09-27:
//  "上传的截图要能点击放大预览吧"). A full-screen viewer on pure black — the
//  same in both app themes, like Photos — with white controls, so its values
//  are fixed rather than read off the active palette. Kept here, never inline
//  in views (DesignSystem is the sanctioned home for visual values).
//

import SwiftUI

enum ScreenshotViewerGeometry {
    /// The viewer's ground: pure black in both themes (C2).
    static let backdrop = Color.black
    /// The controls on it: white, and the counter at ~70 % (C2).
    static let control = Color.white
    static let counterOpacity: Double = 0.7
    /// `removeFromViewer`: the danger colour's light-on-dark variant — the
    /// dark palette's `error.base` (#F87171), ≈ 7.6 : 1 on black.
    static let danger = Tokens.dark.errorBase.color
    /// The ✕: a 44-pt target around a 22-pt glyph.
    static let closeTarget: CGFloat = 44
    static let closeGlyph: CGFloat = 22
    /// Both bars are at least one target tall.
    static let barHeight: CGFloat = 44
    static let barPadding: CGFloat = Tokens.Space.s8
    /// The remove button's own horizontal padding, so its 44-pt row is a
    /// comfortable target around a short word.
    static let removePadding: CGFloat = Tokens.Space.s16

    /// Black between pages while paging, as Photos leaves.
    static let pageGap: CGFloat = 20
    /// Zoom: 1 is aspect-fit; a double tap goes to 2×; a pinch to 4×.
    static let doubleTapZoom: CGFloat = 2
    static let maxZoom: CGFloat = 4

    /// Swipe down to close (C2): released past this share of the screen's
    /// height, or flicked faster than this (pt/s), it closes; otherwise it
    /// springs back. A flick back up above `cancelVelocity` keeps it open.
    static let closeFraction: CGFloat = 0.2
    static let flickVelocity: CGFloat = 1000
    static let cancelVelocity: CGFloat = -300
    /// The black is gone once the picture has travelled this share of the
    /// screen's height; the picture shrinks to at most this much less.
    static let fadeDistance: CGFloat = 0.5
    static let dragShrink: CGFloat = 0.3

    /// Open/close: the picture flies between its tile and the screen. A
    /// critically damped spring, so it lands without a wobble; the fallback
    /// (no tile to fly to) and reduced motion are a short fade.
    static let flight = Animation.spring(duration: 0.34, bounce: 0)
    static let fadeDuration: TimeInterval = 0.18
    /// The fallback's scale as the picture fades (C2: fade + scale).
    static let fadeScale: CGFloat = 0.92
    static let chromeFade: TimeInterval = 0.15
    static let springBack = Animation.spring(response: 0.36, dampingFraction: 0.84)
    /// Removing: the picture that went fades and shrinks while the next one
    /// comes up (a plain cross-fade under reduced motion).
    static let removeDuration: TimeInterval = 0.22
    static let removeOutgoingScale: CGFloat = 0.9
    static let removeIncomingScale: CGFloat = 0.96
    /// A tile's corner, which the picture takes back as it lands.
    static let tileCorner: CGFloat = Tokens.Radius.r12
    /// How long a tap waits for the keyboard to go down before the viewer
    /// opens — so the picture flies from where the tile IS, not from where
    /// it was while the keyboard pushed the sheet up.
    static let keyboardSettle: TimeInterval = 0.35
}
