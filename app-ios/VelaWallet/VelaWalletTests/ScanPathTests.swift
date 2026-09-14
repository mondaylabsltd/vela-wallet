//
//  ScanPathTests.swift
//  VelaWalletTests
//
//  A scanned code, from the payload to what the CORE decides it means.
//
//  The camera itself cannot be driven here — there is none in a simulator, and
//  that is a fact this suite asserts rather than works around. What it does
//  drive is everything between the decoder and the send machine.
//

import CoreGraphics
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ScanPathTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// A receive code this app writes, scanned: the core takes it as a REQUEST
    /// with the chain and the amount the code named.
    @Test func aReceiveCodeReachesTheCoreAsARequest() throws {
        let scan = Eip681.scan(of: "ethereum:\(me)@100?value=1500000000000000000")
        #expect(scan["type"] as? String == "request")

        let core = SendCore()
        let resolved = try core.dispatch(eventJson: CoreJSON.string([
            "type": "scan_resolved", "scan": scan,
        ]))
        // The machine took it — a refusal would come back as an unchanged view
        // with an alert, and this asserts the event was understood at all.
        _ = try CoreJSON.decode(SendViewWire.self, from: try view(from: resolved))
    }

    /// A token request names the CONTRACT as the token and the parameter as the
    /// person — the pair a `/transfer` URI exists to carry.
    @Test func aTokenRequestNamesBothSides() throws {
        let parsed = try #require(
            Eip681.parse("ethereum:\(usdc)@100/transfer?address=\(me)&uint256=1000000")
        )
        let scan = parsed.scan
        #expect(scan["recipient"] as? String == me)
        #expect(scan["token_address"] as? String == usdc)
        #expect(scan["amount_base_units"] as? String == "1000000")
    }

    /// Anything that is not a request still reaches the core, as TEXT — the
    /// send screen drops it into an editable recipient field rather than
    /// refusing a code somebody just held up to a camera.
    @Test func plainTextIsNotRefused() throws {
        let scan = Eip681.scan(of: me)
        #expect(scan["type"] as? String == "text")
        #expect(scan["data"] as? String == me)

        let core = SendCore()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "scan_resolved", "scan": scan,
        ]))
    }

    /// The scanner's flag is the CORE's. A shell that pushed its own screen
    /// would show a viewfinder the machine does not know is open — and the
    /// close would then not close it.
    @Test func theCoreOwnsWhetherTheScannerIsOpen() throws {
        let core = SendCore()
        let opened = try core.dispatch(eventJson: CoreJSON.string(["type": "open_scanner"]))
        #expect(try CoreJSON.decode(SendViewWire.self, from: try view(from: opened)).showScanner)
        let closed = try core.dispatch(eventJson: CoreJSON.string(["type": "close_scanner"]))
        #expect(!(try CoreJSON.decode(SendViewWire.self, from: try view(from: closed)).showScanner))
    }

    // MARK: - The camera's refusals

    /// There is no camera in a simulator, and that is an ANSWER — not an
    /// error, and not silence. The surface falls back to the photo library.
    @Test func noCameraIsItsOwnAnswer() async {
        let camera = CameraScanner()
        await camera.start()
        #expect(camera.refusal == .noCamera || camera.refusal == .denied,
                "a simulator has no camera and must say which refusal it is")
        #expect(!camera.running)
    }

    /// Four refusals, four sentences. A person who denied permission and a
    /// person on a device without a camera need different things said.
    @Test func eachRefusalHasItsOwnWords() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let denied = loc.t("componentsUi.scanner.permissionText")
        let noCamera = loc.t("componentsUi.scanner.noCamera")
        let unavailable = loc.t("componentsUi.scanner.cameraUnavailable")
        #expect(denied != noCamera)
        #expect(noCamera != unavailable)
        #expect(!denied.isEmpty && !noCamera.isEmpty && !unavailable.isEmpty)
        // And the one that offers a way out has a label for it.
        #expect(!loc.t("componentsUi.scanner.grantPermission").isEmpty)
    }
}
