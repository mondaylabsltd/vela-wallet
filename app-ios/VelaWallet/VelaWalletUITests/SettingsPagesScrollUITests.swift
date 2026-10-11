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

import UIKit
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

    // MARK: - Under the status bar, not behind it

    /// Settings, scrolled: nothing of the page is drawn behind the status bar
    /// (iPhone walk 2026-10-10 — a paragraph and a row's value showed through
    /// behind the clock; every page stood on the same cause, `pageGround`).
    ///
    /// The plain Settings page (`VELA_PAGE=settings`: no gallery chips above
    /// it, so it reaches the top of the screen as the real one does), on the
    /// real screen with the system's own status bar. The picture before
    /// scrolling says which of the strip's pixels are the page's ground — the
    /// clock, the island and the battery are not — and after each drag those
    /// same pixels are still ground. `StatusBarGroundTests` reads every page's
    /// strip exactly, in a window with no clock in it; this is the one look
    /// at the screen a person holds.
    func testAScrolledPageIsNotDrawnBehindTheStatusBar() throws {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0", "-AppleLanguages", "(zh-Hans)"]
        app.launchEnvironment["VELA_PAGE"] = "settings"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        XCTAssertTrue(app.staticTexts["设置"].firstMatch.waitForExistence(timeout: 20), "Settings never appeared")
        Thread.sleep(forTimeInterval: 1)

        let bar = XCUIApplication(bundleIdentifier: "com.apple.springboard").statusBars.firstMatch
        let height = bar.exists ? bar.frame.maxY : 44
        XCTAssertGreaterThan(height, 15, "no status bar to look behind")

        let rest = try strip(of: app.screenshot(), points: height)
        attach(app.screenshot(), named: "statusbar-rest")
        // The page's ground: the strip's commonest colour, by far.
        let ground = rest.commonest
        let free = rest.indices(of: ground)
        XCTAssertGreaterThan(free.count, rest.count / 2, "the strip at rest is not mostly ground")

        var worst = 0
        for step in 1...4 {
            // A slow drag, so the page stops where the finger does.
            let from = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.6))
            let to = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.6 - 0.07 * Double(step)))
            from.press(forDuration: 0.1, thenDragTo: to, withVelocity: .slow, thenHoldForDuration: 0.3)
            Thread.sleep(forTimeInterval: 0.8)
            let shot = app.screenshot()
            let now = try strip(of: shot, points: height)
            let strangers = free.filter { !now.matches(ground, at: $0) }.count
            if strangers > worst || step == 4 {
                worst = max(worst, strangers)
                attach(shot, named: "statusbar-scrolled-\(step)")
            }
        }
        // A minute ticking over between two pictures redraws a digit of the
        // clock: a few hundred pixels at most. A page behind the status bar
        // is tens of thousands.
        XCTAssertLessThan(worst, max(1500, free.count / 100),
                          "scrolled, the page is drawn behind the status bar: \(worst) of \(free.count) ground pixels changed")
    }

    /// The top `points` of a screenshot, as RGBA pixels.
    private struct Strip {
        let bytes: [UInt8]
        var count: Int { bytes.count / 4 }

        private func key(_ index: Int) -> UInt32 {
            UInt32(bytes[index * 4]) << 16 | UInt32(bytes[index * 4 + 1]) << 8 | UInt32(bytes[index * 4 + 2])
        }

        var commonest: UInt32 {
            var counts: [UInt32: Int] = [:]
            for index in 0..<count { counts[key(index), default: 0] += 1 }
            return counts.max { $0.value < $1.value }?.key ?? 0
        }

        func matches(_ colour: UInt32, at index: Int) -> Bool {
            let mine = key(index)
            for shift in [UInt32(16), 8, 0] {
                let a = Int((mine >> shift) & 0xFF), b = Int((colour >> shift) & 0xFF)
                if abs(a - b) > 8 { return false }
            }
            return true
        }

        func indices(of colour: UInt32) -> [Int] {
            (0..<count).filter { matches(colour, at: $0) }
        }
    }

    private func strip(of screenshot: XCUIScreenshot, points: CGFloat) throws -> Strip {
        let image = try XCTUnwrap(screenshot.image.cgImage, "the screenshot has no bitmap")
        let scale = screenshot.image.scale
        let width = image.width, height = min(image.height, Int(points * scale))
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        bytes.withUnsafeMutableBytes { buffer in
            guard let context = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return }
            // The image's top rows: drawn so its top edge is the context's.
            context.draw(image, in: CGRect(
                x: 0, y: CGFloat(height - image.height), width: CGFloat(width), height: CGFloat(image.height)
            ))
        }
        return Strip(bytes: bytes)
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
