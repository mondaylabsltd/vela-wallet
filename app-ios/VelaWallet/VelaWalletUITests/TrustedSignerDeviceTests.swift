//
//  TrustedSignerDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 071 on the phone, quickstart C4: a page asks for a signature, the
//  sheet starts at the stored "Sign with" — the Trusted Signer — and the slide
//  opens the signer page in the in-app tab; closing the tab without signing
//  brings back the sheet, unsigned, with the Trusted Signer's own sentence.
//
//  **Nothing here signs.** The page is shown and closed; no passkey ceremony
//  runs. Opt-in, because the signer page must be served:
//
//      cd app-web/trusted-signer && python3 -m http.server 8141 &
//      TEST_RUNNER_VELA_TRUSTED_SIGNER_PAGE=http://localhost:8141/ \
//        xcodebuild test … -only-testing:VelaWalletUITests/TrustedSignerDeviceTests
//
//  Both preferences arrive as launch arguments (the argument domain), so the
//  simulator's stored choices are never touched.
//

import XCTest

final class TrustedSignerDeviceTests: XCTestCase {

    /// Beside the browser suite's 8137, so the two can share a machine.
    private static let dappPort: UInt16 = 8142
    private var server: LocalDappServer?
    private let signerPage = ProcessInfo.processInfo.environment["VELA_TRUSTED_SIGNER_PAGE"]

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipIf(signerPage == nil, "set TEST_RUNNER_VELA_TRUSTED_SIGNER_PAGE to a served signer page")
        let server = try LocalDappServer(html: try LocalDappServer.page(), port: Self.dappPort)
        server.start()
        self.server = server
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    func testClosingTheSignerPageLeavesTheRequestUnsigned() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_URL"] = server?.pageUrl
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launchArguments += ["-vela.signMethod", "trusted_signer", "-vela.trustedSignerUrl", signerPage ?? ""]
        app.launch()

        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        // A launch that names a page opens on it (spec 070); otherwise 探索.
        if !app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 5) {
            app.buttons["探索"].firstMatch.tap()
        }
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)
        app.webViews.buttons["Sign"].firstMatch.tap()

        // The sheet starts at the stored "Sign with".
        let signWith = app.descendants(matching: .any).matching(
            NSPredicate(format: "label CONTAINS %@ AND label CONTAINS %@", "签名方式", "可信签名器")
        ).firstMatch
        XCTAssertTrue(signWith.waitForExistence(timeout: 30), "the sheet did not start at the Trusted Signer")
        signWith.tap()
        XCTAssertTrue(app.staticTexts["在独立的页面上核对并签名——所见即所签。"].waitForExistence(timeout: 5),
                      "the Trusted Signer's line is not under it")
        attach(XCUIScreen.main.screenshot(), named: "trusted-signer-picker")
        signWith.tap()

        let slide = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "滑动以确认")).firstMatch
        XCTAssertTrue(slide.waitForExistence(timeout: 20), "the confirm slide is missing")
        slide.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: slide.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)))

        // The page, in the in-app tab, over the waiting sheet.
        Thread.sleep(forTimeInterval: 10)
        attach(XCUIScreen.main.screenshot(), named: "trusted-signer-page")
        // The tab's own close button lives in Safari's view service; a
        // synthesized tap on the element does not always reach it, a tap at
        // its place on the screen does.
        let close = app.buttons["Close"].firstMatch
        XCTAssertTrue(close.waitForExistence(timeout: 5), "the tab has no close button")
        let frame = close.frame
        app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: frame.midX, dy: frame.midY))
            .tap()

        let closed = app.staticTexts["可信签名器已关闭，没有签名。"]
        XCTAssertTrue(closed.waitForExistence(timeout: 20),
                      "closing the tab did not bring the sheet back, unsigned, with the Trusted Signer's sentence")
        // …and it can be signed another way: the slide is open, and at rest.
        XCTAssertTrue(slide.waitForExistence(timeout: 5), "the request did not stay open")
        let reopened = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: slide)
        wait(for: [reopened], timeout: 10)
        attach(XCUIScreen.main.screenshot(), named: "trusted-signer-closed")
        app.terminate()
    }

    /// 设置 › 高级: "Sign with" and the Trusted Signer page, beside the speed.
    /// The page sheet says a page off getvela.app cannot use this wallet's
    /// passkeys, and an address the core refuses is refused on screen with
    /// nothing stored. Nothing is chosen, so nothing needs putting back.
    func testSettingsShowsHowThisDeviceSigns() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launchArguments += ["-vela.signMethod", "trusted_signer", "-vela.trustedSignerUrl", signerPage ?? ""]
        app.launch()

        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        let settingsTab = app.buttons["设置"].exists ? app.buttons["设置"] : app.staticTexts["设置"]
        settingsTab.firstMatch.tap()
        let row = app.staticTexts["签名方式"].firstMatch
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        if !row.exists { app.staticTexts["高级"].firstMatch.tap() }
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        XCTAssertTrue(row.waitForExistence(timeout: 10), "no 签名方式 row")
        XCTAssertTrue(app.staticTexts["可信签名器"].exists, "the row does not say the stored method")
        XCTAssertTrue(app.staticTexts["localhost"].exists, "the page row does not name the chosen page")
        attach(XCUIScreen.main.screenshot(), named: "settings-rows")

        row.tap()
        XCTAssertTrue(app.staticTexts["你在 Vela 发起的每次签名都从这里开始。单次签名时仍可另选方式。"]
            .waitForExistence(timeout: 10), "the Sign with sheet did not open")
        XCTAssertTrue(app.staticTexts["在独立的页面上核对并签名——所见即所签。"].exists)
        attach(XCUIScreen.main.screenshot(), named: "settings-sign-with")
        app.buttons["关闭"].firstMatch.tap()

        app.staticTexts["可信签名器页面"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["你的通行密钥属于 getvela.app。其他域名上的页面可以显示请求，但无法签名。"]
            .waitForExistence(timeout: 10), "a page off getvela.app is not said to be unable to sign")
        XCTAssertTrue(app.buttons["使用官方页面"].exists, "no way back to the official page")
        let field = app.textFields.firstMatch
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 40) + "http://192.168.1.4/")
        app.buttons["保存"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["请使用 https 页面，或本机回环地址上的页面——浏览器不会在其他地方签名。"]
            .waitForExistence(timeout: 10), "an insecure page was not refused on screen")
        attach(XCUIScreen.main.screenshot(), named: "settings-signer-page-refused")
        app.terminate()
    }

    // MARK: - Plumbing

    private func connect(_ app: XCUIApplication) {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons["批准"].firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        XCTAssertTrue(verdict(app, containing: "#verdict eth_requestAccounts ok"), "the page was never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(verdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")
    }

    private func verdict(_ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if lines.contains(where: { $0.label.contains(fragment) }) { return true }
            Thread.sleep(forTimeInterval: 1)
        }
        return false
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
