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
//  back to the speed that was stored before the test — read off the Settings
//  row, or 标准 (`standard`, the factory default since 2026-10-09) when the
//  row says nothing this test can read.
//

import XCTest

final class FeeSpeedDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The send form's fee row carries a refresh control, and under it the
    /// folded speed control names the tier in force; opened, it offers three
    /// speeds with their own figures and no descriptions; a pick folds it onto
    /// the new speed.
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
        // A name, a price and its gas bid — what each speed buys is Settings'
        // to say, where the default is chosen, not under every payment's.
        XCTAssertFalse(app.staticTexts["手续费最低，适合不着急时"].exists,
                       "the per-payment picker describes the speeds again")

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
    /// stored and read back after a relaunch — then the speed stored before
    /// the test is put back.
    func testSettingsKeepsADefaultSpeed() {
        var app = launch()
        let before = openSpeedSheet(app)
        attach(app.screenshot(), named: "settings-speed-sheet")
        XCTAssertTrue(app.staticTexts["手续费最低，适合不着急时"].exists, "the speeds carry no line on what they buy")
        // A speed that is not the one stored, so the relaunch has a change to keep.
        let pick = before == "较慢" ? "标准" : "较慢"
        tap(option(app, pick), pick)
        settle(2)
        attach(app.screenshot(), named: "settings-speed-picked")
        app.terminate()

        app = launch()
        let after = openSpeedSheet(app)
        attach(app.screenshot(), named: "settings-speed-after-relaunch")
        XCTAssertEqual(after, pick, "the pick was not read back after a relaunch")
        // Put the phone back where it was.
        tap(option(app, before), before)
        settle(1)
        app.terminate()
    }

    // MARK: - Plumbing

    /// The three speeds as the Settings row and its sheet name them (zh).
    private static let speeds = ["超快", "标准", "较慢"]
    /// The factory default (`standard`): what the phone is put back to when
    /// the row's value cannot be read.
    private static let factoryDefault = "标准"

    /// Opens 设置 › 高级 › 交易速度 and answers the speed the row named before
    /// it was tapped — the one stored.
    @discardableResult
    private func openSpeedSheet(_ app: XCUIApplication) -> String {
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
        let stored = storedSpeed(app, row: row)
        tap(row, "交易速度")
        XCTAssertTrue(app.staticTexts["较慢"].waitForExistence(timeout: 10), "the speed sheet did not open")
        settle(1)
        return stored
    }

    /// The row's value: the speed text level with the 交易速度 title — beside
    /// it, or just under it when the two do not fit on one line.
    private func storedSpeed(_ app: XCUIApplication, row: XCUIElement) -> String {
        guard row.exists else { return Self.factoryDefault }
        let title = row.frame
        let candidates = Self.speeds.flatMap { name in
            app.staticTexts.matching(NSPredicate(format: "label == %@", name)).allElementsBoundByIndex
                .map { (name: name, gap: abs($0.frame.midY - title.midY)) }
        }
        guard let nearest = candidates.min(by: { $0.gap < $1.gap }), nearest.gap < title.height * 2 else {
            return Self.factoryDefault
        }
        return nearest.name
    }

    /// A speed in the open sheet. The sheet is the last thing drawn, so its
    /// row is the last match — the Settings row behind it can carry the same
    /// name as its value.
    private func option(_ app: XCUIApplication, _ name: String) -> XCUIElement {
        let matches = app.staticTexts.matching(NSPredicate(format: "label == %@", name))
        let count = matches.count
        return count > 0 ? matches.element(boundBy: count - 1) : matches.firstMatch
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
