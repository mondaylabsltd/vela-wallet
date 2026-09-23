//
//  TextScaleSliderDeviceTests.swift
//  VelaWalletUITests
//
//  The text-size slider under a finger (the founder: iOS 设置中的 AA 字号设置，
//  没有滑动的感觉). It was six tap targets; a drag across it did nothing. Here
//  a drag from one end to the other moves the size, a tap jumps to a stop, a
//  vertical swipe that starts on it scrolls the page and leaves the size
//  alone — and the size is stored and seen on the wallet.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/TextScaleSliderDeviceTests
//
//  The simulator cannot vibrate: the detents are pinned by the unit tests
//  (`TextScaleSliderTests`), this proves the slider. The size is put back to
//  标准 before the test ends.
//

import XCTest

final class TextScaleSliderDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testTheTextSizeSlides() {
        var app = launch()
        openSettings(in: app)
        var slider = self.slider(in: app)
        attach(app.screenshot(), named: "slider-at-rest")

        // To the small end, then across to the large end — by dragging.
        drag(slider, from: 0.9, to: 0.02)
        XCTAssertEqual(slider.value as? String, "1", "a drag to the small end did not reach the smallest size")
        let small = walletHeadingHeight(in: app)
        openSettings(in: app)
        slider = self.slider(in: app)

        drag(slider, from: 0.1, to: 0.98)
        settle(1)
        attach(app.screenshot(), named: "slider-dragged-large")
        XCTAssertEqual(slider.value as? String, "6", "a drag to the large end did not reach the largest size")
        let large = walletHeadingHeight(in: app)
        XCTAssertGreaterThan(large, small * 1.3, "the wallet's text did not grow with the size (\(small) → \(large))")

        // A vertical swipe that starts on the slider scrolls the page and
        // does not touch the size.
        openSettings(in: app)
        slider = self.slider(in: app)
        let top = slider.frame.minY
        slider.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: slider.coordinate(withNormalizedOffset: CGVector(dx: 0.42, dy: -3)))
        settle(1)
        XCTAssertNotEqual(slider.frame.minY, top, "a vertical swipe on the slider did not scroll the page")
        XCTAssertEqual(slider.value as? String, "6", "a vertical swipe on the slider changed the size")

        // Stored: a relaunch reads it back.
        app.terminate()
        app = launch()
        openSettings(in: app)
        slider = self.slider(in: app)
        XCTAssertEqual(slider.value as? String, "6", "the size was not stored")

        // A tap jumps to the tapped stop — 标准, the third, which also puts
        // the phone back where it was.
        tap(stop: 2, of: slider)
        settle(1)
        attach(app.screenshot(), named: "slider-tapped-standard")
        XCTAssertEqual(slider.value as? String, "3", "a tap did not jump to the tapped stop")
        app.terminate()
    }

    /// The page a person is ON while they choose — which is the page they
    /// judge the choice by.
    ///
    /// The owner moved the slider and reported that nothing changed, then that
    /// it changed everywhere BUT here (2026-09-23). It was not a refresh: no
    /// screen in Settings ever applied the size, and neither did onboarding or
    /// the Trusted Signer's sheets — 228 of the app's 494 text sites drew at a
    /// fixed size. `typeRole` now takes the size from the environment, which
    /// the root states once, so the only way to opt OUT is to have done the
    /// arithmetic yourself.
    func testTheSettingsPageGrowsWhileYouChoose() {
        let app = launch()
        openSettings(in: app)
        let slider = self.slider(in: app)

        // A label that is on the page with the slider, whatever the scroll.
        let heading = app.staticTexts["外观"].firstMatch
        XCTAssertTrue(heading.waitForExistence(timeout: 8), "Settings has no 外观 heading")

        drag(slider, from: 0.9, to: 0.02)
        XCTAssertEqual(slider.value as? String, "1", "a drag to the small end did not reach the smallest size")
        let small = heading.frame.height
        attach(app.screenshot(), named: "settings-at-smallest")

        drag(slider, from: 0.1, to: 0.98)
        XCTAssertEqual(slider.value as? String, "6", "a drag to the large end did not reach the largest size")
        let large = heading.frame.height
        attach(app.screenshot(), named: "settings-at-largest")

        // Measured, on the simulator: 43.3 → 53.3 with the fix, and 47.0 →
        // 47.0 without it. A frame grows by less than the 1.65× between 0.82
        // and 1.35 — the label's box is type plus fixed leading, and Dynamic
        // Type compresses the top of its own curve — so the threshold is set
        // where a page that did not move cannot pass, not at the font ratio.
        XCTAssertGreaterThan(
            large, small * 1.15,
            "Settings' own text did not grow with the size (\(small) → \(large))"
        )

        // Put it back: 标准 is the third stop.
        tap(stop: 2, of: slider)
        settle(1)
        XCTAssertEqual(slider.value as? String, "3", "the size was not put back")
        app.terminate()
    }

    // MARK: - Plumbing

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        return app
    }

    private func openSettings(in app: XCUIApplication) {
        tapTab("设置", in: app)
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 10), "Settings did not open")
        settle(1)
    }

    /// The slider, scrolled up clear of the tab bar: it sits under 语言, below
    /// the account and its keys, so on a phone it starts under the bar.
    private func slider(in app: XCUIApplication) -> XCUIElement {
        let slider = app.descendants(matching: .any)["text-scale-slider"].firstMatch
        XCTAssertTrue(slider.waitForExistence(timeout: 8), "no text-size slider on Settings")
        let screen = app.frame.height
        var swipes = 0
        while slider.frame.maxY > screen * 0.75, swipes < 5 {
            // Dragged from above the slider, so the scroll is the page's own.
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.55))
                .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.3)))
            swipes += 1
            settle(0.8)
        }
        XCTAssertTrue(slider.isHittable, "the text-size slider could not be scrolled into reach")
        return slider
    }

    /// A finger put down at one fraction of the slider's width and dragged to
    /// another — the whole row, the two A glyphs included.
    private func drag(_ slider: XCUIElement, from: CGFloat, to: CGFloat) {
        let start = slider.coordinate(withNormalizedOffset: CGVector(dx: from, dy: 0.5))
        let end = slider.coordinate(withNormalizedOffset: CGVector(dx: to, dy: 0.5))
        start.press(forDuration: 0.05, thenDragTo: end)
        settle(1)
    }

    /// The row is  A ·12· track ·12· A  with the stops a thumb-radius (10pt)
    /// in from each end of the track; the glyphs are about 9 and 14pt wide.
    /// Within a few points is enough: a stop is ~55pt from the next.
    private func tap(stop: Int, of slider: XCUIElement) {
        let width = slider.frame.width
        let trackStart: CGFloat = 9 + 12
        let track = width - trackStart - 12 - 14
        let x = trackStart + 10 + (track - 20) * CGFloat(stop) / 5
        slider.coordinate(withNormalizedOffset: CGVector(dx: x / width, dy: 0.5)).tap()
    }

    /// The wallet's own type, at the chosen size: the height of the home's
    /// 资产 heading, which is drawn at the person's text scale.
    private func walletHeadingHeight(in app: XCUIApplication) -> CGFloat {
        tapTab("钱包", in: app)
        let heading = app.staticTexts["资产"].firstMatch
        XCTAssertTrue(heading.waitForExistence(timeout: 10), "the wallet did not come back")
        settle(1)
        return heading.frame.height
    }

    /// A tab bar item, by its label — the bar's own element, not a title that
    /// happens to share the word.
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
