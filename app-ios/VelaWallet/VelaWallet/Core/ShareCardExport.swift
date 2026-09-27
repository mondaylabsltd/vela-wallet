//
//  ShareCardExport.swift
//  VelaWallet
//
//  保存图片 — turning the drawn share card into a file in somebody's album.
//
//  The button has been on the receive sheet since spec 021 and did nothing.
//  What it produces is `ShareCardArtwork` (R4): not a screen, a render product
//  that ends up in a photo library and then in a chat, which is why its colours
//  are mode-invariant and it carries the app icon and the wordmark.
//
//  ## The logo first, then the pixels
//
//  `ImageRenderer` draws one frame, synchronously — a view that loads its own
//  picture would be rendered before the picture arrived. So `render` fetches
//  the network's logo first, gives up after `logoDeadline`, and draws the
//  lettered disc when there is no logo to draw. It is split from `save` so a
//  test can render the very picture the album gets and decode its code.
//
//  ## Add-only permission, asked for at the moment it is needed
//
//  `.addOnly` is the narrowest authorisation Photos offers: it lets the app put
//  one image in and never read the library. A wallet asking to READ somebody's
//  photos to save a QR code would be asking for far more than it needs.
//
//  ## Three outcomes, and the caller says which happened
//
//  Saved, refused, or failed. The corpus has words for the first two
//  (`已保存` / `需要权限`) and the third takes the same failure line the share
//  path uses — a save that silently does nothing is the defect this file
//  replaces.
//

import Photos
import SwiftUI

@MainActor
enum ShareCardExport {

    enum Outcome {
        case saved
        /// The person said no, or the system did.
        case denied
        case failed
    }

    /// How long the save waits for the network's logo before it draws the
    /// lettered disc instead. A save that hangs on a slow logo is a button
    /// that did nothing.
    static let logoDeadline: Duration = .seconds(4)

    /// No disk cache: the chain logos a wallet fetched are a list of where it
    /// holds money, and an erase clears `LogoStore`'s cache, not a second one.
    private static let logoSession = URLSession(configuration: .ephemeral)

    /// Render the card and write it to the photo library, as a PNG — the
    /// code's edges stay the edges they were drawn with.
    static func save(_ card: ShareCardModel) async -> Outcome {
        guard let data = await render(card)?.pngData() else { return .failed }

        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        guard status == .authorized || status == .limited else { return .denied }

        return await withCheckedContinuation { continuation in
            PHPhotoLibrary.shared().performChanges {
                PHAssetCreationRequest.forAsset().addResource(with: .photo, data: data, options: nil)
            } completionHandler: { success, _ in
                continuation.resume(returning: success ? .saved : .failed)
            }
        }
    }

    /// The card as the album gets it: the network's logo fetched (or the
    /// lettered disc), drawn at `ShareCardGeometry.scale` — 960 pixels wide on
    /// every phone, as on the web, whatever the screen's own scale — and as
    /// tall as its content.
    static func render(_ card: ShareCardModel) async -> UIImage? {
        draw(card, logo: await logo(card.networkMark.logoURLs))
    }

    /// The drawing step alone, with the logo already in hand.
    static func draw(_ card: ShareCardModel, logo: UIImage?) -> UIImage? {
        let scale = ShareCardGeometry.scale
        let renderer = ImageRenderer(content:
            ShareCardArtwork(model: card, logo: logo)
                // The code snaps to the pixels of the picture being MADE, not
                // to the screen the button was tapped on.
                .environment(\.displayScale, scale)
                .themed(.light)
        )
        renderer.scale = scale
        // Paper to every edge — nothing in the picture is see-through.
        renderer.isOpaque = true
        return renderer.uiImage
    }

    /// The first candidate that answers with an image, within `logoDeadline`.
    ///
    /// A logo the receive screen already drew is in `LogoStore` and costs
    /// nothing. WebP bytes behind a `.png` name (Gnosis, Tempo) decode like
    /// any other: `UIImage(data:)` reads WebP.
    static func logo(_ urls: [String]) async -> UIImage? {
        if let hit = urls.lazy.compactMap(LogoStore.cached).first { return hit }
        guard !urls.isEmpty else { return nil }
        return await withTaskGroup(of: UIImage?.self) { group in
            group.addTask { await fetch(urls) }
            group.addTask {
                try? await Task.sleep(for: logoDeadline)
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }

    private static func fetch(_ urls: [String]) async -> UIImage? {
        for url in urls {
            guard let target = URL(string: url),
                  let (data, response) = try? await logoSession.data(from: target),
                  (response as? HTTPURLResponse)?.statusCode == 200,
                  let image = UIImage(data: data)
            else { continue }
            return image
        }
        return nil
    }
}
