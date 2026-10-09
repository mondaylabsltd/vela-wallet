//
//  SettingsPagesScrollUITests.swift
//  VelaWalletUITests
//
//  Settings' pushed pages open at their top, and the ‹ goes back to the
//  Settings home where the person left it (device pass 2026-10-09, the same
//  finding as Android's `SettingsPagesTest`): the home and every pushed page
//  shared ONE scroll view, so a page reached by scrolling the home down
//  opened at the offset the home had been scrolled to — 关于 with its title
//  and its ‹ 300 pt above the screen — and the ‹ brought the home back at
//  its top, not where it was left.
//
//  Hermetic: the settings gallery (`VELA_PAGE=settings-gallery`) on ST1b, the
//  home with 高级 open, drawn from fixtures — no wallet, no network. Real taps,
//  read through the frames XCUITest reports.
//
//      xcodebuild test … -only-testing:VelaWalletUITests/SettingsPagesScrollUITests
//

import XCTest

final class SettingsPagesScrollUITests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testAPushedPageOpensAtItsTopAndBackLandsWhereTheHomeWasLeft() {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0", "-AppleLanguages", "(zh-Hans)"]
        app.launchEnvironment["VELA_PAGE"] = "settings-gallery"
        app.launchEnvironment["VELA_SETTINGS_STATE"] = "st1b"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()

        // 关于, the last row of the home: scrolled to, as a person does. Its
        // page is longer than the screen — a page that short of it can only
        // open as far down as its own foot.
        let row = app.staticTexts["关于"].firstMatch
        let back = app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "关闭")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20), "the 关于 row never appeared")
        let unscrolled = row.frame.minY
        for _ in 0..<3 { app.swipeUp() }
        if !row.isHittable { app.swipeDown() }
        XCTAssertTrue(row.isHittable, "the 关于 row is not on screen")
        Thread.sleep(forTimeInterval: 1)
        let left = row.frame.minY
        XCTAssertLessThan(left, unscrolled - 300, "the home did not scroll")
        attach(app.screenshot(), named: "home-scrolled")

        tap(row, "关于")

        // 关于 at its top: its ‹ and its title on screen, under the gallery's
        // chips — not scrolled away above them.
        XCTAssertTrue(back.waitForExistence(timeout: 10), "the 关于 page never opened")
        Thread.sleep(forTimeInterval: 1)
        attach(app.screenshot(), named: "about-opened")
        let title = app.staticTexts["关于"].firstMatch
        XCTAssertTrue(back.isHittable, "关于 opened part-way down: its ‹ is at \(back.frame.minY)")
        XCTAssertTrue(title.isHittable, "关于 opened part-way down: its title is at \(title.frame.minY)")
        XCTAssertLessThan(title.frame.minY, 200, "关于 opened part-way down: its title is at \(title.frame.minY)")
        // The home under the page is drawn for nobody: VoiceOver reads the page alone.
        XCTAssertEqual(app.staticTexts.matching(NSPredicate(format: "label == %@", "关于")).count, 1,
                       "the home's 关于 row is readable under the page")
        XCTAssertFalse(app.staticTexts["语言"].exists, "the home's rows are readable under the page")

        tap(back, "‹")

        // The home, where it was left.
        XCTAssertTrue(row.waitForExistence(timeout: 10), "Back did not come back to the home")
        Thread.sleep(forTimeInterval: 1)
        attach(app.screenshot(), named: "home-again")
        XCTAssertFalse(back.exists, "the 关于 page is still up")
        XCTAssertEqual(row.frame.minY, left, accuracy: 2, "the home lost its place")
    }

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 10) {
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
