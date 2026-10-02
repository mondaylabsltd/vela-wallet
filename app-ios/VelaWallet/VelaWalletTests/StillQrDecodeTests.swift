//
//  StillQrDecodeTests.swift
//  VelaWalletTests
//
//  Spec 090: Vela reads its own receive code from a picture.
//

import CoreGraphics
import Foundation
import ImageIO
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct StillQrDecodeTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// The receive code's value, from the real core: the bare address, or with
    /// "include network" on, the URI for Polygon.
    private func coreCode(includeNetwork: Bool) throws -> String {
        let core = PaymentRequestCore()
        var view: [String: Any] = [:]
        for event: [String: Any] in [
            ["type": "start", "account": me, "recipient": me, "base_url": "https://getvela.app/pay"],
            ["type": "asset_picked", "chain_id": 137, "token_address": NSNull(),
             "symbol": "POL", "decimals": 18, "network_name": "Polygon"],
            ["type": "include_network_changed", "include": includeNetwork],
        ] {
            let result = try core.dispatch(eventJson: CoreJSON.string(event))
            view = try CoreJSON.object(result)["view"] as? [String: Any] ?? [:]
        }
        return try CoreJSON.decode(PaymentRequestViewWire.self, from: view).qrValue
    }

    private func fixture(_ name: String) throws -> CGImage {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("Fixtures/qr/\(name)")
        let source = try #require(CGImageSourceCreateWithURL(url as CFURL, nil), "\(name) is checked in")
        return try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
    }

    /// Vela's own receive code, cut from a 1080×2400 Android screenshot
    /// (modules ~24 px across), read from the picture as it is.
    ///
    /// No ladder here, and none needed: the same picture defeats ZXing and
    /// jsQR at full size (their local binarizer windows sit inside one
    /// module), but CoreImage's `CIDetector` reads both codes — and the full
    /// Android and iPhone screenshots — as they are. If this ever fails, climb
    /// the core's `stillQrSizes` as Android does.
    @Test func velaReadsItsOwnReceiveCodeFromAScreenshot() throws {
        for (name, includeNetwork) in [("receive-code-on.png", true), ("receive-code-off.png", false)] {
            let expected = try coreCode(includeNetwork: includeNetwork)
            #expect(QrDecoder.decode(image: try fixture(name)) == expected, "\(name)")
        }
    }
}
