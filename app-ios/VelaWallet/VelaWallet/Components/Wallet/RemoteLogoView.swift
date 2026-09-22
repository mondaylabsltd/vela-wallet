//
//  RemoteLogoView.swift
//  VelaWallet
//
//  A logo fetched from the chain-data endpoint, drawn OVER the glyph the shell
//  would draw anyway — the port of Android's `RemoteLogo.kt` and the web's
//  `RemoteLogo.svelte` (spec 047).
//
//  Three properties, and each is why this is a view rather than an `AsyncImage`:
//
//  - **The fallback is the whole mark, not a blank circle.** Nothing is drawn
//    until the bytes are here, so a slow network shows the lettermark rather
//    than a hole where an asset's identity should be.
//  - **A miss is remembered.** A token with no logo would otherwise re-fetch on
//    every scroll — one 404 per row per frame.
//  - **The first URL that answers wins.** `Marks` hands over candidates
//    (checksummed path, then lowercase) and the first image ends the search.
//

import SwiftUI

/// The bytes, cached in memory and on disk, with a set of URLs known to have
/// no image behind them.
@MainActor
enum LogoStore {
    private static let cache = NSCache<NSString, UIImage>()
    private static var misses: Set<String> = []

    /// A session with a disk cache, so a relaunch draws yesterday's logos
    /// before the network answers.
    private static let session: URLSession = {
        let config = URLSessionConfiguration.default
        config.urlCache = URLCache(memoryCapacity: 4 << 20, diskCapacity: 32 << 20,
                                   diskPath: "vela-logos")
        config.requestCachePolicy = .returnCacheDataElseLoad
        config.timeoutIntervalForRequest = 8
        return URLSession(configuration: config)
    }()

    static func cached(_ url: String) -> UIImage? { cache.object(forKey: url as NSString) }

    /// The first candidate that answers with an image.
    static func load(_ urls: [String]) async -> UIImage? {
        for url in urls {
            if let hit = cached(url) { return hit }
            if misses.contains(url) { continue }
            guard let target = URL(string: url) else { continue }
            guard let (data, response) = try? await session.data(from: target),
                  (response as? HTTPURLResponse)?.statusCode == 200,
                  let image = UIImage(data: data)
            else {
                misses.insert(url)
                continue
            }
            cache.setObject(image, forKey: url as NSString)
            return image
        }
        return nil
    }

    /// Tests and the erase-device path: forget everything, memory and disk.
    ///
    /// The `URLCache` matters as much as the `NSCache` (spec 081 FR-017): this
    /// session keeps 32 MB of logo bytes under `vela-logos` in the app's cache
    /// directory precisely so a relaunch draws yesterday's logos, which means
    /// it SURVIVES the thing an erase is for. The chain and token logos a
    /// person's wallet fetched are a readable list of what they hold, so a
    /// sweep that cleared memory and left the disk was leaving the inventory.
    static func forgetAll() {
        cache.removeAllObjects()
        misses.removeAll()
        session.configuration.urlCache?.removeAllCachedResponses()
    }
}

/// Draws `fallback` until a logo arrives, then the logo.
struct RemoteLogoView<Fallback: View>: View {
    let urls: [String]
    let size: CGFloat
    @ViewBuilder let fallback: () -> Fallback

    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFill()
            } else {
                fallback()
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        // Keyed on the candidates: a row reused for a different token asks
        // again rather than keeping the last one's picture.
        .task(id: urls) {
            guard !urls.isEmpty else {
                image = nil
                return
            }
            // A cached hit is drawn in the same frame — no flash of the glyph
            // on every scroll.
            if let hit = urls.compactMap(LogoStore.cached).first {
                image = hit
                return
            }
            // A different set of candidates is a different coin. Keeping the
            // last one's picture while the new one loads — or for good, when
            // the new one has no logo — draws somebody else's asset on this
            // row, which is worse than the lettermark.
            image = nil
            let loaded = await LogoStore.load(urls)
            // The id moved on while this was out: its answer is for a row
            // that is no longer here.
            guard !Task.isCancelled else { return }
            image = loaded
        }
    }
}
