//
//  ExploreNavigationScreenTests.swift
//  VelaWalletUITests
//
//  DESIGN N on the screen: the Explore home's resume section, the one top
//  bar over a page with the app's tab bar under it, and the way back and
//  forth between a dApp and the wallet — photographed, so the phone can be
//  laid beside the web boards it must match (E2, E4, E5, E6, E7).
//
//  Two halves:
//  - the gallery's states (`VELA_PAGE=explore`), which need no wallet and
//    run nobody's JavaScript, plus a walk through them;
//  - a live dApp in the parallel space — the harness page served from this
//    runner — left for the wallet and brought back, with the page's own
//    painted state as the proof it was not reloaded.
//
//      xcodebuild test … -only-testing:VelaWalletUITests/ExploreNavigationScreenTests \
//        -resultBundlePath /tmp/nav.xcresult
//      xcrun xcresulttool export attachments --path /tmp/nav.xcresult --output-path /tmp/nav
//

import XCTest

final class ExploreNavigationScreenTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    // MARK: - The gallery's boards

    /// Every board the design changed, in the board's own locale and theme,
    /// and the dark one in English.
    func testTheBoards() throws {
        let boards: [(state: String, lang: String, theme: String)] = [
            ("e1", "zh", "light"), ("e2", "zh", "light"), ("e4", "zh", "light"),
            ("e5", "zh", "light"), ("e6", "zh", "light"), ("e7", "zh", "light"),
            ("e2", "en", "dark"), ("e4", "en", "dark"), ("e6", "en", "dark"),
        ]
        for board in boards {
            let app = gallery(board.state, lang: board.lang, theme: board.theme)
            Thread.sleep(forTimeInterval: 1.5)
            attach(app.screenshot(), named: "\(board.state)-\(board.lang)-\(board.theme)")
            app.terminate()
        }
    }

    /// The walk the boards describe, in the gallery: a resume row opens its
    /// page with the tab bar still under it; the pill edits; ⋯ leads with a
    /// greyed Forward; ‹ with no history is the home; 探索 while browsing is
    /// the home; the header's action is the switcher, and Done comes back.
    func testTheGalleryWalk() throws {
        let app = gallery("e2", lang: "zh", theme: "light")

        // E2: the section, its header and three rows; no count box up top.
        XCTAssertTrue(app.staticTexts["已打开 4 个标签页"].waitForExistence(timeout: 10),
                      "the resume section's header is missing")
        let rows = app.buttons.matching(identifier: "explore.resume.row")
        XCTAssertEqual(rows.count, 3, "the board shows three rows")
        attach(app.screenshot(), named: "walk-01-home")

        // A row: its page, the one top bar, the tab bar still there.
        rows.element(boundBy: 0).tap()
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 5), "no top bar")
        XCTAssertTrue(app.buttons["explore.bar.account"].exists)
        XCTAssertTrue(app.buttons["explore.bar.tabs"].exists)
        XCTAssertTrue(app.buttons["explore.bar.menu"].exists)
        XCTAssertTrue(app.buttons["钱包"].exists, "the app's tab bar left with the page")
        XCTAssertFalse(app.buttons["关闭网页"].exists, "a bar control still says close page")
        XCTAssertEqual(app.buttons["explore.bar.account"].label, "账户, 已连接")
        attach(app.screenshot(), named: "walk-02-page")

        // The pill edits the full address; the cluster steps aside.
        app.staticTexts["app.uniswap.org"].firstMatch.tap()
        XCTAssertTrue(app.textFields["explore.addressField"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["explore.bar.menu"].exists, "⋯ stays while the address is edited")
        attach(app.screenshot(), named: "walk-03-editing")
        // ‹ only puts the field away.
        app.buttons["explore.bar.back"].tap()
        XCTAssertTrue(app.buttons["explore.bar.menu"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["explore.bar.account"].exists, "‹ while editing left the page")

        // ⋯: Forward first, greyed.
        app.buttons["explore.bar.menu"].tap()
        let forward = app.buttons["前进"].firstMatch
        XCTAssertTrue(forward.waitForExistence(timeout: 5), "Forward is missing from the site menu")
        XCTAssertFalse(forward.isEnabled, "nothing ahead: Forward is greyed")
        Thread.sleep(forTimeInterval: 0.8)
        attach(app.screenshot(), named: "walk-04-menu")
        app.buttons["关闭"].firstMatch.tap()
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 5))

        // The count box: the switcher; Done is the page again.
        app.buttons["explore.bar.tabs"].tap()
        XCTAssertTrue(app.buttons["完成"].waitForExistence(timeout: 5), "the switcher did not open")
        attach(app.screenshot(), named: "walk-05-switcher-from-page")
        app.buttons["完成"].tap()
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 5),
                      "Done did not return to the page it was opened from")

        // 探索 while browsing: the home.
        app.buttons["探索"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["已打开 4 个标签页"].waitForExistence(timeout: 5),
                      "探索 while browsing did not return to the home")

        // ‹ with no history: the home too.
        app.buttons.matching(identifier: "explore.resume.row").element(boundBy: 1).tap()
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 5))
        app.buttons["explore.bar.back"].tap()
        XCTAssertTrue(app.staticTexts["已打开 4 个标签页"].waitForExistence(timeout: 5),
                      "‹ with no history did not return to the home")

        // The header's action: the switcher; Done is the home again.
        app.buttons["标签页"].firstMatch.tap()
        XCTAssertTrue(app.buttons["完成"].waitForExistence(timeout: 5))
        attach(app.screenshot(), named: "walk-06-switcher-from-home")
        app.buttons["完成"].tap()
        XCTAssertTrue(app.staticTexts["已打开 4 个标签页"].waitForExistence(timeout: 5),
                      "Done did not return to the home it was opened from")
        app.terminate()
    }

    // MARK: - A live dApp, left and come back to

    /// The owner's two complaints, on a real page: the wallet is one tap from
    /// a dApp, and 探索 comes back to the Explore HOME — with the dApp waiting
    /// one tap away in the resume section, as it was left, not reloaded.
    ///
    /// The page's own painted state is the witness: `#last` shows the answer
    /// to a read made before leaving. A reload would paint the boot line.
    func testADappIsLeftForTheWalletAndResumedAsItWasLeft() throws {
        let server = try LocalDappServer(html: try LocalDappServer.page())
        server.start()
        defer { server.stop() }

        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 40),
                      "the space must be open")

        // A launch URL is a page opened from outside: Explore lands on it.
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 40),
                      "the launch URL did not land on its page")
        XCTAssertTrue(app.buttons["钱包"].exists, "the tab bar is not under the page")
        app.webViews.buttons["Chain"].firstMatch.tap()
        let last = app.webViews.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "#verdict eth_chainId")
        ).firstMatch
        XCTAssertTrue(last.waitForExistence(timeout: 20), "the page's read was never answered")
        attach(app.screenshot(), named: "live-01-page")

        // One tap to the wallet.
        app.buttons["钱包"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 1.5)
        attach(app.screenshot(), named: "live-02-wallet")

        // 探索: the HOME, the dApp waiting in its resume row — the first
        // row, the tab used last. (The space keeps tabs across runs, and a
        // launch URL opens a new tab beside a restored one, so the header's
        // number is whatever the strip holds.)
        app.buttons["探索"].firstMatch.tap()
        XCTAssertTrue(openTabsHeader(app).waitForExistence(timeout: 10),
                      "探索 from the wallet did not land on the Explore home")
        XCTAssertFalse(app.webViews.staticTexts["Vela test dApp"].exists, "探索 landed inside the dApp")
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5), "no resume row for the open dApp")
        attach(app.screenshot(), named: "live-03-home")

        // One tap: the page as it was left.
        row.tap()
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 10))
        XCTAssertTrue(
            app.webViews.staticTexts.containing(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict eth_chainId")
            ).firstMatch.waitForExistence(timeout: 5),
            "the page was reloaded: its read's answer is gone"
        )
        attach(app.screenshot(), named: "live-04-resumed")

        // 探索 while browsing: the home again, the tab kept.
        app.buttons["探索"].firstMatch.tap()
        XCTAssertTrue(openTabsHeader(app).waitForExistence(timeout: 10),
                      "探索 while browsing did not return to the home")
        attach(app.screenshot(), named: "live-05-home-again")

        // And ‹ with no history is the home too, still not reloading anything.
        app.buttons.matching(identifier: "explore.resume.row").firstMatch.tap()
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 10))
        XCTAssertTrue(
            app.webViews.staticTexts.containing(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict eth_chainId")
            ).firstMatch.waitForExistence(timeout: 5),
            "the page was reloaded on the second resume"
        )
        app.buttons["explore.bar.back"].tap()
        XCTAssertTrue(openTabsHeader(app).waitForExistence(timeout: 10),
                      "‹ with no history did not return to the home")
        app.terminate()
    }

    /// The resume section's header, "已打开 {{n}} 个标签页", whatever n is.
    private func openTabsHeader(_ app: XCUIApplication) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@ AND label ENDSWITH %@",
                                             "已打开", "个标签页")).firstMatch
    }

    // MARK: - Plumbing

    /// The gallery's Explore page on `state`, outside the parallel space.
    private func gallery(_ state: String, lang: String, theme: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_PAGE"] = "explore"
        app.launchEnvironment["VELA_STATE"] = state
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(\(lang))"]
        app.launch()
        return app
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
