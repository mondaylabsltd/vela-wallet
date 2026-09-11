//
//  ContactsGeometry.swift
//  VelaWallet
//
//  Contacts geometry + motion the token set does not name (spec 018,
//  research.md D9), measured from docs/design/contacts/C1…C6 at the 390×844
//  design frame (mock pixels ≈ points). Licensed by docs/design-system.md
//  ("if a needed token doesn't exist… propose a semantic name") — values
//  live here, never inline in views (audit-literals).
//

import SwiftUI

enum ContactsGeometry {
    /// Search well (C1: sunken rounded field, ≈33 pt tall → control.sm).
    static let searchFieldHeight: CGFloat = Tokens.Control.sm
    static let searchFieldRadius: CGFloat = Tokens.Radius.r12

    /// Group tile — rounded square with the users-round glyph (C1 ≈35 pt).
    static let groupTile: CGFloat = 36
    static let groupTileRadius: CGFloat = Tokens.Radius.r12
    /// Group row height (C1 divider pitch ≈57 pt).
    static let groupRowHeight: CGFloat = 56

    /// Contact row: 015 row metrics reused (avatar 40, min height 64).
    static let rowAvatar: CGFloat = WalletGeometry.avatar
    static let rowMinHeight: CGFloat = WalletGeometry.rowMinHeight
    /// Group-detail member rows are the tighter variant (C4 ≈57 pt pitch).
    static let memberAvatar: CGFloat = 36
    static let memberRowHeight: CGFloat = 56

    /// A–Z letter-section header: letter + hairline running to the edge.
    static let letterHeaderHeight: CGFloat = 24
    static let letterRuleLeading: CGFloat = Tokens.Space.s16

    /// Index rail (C1 right edge) and its letter-bubble HUD.
    static let indexRailWidth: CGFloat = 20
    static let indexLetterHeight: CGFloat = 11
    static let bubbleSize: CGFloat = 56
    static let bubbleTrailingGap: CGFloat = Tokens.Space.s32

    /// Swipe-revealed actions (转账 / 删除) behind a contact row.
    static let swipeActionWidth: CGFloat = 72
    static var swipeRevealWidth: CGFloat { swipeActionWidth * 2 }
    /// Drag distance past which the reveal snaps open.
    static let swipeOpenThreshold: CGFloat = 40

    /// Detail hero (C2 measures ≈59 pt — see results deviation note).
    static let detailAvatar: CGFloat = 64
    /// Group chips (家人 / + 分组) — compact pills.
    static let chipHeight: CGFloat = 24
    /// Address block: two mono lines + trailing copy button.
    static let addressBlockMinHeight: CGFloat = Tokens.Control.md

    /// Ghost 添加成员 row: dashed circle with a plus.
    static let ghostCircle: CGFloat = 36
    static let ghostDash: CGFloat = 3

    /// Action-menu sheet rows (C5/C6 pitch ≈57 pt).
    static let menuRowHeight: CGFloat = 56
    /// Empty-state icon tile (C3 outline circle).
    static let emptyTile: CGFloat = 72
}

/// Named motion constants for the contacts surfaces (FR-011). Reduced
/// motion swaps rises/slides for the crossfade (SPEC 动效 sheets).
enum ContactsMotion {
    /// Bottom-sheet rise (SPEC: 250ms ease-out).
    static let sheetRise: TimeInterval = Tokens.Motion.base
    /// Swipe-action reveal (SPEC: 250ms ease-out).
    static let swipeReveal: TimeInterval = Tokens.Motion.base
    /// Reduced-motion replacement + content swap (SPEC: 150ms crossfade).
    static let crossfade: TimeInterval = Tokens.Motion.fast
    /// Index-rail bubble HUD (SPEC: fade-in 120ms / fade-out 80ms).
    static let bubbleIn: TimeInterval = 0.12
    static let bubbleOut: TimeInterval = 0.08
    /// Pointer/press feedback on rows (SPEC desktop hover: 120ms).
    static let hover: TimeInterval = 0.12
}
