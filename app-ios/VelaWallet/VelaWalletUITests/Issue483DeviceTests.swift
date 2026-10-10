//
//  Issue483DeviceTests.swift
//  VelaWalletUITests
//
//  Issue #483 on a screen: a dApp's network fee stuck on "Tap to retry"
//  after a Settings change, with "Working out the network fee…" under the
//  confirm. The cause was the root's object graph: a re-run of `RootView.init`
//  (which a text-size change caused) handed the signing sheet a relay bound to
//  a request pool nobody had started, so every fee read died inside the app
//  (`rpc: before_boot`) and no retry could heal it.
//
//  This walks the reporter's path on the local test dApp, against the real
//  chain and relay (the parallel space's Safe on Gnosis; nothing is signed —
//  each sheet is closed with its ✕):
//
//      1. Send dust → the fee prices (the control);
//      2. Settings → Text size one stop larger;
//      3. back to the page → Send dust again → the fee must price again.
//
//  Skipped in the scheme with the rest of the acceptance layer (it needs the
//  network and the space); run it by name:
//
//      xcodebuild test-without-building -xctestrun <no-skip copy> \
//        -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/Issue483DeviceTests
//

import XCTest

final class Issue483DeviceTests: XCTestCase {

    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = true
        let server = try LocalDappServer(html: try LocalDappServer.page())
        server.start()
        self.server = server
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    func testADappFeeStillPricesAfterTheTextSizeChanges() throws {
        let app = launch()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)

        // 1. The control: before any Settings change the fee prices.
        let before = requestAndReadFee(app, shot: "483-1-before-settings")
        XCTAssertTrue(before.priced, "the first request's fee did not price: \(before)")
        closeSheet(app)

        // 2. Text size, one stop up from 标准.
        openSettings(app)
        let slider = textSizeSlider(app)
        tap(stop: 3, of: slider, in: app)
        settle(1.5)
        attach(app.screenshot(), named: "483-2-text-size-changed")

        // 3. The same request again.
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        let after = requestAndReadFee(app, shot: "483-3-after-settings")
        XCTAssertTrue(after.priced, "after a text-size change the dApp fee did not price: \(after)")
        XCTAssertNotEqual(after.value, "点击重试", "the fee row is stuck on Tap to retry")
        XCTAssertFalse(
            after.footer?.contains("正在计算网络费用") == true && after.value == "点击重试",
            "the footer says the fee is being worked out while the row says it failed"
        )
        closeSheet(app)

        // Put the size back for the next suite.
        openSettings(app)
        tap(stop: 2, of: textSizeSlider(app), in: app)
        settle(1)
        app.terminate()
    }

    // MARK: - The fee, as drawn

    private struct FeeReading: CustomStringConvertible {
        let value: String?
        let reason: String?
        let footer: String?
        var priced: Bool { value?.hasPrefix("~") == true }
        var description: String {
            "value=\(value ?? "nil") reason=\(reason ?? "nil") footer=\(footer ?? "nil")"
        }
    }

    /// Send dust, then wait up to 40 s for the fee row to settle — a figure,
    /// or a failure that has stayed failed past two of the core's re-asks.
    private func requestAndReadFee(_ app: XCUIApplication, shot: String) -> FeeReading {
        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["发送"].waitForExistence(timeout: 30), "the signing sheet never opened")
        let deadline = Date().addingTimeInterval(40)
        var reading = read(app)
        while Date() < deadline, !reading.priced {
            settle(1)
            reading = read(app)
        }
        attach(app.screenshot(), named: shot)
        let note = XCTAttachment(string: reading.description)
        note.name = shot + "-reading"
        note.lifetime = .keepAlways
        add(note)
        return reading
    }

    private func read(_ app: XCUIApplication) -> FeeReading {
        let figure = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "~")).firstMatch
        let retry = app.staticTexts["点击重试"].firstMatch
        let estimating = app.staticTexts["估算中..."].firstMatch
        let value: String? = figure.exists ? figure.label
            : retry.exists ? retry.label
            : estimating.exists ? estimating.label : nil
        let reason = app.staticTexts["signing.fee.reason"].firstMatch
        let footer = app.staticTexts["signing.confirmBlock"].firstMatch
        return FeeReading(
            value: value,
            reason: reason.exists ? reason.label : nil,
            footer: footer.exists ? footer.label : nil
        )
    }

    private func closeSheet(_ app: XCUIApplication) {
        let close = app.buttons["signing.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 10), "the sheet has no ✕")
        close.tap()
        XCTAssertTrue(
            waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001", timeout: 30),
            "closing the sheet did not refuse the page"
        )
    }

    // MARK: - Plumbing (BrowserAcceptanceTests' and TextScaleSliderDeviceTests')

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
    }

    private func openExplore(_ app: XCUIApplication) {
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 5) { return }
        tapTab("探索", in: app)
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        if row.waitForExistence(timeout: 10) { row.tap() }
    }

    private func connect(_ app: XCUIApplication) {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(
            NSPredicate(format: "label == %@ OR label == %@", "连接", "批准")
        ).firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_requestAccounts ok"),
                      "the page was never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")
    }

    private func waitForVerdict(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if lines.contains(where: { $0.label.contains(fragment) }) { return true }
            settle(1)
        }
        return false
    }

    private func openSettings(_ app: XCUIApplication) {
        tapTab("设置", in: app)
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 10), "Settings did not open")
        let keys = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", "内置通行密钥")).firstMatch
        XCTAssertTrue(keys.waitForExistence(timeout: 40), "the wallet's keys never arrived on Settings")
        settle(1)
    }

    private func textSizeSlider(_ app: XCUIApplication) -> XCUIElement {
        let slider = app.descendants(matching: .any)["text-scale-slider"].firstMatch
        XCTAssertTrue(slider.waitForExistence(timeout: 8), "no text-size slider on Settings")
        reach(slider, in: app)
        return slider
    }

    private func reach(_ slider: XCUIElement, in app: XCUIApplication) {
        let screen = app.frame.height
        var swipes = 0
        while swipes < 6 {
            let frame = slider.frame
            if frame.maxY > screen * 0.75 {
                page(app, from: 0.55, to: 0.3)
            } else if frame.minY < screen * 0.25 {
                page(app, from: 0.3, to: 0.55)
            } else {
                break
            }
            swipes += 1
            settle(0.8)
        }
    }

    private func page(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)))
    }

    private func tap(stop: Int, of slider: XCUIElement, in app: XCUIApplication) {
        reach(slider, in: app)
        let width = slider.frame.width
        let trackStart: CGFloat = 9 + 4
        let track = width - trackStart - 4 - 14
        let x = trackStart + 22 + (track - 44) * CGFloat(stop) / 5
        slider.coordinate(withNormalizedOffset: CGVector(dx: x / width, dy: 0.5)).tap()
    }

    private func tapTab(_ label: String, in app: XCUIApplication) {
        let button = app.buttons[label]
        if button.exists {
            button.tap()
            return
        }
        let matches = app.staticTexts.matching(identifier: label)
        let last = matches.element(boundBy: max(0, matches.count - 1))
        XCTAssertTrue(last.waitForExistence(timeout: 8), "no tab called \(label)")
        last.tap()
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
