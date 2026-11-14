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

    /// A small code in a small picture — modules 1.5–1.9 px across, under what
    /// `CIDetector` resolves — reads through the doubling retry. The two
    /// pictures are rust/qr-lab's (corpus seed 1, `tiny-modules-013` and
    /// `-021`): exactly the kind `CIDetector` alone missed there (tiny modules
    /// 19/30 as they are, 26/30 with the retry).
    @Test func aCodeWithModulesUnderTwoPixelsReads() throws {
        let cases = [
            ("tiny-modules-013.png",
             "wc:389f23a6a15ebba29fda2df9895a984c066a968d2ec1c0161f021f2f2cf3260a@2?relay-protocol=irn"
                + "&symKey=22701bebc718bd1274202321c0e0d1e302c001e65fd27dc4afdbff326694626f&expiryTimestamp=1794333277"),
            ("tiny-modules-021.png", "0x5Df56f803bBee3284AbD882E53dE704e7539c72E"),
        ]
        for (name, expected) in cases {
            #expect(QrDecoder.decode(image: try fixture(name)) == expected, "\(name)")
        }
    }
}
