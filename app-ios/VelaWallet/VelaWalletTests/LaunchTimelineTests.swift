//
//  LaunchTimelineTests.swift
//  VelaWalletTests
//
//  The launch animation is SwiftUI drawing the shared scene (`LaunchScene`)
//  along `LaunchAnimation.frame(at:)`. Two things can go wrong without any
//  other test noticing: the timeline drifts from the one every other platform
//  plays, or the drawing gets the colours or the fill rule wrong. This holds
//  the first to the scene's own reference samples, and the second to pixels.
//

import CoreGraphics
import Foundation
import SwiftUI
import Testing
@testable import VelaWallet

@MainActor
struct LaunchTimelineTests {

    private static let vectors = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("docs/design/onboarding/launch/launch-timeline.vectors.json")

    private struct Vectors: Decodable {
        let easing: [Double]
        let cases: [Case]
        struct Case: Decodable {
            let ms: Double
            let mark: Point
            let letters: [Double]
        }
        struct Point: Decodable {
            let x: Double
            let y: Double
        }
    }

    // MARK: - Timeline

    /// Every sample, from the same file the web, Android and desktop are held
    /// to. The web's rendering of it was compared pixel for pixel with the
    /// Lottie files this replaced.
    @Test func theTimelineFollowsTheScenesVectors() throws {
        let vectors = try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: Self.vectors))
        let (x1, y1, x2, y2) = LaunchScene.easing
        #expect(vectors.easing == [x1, y1, x2, y2])
        #expect(vectors.cases.count > 10, "the vectors file sampled (almost) nothing")
        for vector in vectors.cases {
            let frame = LaunchAnimation.frame(at: vector.ms)
            #expect(abs(frame.mark.x - vector.mark.x) < 0.05, "\(vector.ms) ms: mark x \(frame.mark.x), want \(vector.mark.x)")
            #expect(abs(frame.mark.y - vector.mark.y) < 0.05, "\(vector.ms) ms: mark y \(frame.mark.y), want \(vector.mark.y)")
            try #require(frame.letters.count == vector.letters.count)
            for (index, (got, want)) in zip(frame.letters, vector.letters).enumerated() {
                #expect(abs(got - want) < 0.005, "\(vector.ms) ms: letter \(index) opacity \(got), want \(want)")
            }
        }
    }

    @Test func theEndsAreHeld() {
        let opening = LaunchAnimation.frame(at: -100)
        #expect(opening.mark == LaunchScene.markFrom)
        #expect(opening.letters.allSatisfy { $0 == 0 })

        let settled = LaunchAnimation.frame(at: LaunchScene.settledMs)
        #expect(settled == LaunchAnimation.finished)
        #expect(LaunchAnimation.finished.mark == LaunchScene.markTo)
        #expect(LaunchAnimation.finished.letters.allSatisfy { $0 == 1 })
        #expect(LaunchAnimation.frame(at: 10_000) == LaunchAnimation.finished)
    }

    // MARK: - Drawing

    @Test func eachAppearanceUsesItsOwnColours() {
        #expect(LaunchAnimation.colors(.dark) == LaunchScene.dark)
        #expect(LaunchAnimation.colors(.light) == LaunchScene.light)
    }

    /// The finished lockup, rendered, sampled at points inside each shape.
    /// The "e" sample sits where its crossbar overlaps its bowl: filled under
    /// the nonzero rule the scene requires, a hole under even-odd.
    @Test(arguments: [LaunchAnimation.Appearance.dark, .light], [LaunchAnimation.FormFactor.phone, .largeScreen])
    func theFinishedLockupIsDrawnInTheScenesColours(
        appearance: LaunchAnimation.Appearance,
        formFactor: LaunchAnimation.FormFactor
    ) throws {
        let colors = LaunchAnimation.colors(appearance)
        let image = try Self.render(appearance: appearance, formFactor: formFactor, ms: LaunchScene.durationMs)
        let mark = LaunchScene.markTo
        let samples: [(String, CGPoint, UInt32?)] = [
            ("sunset", CGPoint(x: mark.x - 5, y: mark.y + 10), colors.sunset),
            ("peach", CGPoint(x: mark.x + 6, y: mark.y + 10), colors.peach),
            ("hull", CGPoint(x: mark.x, y: mark.y + 26), colors.hull),
            ("V stem", CGPoint(x: 108.0, y: 73.0), colors.wordmark),
            ("e crossbar", CGPoint(x: 135.0, y: 63.6), colors.wordmark),
            ("e crossbar over the bowl", CGPoint(x: 143.0, y: 64.6), colors.wordmark),
            ("e counter", CGPoint(x: 135.5, y: 59.0), nil),
            ("between mark and V", CGPoint(x: 88.0, y: 60.0), nil),
        ]
        for (name, point, want) in samples {
            let pixel = image.pixel(at: Self.toImage(point, formFactor: formFactor))
            if let want {
                #expect(pixel.a == 255, "\(appearance)/\(formFactor) \(name): alpha \(pixel.a)")
                #expect(pixel.close(to: want), "\(appearance)/\(formFactor) \(name): \(pixel) want \(String(want, radix: 16))")
            } else {
                #expect(pixel.a == 0, "\(appearance)/\(formFactor) \(name): expected empty, got \(pixel)")
            }
        }
    }

    @Test func theOpeningFrameIsTheMarkAloneAtTheCentre() throws {
        let image = try Self.render(appearance: .dark, formFactor: .phone, ms: 0)
        let colors = LaunchAnimation.colors(.dark)
        let centre = LaunchScene.markFrom
        #expect(image.pixel(at: Self.toImage(CGPoint(x: centre.x, y: centre.y + 26), formFactor: .phone)).close(to: colors.hull))
        // Where the "V" will be, nothing yet.
        #expect(image.pixel(at: Self.toImage(CGPoint(x: 108.0, y: 73.0), formFactor: .phone)).a == 0)
    }

    // MARK: - Rendering

    static let renderScale: CGFloat = 4

    static func toImage(_ point: CGPoint, formFactor: LaunchAnimation.FormFactor) -> CGPoint {
        var p = point
        if formFactor == .largeScreen {
            p = CGPoint(
                x: LaunchScene.largeOffset.x + p.x * LaunchScene.largeScale,
                y: LaunchScene.largeOffset.y + p.y * LaunchScene.largeScale
            )
        }
        return CGPoint(x: p.x * renderScale, y: p.y * renderScale)
    }

    static func render(
        appearance: LaunchAnimation.Appearance,
        formFactor: LaunchAnimation.FormFactor,
        ms: Double
    ) throws -> Bitmap {
        let canvas = LaunchAnimation.canvas(formFactor)
        let renderer = ImageRenderer(
            content: LaunchSceneCanvas(appearance: appearance, formFactor: formFactor, ms: ms)
                .frame(width: canvas.width, height: canvas.height)
        )
        renderer.scale = renderScale
        let image = try #require(renderer.cgImage)
        return try #require(Bitmap(image))
    }

    /// An image redrawn into 8-bit sRGB RGBA, so a sample means the same thing
    /// whatever format the renderer produced.
    struct Bitmap {
        let width: Int
        let height: Int
        let bytes: [UInt8]

        init?(_ image: CGImage) {
            let w = image.width, h = image.height
            var buffer = [UInt8](repeating: 0, count: w * h * 4)
            guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
            let drawn = buffer.withUnsafeMutableBytes { raw -> Bool in
                guard let context = CGContext(
                    data: raw.baseAddress, width: w, height: h,
                    bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
                ) else { return false }
                context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
                return true
            }
            guard drawn else { return nil }
            width = w
            height = h
            bytes = buffer
        }

        struct Pixel: CustomStringConvertible {
            let r: UInt8, g: UInt8, b: UInt8, a: UInt8
            var description: String { String(format: "#%02X%02X%02X a=%d", r, g, b, a) }
            func close(to rgb: UInt32, tolerance: Int = 2) -> Bool {
                abs(Int(r) - Int((rgb >> 16) & 0xFF)) <= tolerance
                    && abs(Int(g) - Int((rgb >> 8) & 0xFF)) <= tolerance
                    && abs(Int(b) - Int(rgb & 0xFF)) <= tolerance
            }
        }

        /// `point` in image pixels, origin top-left.
        func pixel(at point: CGPoint) -> Pixel {
            let x = min(max(Int(point.x), 0), width - 1)
            let y = min(max(Int(point.y), 0), height - 1)
            let i = (y * width + x) * 4
            return Pixel(r: bytes[i], g: bytes[i + 1], b: bytes[i + 2], a: bytes[i + 3])
        }
    }
}
