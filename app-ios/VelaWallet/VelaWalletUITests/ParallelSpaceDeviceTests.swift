//
//  ParallelSpaceDeviceTests.swift
//  VelaWalletUITests
//
//  The wallet with a wallet in it — on the phone, in the parallel space.
//
//  Everything in `DeviceParityTests` is reachable without a session. These are
//  not: real balances, the marks beside them, the receive list, and a dApp
//  asking to connect. The parallel space is the real app with one substitution
//  — `vela-core`'s fixed keyset signs where a passkey would — so a script can
//  reach screens that otherwise need a finger on a sensor.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/ParallelSpaceDeviceTests \
//        -resultBundlePath /tmp/space.xcresult
//
//  **Nothing here spends.** The send is opened and photographed and never
//  confirmed: a slider at the end of it moves real money on a real chain, and
//  that is the founder's to pull.
//

import UIKit
import XCTest

final class ParallelSpaceDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The home, with the golden Safe's own holdings behind it.
    ///
    /// What is being looked at is the MARKS: 058 gave every token and network
    /// its real logo from the chain-data endpoint, with the lettermark as the
    /// fallback. Android and the web have drawn them since 047 and this client
    /// drew three letters in a coloured circle.
    func testTheWalletHomeDrawsItsHoldings() {
        let app = launch()
        // The three action buttons, not the balance label: the label carries
        // the DISPLAY CURRENCY ("总余额 · CNY" on this phone), so asserting one
        // spelling of it asserts a preference.
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened in the parallel space")
        // The reads take a moment; the marks arrive with them.
        settle(8)
        attach(app.screenshot(), named: "space-wallet-home")
        app.terminate()
    }

    /// 收款 — the network list, where every row is a chain's own logo.
    func testTheReceiveListDrawsEveryNetwork() {
        let app = launch()
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened")
        settle(4)
        app.buttons["收款"].tap()
        XCTAssertTrue(app.staticTexts["收款"].waitForExistence(timeout: 12),
                      "the receive screen never opened")
        settle(6)
        attach(app.screenshot(), named: "space-receive-list")
        app.terminate()
    }

    /// 探索 — a real dApp, loaded, in the app's own browser.
    ///
    /// Uniswap is the founder's own test case. What this asserts is that the
    /// page LOADS and the wallet's chrome is around it; connecting and
    /// swapping need decisions a person makes, and the screenshot is what
    /// those decisions get read from.
    func testTheBrowserLoadsUniswap() {
        let app = launch(openUrl: "https://app.uniswap.org/")
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened")
        // `VELA_URL` loads the page into a TAB; it does not change which tab
        // the person is looking at, so the harness has to walk over to it —
        // found here, with a screenshot of the wallet home labelled uniswap.
        // The tab bar is at the bottom of the window; tapping the button by
        // its own coordinate is what survives a label that matches more than
        // one element.
        let explore = app.buttons["探索"].firstMatch
        XCTAssertTrue(explore.waitForExistence(timeout: 8), "no 探索 tab")
        explore.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(3)
        attach(app.screenshot(), named: "space-explore-tab")
        // The browser tab takes the page's own time; give it the same patience
        // a person would — and then some: app.uniswap.org is a large SPA and
        // twenty seconds was not enough on this phone (058's first run showed
        // an empty address bar and a white page).
        settle(60)
        attach(app.screenshot(), named: "space-uniswap")
        // Either the page is there, or the app SAYS WHY it is not. Silence is
        // the one outcome this rejects — a white rectangle under an empty
        // address bar is what 058 found, and what the failure panel exists to
        // make impossible.
        let loaded = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", "uniswap")).firstMatch.exists
        let saidWhy = app.staticTexts["无法加载此页面"].exists
        XCTAssertTrue(
            loaded || saidWhy,
            "the browser showed neither the page nor a reason — it said nothing at all"
        )
        app.terminate()
    }

    /// The same door, with a page that cannot be slow.
    ///
    /// Separates "the browser is broken" from "that dApp is heavy": if this
    /// one renders and Uniswap does not, the finding is about Uniswap.
    func testTheBrowserLoadsAPlainPage() {
        let app = launch(openUrl: "https://example.com/")
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40))
        let explore = app.buttons["探索"].firstMatch
        XCTAssertTrue(explore.waitForExistence(timeout: 8))
        explore.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(10)
        attach(app.screenshot(), named: "space-plain-page")
        app.terminate()
    }

    // MARK: - Plumbing

    private func launch(openUrl: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        // IN the space: `vela-core`'s fixed keyset, the golden Safe, real
        // chains. The flag persists, so every other test in this suite states
        // `0` to stay out of it.
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        // `VELA_URL` is the harness's own door into the browser (053).
        if let openUrl { app.launchEnvironment["VELA_URL"] = openUrl }
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
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
