import XCTest

/// 保存图片 on a phone, end to end: home → 收款 → a network's code → save →
/// the Photos prompt answered → the app says it saved. The picture itself is
/// then read back off the phone (Image Capture) and checked by eye and by
/// decoder — this test proves the button reaches the album.
///
/// Parallel space, like every device suite here: the fixture wallet, so a
/// run never depends on whose account the phone holds. Nothing is signed.
final class ShareCardSaveDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testSaveImagePutsTheCardInThePhotoLibrary() {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1", "-AppleLanguages", "(zh-Hans)"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        tap(app.buttons["收款"].firstMatch, "收款")
        // The first row is Ethereum; its code button.
        tap(app.buttons["扫描二维码"].firstMatch, "Ethereum's code", timeout: 20)

        // A first visit puts the network warning in front of the code.
        let acknowledge = app.buttons["我已了解"]
        if acknowledge.waitForExistence(timeout: 4) { acknowledge.tap() }

        let save = app.buttons["保存图片"]
        XCTAssertTrue(save.waitForExistence(timeout: 20), "保存图片 never appeared")
        attach(app.screenshot(), named: "receive-code")
        save.tap()

        // Photos asks once (add-only). SpringBoard owns the prompt.
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons.matching(
            NSPredicate(format: "label IN %@", ["允许", "Allow", "允许访问", "好"])
        ).firstMatch
        if allow.waitForExistence(timeout: 8) {
            attach(springboard.screenshot(), named: "photos-prompt")
            allow.tap()
        }

        let saved = app.staticTexts["已保存"]
        let refused = app.staticTexts["需要权限"]
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline && !saved.exists && !refused.exists {
            Thread.sleep(forTimeInterval: 0.5)
        }
        attach(app.screenshot(), named: "after-save")
        XCTAssertFalse(refused.exists, "Photos refused — allow Vela in Settings → Privacy → Photos")
        XCTAssertTrue(saved.exists, "the app never said the card was saved")
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
