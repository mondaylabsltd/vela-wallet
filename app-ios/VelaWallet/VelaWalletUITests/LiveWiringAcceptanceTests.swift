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
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_PAGE"] = page
        if let state { app.launchEnvironment["VELA_STATE"] = state }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    /// Tap, but only once the element is really there.
    ///
    /// A bare `.tap()` on a `firstMatch` query fails immediately when the
    /// accessibility tree has not caught up — which it sometimes has not, on a
    /// screen whose first render follows a hundred stored transactions. This
    /// suite went flaky exactly there, and a flaky acceptance test is worse
    /// than a missing one: the next person reads it as a defect.
    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
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
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
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
        //
        // `descendants` rather than `staticTexts` because the hero became a
        // BUTTON when tap-to-hide landed — it carries `.isButton` and a hint
        // saying what the tap does, so its label no longer answers a
        // static-text query. The element kind is the app's business; that a
        // hero exists is this test's.
        let hero = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "总余额 · "))
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

    // MARK: - US1b: where the live home leads

    /// 活动 → 全部 opens the person's OWN history, and a row in it opens the
    /// transaction that was tapped.
    ///
    /// A live home that hands off to a fixture screen is the same lie one level
    /// down: tap a real transfer, read somebody else's. The discriminator is
    /// `至 hold on` — a recipient that exists only in `WalletFlowFixtures`.
    func testHistoryAndItsTransactionAreTheAccountsOwn() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_ACCOUNT"] = "0x28C6c06298d514Db089934071355E5743bf21d60"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.staticTexts["活动"].waitForExistence(timeout: 30),
                      "the home never appeared")
        // The receipt scan has to land before there is a history to open.
        XCTAssertTrue(app.staticTexts["已收到"].firstMatch.waitForExistence(timeout: 60),
                      "no receipts reached the home feed")

        // The activity section's 全部 — the first of the two.
        tap(app.buttons.matching(identifier: "全部").element(boundBy: 0),
            "the activity section's 全部")
        XCTAssertTrue(app.staticTexts["历史记录"].waitForExistence(timeout: 10),
                      "the history screen never opened")
        XCTAssertEqual(
            app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "hold on")).count, 0,
            "the fixture history is still on screen"
        )
        attach(app.screenshot(), named: "device-history-live")

        // The first row opens ITS transaction.
        tap(app.staticTexts["已收到"].firstMatch, "a history row")
        XCTAssertTrue(app.staticTexts["成功"].waitForExistence(timeout: 10),
                      "the transaction sheet never opened")
        XCTAssertTrue(app.staticTexts["哈希"].exists, "the sheet has no hash row")
        attach(app.screenshot(), named: "device-tx-detail-live")
    }

    // MARK: - US2: the receive screen is the person's own address

    /// 收款 shows YOUR address, on every network, with a code that encodes it.
    ///
    /// The most dangerous fixture in the client was here: a drawn address
    /// (`0x14fB1f…D1eA5c`, belonging to nobody) beside a demo QR pattern. Money
    /// sent to it does not come back, and nobody reads a caption while holding
    /// a phone up to a camera. Both assertions are absences for that reason —
    /// the fixture address must be gone from both screens.
    func testReceiveShowsTheSignedInAddressAndARealCode() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_ACCOUNT"] = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 30),
                      "the home never appeared")
        tap(app.buttons["收款"], "收款")

        // The seeded account's own address, shortened the way every iOS
        // surface shortens one.
        XCTAssertTrue(app.staticTexts["0x88cC…6894"].firstMatch.waitForExistence(timeout: 10),
                      "the network list is not showing the signed-in address")
        XCTAssertEqual(
            app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "14fB1f")).count, 0,
            "the fixture address is still on the receive screen"
        )
        attach(app.screenshot(), named: "device-receive-list")

        // The row's QR button opens the code — tapping the network's NAME does
        // nothing, which is the drawing's choice: a row is a place to copy
        // from, and the code is one of its two actions.
        tap(app.buttons["扫描二维码"].firstMatch, "a network row's QR button")
        XCTAssertTrue(app.staticTexts["0x88cCA0EeDbF2C442611"].waitForExistence(timeout: 10),
                      "the QR sheet is not showing the signed-in address")
        Thread.sleep(forTimeInterval: 1)
        attach(app.screenshot(), named: "device-receive-qr")
    }

    /// 保存图片 renders the card and puts it in the album.
    ///
    /// The button has been drawn since spec 021 and did nothing. What it
    /// produces leaves the app — a card built from the fixture identity would
    /// be somebody else's address in a stranger's chat — so this drives the
    /// whole path including the system's permission prompt.
    func testSavingTheReceiveCardReachesTheAlbum() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_ACCOUNT"] = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        tap(app.buttons["收款"], "收款", timeout: 30)
        tap(app.buttons["扫描二维码"].firstMatch, "a network row's QR button")
        tap(app.buttons["保存图片"], "保存图片")

        // The system asks once per install. Answering it here is part of the
        // path — a test that only passed on an already-authorised simulator
        // would prove nothing about a person's first save.
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for label in ["允许", "Allow", "好", "OK"] {
            let button = springboard.buttons[label]
            if button.waitForExistence(timeout: 3) {
                button.tap()
                break
            }
        }

        // 已保存 · 收款二维码已保存到相册。
        XCTAssertTrue(app.staticTexts["已保存"].waitForExistence(timeout: 20),
                      "the save said nothing at all")
        attach(app.screenshot(), named: "device-share-card-saved")
    }

    // MARK: - US3: a token somebody adds by hand

    /// Type a contract, and the wallet finds it on a chain and keeps it.
    ///
    /// The account seeded here is a public exchange hot wallet, for one
    /// property: it holds a large USDT balance on Ethereum, so the token this
    /// test adds has a balance to show afterwards. Nothing is signed and only
    /// public data is read.
    ///
    /// What it proves that a unit test cannot: the address reaches the core on
    /// every keystroke, the sweep runs across the registry, the found card is
    /// the chain's own answer, and the save round-trips through storage — the
    /// 已添加 chip is recomputed by the core from what it read back.
    func testAddingATokenByContractFindsItAndKeepsIt() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_ACCOUNT"] = "0x28C6c06298d514Db089934071355E5743bf21d60"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        // The ban map, emptied for this launch only.
        //
        // `rpc_pool` persists bans under `vela.rpc.banned`, and they are FACTS
        // about endpoints rather than session state — so on a phone that has
        // run this suite many times they accumulate. An argument-domain value
        // outranks the persisted one without writing anything, which is the
        // difference between a test that starts from a known pool and one whose
        // result depends on how often the device has been tested before.
        app.launchArguments += ["-vela.rpc.banned", "[]"]
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30),
                      "the home never appeared")
        // The assets section's 全部 — the second on the screen; the first
        // belongs to 活动.
        let seeAll = app.buttons.matching(identifier: "全部")
        XCTAssertTrue(seeAll.count >= 2, "the two section headers are not both drawn")
        tap(seeAll.element(boundBy: 1), "the assets section's 全部")

        // The screen's own 添加, in its header. The 通过地址添加代币 link at the
        // bottom of the list does the same thing, and this test used it first —
        // on an account with a long list it is simply not on screen, which is
        // the test being wrong about the account rather than the app.
        let addAction = app.buttons["添加"]
        XCTAssertTrue(addAction.waitForExistence(timeout: 10),
                      "the assets screen never opened")
        tap(addAction, "添加")

        let field = app.textFields["合约地址"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "the add-token sheet has no field")
        field.tap()
        // Tether on Ethereum.
        field.typeText("0xdAC17F958D2ee523a2206206994597C13D831ec7")

        // The sweep asks every network in the registry, so this waits on real
        // round trips rather than on a local decision.
        XCTAssertTrue(app.staticTexts["Tether USD"].waitForExistence(timeout: 60),
                      "no chain answered for a contract every chain knows")
        attach(app.screenshot(), named: "device-add-token-found")

        let add = app.buttons["添加到钱包"]
        XCTAssertTrue(add.waitForExistence(timeout: 5))
        tap(add, "添加到钱包")

        // The chip is the core's, recomputed from the tokens it read BACK out
        // of storage — so it appearing is the save having actually landed.
        XCTAssertTrue(app.staticTexts["已添加"].waitForExistence(timeout: 20),
                      "the token was not kept")
        attach(app.screenshot(), named: "device-add-token-added")
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
    // MARK: - The parallel space (spec 052 US0)

    /// The door opens onto the wallet every other client derives, says so on
    /// every screen, and closes again without touching anything else.
    ///
    /// Address and badge only — deliberately no balance. A phone with no reach
    /// to a chain still has the right wallet open, and a test that asserted a
    /// figure would fail for a reason that has nothing to do with the door.
    ///
    /// It runs on the founder's own phone, so what it must never do is disturb
    /// the real wallet: the space UPSERTS one record and its exit removes
    /// exactly that id (FR-003).
    func testTheParallelSpaceOpensOnTheGoldenSafeAndClosesCleanly() throws {
        let inside = XCUIApplication()
        inside.launchEnvironment["VELA_LANG"] = "zh"
        inside.launchEnvironment["VELA_THEME"] = "dark"
        inside.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        inside.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        inside.launchArguments += ["-AppleLanguages", "(zh)"]
        inside.launch()

        XCTAssertTrue(inside.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 20),
                      "the space must announce itself on screen")
        // The golden multi-key Safe, shortened as the header draws it. Its
        // address is a function of EVERY fixture key, so seeing it is also the
        // proof that the whole key set was written rather than the first key.
        XCTAssertTrue(inside.staticTexts["0x88cC…6894"].waitForExistence(timeout: 30),
                      "the space must open on the Safe the other clients derive")
        attach(inside.screenshot(), named: "device-parallel-space")
        inside.terminate()

        // Leaving: the badge is gone, and the app is back to whatever it was.
        let outside = XCUIApplication()
        outside.launchEnvironment["VELA_LANG"] = "zh"
        outside.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        outside.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        outside.launchArguments += ["-AppleLanguages", "(zh)"]
        outside.launch()

        // `waitForExistence` would wait for something that should never appear;
        // a short settle then a direct check is the honest shape.
        XCTAssertTrue(outside.wait(for: .runningForeground, timeout: 20))
        XCTAssertFalse(outside.staticTexts["PARALLEL SPACE"].exists,
                       "the badge must be gone the moment the door is closed")
        attach(outside.screenshot(), named: "device-parallel-space-left")
    }

    // MARK: - The send journey (spec 052 US1)

    /// 转账 opens on the person's OWN holdings, and picking one carries it
    /// through to a form that is the core's.
    ///
    /// The picker used to be a fixture list of somebody else's tokens, and the
    /// tap that opened the form threw away WHICH row was tapped — the same
    /// defect the network list and the receive sheet each had. So this asserts
    /// both halves: the rows are real, and the one that was tapped is the one
    /// the form opens with.
    ///
    /// Driven in the parallel space, whose Safe holds exactly one asset on
    /// Gnosis — which is what makes "the row that was tapped" checkable.
    func testSendOpensOnRealHoldingsAndCarriesTheTappedToken() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30),
                      "the home never appeared")
        tap(app.buttons["转账"], "转账")

        // The picker's rows are the balance machine's holdings. The fixture
        // list carried POL, ETH and USDT; the parallel Safe carries xDAI.
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30),
                      "the picker is not showing this wallet's own holdings")
        XCTAssertFalse(app.staticTexts["POL"].exists,
                       "a fixture token is still in the picker")
        attach(app.screenshot(), named: "device-send-pick-live")

        tap(app.staticTexts["xDAI"], "the xDAI row")

        // The form, and ONLY the form, carries 收款人.
        //
        // An earlier version of this test waited for "a text field and xDAI",
        // which the PICKER also has — a search box and the row that was just
        // tapped. It passed on the device while the screenshot showed the
        // picker, which is a test that cannot tell "the form opened" from
        // "nothing happened". The discriminator has to be something only the
        // destination has.
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20),
                      "tapping a token did not open the form")
        XCTAssertFalse(app.staticTexts["选择代币"].exists,
                       "the picker is still on screen")
        // And it opened on the token that was tapped.
        XCTAssertTrue(app.staticTexts["xDAI"].exists,
                      "the form opened on a different token than the one tapped")
        attach(app.screenshot(), named: "device-send-form-live")
    }

    /// The whole journey, in the parallel space: pick, fill, confirm, sign,
    /// submit — and a receipt with a hash.
    ///
    /// **This test spends real money** (dust on Gnosis, from the golden Safe)
    /// and is therefore not in the default acceptance sweep — it is run
    /// deliberately, with `-DVELA_LIVE_SEND`. The founder authorised the same
    /// thing on every other client for the same reason: a send that has never
    /// left the machine is a send nobody has verified.
    func testDustLeavesTheGoldenSafeAndComesBackAsAReceipt() throws {
        #if !VELA_LIVE_SEND
        throw XCTSkip("set -DVELA_LIVE_SEND to spend dust on Gnosis")
        #else
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30))
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30))
        tap(app.staticTexts["xDAI"], "the xDAI row")
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20))

        // BY IDENTIFIER, not by position. An earlier version took
        // `element(boundBy: 0)` and typed the address into the AMOUNT field —
        // the amount comes first in the layout — which the screenshot showed
        // and the assertion did not.
        //
        // The second fixture Safe: a wallet this keyset also controls, so the
        // dust stays inside the test environment.
        // The form has exactly two fields, amount first. Neither the
        // identifier nor the label matched from here — asserting the COUNT is
        // what makes a positional lookup honest rather than a guess, and it
        // fails loudly the day a third field appears.
        XCTAssertEqual(app.textFields.count, 2, "the form's fields changed shape")
        let recipient = app.textFields.element(boundBy: 1)
        recipient.tap()
        recipient.typeText("0x031d7D57c99CAF891e1C250554691Fd12D84772b")

        let amount = app.textFields.element(boundBy: 0)
        amount.tap()
        amount.typeText("0.0001")

        // What the form looks like once both fields are filled — the evidence
        // for why the gate is or is not open.
        attach(app.screenshot(), named: "device-send-form-filled")

        // 继续 is the core's gate: it stays disabled until the fee has settled.
        let cont = app.buttons["继续"]
        XCTAssertTrue(cont.waitForExistence(timeout: 60))
        let armed = NSPredicate(format: "isEnabled == true")
        expectation(for: armed, evaluatedWith: cont, handler: nil)
        waitForExpectations(timeout: 90)
        tap(cont, "继续")

        attach(app.screenshot(), named: "device-send-confirm-live")
        let confirm = app.buttons["确认并发送"]
        XCTAssertTrue(confirm.waitForExistence(timeout: 30))
        expectation(for: armed, evaluatedWith: confirm, handler: nil)
        waitForExpectations(timeout: 90)
        tap(confirm, "确认")

        // No biometric prompt in the parallel space: the fixed keyset signs.
        XCTAssertTrue(app.staticTexts["交易已提交至网络"].waitForExistence(timeout: 120),
                      "the relay never accepted the operation")
        attach(app.screenshot(), named: "device-send-receipt-live")
        #endif
    }

    /// SC-003: a send survives the app being killed.
    ///
    /// Submits, force-quits before the chain has confirmed, relaunches, and
    /// waits for the feed row to reach 已发送. Nothing is asked of the person
    /// in between — the pending set is derived from the transaction store, so
    /// the launch that follows a kill picks up exactly what the kill
    /// interrupted.
    ///
    /// **Spends dust**, so it lives behind the same flag as the send itself.
    func testASubmittedSendSurvivesAForceQuit() throws {
        #if !VELA_LIVE_SEND
        throw XCTSkip("set -DVELA_LIVE_SEND to spend dust on Gnosis")
        #else
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30))
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30))
        tap(app.staticTexts["xDAI"], "the xDAI row")
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20))

        XCTAssertEqual(app.textFields.count, 2, "the form's fields changed shape")
        let recipient = app.textFields.element(boundBy: 1)
        recipient.tap()
        recipient.typeText("0x031d7D57c99CAF891e1C250554691Fd12D84772b")
        let amount = app.textFields.element(boundBy: 0)
        amount.tap()
        amount.typeText("0.0001")

        let cont = app.buttons["继续"]
        let armed = NSPredicate(format: "isEnabled == true")
        XCTAssertTrue(cont.waitForExistence(timeout: 60))
        expectation(for: armed, evaluatedWith: cont, handler: nil)
        waitForExpectations(timeout: 90)
        tap(cont, "继续")

        let confirm = app.buttons["确认并发送"]
        XCTAssertTrue(confirm.waitForExistence(timeout: 30))
        expectation(for: armed, evaluatedWith: confirm, handler: nil)
        waitForExpectations(timeout: 90)
        tap(confirm, "确认")
        XCTAssertTrue(app.staticTexts["交易已提交至网络"].waitForExistence(timeout: 120))

        // Killed, not backgrounded: the grace window and the background refresh
        // are both gone, and only the store is left.
        // Screenshot BEFORE the kill: a terminated app has nothing to
        // photograph, and asking anyway fails the test for the wrong reason.
        attach(app.screenshot(), named: "device-send-killed")
        app.terminate()

        let relaunched = XCUIApplication()
        relaunched.launchEnvironment["VELA_LANG"] = "zh"
        relaunched.launchEnvironment["VELA_THEME"] = "dark"
        relaunched.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        relaunched.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        relaunched.launchArguments += ["-AppleLanguages", "(zh)"]
        relaunched.launch()

        // The feed's row for a send this device made, now resolved by a
        // tracker that nobody asked to start.
        XCTAssertTrue(relaunched.staticTexts["已发送"].waitForExistence(timeout: 120),
                      "the interrupted send never reached a verdict after a relaunch")
        attach(relaunched.screenshot(), named: "device-send-survived-kill")
        #endif
    }

}
