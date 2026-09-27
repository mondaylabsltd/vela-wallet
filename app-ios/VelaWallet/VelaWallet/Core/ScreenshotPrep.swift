//
//  ScreenshotPrep.swift
//  VelaWallet
//
//  What a screenshot becomes before it may leave the phone (feedback with
//  screenshots, 2026-09-26). The founder ruled screenshots PUBLIC — they are
//  shown inline in the GitHub issue — so what is sent is only the pixels:
//
//  - decoded (PNG, JPEG, HEIC, WebP — whatever UIImage reads), drawn upright;
//  - scaled so the longest edge is at most 1920 px, never up;
//  - re-encoded as JPEG at 0.85, which drops the original's metadata — its
//    location, its camera, its time. Not optional, and not skipped for a PNG
//    that is already small: re-encoding IS the privacy guarantee;
//  - and any APP1 segment (EXIF, XMP) the encoder itself writes is cut out, so
//    the guarantee does not rest on one encoder's defaults.
//
//  Still over the endpoint's 2,000,000 bytes: again at 0.7, then at a 1440 px
//  edge. The same ladder the web and Android run.
//

import Foundation
import ImageIO
import UIKit
import UniformTypeIdentifiers

enum ScreenshotPrep {

    /// At most this many per report (the endpoint's `MAX_SCREENSHOTS`).
    static let maxCount = 5
    /// The longest edge a screenshot is sent at, then the fallback edge.
    static let maxEdge: CGFloat = 1920
    static let fallbackEdge: CGFloat = 1440
    static let quality: CGFloat = 0.85
    static let fallbackQuality: CGFloat = 0.7
    /// The endpoint's per-screenshot cap, decoded (`MAX_SCREENSHOT_BYTES`).
    static let maxBytes = 2_000_000

    /// A screenshot ready to send: the JPEG bytes, and the thumbnail drawn
    /// from THOSE bytes (never from the original, which is not kept).
    struct Prepared: Equatable {
        let jpeg: Data
        let pixelSize: CGSize
    }

    /// The picked bytes as a sendable JPEG, or `nil` when they are not an
    /// image this phone can decode. Pure and synchronous — call it off the
    /// main actor.
    nonisolated static func prepare(_ data: Data) -> Prepared? {
        guard let image = UIImage(data: data), image.size.width > 0, image.size.height > 0 else {
            return nil
        }
        for (edge, quality) in [(maxEdge, quality), (maxEdge, fallbackQuality), (fallbackEdge, fallbackQuality)] {
            guard let rendered = render(image, longestEdge: edge),
                  let jpeg = encode(rendered, quality: quality)
            else { return nil }
            if jpeg.count <= maxBytes || edge == fallbackEdge {
                return Prepared(jpeg: jpeg, pixelSize: CGSize(width: rendered.width, height: rendered.height))
            }
        }
        return nil
    }

    /// Upright, opaque pixels no larger than `longestEdge` on their longest
    /// side. Drawing the UIImage applies its EXIF orientation, so a portrait
    /// photo is sent the way it was seen.
    nonisolated static func render(_ image: UIImage, longestEdge: CGFloat) -> CGImage? {
        let width = image.size.width * image.scale
        let height = image.size.height * image.scale
        let factor = min(1, longestEdge / max(width, height))
        let target = CGSize(width: (width * factor).rounded(), height: (height * factor).rounded())
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        let renderer = UIGraphicsImageRenderer(size: target, format: format)
        let drawn = renderer.image { context in
            // A transparent PNG has nothing behind it; JPEG cannot say
            // "transparent", and black would hide dark-mode text.
            UIColor.white.setFill()
            context.fill(CGRect(origin: .zero, size: target))
            image.draw(in: CGRect(origin: .zero, size: target))
        }
        return drawn.cgImage
    }

    /// JPEG with no metadata of its own, and no APP1 segment whatever the
    /// encoder decided to write.
    nonisolated static func encode(_ image: CGImage, quality: CGFloat) -> Data? {
        let out = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            out, UTType.jpeg.identifier as CFString, 1, nil
        ) else { return nil }
        CGImageDestinationAddImage(destination, image, [
            kCGImageDestinationLossyCompressionQuality: quality,
        ] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return stripAPP1(out as Data)
    }

    /// The JPEG with every APP1 segment (EXIF, XMP) removed. Walks the marker
    /// segments up to the start of the scan; anything it cannot parse is
    /// handed back untouched only if it contains no APP1 at all.
    nonisolated static func stripAPP1(_ jpeg: Data) -> Data? {
        let bytes = [UInt8](jpeg)
        guard bytes.count > 4, bytes[0] == 0xFF, bytes[1] == 0xD8 else { return nil }
        var out: [UInt8] = [0xFF, 0xD8]
        var index = 2
        while index + 4 <= bytes.count {
            guard bytes[index] == 0xFF else { return nil }
            let marker = bytes[index + 1]
            // Start of scan: the rest is image data, copied as it is.
            if marker == 0xDA {
                out.append(contentsOf: bytes[index...])
                return Data(out)
            }
            let length = Int(bytes[index + 2]) << 8 | Int(bytes[index + 3])
            guard length >= 2, index + 2 + length <= bytes.count else { return nil }
            if marker != 0xE1 {
                out.append(contentsOf: bytes[index..<(index + 2 + length)])
            }
            index += 2 + length
        }
        return nil
    }

    /// Whether `jpeg` carries an APP1 segment — the test seam for the
    /// privacy guarantee.
    nonisolated static func hasAPP1(_ jpeg: Data) -> Bool {
        let bytes = [UInt8](jpeg)
        var index = 2
        while index + 4 <= bytes.count, bytes[index] == 0xFF {
            let marker = bytes[index + 1]
            if marker == 0xDA { return false }
            if marker == 0xE1 { return true }
            index += 2 + (Int(bytes[index + 2]) << 8 | Int(bytes[index + 3]))
        }
        return false
    }
}
