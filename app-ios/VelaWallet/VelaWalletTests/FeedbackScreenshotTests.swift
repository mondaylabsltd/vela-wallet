//
//  FeedbackScreenshotTests.swift
//  VelaWalletTests
//
//  Feedback with screenshots (2026-09-26). Screenshots are PUBLIC, so what
//  leaves the phone must be only re-encoded pixels: a JPEG with no EXIF APP1
//  segment (no location, no camera), at most 1920 px on its longest edge,
//  under the endpoint's 2,000,000 bytes — in tile order, at most five. The
//  network is ALWAYS stubbed.
//

import Foundation
import ImageIO
import Testing
import UIKit
import UniformTypeIdentifiers
@testable import VelaWallet

/// `timeLimit`: some waits here are for a task the test itself started to
/// reach a point (`Waits.swift`); one that never does is a hang, reported here.
@MainActor
@Suite(.timeLimit(.minutes(5)))
struct FeedbackScreenshotTests {
    private let stubURL = "https://example.test/api/bug-report"

    // MARK: - Test images

    /// A drawn "screenshot": stripes, so JPEG has something to compress.
    private func drawn(_ width: Int, _ height: Int, alpha: Bool = false) -> UIImage {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = !alpha
        return UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format).image { context in
            for stripe in 0..<20 {
                UIColor(hue: CGFloat(stripe) / 20, saturation: 0.6, brightness: 0.9, alpha: alpha ? 0.5 : 1).setFill()
                context.fill(CGRect(x: 0, y: CGFloat(stripe * height / 20), width: CGFloat(width), height: CGFloat(height / 20)))
            }
        }
    }

    private func png(_ width: Int, _ height: Int, alpha: Bool = false) -> Data {
        drawn(width, height, alpha: alpha).pngData()!
    }

    /// A camera-style JPEG carrying EXIF — a location and a camera — and an
    /// orientation tag, written by ImageIO the way a phone photo is.
    private func photoWithLocation(width: Int, height: Int, orientation: Int = 1) -> Data {
        let out = NSMutableData()
        let destination = CGImageDestinationCreateWithData(out, UTType.jpeg.identifier as CFString, 1, nil)!
        let properties: [CFString: Any] = [
            kCGImagePropertyOrientation: orientation,
            kCGImagePropertyExifDictionary: [
                kCGImagePropertyExifUserComment: "secret camera note",
                kCGImagePropertyExifLensModel: "Test Lens 26mm",
            ],
            kCGImagePropertyGPSDictionary: [
                kCGImagePropertyGPSLatitude: 31.2304, kCGImagePropertyGPSLatitudeRef: "N",
                kCGImagePropertyGPSLongitude: 121.4737, kCGImagePropertyGPSLongitudeRef: "E",
            ],
        ]
        CGImageDestinationAddImage(destination, drawn(width, height).cgImage!, properties as CFDictionary)
        CGImageDestinationFinalize(destination)
        return out as Data
    }

    /// Random noise: the worst case for JPEG, which is what walks the ladder.
    private func noise(_ side: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: side * side * 4)
        var seed: UInt32 = 0x9E37_79B9
        for index in stride(from: 0, to: bytes.count, by: 4) {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5
            bytes[index] = UInt8(truncatingIfNeeded: seed)
            bytes[index + 1] = UInt8(truncatingIfNeeded: seed >> 8)
            bytes[index + 2] = UInt8(truncatingIfNeeded: seed >> 16)
            bytes[index + 3] = 255
        }
        let provider = CGDataProvider(data: Data(bytes) as CFData)!
        let image = CGImage(
            width: side, height: side, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: side * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent
        )!
        return UIImage(cgImage: image).pngData()!
    }

    /// What a JPEG actually is: its pixel size and whether it carries any
    /// EXIF or GPS dictionary once decoded.
    private func inspect(_ jpeg: Data) -> (width: Int, height: Int, exif: Bool, gps: Bool) {
        let source = CGImageSourceCreateWithData(jpeg as CFData, nil)!
        let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] ?? [:]
        let exif = properties[kCGImagePropertyExifDictionary] as? [CFString: Any] ?? [:]
        return (
            properties[kCGImagePropertyPixelWidth] as? Int ?? 0,
            properties[kCGImagePropertyPixelHeight] as? Int ?? 0,
            exif[kCGImagePropertyExifUserComment] != nil || exif[kCGImagePropertyExifLensModel] != nil,
            properties[kCGImagePropertyGPSDictionary] != nil
        )
    }

    private func isJPEG(_ data: Data) -> Bool { data.prefix(3) == Data([0xFF, 0xD8, 0xFF]) }

    // MARK: - Preparing a screenshot

    @Test func aLargeScreenshotIsScaledToTheLongestEdgeAsJPEGWithNoAPP1() throws {
        let prepared = try #require(ScreenshotPrep.prepare(png(3000, 2000)))
        #expect(isJPEG(prepared.jpeg))
        #expect(!ScreenshotPrep.hasAPP1(prepared.jpeg))
        let seen = inspect(prepared.jpeg)
        #expect(max(seen.width, seen.height) == 1920)
        #expect(seen.width == 1920 && seen.height == 1280, "aspect kept: \(seen)")
        #expect(prepared.jpeg.count <= ScreenshotPrep.maxBytes)
    }

    /// Re-encoding is the privacy guarantee: a photo's location and camera
    /// never ship, even when the image is small enough to send as it was.
    @Test func aPhotosLocationAndCameraAreGone() throws {
        let photo = photoWithLocation(width: 800, height: 600)
        #expect(ScreenshotPrep.hasAPP1(photo), "the test photo must carry EXIF to prove anything")
        #expect(inspect(photo).gps)
        let prepared = try #require(ScreenshotPrep.prepare(photo))
        #expect(!ScreenshotPrep.hasAPP1(prepared.jpeg))
        let seen = inspect(prepared.jpeg)
        #expect(!seen.gps, "a location survived")
        #expect(!seen.exif, "the camera's EXIF survived")
        #expect(!String(decoding: prepared.jpeg, as: UTF8.self).contains("secret camera note"))
    }

    /// Drawn upright: a portrait photo stored sideways is sent the way it was seen.
    @Test func theOrientationIsAppliedNotCarried() throws {
        // Stored 400×300, tagged "rotate 90° CW" — seen as 300×400.
        let prepared = try #require(ScreenshotPrep.prepare(photoWithLocation(width: 400, height: 300, orientation: 6)))
        let seen = inspect(prepared.jpeg)
        #expect(seen.width == 300 && seen.height == 400, "\(seen)")
    }

    /// Never upscaled, and re-encoded even when small — a PNG is not sent as a PNG.
    @Test func aSmallPNGIsReencodedNotEnlarged() throws {
        let prepared = try #require(ScreenshotPrep.prepare(png(120, 80, alpha: true)))
        #expect(isJPEG(prepared.jpeg))
        #expect(!ScreenshotPrep.hasAPP1(prepared.jpeg))
        let seen = inspect(prepared.jpeg)
        #expect(seen.width == 120 && seen.height == 80)
    }

    /// The worst case walks the ladder (0.85 → 0.7 → a 1440 edge) until it fits.
    @Test func anImageOverTheCapWalksTheLadder() throws {
        let prepared = try #require(ScreenshotPrep.prepare(noise(2400)))
        #expect(prepared.jpeg.count <= ScreenshotPrep.maxBytes, "\(prepared.jpeg.count) bytes")
        #expect(max(inspect(prepared.jpeg).width, inspect(prepared.jpeg).height) <= 1920)
        #expect(!ScreenshotPrep.hasAPP1(prepared.jpeg))
    }

    @Test func somethingThatIsNotAnImageIsRefused() {
        #expect(ScreenshotPrep.prepare(Data("%PDF-1.7 not an image".utf8)) == nil)
        #expect(ScreenshotPrep.prepare(Data()) == nil)
    }

    // MARK: - The payload

    @Test func aTextOnlyReportHasNoScreenshotsField() throws {
        let report = BugReport.build(what: "x", steps: "", environmentLines: [], version: "1")
        let object = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(report)) as? [String: Any])
        #expect(object["screenshots"] == nil)
        #expect(object.keys.sorted() == [
            "appVersion", "area", "client", "environment", "fingerprint", "os", "steps", "what",
        ])
    }

    /// Base64 that decodes to the prepared JPEGs — no APP1, ≤ 1920 — in tile
    /// order, and never more than five.
    @Test func screenshotsRideAsPlainBase64InTileOrder() throws {
        let images = try (0..<6).map { index in
            try #require(ScreenshotPrep.prepare(png(2400 + index * 100, 1000))).jpeg
        }
        let report = BugReport.build(what: "x", steps: "", environmentLines: [], version: "1", screenshots: images)
        let sent = try #require(report.screenshots)
        #expect(sent.count == 5, "at most five")
        for (index, text) in sent.enumerated() {
            #expect(!text.hasPrefix("data:"))
            #expect(!text.contains("\n"))
            let bytes = try #require(Data(base64Encoded: text), "not standard padded base64")
            #expect(bytes == images[index], "out of tile order at \(index)")
            #expect(isJPEG(bytes))
            #expect(!ScreenshotPrep.hasAPP1(bytes))
            let seen = inspect(bytes)
            #expect(max(seen.width, seen.height) <= 1920)
        }
    }

    // MARK: - The tray

    @Test func pickingPastFiveTakesTheFirstAndSaysSo() async {
        let sender = FeedbackSender(endpoint: stubURL, transport: { _ in throw URLError(.cancelled) })
        await sender.add(datas: (0..<3).map { _ in png(200, 100) })
        #expect(sender.shots.count == 3)
        #expect(sender.notice == nil)
        await sender.add(datas: (0..<4).map { _ in png(200, 100) })
        #expect(sender.shots.count == 5, "the first (5 − n) are taken")
        #expect(sender.notice == .limit)
        #expect(sender.room == 0)
        #expect(sender.shots.allSatisfy { $0.prepared != nil })
    }

    @Test func aFileThatIsNotAnImageIsRefusedWithItsOwnLine() async {
        let sender = FeedbackSender(endpoint: stubURL, transport: { _ in throw URLError(.cancelled) })
        await sender.add(datas: [png(200, 100), Data("not an image".utf8), nil])
        #expect(sender.shots.count == 1)
        #expect(sender.notice == .unsupported)
    }

    /// Removing re-indexes what is left, keeps its order, and clears a notice.
    @Test func removingKeepsTheOrderOfTheRest() async throws {
        let sender = FeedbackSender(endpoint: stubURL, transport: { _ in throw URLError(.cancelled) })
        await sender.add(datas: [png(100, 100), png(200, 100), png(300, 100), png(400, 100), png(500, 100), png(600, 100)])
        #expect(sender.notice == .limit)
        let widths = sender.shots.map { Int($0.prepared?.pixelSize.width ?? 0) }
        #expect(widths == [100, 200, 300, 400, 500])
        sender.remove(sender.shots[1].id)
        #expect(sender.shots.map { Int($0.prepared?.pixelSize.width ?? 0) } == [100, 300, 400, 500])
        #expect(sender.notice == nil)
        #expect(sender.room == 1)
    }

    // MARK: - Outcomes

    @Test func aFiledReportWithDroppedScreenshotsSaysSo() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":9,"url":"https://github.com/x/y/issues/9","deduped":false,"screenshots":2,"screenshotsDropped":1}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.add(datas: [png(2400, 1200), png(800, 600)])
        await sender.send(what: "Froze", steps: "", previewLines: ["App version: v1"], version: "1")
        #expect(sender.state == .filed(number: 9, url: "https://github.com/x/y/issues/9", deduped: false, screenshotsDropped: 1))
        // The request carried both, as JSON, with the 30 s budget.
        let request = try #require(stub.requests.first)
        #expect(request.timeoutInterval == BugReport.screenshotTimeout)
        let sent = try JSONDecoder().decode(BugReport.Payload.self, from: try #require(request.httpBody))
        #expect(sent.screenshots?.count == 2)
        #expect(sent.screenshots?.first.flatMap { Data(base64Encoded: $0) } == sender.shots.first?.prepared?.jpeg)
    }

    @Test(arguments: [415, 413, 400, 503])
    func aRefusedReportWithScreenshotsFallsBackAndSaysTheyStayBehind(status: Int) async {
        let stub = StubEndpoint()
        stub.answer = .status(status, #"{"error":"unsupported_screenshot"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.add(datas: [png(800, 600), png(600, 800)])
        await sender.send(what: "Froze", steps: "", previewLines: [], version: "1")
        guard case .fallback(let url, let carried) = sender.state else {
            Issue.record("\(status) did not fall back: \(sender.state)")
            return
        }
        #expect(carried == 2, "the fallback line needs to know screenshots were attached")
        #expect(url.contains("what=Froze"))
        #expect(!url.contains("screenshots"), "images never ride in the URL")
    }

    @Test func textOnlyKeepsTheTenSecondBudgetAndNoField() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":1,"url":"https://github.com/x/y/issues/1"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.send(what: "Froze", steps: "", previewLines: [], version: "1")
        let request = try #require(stub.requests.first)
        #expect(request.timeoutInterval == BugReport.timeout)
        let body = try #require(request.httpBody)
        let object = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(object["screenshots"] == nil)
    }

    /// Busy while the endpoint is out; screenshots never block Send.
    @Test func theSenderIsBusyWhileTheEndpointIsOut() async {
        var release: CheckedContinuation<Void, Never>?
        let sender = FeedbackSender(endpoint: stubURL, transport: { request in
            await withCheckedContinuation { release = $0 }
            return (Data(#"{"number":3,"url":"https://github.com/x/y/issues/3"}"#.utf8),
                    HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
        })
        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: [], version: "1") }
        while release == nil { await Task.yield() }
        #expect(sender.sending)
        release?.resume()
        await sending.value
        #expect(!sender.sending)
        #expect(sender.state == .filed(number: 3, url: "https://github.com/x/y/issues/3", deduped: false, screenshotsDropped: 0))
    }

    /// v2 A4: each preview row is a label and a value, split at the FIRST ": ".
    @Test func aPreviewRowSplitsAtItsFirstColon() {
        #expect(FeedbackSheetBody.split("App version: v0.9.4 (build 1)") == ("App version", "v0.9.4 (build 1)"))
        #expect(FeedbackSheetBody.split("Unreachable RPCs: a: b") == ("Unreachable RPCs", "a: b"))
        #expect(FeedbackSheetBody.split("no colon here") == (nil, "no colon here"))
    }

    /// v2 A1: a refusal and the public warning are separate facts — a refusal
    /// never takes the place of the attached images.
    @Test func aRefusalLeavesTheAttachedImagesInPlace() async {
        let sender = FeedbackSender(endpoint: stubURL, transport: { _ in throw URLError(.cancelled) })
        await sender.add(datas: [png(200, 100), Data("x".utf8)])
        #expect(sender.notice == .unsupported)
        #expect(sender.shots.count == 1, "the warning line needs the images still there")
    }

    /// Send pressed while a tile is still being prepared waits for it — busy
    /// the whole time — and sends it; it is never silently left behind.
    @Test func sendWaitsForATileStillBeingPrepared() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":5,"url":"https://github.com/x/y/issues/5"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.add(datas: [png(300, 200)])
        var release: CheckedContinuation<Void, Never>?
        let slow = png(1200, 800)
        sender.attach([{ await withCheckedContinuation { release = $0 }; return slow }])
        while release == nil { await Task.yield() }
        #expect(sender.shots.count == 2 && sender.shots[1].prepared == nil, "the second tile is processing")

        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: [], version: "1") }
        // Send has started — the state it sets, not 20 yields (`Waits.swift`).
        await Wait.until { sender.sending }
        #expect(sender.sending, "busy while the tile finishes")
        #expect(stub.requests.isEmpty, "nothing leaves before every tile is ready")

        release?.resume()
        await sending.value
        let request = try #require(stub.requests.first)
        let sent = try JSONDecoder().decode(BugReport.Payload.self, from: try #require(request.httpBody))
        #expect(sent.screenshots?.count == 2, "the late tile rode along")
        #expect(sent.screenshots?.last.flatMap { Data(base64Encoded: $0) } == sender.shots.last?.prepared?.jpeg)
        #expect(sender.state == .filed(number: 5, url: "https://github.com/x/y/issues/5", deduped: false, screenshotsDropped: 0))
    }

    /// A tile that fails while Send waits becomes the unsupported line and is
    /// dropped; the rest are sent.
    @Test func aTileThatFailsWhileSendWaitsIsDroppedAndSaid() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":6,"url":"https://github.com/x/y/issues/6"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.add(datas: [png(300, 200)])
        var release: CheckedContinuation<Void, Never>?
        sender.attach([{ await withCheckedContinuation { release = $0 }; return Data("not an image".utf8) }])
        while release == nil { await Task.yield() }
        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: [], version: "1") }
        await Wait.until { sender.sending }
        #expect(stub.requests.isEmpty)
        release?.resume()
        await sending.value
        #expect(sender.notice == .unsupported)
        #expect(sender.shots.count == 1)
        let request = try #require(stub.requests.first)
        let sent = try JSONDecoder().decode(BugReport.Payload.self, from: try #require(request.httpBody))
        #expect(sent.screenshots?.count == 1)
    }

    /// v3 B9: nothing is added or removed while the report is on its way.
    @Test func theTrayIsInertWhileSending() async {
        var release: CheckedContinuation<Void, Never>?
        let sender = FeedbackSender(endpoint: stubURL, transport: { request in
            await withCheckedContinuation { release = $0 }
            return (Data(#"{"number":2,"url":"https://github.com/x/y/issues/2"}"#.utf8),
                    HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
        })
        await sender.add(datas: [png(200, 100)])
        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: [], version: "1") }
        while release == nil { await Task.yield() }
        sender.remove(sender.shots[0].id)
        await sender.add(datas: [png(300, 100)])
        #expect(sender.shots.count == 1, "the tray did not move while sending")
        release?.resume()
        await sending.value
    }

    // MARK: - Copy

    @Test func theSheetsNewWordsAreFilled() {
        let loc = Loc(overrideTag: "en")
        let model = SettingsFixtures.build(.st15, loc: loc).feedback
        #expect(model.screenshotsHint == "Optional · up to 5")
        #expect(model.screenshotsLimit.contains("5"))
        #expect(model.removeScreenshot.contains("{{index}}"))
        #expect(!model.screenshotsPublic.isEmpty && !model.fallbackScreenshots.isEmpty)
        #expect(!model.screenshotsDropped.isEmpty && !model.done.isEmpty)
        #expect(!model.consent.localizedCaseInsensitiveContains("seed phrase"))
    }
}
