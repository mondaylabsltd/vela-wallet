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

/// Opens the identicon viewer for a seed. Provided once, high in the tree;
/// `nil` where there is no viewer to open (the gallery, a share card being
/// rendered to an image).
///
/// This is the mechanism Android uses (`LocalIdenticonViewer`) and the reason
/// its viewer reaches every avatar while iOS's reached exactly one: the
/// AVATAR opens it, so a new call site needs no wiring and cannot forget.
private struct IdenticonViewerKey: EnvironmentKey {
    static let defaultValue: ((String, String?) -> Void)? = nil
}

extension EnvironmentValues {
    var identiconViewer: ((String, String?) -> Void)? {
        get { self[IdenticonViewerKey.self] }
        set { self[IdenticonViewerKey.self] = newValue }
    }
}

/// Which artwork an avatar draws — identicon or initials.
///
/// **In the environment, not only in a static.** `AvatarPreference.style` is
/// read by this view and changing it invalidates nothing, so picking 首字母
/// stored the choice and left every avatar on screen exactly as it was (the
/// founder, 2026-09-15; proved on the device by cropping the account row's
/// avatar and comparing the bytes). Android has drawn from `LocalAvatarStyle`
/// since 049 for this reason. The static remains the default, so previews and
/// the gallery need no injection.
private struct AvatarStyleKey: EnvironmentKey {
    static var defaultValue: AvatarStyle { AvatarPreference.style }
}

extension EnvironmentValues {
    var avatarStyle: AvatarStyle {
        get { self[AvatarStyleKey.self] }
        set { self[AvatarStyleKey.self] = newValue }
    }
}

struct IdenticonAvatar: View {
    @Environment(\.theme) private var theme
    @Environment(\.displayScale) private var displayScale
    @Environment(\.identiconProvider) private var provider
    @Environment(\.identiconViewer) private var openViewer
    @Environment(\.avatarStyle) private var style

    let seed: String
    let size: CGFloat
    /// The name this avatar belongs to, for the initials style. Absent where
    /// there is no name to take one from — an address alone has no initial,
    /// and a letter cut from hex would be a face for nobody.
    var name: String?
    /// The founder, spec 048: 说好了 identicon 要能点击放大. Every artwork drawn
    /// from an address opens the viewer. An avatar that is part of a control
    /// somebody else owns — a row that navigates, a card being rendered to an
    /// image — passes `false` rather than stealing the tap.
    var tappable = true

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
            } else if style == .initials, let initial = initial {
                // 首字母 — the style a person chose (spec 057). It was stored
                // and read by nothing, which is the same defect Android 049
                // found on its own settings page: a control that previews
                // itself and changes no real surface.
                Circle().fill(theme.bgRaised)
                    .overlay(
                        // A type ROLE scaled to this circle, not a system font
                        // at a computed size: the same avatar is drawn at five
                        // diameters, and the sanctioned seam is the only way a
                        // letter here wears the same face as every other.
                        Text(verbatim: initial)
                            .typeRole(Typography.rowTitle.scaled(
                                size / WalletGeometry.avatar
                            ))
                            .foregroundStyle(theme.fgBase)
                    )
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
        .modifier(IdenticonTap(
            open: tappable && !seed.isEmpty ? openViewer : nil,
            seed: seed,
            name: name
        ))
        .accessibilityHidden(true)
    }

    /// The first CHARACTER of the name, uppercased where that means anything.
    ///
    /// Not the first letter of the address: "0x" is every address's initial,
    /// and a wall of identical circles is worse than no choice at all. With no
    /// name, the identicon stands whatever the preference says.
    private var initial: String? {
        guard let first = (name ?? "").trimmingCharacters(in: .whitespaces).first
        else { return nil }
        return String(first).uppercased()
    }
}

/// The tap, applied only where there is a viewer to open.
///
/// A modifier rather than an `if` inside `body`: adding and removing a gesture
/// changes the view's identity, and an avatar that re-identified itself when a
/// sheet appeared would re-decode its image.
private struct IdenticonTap: ViewModifier {
    let open: ((String, String?) -> Void)?
    let seed: String
    let name: String?

    func body(content: Content) -> some View {
        if let open {
            content
                .contentShape(Circle())
                .onTapGesture { open(seed, name) }
        } else {
            content
        }
    }
}

/// Which avatar style is in force.
///
/// A shared value for the same reason `Formats.current` is: every avatar in the
/// app reads it, and threading it through twelve call sites would mean twelve
/// chances to forget one — which is how it came to be stored and read nowhere.
enum AvatarPreference {
    nonisolated(unsafe) static var style: AvatarStyle = .identicon

    @MainActor
    static func apply(_ preferences: Preferences) {
        style = preferences.avatarStyle
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
