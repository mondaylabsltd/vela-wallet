//
//  QrDecoder.swift
//  VelaWallet
//
//  ONE decoder, for camera frames and for photos (spec 055 D1).
//
//  **CoreImage's `CIDetector`, not Vision.** D1 originally chose
//  `VNDetectBarcodesRequest` because it reads both a sample buffer and a still
//  image. It does — on a device. In the simulator it finds nothing at all: a
//  code this app itself rendered, large and square and with a four-module quiet
//  zone, came back with zero observations from Vision and was read correctly by
//  `CIDetector` in the same test. A decoder that cannot be exercised in the
//  hermetic suite is a decoder nobody can prove works, so this is the one.
//
//  `CIDetector` satisfies the same rule that drove D1: it reads a `CIImage`,
//  and a `CIImage` comes from a camera's pixel buffer and from a photo alike.
//  `AVCaptureMetadataOutput` would have meant a SECOND decoder for the library,
//  and two decoders eventually disagree about what a code says.
//

import CoreImage
import CoreMedia
import Foundation
#if canImport(UIKit)
import UIKit
#endif

enum QrDecoder {

    /// One detector for the life of the process.
    ///
    /// Building one costs real time (it compiles its own pipeline), and the
    /// scanner asks on every frame — thirty times a second, on a phone.
    private static let detector: CIDetector? = CIDetector(
        ofType: CIDetectorTypeQRCode,
        context: CIContext(options: [.useSoftwareRenderer: false]),
        options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]
    )

    /// The payload of the first QR code in a camera frame, or `nil`.
    ///
    /// `orientation` is the EXIF value for how the buffer is rotated relative
    /// to the person holding the phone — a portrait capture from the back
    /// camera arrives on its side, and a sideways code reads as no code.
    static func decode(sampleBuffer: CMSampleBuffer, orientation: Int32) -> String? {
        guard let buffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return nil }
        let image = CIImage(cvPixelBuffer: buffer).oriented(forExifOrientation: orientation)
        return payload(in: image)
    }

    /// The payload of the first QR code in a still image, or `nil`.
    static func decode(image: CGImage) -> String? {
        payload(in: CIImage(cgImage: image))
    }

    #if canImport(UIKit)
    /// A picked photo. `UIImage.cgImage` is `nil` for a CIImage-backed image,
    /// which is what a filtered photo can be — so that case reads the `CIImage`
    /// directly rather than refusing.
    static func decode(image: UIImage) -> String? {
        if let ciImage = image.ciImage { return payload(in: ciImage) }
        guard let cgImage = image.cgImage else { return nil }
        // A photo carries its rotation in metadata; the bitmap underneath is
        // whatever the sensor produced.
        return payload(in: CIImage(cgImage: cgImage)
            .oriented(forExifOrientation: exif(for: image.imageOrientation)))
    }

    private static func exif(for orientation: UIImage.Orientation) -> Int32 {
        switch orientation {
        case .up: 1
        case .upMirrored: 2
        case .down: 3
        case .downMirrored: 4
        case .leftMirrored: 5
        case .right: 6
        case .rightMirrored: 7
        case .left: 8
        @unknown default: 1
        }
    }
    #endif

    private static func payload(in image: CIImage) -> String? {
        guard let features = detector?.features(in: image) as? [CIQRCodeFeature] else { return nil }
        for feature in features {
            if let message = feature.messageString, !message.isEmpty { return message }
        }
        return nil
    }
}
