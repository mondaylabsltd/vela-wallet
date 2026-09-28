//
//  DappBrowserStabilityProbeTests.swift
//  VelaWalletUITests
//
//  Spec 079's iOS pass: the same checkpoints the Android device pass walked
//  (consent, signing sheet dismissal, the status after signing, the fee row,
//  pickers, site menu, tabs, a failed load and its retry), each one a named
//  screenshot plus the visible texts. It asserts nothing about the answers —
//  it is the evidence the audit table is checked against, and after the fix,
//  the evidence the fix is checked against.
//
//  Parallel space, so nothing here needs a finger. The one transaction it may
//  send is the harness's Send dust from the fixture Safe.
//

import XCTest

final class DappBrowserStabilityProbeTests: XCTestCase {

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

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    /// A screenshot and the texts on screen, under one step name.
    private func record(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
        let labels = (app.staticTexts.allElementsBoundByIndex + app.buttons.allElementsBoundByIndex)
            .prefix(120)
            .compactMap { el -> String? in
                let label = el.label
                return label.isEmpty ? nil : "\(el.elementType == .button ? "B" : "T") \(label)"
            }
            .joined(separator: "\n")
        let text = XCTAttachment(string: labels)
        text.name = name + ".txt"
        text.lifetime = .keepAlways
        add(text)
    }

    /// A few frames at an interval — what the person sees while something runs.
    private func frames(_ app: XCUIApplication, _ name: String, count: Int, every: TimeInterval) {
        for index in 1...count {
            record(app, "\(name)-\(String(format: "%02d", index))")
            Thread.sleep(forTimeInterval: every)
        }
    }

    private func sheetOpen(_ app: XCUIApplication) -> Bool {
        app.staticTexts.containing(NSPredicate(format: "label CONTAINS[c] '滑动以确认' OR label CONTAINS[c] '签名消息' OR label CONTAINS[c] '签名账户'")).firstMatch.exists
            || app.otherElements.containing(NSPredicate(format: "label CONTAINS[c] '滑动以确认'")).firstMatch.exists
    }

    /// Drags the slide (the element whose label starts "滑动以确认") to its end.
    private func slide(_ app: XCUIApplication) {
        let slider = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS[c] '滑动以确认'")).firstMatch
        guard slider.waitForExistence(timeout: 10) else { return }
        let start = slider.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.5))
        let end = slider.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5))
        start.press(forDuration: 0.15, thenDragTo: end)
    }

    func testProbeTheBrowserCheckpoints() throws {
        let app = launch()
        let tab = app.buttons["探索"].firstMatch
        if tab.waitForExistence(timeout: 5), tab.isHittable { tab.tap() }
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 40))
        record(app, "01-page")

        // Consent.
        app.webViews.buttons["Connect"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 2)
        record(app, "02-consent")
        for label in ["批准", "连接"] {
            let approve = app.buttons[label].firstMatch
            if approve.exists, approve.isHittable { approve.tap(); break }
        }
        Thread.sleep(forTimeInterval: 2)
        record(app, "03-connected")

        // Message signature: the sheet, then the three accidental closes.
        app.webViews.buttons["Sign"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 3)
        record(app, "04-sign-sheet")
        let header = app.staticTexts.containing(NSPredicate(format: "label CONTAINS[c] '签名'")).firstMatch
        if header.exists { header.swipeDown(velocity: .fast) }
        Thread.sleep(forTimeInterval: 1.5)
        record(app, "05-after-swipe-down")
        let stillAfterSwipe = sheetOpen(app)
        if !stillAfterSwipe {
            app.webViews.buttons["Sign"].firstMatch.tap()
            Thread.sleep(forTimeInterval: 3)
        }
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.08)).tap()
        Thread.sleep(forTimeInterval: 1.5)
        record(app, "06-after-scrim-tap")
        let stillAfterScrim = sheetOpen(app)
        XCTContext.runActivity(named: "dismissal: swipe=\(stillAfterSwipe ? "stays" : "CLOSES") scrim=\(stillAfterScrim ? "stays" : "CLOSES")") { _ in }
        if !stillAfterScrim {
            app.webViews.buttons["Sign"].firstMatch.tap()
            Thread.sleep(forTimeInterval: 3)
        }
        slide(app)
        frames(app, "07-after-sign", count: 8, every: 0.5)

        // A transaction: the fee row, then what follows the slide.
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 2)
        app.webViews.buttons["Send dust"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 6)
        record(app, "08-send-sheet")
        slide(app)
        frames(app, "09-after-send", count: 24, every: 1.5)
        if sheetOpen(app) {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.08)).tap()
            Thread.sleep(forTimeInterval: 1)
        }

        // Chrome: site menu, connection panel, pickers, tabs.
        let menu = app.buttons["站点菜单"].firstMatch
        if menu.waitForExistence(timeout: 5) {
            menu.tap()
            Thread.sleep(forTimeInterval: 1.5)
            record(app, "10-site-menu")
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.08)).tap()
            Thread.sleep(forTimeInterval: 1)
        }
        let account = app.buttons["账户"].firstMatch
        if account.waitForExistence(timeout: 5) {
            account.tap()
            Thread.sleep(forTimeInterval: 1.5)
            record(app, "11-connection-panel")
            let network = app.buttons.containing(NSPredicate(format: "label CONTAINS[c] '网络'")).firstMatch
            if network.exists {
                network.tap()
                Thread.sleep(forTimeInterval: 1.5)
                record(app, "12-network-picker")
            }
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.08)).tap()
            Thread.sleep(forTimeInterval: 1)
        }
        let tabs = app.buttons["标签页"].firstMatch
        if tabs.waitForExistence(timeout: 5) {
            tabs.tap()
            Thread.sleep(forTimeInterval: 1.5)
            record(app, "13-tabs")
            let done = app.buttons["完成"].firstMatch
            if done.exists { done.tap() }
            Thread.sleep(forTimeInterval: 1)
        }

        // A load that fails (nothing listens on port 1), then Retry.
        let host = app.staticTexts["127.0.0.1:8137"].firstMatch
        if host.waitForExistence(timeout: 5) {
            host.tap()
            Thread.sleep(forTimeInterval: 1)
            app.typeText("http://127.0.0.1:1/\n")
            Thread.sleep(forTimeInterval: 4)
            record(app, "14-load-failed")
            let retry = app.buttons["重试"].firstMatch
            if retry.exists {
                retry.tap()
                frames(app, "15-retry", count: 6, every: 0.4)
            }
        }
    }
}
