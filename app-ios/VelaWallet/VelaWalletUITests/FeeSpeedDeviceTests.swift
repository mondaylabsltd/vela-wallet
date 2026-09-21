//
//  FeeSpeedDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 069 on the phone: the fee you can refresh, the speed you can choose,
//  and the default speed in Settings — in the parallel space, on real chains.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/FeeSpeedDeviceTests \
//        -resultBundlePath /tmp/speed.xcresult
//
//  **Nothing here spends.** The send form is opened, its speed control opened
//  and a speed picked — and it is never confirmed. The Settings pick is put
//  back to the factory `fast` before the test ends.
//

import XCTest

final class FeeSpeedDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The send form's fee row carries a refresh control, and under it the
    /// folded speed control names the tier in force; opened, it offers three
    /// speeds with their own figures; a pick folds it onto the new speed.
    func testTheSendFormOffersASpeedAndARefresh() {
        let app = launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        tap(app.buttons["转账"], "转账")
        tap(app.staticTexts["xDAI"].firstMatch, "the xDAI row", timeout: 30)
        let speed = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "速度")).firstMatch
        XCTAssertTrue(speed.waitForExistence(timeout: 20), "the speed control never appeared under the fee row")
        XCTAssertTrue(app.buttons["刷新费用"].waitForExistence(timeout: 5), "the fee row has no refresh control")
        settle(8)
        attach(app.screenshot(), named: "speed-folded")

        speed.tap()
        XCTAssertTrue(app.staticTexts["较慢"].waitForExistence(timeout: 10), "opening the control offered no speeds")
        XCTAssertTrue(app.staticTexts["仅这一笔，下次仍用默认"].exists, "the one-shot promise is not said")
        // Each option prices itself: give the previews their round trips.
        settle(12)
        attach(app.screenshot(), named: "speed-open")

        tap(app.staticTexts["较慢"], "较慢")
        settle(6)
        XCTAssertFalse(app.staticTexts["仅这一笔，下次仍用默认"].exists, "a pick did not fold the control")
        attach(app.screenshot(), named: "speed-picked-slow")

        tap(app.buttons["刷新费用"], "刷新费用")
        settle(6)
        attach(app.screenshot(), named: "speed-refreshed")
        app.terminate()
    }

    /// 设置 › 高级 › 交易速度: three speeds with what each buys; a pick is
    /// stored and read back after a relaunch — then put back to 超快.
    func testSettingsKeepsADefaultSpeed() {
        var app = launch()
        openSpeedSheet(app)
        attach(app.screenshot(), named: "settings-speed-sheet")
        XCTAssertTrue(app.staticTexts["手续费最低，适合不着急时"].exists, "the speeds carry no line on what they buy")
        tap(app.staticTexts["标准"].firstMatch, "标准")
        settle(2)
        attach(app.screenshot(), named: "settings-speed-standard")
        app.terminate()

        app = launch()
        openSpeedSheet(app)
        attach(app.screenshot(), named: "settings-speed-after-relaunch")
        // Put the phone back where it was.
        tap(app.staticTexts["超快（默认）"].exists ? app.staticTexts["超快（默认）"] : app.staticTexts["超快"].firstMatch, "超快")
        settle(1)
        app.terminate()
    }

    // MARK: - Plumbing

    private func openSpeedSheet(_ app: XCUIApplication) {
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        let settingsTab = app.buttons["设置"].exists ? app.buttons["设置"] : app.staticTexts["设置"]
        tap(settingsTab, "设置")
        settle(2)
        app.swipeUp()
        app.swipeUp()
        settle(1)
        let row = app.staticTexts["交易速度"].firstMatch
        if !row.exists { tap(app.staticTexts["高级"].firstMatch, "高级") }
        settle(1)
        if !row.isHittable { app.swipeUp() }
        tap(row, "交易速度")
        XCTAssertTrue(app.staticTexts["较慢"].waitForExistence(timeout: 10), "the speed sheet did not open")
        settle(1)
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
    }

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
