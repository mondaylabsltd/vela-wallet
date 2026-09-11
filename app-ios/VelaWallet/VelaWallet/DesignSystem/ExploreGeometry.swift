//
//  ExploreGeometry.swift
//  VelaWallet
//
//  Explore + signing geometry the token set does not name (spec 022),
//  MEASURED off design/explore/*.png at the 392×844 design frame.
//  Licensed by docs/design-system.md ("if a needed token doesn't exist…
//  propose a semantic name") — kept here, never inline in views.
//

import SwiftUI

enum ExploreGeometry {
    /// Favourites tile avatar (mock E2: x33–88, so 56).
    static let tileAvatar: CGFloat = 56
    /// Its label sits one type step below a row label so "PancakeSwap" fits
    /// a quarter of the frame without an ellipsis, as it does in the mock.
    static let tileColumns = 4
    /// Site-row avatar (mock E2, 最近的 dApp rows).
    static let rowAvatar: CGFloat = 40
    /// Start-page search box (mock E2: y116–163).
    static let searchField: CGFloat = 48
    /// Browsing address pill and the toolbar under the page (mock E4).
    static let addressPill: CGFloat = 40
    static let browserBar: CGFloat = 56
    /// The boxed tab count, in the header and in the toolbar (mock E2/E4).
    static let tabCount: CGFloat = 26
    /// Tab-switcher card preview (mock E5: two columns, 3:4 previews).
    static let tabCardAspect: CGFloat = 3.0 / 4.0
    /// dApp avatar in the signing header and the chip beside it (mock CS1).
    static let signingAvatar: CGFloat = 36
    static let networkChip: CGFloat = 26
    /// Slide-to-confirm: 342×56 track, 48 knob (mock CS1, row y=770).
    static let slideTrack: CGFloat = 56
    static let slideKnob: CGFloat = 48
    /// Fraction of the track the knob must cross to commit (SPEC 动效).
    static let slideCommit: CGFloat = 0.88
    /// The token mark beside a hero amount.
    static let tokenMark: CGFloat = 22
    /// The sail on the empty start page (mock E1).
    static let emptyMark: CGFloat = 56
    /// The identicon in the signer row and the account chip.
    static let signerAvatar: CGFloat = 18
    static let accountChipAvatar: CGFloat = 16
}

/// Site and token brand colours. CONTENT, not theme: a site's pink is its
/// own, and a design token cannot name it (the rule ChainPalette follows).
enum BrandPalette {
    static let uniswap = TokenColor(argb: 0xFFFF_007A).color
    static let aave = TokenColor(argb: 0xFF8B_6DFF).color
    static let pancake = TokenColor(argb: 0xFF1F_C7D4).color
    static let polymarket = TokenColor(argb: 0xFF42_67F4).color
    static let opensea = TokenColor(argb: 0xFF20_81E2).color
    static let lido = TokenColor(argb: 0xFFF0_616D).color
    static let ens = TokenColor(argb: 0xFF52_84FF).color
    static let hyperliquid = TokenColor(argb: 0xFF50_D2C1).color
    static let curve = TokenColor(argb: 0xFF7B_7BE8).color
    static let limitless = TokenColor(argb: 0xFF8B_6DFF).color
    static let oneinch = TokenColor(argb: 0xFFC2_352D).color
    static let morpho = TokenColor(argb: 0xFF2E_5BFF).color
    static let safe = TokenColor(argb: 0xFF12_FF80).color
    static let unknown = TokenColor(argb: 0xFF6E_6B62).color

    static let usdc = TokenColor(argb: 0xFF27_75CA).color
    static let eth = TokenColor(argb: 0xFF62_7EEA).color
    static let weth = TokenColor(argb: 0xFF8A_92B2).color
    static let spweth = TokenColor(argb: 0xFF4C_6FFF).color
    static let usdt = TokenColor(argb: 0xFF26_A17B).color
    static let contact = TokenColor(argb: 0xFFE8_572A).color

    /// The page a stand-in dApp draws inside the browser (spec 022 §2). Its
    /// palette is the SITE's, so it lives beside the other content colours
    /// rather than pretending to be part of our system.
    enum DemoPage {
        static let surface = TokenColor(argb: 0xFFF0_EFEC).color
        static let card = TokenColor(argb: 0xFFFF_FFFF).color
        static let field = TokenColor(argb: 0xFFF5_F3EF).color
        static let ink = TokenColor(argb: 0xFF1A_1A18).color
        static let inkMuted = TokenColor(argb: 0xFF8C_887E).color
    }
}
