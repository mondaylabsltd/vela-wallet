//
//  WalletGeometry.swift
//  VelaWallet
//
//  Wallet-home geometry the token set does not name (spec 015), measured
//  from docs/design/wallet/H1 at the 390×844 design frame (@2x pixels ÷ 2).
//  Licensed by docs/design-system.md ("if a needed token doesn't exist…
//  propose a semantic name") — kept here, never inline in views.
//

import SwiftUI

enum WalletGeometry {
    /// Identicon avatar in the wallet header (mock: 80 px @2x).
    static let avatar: CGFloat = 40
    /// The identicon viewer's artwork (spec 019 founder call) — big enough to
    /// read as a picture rather than an avatar, and to sit above a wrapped
    /// address on the narrowest phone.
    static let identiconViewer: CGFloat = 160
    /// Leading circle of activity/asset rows (mock: 80 px @2x).
    static let rowIcon: CGFloat = 40
    /// The editable amount on a split row (spec 054). Wide enough for a
    /// six-figure payroll line and narrow enough to leave the address room —
    /// measured against the SD2b drawing's own amount column.
    static let splitAmountWidth: CGFloat = 96
    /// The batch importer's paste box (spec 054) — four mono lines, matching
    /// the `lineLimit: 4` the drawn field used.
    static let batchPasteHeight: CGFloat = 96
    /// Chain-dot badge on row icons (mock: 24 px @2x).
    static let badge: CGFloat = 12
    /// Ring separating the badge from the icon (bg-colored).
    static let badgeRing: CGFloat = 2
    /// Network-pill chain dots and their overlap in the all-networks variant.
    static let pillDot: CGFloat = 14
    static let pillDotOverlap: CGFloat = 7
    /// Pill height — design-system control.sm.
    static let pillHeight: CGFloat = Tokens.Control.sm
    /// Zero-live pulsing indicator dot (H2).
    static let liveDot: CGFloat = 8
    /// Hidden-balance dot glyphs (H5: six large dots) and their count.
    static let hiddenDot: CGFloat = 18
    static let hiddenDotCount = 6
    /// Hero amount may shrink to this factor before it would clip (H7 at
    /// 1.35× — spec edge case: render fully, never clip).
    static let heroMinScale: CGFloat = 0.7
    /// 收款/转账/扫码 card minimum height (mock ≈ 136 px @2x).
    static let actionCardHeight: CGFloat = 68
    /// Activity/asset row minimum height (mock ≈ 128 px @2x).
    static let rowMinHeight: CGFloat = 64
    /// Divider inset that aligns with the row text column (icon + gap).
    static let rowDividerInset: CGFloat = 52
    /// Empty-state outline icon point size.
    static let emptyIcon: CGFloat = 28
    /// Custom tab-bar content height (excl. bottom safe area).
    static let tabBarHeight: CGFloat = 56
    /// Skeleton geometry (H3): balance block + row bars.
    static let skeletonBalanceWidth: CGFloat = 210
    static let skeletonBalanceHeight: CGFloat = 44
    static let skeletonBarHeight: CGFloat = 14
    static let skeletonTitleWidth: CGFloat = 120
    static let skeletonValueWidth: CGFloat = 64
    /// H3 skeleton row counts (mock: two activity rows, three asset rows).
    static let skeletonActivityRows = 2
    static let skeletonAssetRows = 3
    /// Chain-select sheet (H8): drag handle, fixed detent, row dot.
    static let sheetHandleWidth: CGFloat = 36
    static let sheetHandleHeight: CGFloat = 5
    static let chainSheetHeight: CGFloat = 420
    static let chainRowHeight: CGFloat = 44
    static let chainDot: CGFloat = 10
    /// The slot a chain row's logo fills, with the dot centred in it, so rows
    /// with and without a logo keep their names on one line.
    static let chainLogo: CGFloat = 20
    /// QR placeholder: 21×21 module grid (data-model.md) on an always-white
    /// card. Inks are mode-invariant (web: color-fixed shadowInk / onAccent).
    static let qrModules = 21
    static let qrSize: CGFloat = 200
    static let qrCard = TokenColor(argb: 0xFFFFFFFF).color
    static let qrInk = TokenColor(argb: 0xFF1A1A18).color
    /// Gallery identicon-board tile.
    static let identiconTile: CGFloat = 64
}

/// Fixture-only chain palette (spec 015 data-model.md) — DATA colors carried
/// by the wallet fixtures, not theme tokens. They live in DesignSystem/
/// because it is the sanctioned home of literal color values; fixtures and
/// components reference these names only.
enum ChainPalette {
    static let bnb = TokenColor(argb: 0xFFF0B90B).color // #F0B90B
    static let ethereum = TokenColor(argb: 0xFF627EEA).color // #627EEA
    static let arbitrum = TokenColor(argb: 0xFF28A0F0).color // #28A0F0
    static let gnosis = TokenColor(argb: 0xFF21BCA5).color // #21BCA5
    static let base = TokenColor(argb: 0xFF0052FF).color // #0052FF
    static let polygon = TokenColor(argb: 0xFF8247E5).color // #8247E5
    // Spec 021: the receive list is the first screen to draw all eight
    // supported networks, not just the six the home holds balances on.
    static let optimism = TokenColor(argb: 0xFFFF0420).color // #FF0420
    static let avalanche = TokenColor(argb: 0xFFE84142).color // #E84142
    /// A chain with no brand colour in the drawings — Tempo and X Layer wear it
    /// in `SettingsFixtures`, and since spec 050 so does every chain the core
    /// knows about that nobody drew. Neutral on purpose: a chain must never be
    /// handed somebody else's brand.
    static let unbranded = TokenColor(argb: 0xFF8C8C8C).color // #8C8C8C
    /// All-networks pill dots, in mock order (Ethereum, Polygon, BNB).
    static let pillDots: [Color] = [ethereum, polygon, bnb]
}

// MARK: - Wallet text scale (FR-011)

/// Gallery text-scale multiplier (1.0× / 1.35×, mock H7x). Components
/// multiply their type roles by this value; geometry stays fixed, rows grow
/// vertically. Distinct from OS Dynamic Type, which stays active on top.
private struct WalletTextScaleKey: EnvironmentKey {
    static let defaultValue: CGFloat = 1
}

extension EnvironmentValues {
    var walletTextScale: CGFloat {
        get { self[WalletTextScaleKey.self] }
        set { self[WalletTextScaleKey.self] = newValue }
    }
}

/// The size a person chose in 设置 → 字号 (spec 056).
///
/// A shared value rather than a parameter threaded through every model, for the
/// same reason `Formats.current` is: every renderer only READS it, and it is
/// written once at boot and again when somebody moves the slider.
///
/// It MULTIPLIES a screen's own scale rather than replacing it — the gallery's
/// 1.35× chip and a person's "large" are two different statements about the
/// same text, and a screen that replaced one with the other would make the
/// gallery lie about what somebody sees.
enum UiScale {
    nonisolated(unsafe) static var factor: CGFloat = 1

    @MainActor
    static func apply(_ preferences: Preferences) {
        factor = preferences.textScale.factor
    }
}

extension View {
    /// A screen's own text scale, times the person's chosen size.
    func walletTextScale(_ modelScale: CGFloat) -> some View {
        environment(\.walletTextScale, modelScale * UiScale.factor)
    }
}
