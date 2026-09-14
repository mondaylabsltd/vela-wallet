//
//  IdenticonAvatar.swift
//  VelaWallet
//
//  Nimiq identicon avatar via vela-core (spec 015 US3 / research D1):
//  normalize the seed, rasterize to PNG at device scale, cache by
//  seed+pixel-size. Empty/invalid seeds fall back to the shared
//  placeholder artwork; a dead FFI falls back to a plain themed circle.
//  The renderer is environment-injectable so #Previews can run without
//  the native dylib (FR-006 — no initial-letter path anywhere).
//

import SwiftUI
import VelaCore

/// PNG renderer contract. `.live` calls vela-core; `.previewSafe` returns
/// nil so the avatar renders its plain-circle fallback.
struct IdenticonProvider {
    let png: (_ seed: String, _ sizePx: UInt32) -> Data?

    static let live = IdenticonProvider { seed, sizePx in
        let normalized = identiconNormalizeSeed(seed: seed)
        if normalized.isEmpty {
            return try? identiconPlaceholderPng(sizePx: sizePx)
        }
        if let data = try? identiconPng(seed: normalized, sizePx: sizePx) {
            return data
        }
        return try? identiconPlaceholderPng(sizePx: sizePx)
    }

    static let previewSafe = IdenticonProvider { _, _ in nil }
}

private struct IdenticonProviderKey: EnvironmentKey {
    static let defaultValue = IdenticonProvider.live
}

extension EnvironmentValues {
    var identiconProvider: IdenticonProvider {
        get { self[IdenticonProviderKey.self] }
        set { self[IdenticonProviderKey.self] = newValue }
    }
}

/// Decoded-image cache keyed by seed + pixel size (identicons are pure
/// functions of both, so entries never invalidate).
@MainActor
private enum IdenticonCache {
    private static let cache = NSCache<NSString, UIImage>()

    static func image(seed: String, sizePx: UInt32, scale: CGFloat, provider: IdenticonProvider) -> UIImage? {
        let key = "\(seed)|\(sizePx)" as NSString
        if let hit = cache.object(forKey: key) { return hit }
        guard let data = provider.png(seed, sizePx),
              let image = UIImage(data: data, scale: scale) else { return nil }
        cache.setObject(image, forKey: key)
        return image
    }
}

struct IdenticonAvatar: View {
    @Environment(\.theme) private var theme
    @Environment(\.displayScale) private var displayScale
    @Environment(\.identiconProvider) private var provider

    let seed: String
    let size: CGFloat

    var body: some View {
        Group {
            // An empty seed draws NOTHING but a themed circle.
            //
            // An identicon is an identity claim, and the founder's
            // anti-poisoning rule is that only a real address earns one — a
            // picture drawn from a placeholder is a face for nobody, and on the
            // send form it sat above an empty recipient field looking like
            // somebody had been chosen (device-found, spec 052 phase 3).
            if seed.isEmpty {
                Circle().fill(theme.bgSunken)
            } else if let image = IdenticonCache.image(
                seed: seed,
                sizePx: UInt32(max(1, (size * displayScale).rounded())),
                scale: displayScale,
                provider: provider
            ) {
                Image(uiImage: image)
                    .resizable()
                    .interpolation(.high)
            } else {
                // FFI unavailable (previews without the dylib): themed circle.
                Circle().fill(theme.bgSunken)
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .accessibilityHidden(true)
    }
}

#Preview("Identicon live") {
    HStack(spacing: Tokens.Space.s12) {
        IdenticonAvatar(seed: "alice", size: WalletGeometry.avatar)
        IdenticonAvatar(seed: "bob", size: WalletGeometry.avatar)
        IdenticonAvatar(seed: "", size: WalletGeometry.avatar)
    }
    .padding(Tokens.Space.s24)
    .themed(.light)
}

#Preview("Identicon preview-safe fallback") {
    IdenticonAvatar(seed: "alice", size: WalletGeometry.avatar)
        .environment(\.identiconProvider, .previewSafe)
        .padding(Tokens.Space.s24)
        .themed(.dark)
        .background(Tokens.dark.bgBase.color)
}
