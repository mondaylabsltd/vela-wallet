//
//  SettingsParityDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 072 on the simulator: the settings controls that were drawn and did
//  nothing, TAPPED, and the effect looked for where a person would look.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/SettingsParityDeviceTests
//
//  **Simulator only.** `testEraseReturnsToTheFirstRun` erases the device it
//  runs on — never point it at a phone with a real wallet. It stands its own
//  wallet up first (the parallel space), so an empty simulator is enough.
//

import UIKit
import XCTest

final class SettingsParityDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// 跟随系统 — selectable again after 深色, and it paints what the system
    /// paints. It was a segment whose tap stored nothing, so once 深色 was
    /// picked the system's theme could not be chosen again.
    ///
    /// Assumes the simulator's own appearance is LIGHT (`xcrun simctl ui <sim>
    /// appearance light`), which is what makes "follows the system" visible.
    func testFollowSystemIsSelectableAfterDark() {
        let app = launch(page: "settings-live")
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 20))
        settle()

        tap(app.staticTexts["深色"], in: app)
        settle(0.8)
        let dark = app.screenshot()
        attach(dark, named: "parity-theme-dark")

        tap(app.staticTexts["跟随系统"], in: app)
        settle(0.8)
        let system = app.screenshot()
        attach(system, named: "parity-theme-system")

        tap(app.staticTexts["浅色"], in: app)
        settle(0.8)
        let light = app.screenshot()
        attach(light, named: "parity-theme-light")

        XCTAssertNotEqual(dark.pngRepresentation, system.pngRepresentation,
                          "跟随系统 did not leave dark — the tap stored nothing")
        XCTAssertEqual(background(of: system), background(of: light),
                       "跟随系统 on a light simulator did not paint light")
        tap(app.staticTexts["跟随系统"], in: app)
        app.terminate()
    }

    /// The tab bar under Settings: 通讯录 and 探索 leave for their sections, as
    /// 钱包 always did. They took the tap and stayed on Settings.
    func testTheTabBarLeavesSettingsForEverySection() {
        let app = launchInWallet(pin: true)
        openSettingsTab(in: app)

        for tab in ["通讯录", "探索", "钱包"] {
            tapTab(tab, in: app)
            settle(1.5)
            attach(app.screenshot(), named: "parity-tab-\(tab)")
            XCTAssertFalse(app.staticTexts["高级"].exists, "\(tab) did not leave Settings")
            openSettingsTab(in: app)
        }
        app.terminate()
    }

    /// RPC 供应商 and 服务节点 are the core's view: nobody's fixture key, the
    /// saved state per provider, and the four endpoints' own pills.
    func testProvidersAndEndpointsAreTheCoresView() {
        let app = launch(page: "settings-live")
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 20))
        settle()
        if !app.staticTexts["RPC 供应商"].exists { app.staticTexts["高级"].tap(); settle(1) }
        attach(app.screenshot(), named: "parity-advanced-rows")

        open(row: "RPC 供应商", in: app)
        settle(1.5)
        attach(app.screenshot(), named: "parity-providers")
        XCTAssertFalse(
            app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "alch_k3y")).firstMatch.exists,
            "the providers page still shows the fixture's key"
        )
        XCTAssertTrue(app.staticTexts["未设置"].firstMatch.exists || app.textFields["未设置"].firstMatch.exists,
                      "no provider says it is not set")

        back(in: app)
        settle(1)
        open(row: "服务节点", in: app)
        settle(3)
        attach(app.screenshot(), named: "parity-endpoints")
        // The page is the endpoints page, its four fields the core's: each
        // with its own label, and none wearing the drawing's fixed latency.
        XCTAssertTrue(app.staticTexts["通行密钥索引"].waitForExistence(timeout: 5),
                      "the endpoints page did not open")
        XCTAssertFalse(app.staticTexts["62ms"].exists, "an endpoint wears the fixture's latency")
        app.terminate()
    }

    /// 清理数据 → 全部清除: the app starts over on Welcome, with no sign-out
    /// sheet to answer afterwards — and a relaunch finds nothing either.
    func testEraseReturnsToTheFirstRun() {
        // A wallet to erase: the parallel space, entered once so its flag is
        // STORED — the erase must take the flag with everything else.
        let enter = launchInWallet(pin: true)
        enter.terminate()

        let app = launchInWallet(pin: false)
        openSettingsTab(in: app)
        app.swipeUp()
        app.swipeUp()
        settle(1)
        let card = app.staticTexts["清理数据"]
        XCTAssertTrue(card.waitForExistence(timeout: 8), "no erase card")
        card.tap()
        settle(1.5)
        attach(app.screenshot(), named: "parity-erase-sheet")
        tap(element(labelled: "全部清除", in: app), in: app)

        XCTAssertTrue(app.buttons["创建钱包"].waitForExistence(timeout: 15)
                      || app.staticTexts["创建钱包"].waitForExistence(timeout: 1),
                      "the erase did not return to the first run")
        settle(1.5)
        attach(app.screenshot(), named: "parity-erase-first-run")
        XCTAssertFalse(app.staticTexts["退出登录"].exists, "a sign-out sheet followed the erase")
        app.terminate()

        let again = XCUIApplication()
        again.launchEnvironment["VELA_LANG"] = "zh"
        again.launch()
        XCTAssertTrue(again.buttons["创建钱包"].waitForExistence(timeout: 20)
                      || again.staticTexts["创建钱包"].exists,
                      "a relaunch found a wallet the erase should have taken")
        settle(4)
        attach(again.screenshot(), named: "parity-erase-relaunch")
        again.terminate()
    }

    // MARK: - Plumbing

    private func launch(page: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PAGE"] = page
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launch()
        return app
    }

    /// The app's own navigation over a wallet — the parallel space's, pinned
    /// by the environment or kept by its stored flag.
    private func launchInWallet(pin: Bool) -> XCUIApplication {
        let app = XCUIApplication()
        if pin { app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1" }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launch()
        let name = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH %@", "Parallel")).firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 30), "no wallet to work in")
        settle(1.5)
        return app
    }

    private func openSettingsTab(in app: XCUIApplication) {
        tapTab("设置", in: app)
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 10), "Settings did not open")
        settle(1)
    }

    /// A tab bar item, by its label — the bar's own element, not a title that
    /// happens to share the word.
    private func tapTab(_ label: String, in app: XCUIApplication) {
        let button = app.buttons[label]
        if button.exists {
            button.tap()
            return
        }
        // The bar is the LAST element with this label on screen.
        let matches = app.staticTexts.matching(identifier: label)
        let last = matches.element(boundBy: max(0, matches.count - 1))
        XCTAssertTrue(last.waitForExistence(timeout: 8), "no tab called \(label)")
        last.tap()
    }

    /// The page header's back chevron (its accessible name is 关闭).
    private func back(in app: XCUIApplication) {
        let chevron = app.descendants(matching: .any)["关闭"].firstMatch
        if chevron.exists {
            chevron.tap()
        } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.1)).tap()
        }
    }

    private func open(row: String, in app: XCUIApplication) {
        let label = app.staticTexts[row]
        XCTAssertTrue(label.waitForExistence(timeout: 8), "no row called \(row)")
        // Below the fold (the advanced rows are), scroll it into reach first:
        // a tap on an off-screen row lands on nothing.
        var swipes = 0
        while !label.isHittable, swipes < 4 {
            app.swipeUp()
            swipes += 1
        }
        label.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(1.0)
    }

    private func element(labelled text: String, in app: XCUIApplication) -> XCUIElement {
        let button = app.buttons[text]
        if button.exists { return button }
        return app.staticTexts[text]
    }

    private func tap(_ element: XCUIElement, in app: XCUIApplication) {
        XCTAssertTrue(element.waitForExistence(timeout: 8), "nothing to tap: \(element.description)")
        element.tap()
    }

    private func settle(_ seconds: TimeInterval = 2.5) {
        Thread.sleep(forTimeInterval: seconds)
    }

    /// One pixel of the page's background, clear of every control.
    private func background(of screenshot: XCUIScreenshot) -> Data? {
        let image = screenshot.image
        let scale = image.scale
        let rect = CGRect(x: 4 * scale, y: image.size.height * scale * 0.5, width: 2 * scale, height: 2 * scale)
        guard let cg = image.cgImage?.cropping(to: rect) else { return nil }
        return UIImage(cgImage: cg).pngData()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
