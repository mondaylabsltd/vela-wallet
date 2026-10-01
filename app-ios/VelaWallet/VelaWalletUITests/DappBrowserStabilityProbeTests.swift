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

    private func launch(url: String = LocalDappServer.url) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = url
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
        // One snapshot of the whole tree: reading elements one by one raced the
        // screen (an element counted, then gone before its label was read).
        var found: [String] = []
        func walk(_ node: XCUIElementSnapshot) {
            if !node.label.isEmpty, node.elementType == .staticText || node.elementType == .button {
                found.append("\(node.elementType == .button ? "B" : "T") \(node.label)")
            }
            node.children.forEach(walk)
        }
        if let root = try? app.snapshot() { walk(root) }
        let labels = found.prefix(160).joined(separator: "\n")
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
        // Spec 079: the ✕ is the one way out; the scrim no longer closes it.
        let close = app.buttons["关闭"].firstMatch
        if close.exists, close.isHittable {
            close.tap()
            Thread.sleep(forTimeInterval: 1)
            record(app, "09z-closed-by-x")
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

    /// Spec 079 US3 on the device: a page nobody answers (port 1 on this phone)
    /// — the Vela panel with its reason and host, then Retry keeps the panel up
    /// and says "正在重试…"; never WebKit's own page, never a blank.
    func testProbeALoadThatFails() throws {
        let app = launch(url: "http://127.0.0.1:1/")
        let tab = app.buttons["探索"].firstMatch
        if tab.waitForExistence(timeout: 5), tab.isHittable { tab.tap() }
        let retry = app.buttons["重试"].firstMatch
        _ = retry.waitForExistence(timeout: 20)
        record(app, "14-load-failed")
        frames(app, "14b-after-failure", count: 8, every: 1.5)
        if retry.exists, retry.isHittable {
            retry.tap()
            frames(app, "15-retry", count: 8, every: 0.5)
        }
    }

    /// Spec 082 T122 (RH4, G32, W5): a site that accepts the connection and
    /// never answers — the stall no failure callback ever reports (WebKit
    /// gave up at ~75 s). The browser's own watchdog gives up at the core's
    /// budget: the panel at 20 ± 2 s, then the automatic retry, whose button
    /// is BUSY — "正在重试…" at full colour — never dimmed. Hermetic: an
    /// in-process loopback listener, no proxy.
    func testProbeASiteThatNeverAnswers() throws {
        let silent = try SilentListener()
        XCTAssertTrue(silent.start(), "the silent listener never got a port")
        defer { silent.stop() }

        // The harness page first, then the silent site typed into the bar —
        // so the clock starts at the Go, not somewhere in the launch.
        let app = launch()
        let tab = app.buttons["探索"].firstMatch
        if tab.waitForExistence(timeout: 5), tab.isHittable { tab.tap() }
        let host = app.staticTexts["127.0.0.1:\(LocalDappServer.port)"].firstMatch
        XCTAssertTrue(host.waitForExistence(timeout: 40), "the harness page never loaded")
        host.tap()
        let field = app.textFields["explore.addressField"].firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.clearAndType(silent.url + "\n")
        let asked = Date()
        record(app, "16-never-answers-asked")
        // The bar keeps the page on screen while the silent site hangs (RE1).
        XCTAssertTrue(app.staticTexts["127.0.0.1:\(LocalDappServer.port)"].firstMatch.exists,
                      "the bar renamed itself to a page that has not arrived")

        let panel = app.descendants(matching: .any)["explore.loadFailed"].firstMatch
        XCTAssertTrue(panel.waitForExistence(timeout: 40), "no panel for a site that never answers")
        let elapsed = Date().timeIntervalSince(asked)
        record(app, "17-never-answers-panel")
        XCTContext.runActivity(named: String(format: "panel after %.1f s", elapsed)) { _ in }
        XCTAssertGreaterThanOrEqual(elapsed, 18, "the watchdog cut a load before its budget")
        XCTAssertLessThanOrEqual(elapsed, 22, "the panel is the watchdog's, at 20 ± 2 s")

        // The automatic retry at +2 s: the same button, busy, never dimmed.
        // (The busy button ignores taps, so XCUITest may read it as not
        // enabled; what the house rule forbids is the DIMMING, which the
        // screenshot below shows is absent.)
        let busy = app.buttons["正在重试…"].firstMatch
        XCTAssertTrue(busy.waitForExistence(timeout: 8), "the retry does not say it is retrying")
        record(app, "18-never-answers-retrying")
    }
}

private extension XCUIElement {
    /// Replace whatever the field holds with `text`.
    func clearAndType(_ text: String) {
        tap()
        if let current = value as? String, !current.isEmpty {
            typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count))
        }
        typeText(text)
    }
}
