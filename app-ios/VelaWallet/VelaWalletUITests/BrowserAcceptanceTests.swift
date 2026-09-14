//
//  BrowserAcceptanceTests.swift
//  VelaWalletUITests
//
//  Spec 053, on the device. The browser is the first surface in this program
//  that runs code the wallet did not write, so every claim here is read off a
//  real page rather than off a model.
//
//  ## How a page is read
//
//  XCUITest sees a `WKWebView` through the accessibility tree, which means it
//  sees what was **painted**. JavaScript state nobody rendered is invisible.
//  So the harness page prints one `#verdict <name> <value>` line per outcome,
//  and these tests wait for those lines.
//
//  ## Where the page comes from
//
//  `LocalDappServer`, inside this runner, on `127.0.0.1:8137`. Not `file://`:
//  `dapp_origin_of` gives a file URL no origin, so the permissions machine
//  would refuse everything and a green run would mean nothing.
//

import XCTest

final class BrowserAcceptanceTests: XCTestCase {

    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = false
        let server = try LocalDappServer(html: try LocalDappServer.page())
        server.start()
        self.server = server
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    /// The app, inside the parallel space, with the harness page open.
    ///
    /// The space rather than the founder's own wallet because everything after
    /// phase 1 signs something, and a device suite that needs a finger on every
    /// run is a suite nobody runs.
    private func launchBrowsing() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// Wait for a `#verdict` line whose text contains `fragment`.
    @discardableResult
    private func waitForVerdict(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> Bool {
        let predicate = NSPredicate(format: "label CONTAINS %@", fragment)
        let element = app.webViews.staticTexts.containing(predicate).firstMatch
        return element.waitForExistence(timeout: timeout)
    }

    // MARK: - US1: a real page, with the wallet inside it

    /// 探索 loads a real page over the network, and the page finds this wallet
    /// before its own scripts run.
    ///
    /// Three claims in one pass, because they fail together: the engine is a
    /// real `WKWebView` (a page renders), the provider was injected into the
    /// page's own content world (`window.ethereum` exists at all), and the
    /// bundled bytes are the extension's (the announcement carries Vela's real
    /// name and reverse-domain id).
    ///
    /// The content world is the part worth stating. A `WKUserScript` defaults
    /// to an ISOLATED world, where a `window.ethereum` runs, errors nowhere,
    /// and is invisible to every dApp on earth.
    func testTheExploreTabRunsARealPageThatFindsTheWallet() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30),
                      "the space must be open — everything after this signs something")

        app.buttons["探索"].firstMatch.tap()

        // The page's own <h1>, painted by WebKit rather than by SwiftUI.
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30),
                      "no page rendered — the engine never loaded, or the URL never reached it")

        XCTAssertTrue(waitForVerdict(app, containing: "#verdict announce Vela Wallet app.getvela"),
                      "the page did not hear the discovery announcement: the provider is missing, or it was injected into an isolated content world where no dApp can see it")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict legacy present isVela=true"),
                      "window.ethereum is not the wallet's, or not there at all")

        attach(app.screenshot(), named: "device-browser-announce")
    }

    /// The address bar shows the page's own host, and the padlock tells the
    /// truth about the scheme.
    ///
    /// The harness is served over http on loopback, so the padlock must be
    /// ABSENT here. A browser chrome that claimed a lock it did not have would
    /// be the single most dangerous thing in this cut.
    func testTheAddressBarShowsThePagesOwnHost() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        app.buttons["探索"].firstMatch.tap()

        XCTAssertTrue(app.staticTexts["127.0.0.1:8137"].waitForExistence(timeout: 30),
                      "the address bar shows a host the page is not on")
        XCTAssertFalse(app.images["explore.secureSite"].exists,
                       "an http page must not be drawn with a padlock")

        attach(app.screenshot(), named: "device-browser-address-bar")
    }

    // MARK: - US2: the browser remembers

    /// A pinned site and a visited one survive the app being killed.
    ///
    /// Both halves matter and they come from **different machines**:
    /// `explore_sites` keeps the favourite and `browser_history` keeps the
    /// visit, in two documents, with two different rules about what an
    /// unreadable one means. A test that checked only one would pass while the
    /// other silently wrote nothing.
    ///
    /// The visit half is also the proof that the controller's wait works: that
    /// machine has no `ready` flag, and a visit recorded before its store
    /// answers is dropped without a word.
    func testFavouritesAndRecentsSurviveAForceQuit() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        app.buttons["探索"].firstMatch.tap()
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        // The star in the browser toolbar.
        let star = app.buttons["explore.addToFavorites"].firstMatch
        if star.waitForExistence(timeout: 10) {
            star.tap()
        } else {
            // Labelled by its corpus string rather than an identifier on this
            // build — either is fine, and failing here would be failing on the
            // wrong thing.
            app.buttons["添加到收藏"].firstMatch.tap()
        }
        attach(app.screenshot(), named: "device-browser-favourited")

        app.terminate()

        // Relaunch WITHOUT a URL. Anything that comes back came off the disk.
        let again = XCUIApplication()
        again.launchEnvironment["VELA_LANG"] = "zh"
        again.launchEnvironment["VELA_THEME"] = "dark"
        again.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        again.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        again.launchArguments += ["-AppleLanguages", "(zh)"]
        again.launch()
        XCTAssertTrue(again.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        again.buttons["探索"].firstMatch.tap()

        // **The tab came back, with its page.** That is what a browser does,
        // and it is the first half of the memory: the strip is in
        // `explore_sites`' document, so the engine is rebuilt and the URL
        // reloaded with nothing asking it to.
        XCTAssertTrue(again.staticTexts["127.0.0.1:8137"].waitForExistence(timeout: 30),
                      "the open tab did not survive the relaunch")
        attach(again.screenshot(), named: "device-browser-tab-restored")

        // Leave the page for the start page. The tab stays; only the view
        // changes.
        again.buttons["关闭网页"].firstMatch.tap()

        XCTAssertTrue(again.staticTexts["最近的 dApp"].waitForExistence(timeout: 20),
                      "the recents section is missing — the visit was never recorded, or it was recorded before the history store answered and was dropped")
        XCTAssertTrue(again.staticTexts["收藏"].waitForExistence(timeout: 10),
                      "the favourites section is missing — nothing was pinned")
        // The host appears in BOTH sections; one match is enough to prove the
        // two documents came back.
        XCTAssertTrue(again.staticTexts["127.0.0.1:8137"].firstMatch.waitForExistence(timeout: 10),
                      "neither the favourite nor the recent survived the relaunch")
        attach(again.screenshot(), named: "device-browser-remembered")
    }
}
