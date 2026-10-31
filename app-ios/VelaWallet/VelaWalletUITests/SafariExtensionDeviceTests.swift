//
//  SafariExtensionDeviceTests.swift
//  VelaWalletUITests
//
//  The Safari Web Extension, end to end on a simulator (native AUDIT §6.5):
//  Settings turns it on, Safari opens a dApp, and the dApp connects, signs and
//  sends through it — the extension being the web wallet's own code, packaged.
//
//  Simulator only, and opt-in: it changes Safari's settings and drives Safari
//  itself, so it runs when `VELA_SAFARI_E2E=1` reaches the runner
//  (`TEST_RUNNER_VELA_SAFARI_E2E=1 xcodebuild test …`). The wallet is the
//  development package's parallel space (fixed throwaway keys — the passkey is
//  the one thing faked), pointed at the fork harness
//  (`scripts/onchain/multisig-fork/up.sh`) by the package's harness doorway:
//
//    TEST_RUNNER_VELA_HARNESS_FORK=9845  TEST_RUNNER_VELA_HARNESS_RELAY=9890
//    TEST_RUNNER_VELA_HARNESS_GENESIS=9887 TEST_RUNNER_VELA_HARNESS_PEER=9892
//    TEST_RUNNER_VELA_SAFARI_SHOTS=<dir>   screenshots and Safari's page text
//
//  Each step is its own test, in order (`test01…`), so a run can stop at the
//  step a person wants to look at, and Safari's state carries from one to the
//  next exactly as it would for a person.
//

import XCTest

final class SafariExtensionDeviceTests: XCTestCase {

    private let env = ProcessInfo.processInfo.environment
    private let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
    private let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
    private static var server: LocalDappServer?
    static let dappPort: UInt16 = 9893

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipUnless(DeviceSafety.onSimulator, "drives Safari and its settings: simulator only")
        try XCTSkipUnless(env["VELA_SAFARI_E2E"] == "1", "set TEST_RUNNER_VELA_SAFARI_E2E=1 to drive Safari")
    }

    // MARK: - Evidence

    private var shots: URL? {
        env["VELA_SAFARI_SHOTS"].map { URL(fileURLWithPath: $0, isDirectory: true) }
    }

    /// A screenshot (attached, and written to `VELA_SAFARI_SHOTS`) plus the
    /// app's element tree beside it — what the step looked like, and why.
    func capture(_ name: String, _ app: XCUIApplication) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = shots else { return }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? shot.pngRepresentation.write(to: dir.appendingPathComponent("\(name).png"))
        try? app.debugDescription.write(
            to: dir.appendingPathComponent("\(name).tree.txt"), atomically: true, encoding: .utf8)
    }

    // MARK: - The dApp

    private func harnessPort(_ key: String, _ fallback: Int) -> Int {
        Int(env["VELA_HARNESS_\(key)"] ?? "") ?? fallback
    }

    private func dappHTML() throws -> String {
        let bundle = Bundle(for: SafariExtensionDeviceTests.self)
        let url = try XCTUnwrap(bundle.url(forResource: "safari-testdapp", withExtension: "html"))
        let fork = harnessPort("FORK", 9845)
        let seed = "dev-seed.html?genesis=http://127.0.0.1:\(harnessPort("GENESIS", 9887))"
            + "&relay=http://127.0.0.1:\(harnessPort("RELAY", 9890))"
            + "&peerconnect=ws://127.0.0.1:\(harnessPort("PEER", 9892))"
            + "&rpc=100=http://127.0.0.1:\(fork)"
            + "&rpc=8453=http://127.0.0.1:\(harnessPort("UNREAD", 9846))"
            + "&locale=\(env["VELA_SAFARI_LOCALE"] ?? "en")"
        // The trusted signing page under test, served on this machine's
        // loopback (`app-web/trusted-signer/samples/serve-dist.mjs --port …`).
        let handoff = "dev-handoff.html?lang=\(env["VELA_SAFARI_LOCALE"] ?? "en")&amp;page="
            + (env["VELA_SIGNER_PAGE"] ?? "http://127.0.0.1:9894/")
        return try String(contentsOf: url, encoding: .utf8)
            .replacingOccurrences(of: "__SEED__", with: seed.replacingOccurrences(of: "&", with: "&amp;"))
            .replacingOccurrences(of: "__HANDOFF__", with: handoff)
    }

    private func serveDapp() throws {
        if Self.server == nil {
            let server = try LocalDappServer(html: try dappHTML(), port: Self.dappPort)
            server.start()
            Self.server = server
        }
    }

    private var dappURL: URL { URL(string: "http://127.0.0.1:\(Self.dappPort)/")! }

    // MARK: - Steps

    /// Settings → Apps → Safari → Extensions → Vela Wallet: on, every website allowed.
    func test01_enableExtension() throws {
        settings.terminate()
        settings.launch()
        try tapRow(in: settings, id: "com.apple.settings.apps", label: "Apps")
        capture("01a-settings-apps", settings)
        try tapRow(in: settings, id: nil, label: "Safari")
        capture("01b-settings-safari", settings)
        try tapRow(in: settings, id: nil, label: "Extensions")
        capture("01c-settings-extensions", settings)
        try tapRow(in: settings, id: nil, label: "Vela Wallet")
        let allow = settings.switches["Allow Extension"]
        XCTAssertTrue(allow.waitForExistence(timeout: 5), "Safari lists the Vela extension")
        if (allow.value as? String) != "1" { allow.switches.firstMatch.exists ? allow.switches.firstMatch.tap() : allow.tap() }
        sleep(1)
        capture("01d-settings-vela", settings)
        XCTAssertEqual(allow.value as? String, "1", "the extension is on")
        // Website access: the harness's loopback, the signing page, and every
        // other site a person would visit — "Allow", as a person grants it.
        for site in ["127.0.0.1", "sign.getvela.app", "getvela.app", "localhost", "Other Websites"] {
            let cell = settings.cells[site]
            for _ in 0..<4 where !(cell.exists && cell.isHittable) { settings.swipeUp() }
            guard cell.exists else { continue }
            cell.tap()
            let choice = settings.cells["Allow"].exists ? settings.cells["Allow"] : settings.buttons["Allow"]
            if choice.waitForExistence(timeout: 3) { choice.tap() }
            capture("01e-allow-\(site)", settings)
            if settings.navigationBars.buttons.firstMatch.exists { settings.navigationBars.buttons.firstMatch.tap() }
            sleep(1)
        }
        capture("01f-settings-vela-allowed", settings)
    }

    /// Open the dApp in Safari: the provider is in the page (EIP-6963 + window.ethereum).
    func test02_providerInDapp() throws {
        try serveDapp()
        openInSafari(dappURL)
        let announced = pageText(containing: "provider:app.getvela")
        XCTAssertTrue(announced.waitForExistence(timeout: 20), "the page heard Vela announce itself")
        capture("02-dapp-provider", safari)
    }

    /// The extension's own page, as Safari runs it: what this Safari gives.
    func test03_probe() throws {
        try serveDapp()
        openInSafari(dappURL)
        let link = safari.links["Open the extension probe"]
        XCTAssertTrue(link.waitForExistence(timeout: 15))
        link.tap()
        let done = pageText(containing: "probe:done")
        XCTAssertTrue(done.waitForExistence(timeout: 20), "the probe ran")
        capture("03a-probe", safari)
        let passkey = safari.buttons["Try a getvela.app passkey here"]
        if passkey.exists {
            tapWhenReady(passkey)
            sleep(4)
            capture("03b-probe-passkey", safari)
            // A system sheet may be up: dismiss it and read again.
            if safari.buttons["Cancel"].exists { safari.buttons["Cancel"].tap() }
            sleep(2)
            capture("03c-probe-passkey-after", safari)
        }
    }

    /// The harness doorway: point the wallet at the fork, then enter the parallel space.
    func test04_seedParallelWallet() throws {
        try serveDapp()
        openInSafari(dappURL)
        let link = safari.links["Seed the harness wallet"]
        XCTAssertTrue(link.waitForExistence(timeout: 15))
        link.tap()
        let enter = safari.buttons["Enter (seed fixture wallet)"]
        XCTAssertTrue(enter.waitForExistence(timeout: 40), "the parallel space opened in the extension")
        tapWhenReady(enter)
        capture("04a-parallel", safari)
        sleep(8)
        capture("04b-wallet", safari)
    }

    /// The dApp asks; the extension raises the wallet's sheet in a tab of the
    /// dApp's; the person decides; the dApp hears. `key` is the dApp's button
    /// and its result's name (the dApp titles itself `<key>:ok|error`).
    func ask(_ key: String, button: String, decide labels: [String], shot: String) throws -> Bool {
        try serveDapp()
        if !safari.buttons[button].exists {
            openInSafari(dappURL)
        }
        let ask = safari.buttons[button]
        XCTAssertTrue(ask.waitForExistence(timeout: 20), "the dApp shows \(button)")
        tapWhenReady(ask)
        sleep(3)
        capture("\(shot)a-request", safari)
        let tapped = labels.isEmpty ? "none" : tapFirst(labels, in: safari, timeout: 60)
        capture("\(shot)b-decided-\(tapped ?? "nothing")", safari)
        let answered = pageText(containing: "\(key):")
        let heard = answered.waitForExistence(timeout: 90)
        sleep(1)
        capture("\(shot)c-dapp-heard", safari)
        XCTAssertNotNil(tapped, "the wallet offered one of \(labels)")
        XCTAssertTrue(heard, "the dApp heard an answer to \(key)")
        return pageText(containing: "\(key):ok").exists
    }

    func test05_connect() throws {
        XCTAssertTrue(try ask("connect", button: "Connect", decide: ["Connect", "连接"], shot: "05"), "connected")
    }

    func test06_switchToGnosis() throws {
        XCTAssertTrue(try ask("switch", button: "Switch to Gnosis", decide: [], shot: "06"), "on Gnosis")
    }

    func test07_personalSign() throws {
        XCTAssertTrue(try ask("sign", button: "Sign message", decide: ["Confirm", "确认"], shot: "07"),
                      "signed")
    }

    func test08_sendTransaction() throws {
        XCTAssertTrue(try ask("send", button: "Send 0.001", decide: ["Confirm send", "确认发送"],
                              shot: "08"), "sent")
    }

    /// The trusted signing page, opened from the extension's own origin: Safari
    /// gives it an opener, Vela's `safari-attest.js` vouches for that origin
    /// inside the page, and the page OFFERS to sign (it does not refuse) — then
    /// the person declines there, and the answer travels back to the opener.
    func test09_signingPageAnswersTheExtension() throws {
        try serveDapp()
        openInSafari(dappURL)
        let link = safari.links["Hand off to the signing page"]
        XCTAssertTrue(link.waitForExistence(timeout: 15))
        link.tap()
        let open = safari.buttons["Open the signing page"]
        XCTAssertTrue(open.waitForExistence(timeout: 15))
        tapWhenReady(open)
        sleep(6)
        capture("09a-signing-page", safari)
        let refused = pageText(containing: "sends a signature only to the Vela wallet")
        XCTAssertFalse(refused.exists, "the page did not refuse the attested extension")
        // Decline on the page: the page's own close/reject.
        let declined = tapFirst(["Reject", "Decline", "Cancel", "Don’t sign", "Close"], in: safari, timeout: 20)
        sleep(3)
        capture("09b-declined-\(declined ?? "none")", safari)
    }

    /// The other direction: the page's answer reaches the extension. `eth_sign`
    /// is refused by the page whoever asks; in a session the refusal is
    /// answered at once (`refused`), through the content script, and the
    /// extension page that asked hears it and closes the page's tab.
    func test10_signingPageAnswerReachesTheExtension() throws {
        try serveDapp()
        openInSafari(dappURL)
        let link = safari.links["Hand off a request the page refuses"]
        XCTAssertTrue(link.waitForExistence(timeout: 15))
        link.tap()
        let open = safari.buttons["Open the signing page"]
        XCTAssertTrue(open.waitForExistence(timeout: 15))
        tapWhenReady(open)
        sleep(3)
        capture("10a-refused-on-page", safari)
        let heard = pageText(containing: "handoff:error:refused")
        XCTAssertTrue(heard.waitForExistence(timeout: 30), "the extension heard the page's refusal")
        capture("10b-extension-heard", safari)
    }

    /// Web content reports buttons before it can take a tap: wait for that,
    /// then tap — by coordinate if Safari never calls it hittable.
    func tapWhenReady(_ element: XCUIElement, timeout: TimeInterval = 30) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline, !(element.exists && element.isHittable) { usleep(500_000) }
        if element.isHittable {
            element.tap()
        } else {
            element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        }
    }

    /// Tap the first of `labels` that `app` shows (a web button or link).
    @discardableResult
    func tapFirst(_ labels: [String], in app: XCUIApplication, timeout: TimeInterval = 30) -> String? {
        let start = Date()
        let deadline = start.addingTimeInterval(timeout)
        while Date() < deadline {
            for label in labels {
                for element in [app.buttons[label], app.links[label]] where element.exists {
                    // Safari can report a focused web button as not hittable
                    // for as long as it keeps focus: after a few seconds of
                    // that, tap where it is drawn.
                    if element.isHittable {
                        element.tap()
                        return label
                    }
                    if Date().timeIntervalSince(start) > 6, element.frame.height > 0 {
                        element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
                        return label
                    }
                }
            }
            usleep(500_000)
        }
        return nil
    }

    func openInSafari(_ url: URL) {
        XCUIDevice.shared.system.open(url)
        XCTAssertTrue(safari.wait(for: .runningForeground, timeout: 15))
        sleep(2)
        // First-run sheets Safari may raise.
        for label in ["Continue", "Not Now", "Close"] where safari.buttons[label].exists {
            safari.buttons[label].tap()
        }
    }

    func pageText(containing text: String) -> XCUIElement {
        safari.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
    }

    /// Scroll `app` until a row is hittable, then tap it.
    private func tapRow(in app: XCUIApplication, id: String?, label: String) throws {
        let byId = id.map { app.buttons[$0] }
        let candidates = [byId, app.buttons[label], app.cells[label], app.staticTexts[label]].compactMap { $0 }
        for _ in 0..<8 {
            if let row = candidates.first(where: { $0.exists && $0.isHittable }) {
                row.tap()
                _ = app.wait(for: .runningForeground, timeout: 2)
                sleep(1)
                return
            }
            app.swipeUp()
        }
        capture("missing-\(label)", app)
        XCTFail("no row \(label)")
    }
}
