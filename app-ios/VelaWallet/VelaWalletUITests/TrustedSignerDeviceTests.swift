//
//  TrustedSignerDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 071 on the phone, quickstart C4: 设置 › 高级 names the Trusted Signer
//  page this device opens, says a page off getvela.app cannot use this
//  wallet's passkeys, and refuses an address the core refuses — on screen,
//  with nothing stored.
//
//  How a signature is routed is not a setting (founder, 2026-09-26): the
//  account signs with the key it signed in with, so this page offers no
//  "Sign with" of any kind.
//
//  **Nothing here signs.** The page address arrives as a launch argument (the
//  argument domain), so the simulator's stored choice is never touched:
//
//      TEST_RUNNER_VELA_TRUSTED_SIGNER_PAGE=http://localhost:8141/ \
//        xcodebuild test … -only-testing:VelaWalletUITests/TrustedSignerDeviceTests
//

import XCTest

final class TrustedSignerDeviceTests: XCTestCase {

    private let signerPage = ProcessInfo.processInfo.environment["VELA_TRUSTED_SIGNER_PAGE"]

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipIf(signerPage == nil, "set TEST_RUNNER_VELA_TRUSTED_SIGNER_PAGE to a signer page address")
    }

    func testSettingsShowsTheTrustedSignerPage() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launchArguments += ["-vela.trustedSignerUrl", signerPage ?? ""]
        app.launch()

        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        let settingsTab = app.buttons["设置"].exists ? app.buttons["设置"] : app.staticTexts["设置"]
        settingsTab.firstMatch.tap()
        let row = app.staticTexts["可信签名器页面"].firstMatch
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        if !row.exists { app.staticTexts["高级"].firstMatch.tap() }
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        XCTAssertTrue(row.waitForExistence(timeout: 10), "no 可信签名器页面 row")
        XCTAssertTrue(app.staticTexts["localhost"].exists, "the page row does not name the chosen page")
        XCTAssertFalse(app.staticTexts["签名方式"].exists, "a default \"Sign with\" is back in Settings")
        attach(XCUIScreen.main.screenshot(), named: "settings-rows")

        row.tap()
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

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
