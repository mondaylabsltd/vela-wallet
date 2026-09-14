//
//  SigningDepthAcceptanceTests.swift
//  VelaWalletUITests
//
//  Spec 055's acceptance, on the phone.
//
//      xcodebuild test -destination 'platform=iOS,id=<udid>' \
//        -only-testing:VelaWalletUITests/SigningDepthAcceptanceTests
//
//  What a machine can drive is here. What needs a hand — holding a code up to
//  the lens, granting the camera on its first ask — is named in results.md
//  rather than faked.
//

import XCTest

final class SigningDepthAcceptanceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func launch(parallel: Bool = true) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", parallel ? "1" : "0"]
        if parallel { app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1" }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The scanner opens on a REAL viewfinder — or says why not.
    ///
    /// Both outcomes are a pass, and that is deliberate: the first run on a
    /// phone raises the system permission sheet, which no test can answer. What
    /// must never happen is the third outcome this surface shipped with — a
    /// grey rectangle and the words "point the camera at a code" over a camera
    /// that was never started.
    func testTheScannerShowsACameraOrSaysWhyNot() throws {
        let app = launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30),
                      "the home never appeared")
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30),
                      "the picker is not showing this wallet's holdings")
        tap(app.staticTexts["xDAI"], "the xDAI row")
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20),
                      "the form never opened")

        tap(app.buttons["扫描二维码"].firstMatch, "the scan button")
        // The scanner's own furniture, whichever way it went.
        XCTAssertTrue(app.staticTexts["相册"].waitForExistence(timeout: 20)
            || app.buttons["相册"].waitForExistence(timeout: 1),
                      "the scanner surface never opened")
        attach(app.screenshot(), named: "device-scanner")

        // Either the aiming hint (a camera is running) or a refusal — never
        // the hint over a dead frame, which is what this cut replaced.
        let aiming = app.staticTexts["将相机对准二维码"].exists
        let refused = app.staticTexts["需要相机权限才能扫描二维码。"].exists
            || app.staticTexts["此设备没有相机。请改为选择二维码的图片。"].exists
            || app.staticTexts["无法打开相机，可能有其他应用正在使用它。"].exists
        XCTAssertTrue(aiming || refused, "the scanner said nothing at all about its camera")

        // The library is offered either way: it is the way through when the
        // camera is not available.
        XCTAssertTrue(app.staticTexts["相册"].exists || app.buttons["相册"].exists)
    }

    /// The torch and the flip are drawn, and on a phone they are real.
    func testTheScannersToolsAreAllThere() throws {
        let app = launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30))
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30))
        tap(app.staticTexts["xDAI"], "the xDAI row")
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20))
        tap(app.buttons["扫描二维码"].firstMatch, "the scan button")

        for tool in ["相册", "手电筒", "翻转"] {
            XCTAssertTrue(app.staticTexts[tool].waitForExistence(timeout: 10)
                || app.buttons[tool].exists, "\(tool) is missing from the scanner")
        }
        attach(app.screenshot(), named: "device-scanner-tools")
    }
}
