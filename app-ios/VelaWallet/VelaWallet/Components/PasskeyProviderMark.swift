//
//  PasskeyProviderMark.swift
//  VelaWallet
//
//  The mark of the vault holding a passkey — Apple Passwords, 1Password,
//  Windows Hello — rasterized by vela-core from the AAGUID the authenticator
//  reported at registration (spec 019, founder call 2026-08-26).
//
//  Four sources, in order: the provider's own mark from the compiled catalog;
//  the directory service's mark for a model no catalog carries (hardware keys);
//  the security-key artwork when neither can name it but the authenticator at
//  least said what KIND it is; and, for a key list that wants its slot filled,
//  the method glyph this row always had.
//  A platform authenticator the catalog cannot name gets the last of the three,
//  because "a passkey on this device" is all that is known about it.
//
//  The lookup is offline by construction — a directory service would learn
//  which vault holds this wallet's key, and that is nobody's business.
//

import SwiftUI
import VelaCore

/// Decoded-image cache keyed by AAGUID + theme + pixel size (the catalog is a
/// pure function of all three, so entries never invalidate).
@MainActor
private enum PasskeyMarkCache {
    private static let cache = NSCache<NSString, UIImage>()
    /// Misses are remembered too: an unknown AAGUID must not re-enter the FFI
    /// on every redraw of a list that scrolls.
    private static var misses: Set<String> = []

    static func provider(aaguid: String, dark: Bool, sizePx: UInt32, scale: CGFloat) -> UIImage? {
        decoded(key: "p|\(aaguid)|\(dark)|\(sizePx)", scale: scale) {
            try? passkeyProviderPng(aaguid: aaguid, dark: dark, sizePx: sizePx)
        }
    }

    /// The security-key artwork, in the caller's palette. Keyed by the palette
    /// too: a light-mode image must not survive into dark mode.
    static func fallback(
        key: CreateKeyRow,
        palette: MarkPalette,
        sizePx: UInt32,
        scale: CGFloat
    ) -> UIImage? {
        let id = "f|\(key.authenticatorAttachment)|\(key.transports)|\(key.kind.rawValue)"
        return decoded(key: "\(id)|\(palette.strong)|\(sizePx)", scale: scale) {
            try? passkeyFallbackPng(
                authenticatorAttachment: key.authenticatorAttachment,
                transports: key.transports,
                choseSecurityKey: key.kind == .securityKey,
                strong: palette.strong,
                soft: palette.soft,
                hole: palette.hole,
                sizePx: sizePx
            )
        }
    }

    private static func decoded(
        key: String,
        scale: CGFloat,
        bytes: () -> Data??
    ) -> UIImage? {
        if misses.contains(key) { return nil }
        if let hit = cache.object(forKey: key as NSString) { return hit }
        guard let data = bytes() ?? nil, let image = UIImage(data: data, scale: scale) else {
            misses.insert(key)
            return nil
        }
        cache.setObject(image, forKey: key as NSString)
        return image
    }
}

/// The three colour slots the fallback artwork wears — the app's tokens, not a
/// vendor's greys.
struct MarkPalette {
    let strong: String
    let soft: String
    let hole: String
}

struct PasskeyProviderMark: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.displayScale) private var displayScale

    /// The row this mark stands for; everything comes off it.
    let key: CreateKeyRow
    /// The mark's accessible label.
    let label: String
    var size: CGFloat = Tokens.Control.sm
    /// Draw the method's glyph when there is no artwork at all. The key list
    /// wants a filled slot; the done card wants nothing rather than a
    /// placeholder.
    var glyphFallback: Bool = false

    var body: some View {
        if let image = mark {
            Image(uiImage: image)
                .resizable()
                .frame(width: size, height: size)
                .accessibilityLabel(label)
        } else if glyphFallback {
            Image(systemName: key.kind == .securityKey ? "key.horizontal" : "person.badge.key")
                .foregroundStyle(theme.fgMuted)
                .frame(width: size, height: size)
                .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r8))
        }
    }

    private var mark: UIImage? {
        let pixels = UInt32((size * displayScale).rounded())
        let dark = colorScheme == .dark
        if !key.aaguid.isEmpty,
           let provider = PasskeyMarkCache.provider(
               aaguid: key.aaguid,
               dark: dark,
               sizePx: pixels,
               scale: displayScale
           ) {
            return provider
        }
        if !key.aaguid.isEmpty,
           let listed = PasskeyDirectory.shared.entry(aaguid: key.aaguid, dark: dark),
           let mark = PasskeyDirectory.shared.mark(for: listed, sizePx: pixels) {
            return mark
        }
        return PasskeyMarkCache.fallback(
            key: key,
            palette: theme.markPalette,
            sizePx: pixels,
            scale: displayScale
        )
    }
}
