//
//  AmountTextDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 073 on the simulator: a decimal-comma keypad's "0,5" in the send
//  amount is 0.5 — the field shows what the machine has. It reached the core
//  raw before, and in fiat mode "0,5" was read as 0.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/AmountTextDeviceTests
//
//  **Nothing here spends.** The send form is opened and typed into, never
//  continued.
//

import XCTest

final class AmountTextDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The region is German, so the decimal pad's only mark is "," and the
    /// app's number preset (automatic) is dot-comma.
    func testADecimalCommaIsTheDecimalMark() {
        let app = launch(numberFormat: nil)
        let field = openSendAmount(app)
        field.typeText("0,5")
        settle(1.5)
        attach(app.screenshot(), named: "amount-dot-comma")
        XCTAssertEqual(field.value as? String, "0.5", "the comma did not become the decimal mark")
        app.terminate()
    }

    /// The pad follows the region ("," only) while the app's preset is
    /// comma-dot: one typed comma is still the decimal mark, never grouping —
    /// dropping it left no way to type a fraction at all.
    func testOneTypedCommaIsTheDecimalMarkWhenThePadAndThePresetDisagree() {
        let app = launch(numberFormat: "comma_dot")
        let field = openSendAmount(app)
        field.typeText("0")
        field.typeText(",")
        field.typeText("5")
        settle(1.5)
        attach(app.screenshot(), named: "amount-comma-dot")
        XCTAssertEqual(field.value as? String, "0.5", "the typed comma was dropped as grouping")
        app.terminate()
    }

    // MARK: - Plumbing

    private func openSendAmount(_ app: XCUIApplication) -> XCUIElement {
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        settle(2)
        tap(app.buttons["转账"], "转账")
        // Either the token picker, or — when the wallet already knows which
        // token — the form itself.
        let field = app.textFields["send.amount"]
        let picker = app.textFields["搜索代币..."]
        let deadline = Date().addingTimeInterval(30)
        while !field.exists && !picker.exists && Date() < deadline { settle(0.5) }
        if picker.exists {
            // The picker's own row: the home's asset list has an xDAI line
            // too, and a tap on it behind the picker opens nothing.
            let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "xDAI,")).firstMatch
            tap(row, "the picker's xDAI row", timeout: 30)
        }
        tap(field, "the amount field", timeout: 30)
        settle(0.5)
        return field
    }

    private func launch(numberFormat: String?) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "de_DE"]
        if let numberFormat {
            app.launchArguments += [
                "-vela.localePrefs",
                "{\"numberFormat\":\"\(numberFormat)\",\"dateFormat\":\"auto\",\"timeFormat\":\"auto\"}",
            ]
        }
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
