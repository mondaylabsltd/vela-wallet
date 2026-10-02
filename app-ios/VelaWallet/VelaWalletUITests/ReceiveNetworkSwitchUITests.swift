import XCTest

/// Spec 090 on a phone: home → 收款 → a network's code, with the
/// "include network" switch off (the bare address), then on (the code names
/// the network, and the calm hint appears under the switch).
///
/// Parallel space, like every device suite here: the fixture wallet, so a run
/// never depends on whose account the phone holds. Nothing is signed. The two
/// screenshots are the review artefacts.
final class ReceiveNetworkSwitchUITests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testTheSwitchTurnsTheHintOnAndOff() {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1", "-AppleLanguages", "(zh-Hans)"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        tap(app.buttons["收款"].firstMatch, "收款")
        tap(app.buttons["显示二维码"].firstMatch, "the first network's code", timeout: 20)

        // A first visit puts the network warning in front of the code.
        let acknowledge = app.buttons["我已了解"]
        if acknowledge.waitForExistence(timeout: 4) { acknowledge.tap() }

        let toggle = app.switches["receive.includeNetwork"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 20), "the switch never appeared")
        let hint = app.staticTexts["部分钱包无法识别。对方扫不出来时请关掉。"]
        XCTAssertFalse(hint.exists, "the hint shows while the switch is off")
        XCTAssertEqual(toggle.value as? String, "0")
        Thread.sleep(forTimeInterval: 1)
        attach(app.screenshot(), named: "receive-switch-off")

        toggle.tap()
        XCTAssertTrue(hint.waitForExistence(timeout: 5), "the hint never appeared")
        XCTAssertEqual(toggle.value as? String, "1")
        Thread.sleep(forTimeInterval: 1)
        attach(app.screenshot(), named: "receive-switch-on")

        toggle.tap()
        let gone = NSPredicate(format: "exists == false")
        expectation(for: gone, evaluatedWith: hint)
        waitForExpectations(timeout: 5)
        XCTAssertEqual(toggle.value as? String, "0")
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
}
