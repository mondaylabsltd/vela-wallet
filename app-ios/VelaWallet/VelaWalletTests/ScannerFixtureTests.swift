//
//  ScannerFixtureTests.swift
//  VelaWalletTests
//
//  PR 3 note 7 — PRIVACY. In a gallery, fixture, board or dev-flow session the
//  scanner draws a fixture frame and NEVER opens the camera. A desktop gallery
//  sweep started the real one and captured a frame of the person at the
//  machine; the same must be impossible here.
//
//  The gate is `CameraScanner.fixtureOnly`, decided from the session's own
//  signal (`PageOverride.isDevSession`: any `VELA_PAGE` override) and checked
//  at the one place the camera is asked for and the one place it is started.
//  These tests hold that a fixture scanner never gets past it — no device
//  discovery, no permission prompt, no session made, nothing running —
//  whichever of its doors is knocked on; that the signal covers every page
//  override there is; and that no other file in the app starts a capture
//  session at all.
//
//  Nothing here opens a camera: the one scanner that is let past the gate is
//  let past it on a simulator only (`ScanPathTests.noCameraIsItsOwnAnswer`).
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ScannerFixtureTests {

    /// Every page override is a dev session — the galleries, the boards and
    /// the "-live" dev flows alike — and no override is not one.
    @Test func everyPageOverrideIsADevSession() {
        let pages = [
            "wallet", "gallery", "contacts", "contacts-live", "contacts-gallery", "settings-live",
            "flows-gallery", "settings", "settings-gallery", "explore", "signing", "handoff",
            "pr2", "pr3", "pr3b",
            // A page name this build has never heard of still counts: the
            // raw variable is what is read.
            "some-future-gallery",
        ]
        for page in pages {
            #expect(PageOverride.isDevSession(["VELA_PAGE": page]), "\(page) may open a real camera")
        }
        // The onboarding gallery has its own switch.
        #expect(PageOverride.isDevSession(["VELA_GALLERY": "1"]))
        #expect(!PageOverride.isDevSession(["VELA_GALLERY": "0"]))
        #expect(!PageOverride.isDevSession([:]))
        #expect(!PageOverride.isDevSession(["VELA_PAGE": ""]))
        #expect(!PageOverride.isDevSession(["VELA_STATE": "s1", "VELA_THEME": "dark"]))
    }

    /// A fixture scanner never reaches for the hardware — from `start`, from
    /// a second `start`, from the lens flip (which reconfigures), the torch
    /// or `stop` — and never even makes a capture session. It is not a
    /// refusal either: the surface draws its fixture frame, and "no camera"
    /// under it would be a sentence about a camera nobody asked for.
    @Test func aFixtureScannerNeverReachesTheCamera() async {
        let camera = CameraScanner(fixtureOnly: true)
        var scanned: [String] = []
        camera.onScan = { scanned.append($0) }

        await camera.start()
        await camera.start()
        camera.flip()
        camera.toggleTorch()
        // The flip's reconfigure runs in a task of its own: let it.
        for _ in 0..<5 { await Task.yield() }
        try? await Task.sleep(nanoseconds: 50_000_000)
        camera.stop()

        #expect(camera.hardwareReaches == 0, "a fixture scanner got past the gate \(camera.hardwareReaches)×")
        #expect(!camera.sessionMade, "a capture session was made in a fixture session")
        #expect(!camera.running)
        #expect(camera.refusal == nil, "the fixture frame is not a refusal")
        #expect(!camera.torchOn && !camera.canFlip && !camera.hasTorch)
        #expect(scanned.isEmpty)
    }

    /// The default is the session's own signal — and the unit-test host is
    /// not a page override, so the scanner the app graph makes there is the
    /// real one (which, on a simulator, has no camera to find).
    @Test func theScannerTakesTheSessionsSignalByDefault() {
        #expect(CameraScanner().fixtureOnly == PageOverride.isDevSession())
        #expect(CameraScanner(fixtureOnly: true).fixtureOnly)
        #expect(!CameraScanner(fixtureOnly: false).fixtureOnly)
    }

    /// A drawing with no live inputs is a gallery or a screenshot sweep: the
    /// fixture frame. Live inputs say which they are.
    @Test func theSurfaceDrawsTheFixtureFrameWhereThereIsNoCamera() {
        #expect(!ScanInputs().fixture, "live inputs are a real scanner unless they say otherwise")
        var inputs = ScanInputs()
        inputs.fixture = CameraScanner(fixtureOnly: true).fixtureOnly
        #expect(inputs.fixture && inputs.session == nil)
    }

    /// The camera is started in ONE file, behind that gate: no other source
    /// in the app starts a capture session, asks for the camera, or builds a
    /// device input — so there is no second door to gate.
    @Test func noOtherFileStartsACaptureSession() throws {
        let app = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet")
        let files = try #require(FileManager.default.enumerator(at: app, includingPropertiesForKeys: nil))
        let calls = ["startRunning(", "requestAccess(for", "AVCaptureDeviceInput(", "DiscoverySession("]
        var scanned = 0
        var gate = ""
        var offenders: [String] = []
        for case let url as URL in files where url.pathExtension == "swift" {
            scanned += 1
            let source = try String(contentsOf: url, encoding: .utf8)
            if url.lastPathComponent == "CameraScanner.swift" {
                gate = source
                continue
            }
            for (number, line) in source.components(separatedBy: "\n").enumerated() {
                let code = line.components(separatedBy: "//").first ?? line
                if calls.contains(where: code.contains) {
                    offenders.append("\(url.lastPathComponent):\(number + 1)")
                }
            }
        }
        #expect(scanned > 200, "the scan found no sources (\(scanned))")
        #expect(offenders.isEmpty, "the camera is reached outside CameraScanner: \(offenders)")

        // And in that file, both ways in begin at the gate: the function
        // that asks for the camera, and the one that configures and starts it.
        for entry in ["func start() async {", "private func configure() async {"] {
            let body = try #require(gate.components(separatedBy: entry).dropFirst().first, "no `\(entry)`")
            let firstStatement = body.components(separatedBy: "\n")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .first { !$0.isEmpty && !$0.hasPrefix("//") }
            #expect(firstStatement == "guard !fixtureOnly else { return }",
                    "`\(entry)` does not begin at the gate: \(firstStatement ?? "")")
        }
        #expect(gate.components(separatedBy: "startRunning(").count == 2, "the session is started in more than one place")
    }
}
