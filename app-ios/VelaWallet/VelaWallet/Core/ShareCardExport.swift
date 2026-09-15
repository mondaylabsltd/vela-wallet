//
//  ShareCardExport.swift
//  VelaWallet
//
//  保存图片 — turning the drawn share card into a file in somebody's album.
//
//  The button has been on the receive sheet since spec 021 and did nothing.
//  What it produces is `ShareCardArtwork` (R4): not a screen, a render product
//  that ends up in a photo library and then in a chat, which is why its colours
//  are mode-invariant and it carries the wordmark.
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

    /// Render the card and write it to the photo library.
    ///
    /// Rendered at the device's scale so the code is sharp on a screen it is
    /// scanned FROM — a downsampled QR is one a camera has to work at.
    static func save(_ card: ShareCardModel, scheme: ColorScheme, scale: CGFloat) async -> Outcome {
        let renderer = ImageRenderer(content:
            ShareCardArtwork(model: card)
                .frame(width: WalletFlowGeometry.shareCardWidth)
                .themed(scheme)
        )
        renderer.scale = scale
        guard let image = renderer.uiImage else { return .failed }

        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        guard status == .authorized || status == .limited else { return .denied }

        return await withCheckedContinuation { continuation in
            PHPhotoLibrary.shared().performChanges {
                PHAssetChangeRequest.creationRequestForAsset(from: image)
            } completionHandler: { success, _ in
                continuation.resume(returning: success ? .saved : .failed)
            }
        }
    }
}
