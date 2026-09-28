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
import Observation
import VelaCore

/// The bytes, cached in memory and on disk, with the URLs known to have no
/// image behind them — and for how long (spec 082 RE10, W20).
///
/// How long a miss lasts is the core's rule (`markMissTtlMs`): a 404, a
/// refusal or bytes that are not an image are misses for the session; a 5xx,
/// a throttle or a connection that never answered is transient — asked again
/// after a minute, and at once when the network comes back. Before 082 every
/// miss lasted the session, so a launch behind a dead proxy drew letters for
/// good.
@MainActor
enum LogoStore {
    private static let cache = NSCache<NSString, UIImage>()
    /// url → when the miss ends (ms since 1970), `nil` = the session.
    private static var misses: [String: Double?] = [:]
    /// The clock, a seam.
    static var now: () -> Double = { Date().timeIntervalSince1970 * 1000 }
    /// The one re-ask scheduled for the earliest transient miss.
    private static var reask: Task<Void, Never>?

    /// A session with a disk cache, so a relaunch draws yesterday's logos
    /// before the network answers.
    private static let session: URLSession = {
        let config = URLSessionConfiguration.default
        config.urlCache = URLCache(memoryCapacity: 4 << 20, diskCapacity: 32 << 20,
                                   diskPath: "vela-logos")
        config.requestCachePolicy = .returnCacheDataElseLoad
        config.timeoutIntervalForRequest = 8
        return URLSession.vela(config)
    }()

    static func cached(_ url: String) -> UIImage? { cache.object(forKey: url as NSString) }

    /// The first candidate that answers with an image.
    static func load(_ urls: [String]) async -> UIImage? {
        for url in urls {
            if let hit = cached(url) { return hit }
            if isMissed(url) { continue }
            guard let target = URL(string: url) else { continue }
            guard let (data, response) = try? await session.data(from: target) else {
                recordMiss(url, status: nil, notAnImage: false)
                continue
            }
            let status = (response as? HTTPURLResponse)?.statusCode
            guard status == 200, let image = UIImage(data: data) else {
                recordMiss(url, status: status == 200 ? nil : status, notAnImage: status == 200)
                continue
            }
            cache.setObject(image, forKey: url as NSString)
            return image
        }
        return nil
    }

    /// Whether `url` is known to have no image right now. A transient miss
    /// whose minute is up is forgotten here.
    static func isMissed(_ url: String) -> Bool {
        guard let entry = misses[url] else { return false }
        guard let until = entry else { return true }
        if until > now() { return true }
        misses.removeValue(forKey: url)
        return false
    }

    /// A failed fetch, remembered for as long as the core says: by status
    /// when the server answered, else as bytes that are not an image or as a
    /// connection that never answered.
    static func recordMiss(_ url: String, status: Int?, notAnImage: Bool) {
        let kind = notAnImage ? "not_an_image" : "transport"
        let ttl = markMissTtlMs(kind: kind, status: status.flatMap { UInt16(exactly: $0) })
        // `updateValue`, not a subscript: assigning a `nil` expiry through
        // the subscript would REMOVE the entry — a session miss forgotten.
        misses.updateValue(ttl.map { now() + Double($0) }, forKey: url)
        if let ttl { scheduleReask(afterMs: Double(ttl)) }
    }

    /// The network came back (`NetWatch`): every transient miss is forgotten
    /// and the logos on screen ask again. A 404 stays a 404.
    static func networkCameBack() {
        let before = misses.count
        misses = misses.filter { $0.value == nil }
        if misses.count != before { LogoEpoch.shared.bump() }
    }

    /// When the earliest transient miss is up, the logos on screen ask again
    /// — no relaunch, no scroll needed.
    private static func scheduleReask(afterMs: Double) {
        guard reask == nil else { return }
        reask = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(max(0, afterMs)) * 1_000_000)
            reask = nil
            guard !Task.isCancelled else { return }
            LogoEpoch.shared.bump()
        }
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
        reask?.cancel()
        reask = nil
        session.configuration.urlCache?.removeAllCachedResponses()
    }
}

/// Bumped when forgotten misses may now load — what a logo view's task is
/// keyed on beside its candidates, so it asks again by itself.
@MainActor
@Observable
final class LogoEpoch {
    static let shared = LogoEpoch()
    private(set) var value = 0
    func bump() { value &+= 1 }
}

/// A logo view's task identity: its candidates, and the miss epoch.
private struct LogoAsk: Equatable {
    let urls: [String]
    let epoch: Int
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
        // again rather than keeping the last one's picture — and on the miss
        // epoch, so a logo a dead network kept away comes back by itself.
        .task(id: LogoAsk(urls: urls, epoch: LogoEpoch.shared.value)) {
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
            // row, which is worse than the lettermark. (The same candidates
            // asked again after a miss had no picture to keep.)
            image = nil
            let loaded = await LogoStore.load(urls)
            // The id moved on while this was out: its answer is for a row
            // that is no longer here.
            guard !Task.isCancelled else { return }
            image = loaded
        }
    }
}
