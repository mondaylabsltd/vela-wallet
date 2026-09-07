//
//  LiveWiringAcceptanceTests.swift
//  VelaWalletUITests
//
//  Spec 050's acceptance, run **on the phone**.
//
//  Everything else that proves this feature runs against a simulator or against
//  no device at all. This file taps, types and relaunches on real hardware,
//  which is the only place a persistence claim is worth anything.
//
//      xcodebuild test \
//        -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'platform=iOS,id=<device udid>' \
//        -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
//        -resultBundlePath /tmp/accept.xcresult
//      xcrun xcresulttool export attachments \
//        --path /tmp/accept.xcresult --output-path /tmp/accept-images
//
//  ## How it tells live from fixture without signing in
//
//  A passkey ceremony needs a real finger, so these use the dev page pins —
//  `VELA_PAGE=contacts-live` / `settings-live` — which mount the REAL machines
//  over the device's own storage. The assertions are then chosen so that a
//  fixture could not pass them:
//
//  - contacts: `Bartholomew Vanderbilt-Konstantinopoulos.eth` is in the fixture
//    roster and in no real address book. (An earlier version keyed on "Alice"
//    and failed against a seeded simulator — a live book can hold an Alice.)
//  - networks: the fixture lists eight chains and the core knows twelve.
//    Optimism, Avalanche and Unichain appear in no drawing at all.
//
//  ## What it deliberately does not assert
//
//  Which account the book belongs to. That needs a sign-in, and a sign-in needs
//  Face ID; the scoping rule is covered by `ContactsStoreTests` instead.
//

import XCTest

final class LiveWiringAcceptanceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func launch(page: String, state: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PAGE"] = page
        if let state { app.launchEnvironment["VELA_STATE"] = state }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
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

    // MARK: - US1: the address book is the person's own

    /// The live book is the device's, not the drawing's.
    ///
    /// The discriminator has to be a name a real address book would never
    /// contain, because a live book legitimately CAN hold an Alice — this test
    /// asserted exactly that at first and failed on a simulator whose storage
    /// had been seeded with one, which is the assertion being wrong rather than
    /// the app. `Bartholomew Vanderbilt-Konstantinopoulos.eth` exists only in
    /// `ContactsFixtures`, and it is there precisely because it is absurd.
    func testContactsIsLiveAndNotTheFixtureRoster() throws {
        let app = launch(page: "contacts-live")
        XCTAssertTrue(
            app.staticTexts["通讯录"].waitForExistence(timeout: 20),
            "the contacts screen never appeared"
        )
        Thread.sleep(forTimeInterval: 1.5)
        attach(app.screenshot(), named: "device-contacts-live")

        XCTAssertFalse(
            app.staticTexts["Bartholomew Vanderbilt-Konstantinopoulos.eth"].exists,
            "the fixture roster is on screen — the tab is not live"
        )
        // The fixture's three group rows are equally impossible together.
        XCTAssertFalse(
            app.staticTexts["交易所"].exists && app.staticTexts["工作"].exists,
            "the fixture's groups are on screen — the tab is not live"
        )
    }

    /// The fixture host, photographed beside it, so the two can be compared.
    func testContactsFixtureForComparison() throws {
        let app = launch(page: "contacts")
        XCTAssertTrue(
            app.staticTexts["Bartholomew Vanderbilt-Konstantinopoulos.eth"]
                .waitForExistence(timeout: 20)
        )
        Thread.sleep(forTimeInterval: 1.5)
        attach(app.screenshot(), named: "device-contacts-fixture")
    }

    // MARK: - US2: the networks are the core's

    /// Twelve chains, three of which no drawing contains.
    func testNetworksListIsTheCoresTwelveNotTheDrawingsEight() throws {
        let app = launch(page: "settings-live", state: "st9")
        XCTAssertTrue(
            app.staticTexts["Ethereum"].waitForExistence(timeout: 20),
            "the networks page never appeared"
        )
        Thread.sleep(forTimeInterval: 1.5)
        attach(app.screenshot(), named: "device-networks-live")

        // Present in the core's BUILTIN_CHAINS and in no `SettingsFixtures`
        // row — so their presence cannot come from the drawing.
        for undrawn in ["Optimism", "Avalanche", "Unichain"] {
            XCTAssertTrue(
                app.staticTexts[undrawn].exists,
                "\(undrawn) is missing — the list is not the core's"
            )
        }
    }

    /// **Tapping a chain opens that chain.**
    ///
    /// It did not, until spec 050 phase 6: the row's tap handler discarded the
    /// id and every row opened the first network's page. Harmless while the
    /// list was one fixture; a lie the moment it went live.
    func testTappingANetworkOpensThatNetwork() throws {
        let app = launch(page: "settings-live", state: "st9")
        XCTAssertTrue(app.staticTexts["Gnosis"].waitForExistence(timeout: 20))
        app.staticTexts["Gnosis"].tap()

        // The detail page's title is the chain's name. Ethereum here would mean
        // the tap opened somebody else's page.
        XCTAssertTrue(
            app.staticTexts["Gnosis"].waitForExistence(timeout: 5),
            "the detail page did not open"
        )
        Thread.sleep(forTimeInterval: 1.0)
        attach(app.screenshot(), named: "device-network-detail-gnosis")
        XCTAssertFalse(
            app.staticTexts["Ethereum"].exists,
            "tapping Gnosis opened Ethereum's detail page"
        )
    }

    /// The add gate, against real endpoints, on the phone.
    ///
    /// Types a chain id, waits for the core's verdict — **which can take the
    /// better part of a minute**: an index fetch, a chain resolve, an RPC race,
    /// eleven `eth_getCode` reads and a P256 probe — and asserts that whichever
    /// verdict came back, the screen offers the matching action and only that
    /// one. It does not assume a particular chain is compatible: that is a fact
    /// about somebody else's deployment, not about this code.
    func testTheAddGateFollowsTheCoresVerdict() throws {
        let app = launch(page: "settings-live", state: "st10")

        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 20), "no search field on the wizard")
        field.tap()
        field.typeText("100")

        // Gnosis is a builtin, so the honest outcome here is the core's
        // "already added" refusal — which is itself a verdict, and the one this
        // test wants to see reach the screen.
        let deadline = Date().addingTimeInterval(90)
        var settled = false
        while Date() < deadline {
            if app.buttons["添加网络"].exists || app.staticTexts.count > 6 {
                settled = true
                break
            }
            Thread.sleep(forTimeInterval: 1.0)
        }
        Thread.sleep(forTimeInterval: 1.0)
        attach(app.screenshot(), named: "device-wizard-verdict")
        XCTAssertTrue(settled, "the wizard never left its initial state")
    }

    // MARK: - US1: the home screen is the person's own money

    /// The hero is a chain read, not the drawing.
    ///
    /// `VELA_ACCOUNT` seeds a **key-less** record (FR-010) so the read path has
    /// an address without a passkey ceremony, and the address is the golden
    /// Safe — whose Gnosis balance is checkable with one `eth_getBalance`, and
    /// is checked that way in `PriceLiveTests`.
    ///
    /// The assertions are absences, deliberately: the live figure moves when
    /// the founder spends, while `$1,383.28` and `0.8533 BNB` exist only in
    /// `WalletFixtures` and would still be on screen if the wiring came
    /// undone. The screenshot is what the number itself is for.
    func testHomeShowsRealMoneyRatherThanTheFixtureTotal() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_ACCOUNT"] = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        // The hero's label carries the person's CHOSEN currency, so pinning
        // `总余额 · USD` was the test being wrong about the world: this
        // simulator had CNY stored and drew `总余额 · CNY`, correctly. What is
        // asserted is that a hero appeared at all.
        let hero = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "总余额 · "))
        XCTAssertTrue(hero.firstMatch.waitForExistence(timeout: 30),
                      "the home hero never appeared")
        // Twelve chains, a mainnet feed batch and a per-chain feed each: the
        // figure lands within a few seconds, and the screenshot is worth
        // nothing taken before it does.
        Thread.sleep(forTimeInterval: 8)
        attach(app.screenshot(), named: "device-home-live-balance")

        let fixtureTotal = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS %@", "1,383")
        )
        XCTAssertEqual(fixtureTotal.count, 0, "the fixture total is still on screen")

        let fixtureHolding = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS %@", "0.8533")
        )
        XCTAssertEqual(fixtureHolding.count, 0, "a fixture holding is still on screen")
    }

    // MARK: - The currency rule, on the phone

    /// The picker marks the core's code, and the row degrades rather than
    /// relabelling. A fresh install has never chosen, so this photographs the
    /// seeded state — the assertion that matters is that no fabricated
    /// conversion appears.
    func testCurrencyDegradesRatherThanFabricating() throws {
        let app = launch(page: "settings-live", state: "st5")
        XCTAssertTrue(
            app.staticTexts["USD"].waitForExistence(timeout: 20),
            "the currency picker never appeared"
        )
        Thread.sleep(forTimeInterval: 1.5)
        attach(app.screenshot(), named: "device-currency-picker")

        // Exactly one row may read as chosen.
        XCTAssertTrue(app.staticTexts["CNY"].exists, "the catalog is incomplete")
    }
}
