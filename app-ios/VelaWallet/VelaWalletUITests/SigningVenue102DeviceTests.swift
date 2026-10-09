//
//  SigningVenue102DeviceTests.swift
//  VelaWalletUITests
//
//  Spec 102's integration walk on a phone: what the owner sees of "where you
//  review and sign", photographed on the device itself. Skipped in the scheme:
//  it reaches the network (the official page's live check) and walks the
//  parallel space, which a CI leg should not.
//
//      xcodebuild test-without-building -xctestrun <copy with the skip removed> \
//        -destination 'platform=iOS,id=<hw-udid>' \
//        -only-testing:VelaWalletUITests/SigningVenue102DeviceTests/<test>
//
//  Run one test at a time, in order; `test9` puts the phone back. Entering
//  the space re-writes its fixture record on every launch — the venue with
//  it — so a test that needs the page venue chooses it in its own launch.
//
//  The owner's wallet is on the phone, so the rules are the walk's own:
//
//  - Everything that touches an account happens INSIDE the parallel space,
//    on its fixture Safe — never on the owner's accounts.
//  - Nothing is confirmed, signed or sent. A send stops at its confirm; a
//    site's request is declined; the signing page, when it is opened at all
//    (`VELA_OPEN_PAGE=1`, only once Vela's page build is live), is closed
//    without signing.
//  - No language or theme is stored: `VELA_LANG` and `-AppleLanguages` pin
//    them for the launch only.
//  - `test0` photographs the phone as it was (`VELA_WAIT` seconds after
//    launch, 8 by default); `test9` puts the venue back to In Vela, leaves
//    the space, and photographs the phone again; `test9b` closes the browser
//    tab the walk's test page opened.
//  - `testSelfHostedPageOnTheSimulator` is the self-hosted half (add, trust,
//    rename, refused venue, remove, added again → asked again): it needs the
//    Mac's loopback, so a simulator only, with a `dist/` copy of a build of
//    its own served on `localhost:8140`.
//

import XCTest

final class SigningVenue102DeviceTests: XCTestCase {

    private var server: LocalDappServer?

    /// The parallel space's fixture Safe — the account's OWN address, the
    /// only recipient a send here is ever started to.
    private static let ownSafe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    // MARK: - The phone as it was

    /// No pin, no space change: whatever account, space and language the
    /// owner left the app in — home, then Settings.
    func test0RecordsThePhoneAsItIs() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        Thread.sleep(forTimeInterval: Double(ProcessInfo.processInfo.environment["VELA_WAIT"] ?? "8") ?? 8)
        attach("00-before-home")
        printTexts(app, "BEFORE-HOME")
        openSettings(app)
        Thread.sleep(forTimeInterval: 2)
        attach("01-before-settings")
        printTexts(app, "BEFORE-SETTINGS")
        app.swipeUp()
        Thread.sleep(forTimeInterval: 1)
        attach("02-before-settings-lower")
        app.terminate()
    }

    // MARK: - 1. Settings → Signing pages (official, live)

    func test1SigningPages() throws {
        let app = launchInSpace()
        openSettings(app)
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 10), "Settings did not open")
        openAdvancedRow("签名页", in: app)
        let official = app.descendants(matching: .any)["signingPage.row.sign.getvela.app"].firstMatch
        XCTAssertTrue(official.waitForExistence(timeout: 10), "Signing pages did not open")
        attach("10-signing-pages-first-look")
        waitForAVerdict(in: app, timeout: 30)
        attach("11-signing-pages-checked")
        let line = verdictText(in: app)
        print("OFFICIAL-LINE", line ?? "none")
        XCTAssertNotNil(line, "the official page was never checked")
        XCTAssertTrue(app.staticTexts["Vela 官方签名页"].exists, "the official row is not named")
        app.terminate()
    }

    // MARK: - 1b. A self-hosted page (simulator: the Mac's localhost)

    /// `http://localhost:8140/` — a copy of `dist/` with a build of its own
    /// (one byte changed, so its hash is new to Vela), served on the Mac,
    /// which is the simulator's localhost. Added, asked about, trusted,
    /// renamed; the account's venue sheet refuses it with the reason; then
    /// removed. A phone cannot reach the Mac's loopback, so: simulator only.
    func testSelfHostedPageOnTheSimulator() throws {
        try XCTSkipUnless(DeviceSafety.onSimulator, "the page is on the Mac's localhost")
        let app = launchInSpace()
        openSettings(app)
        openAdvancedRow("签名页", in: app)
        XCTAssertTrue(app.descendants(matching: .any)["signingPage.row.sign.getvela.app"].firstMatch
            .waitForExistence(timeout: 10), "Signing pages did not open")
        waitForAVerdict(in: app, timeout: 30)
        attach("s10-signing-pages")

        let field = app.textFields["signingPage.addField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("http://localhost:8140/")
        app.buttons["signingPage.addSave"].tap()
        Thread.sleep(forTimeInterval: 2)
        let addError = app.descendants(matching: .any)["signingPage.addError"].firstMatch
        print("ADD-ERROR-AFTER-SAVE", addError.exists ? addError.label : "none")
        XCTAssertFalse(addError.exists, "a page just saved is refused as already saved")
        let trust = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@ AND identifier CONTAINS %@",
                                  "signingPage.trust.", "localhost")).firstMatch
        XCTAssertTrue(trust.waitForExistence(timeout: 30), "the new build was not asked about")
        if app.keyboards.count > 0 { app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap() }
        Thread.sleep(forTimeInterval: 1)
        attach("s11-self-hosted-asks")
        print("ASK-LINE", app.descendants(matching: .any)["integrity.askTrust"].firstMatch.label)

        trust.tap()
        XCTAssertTrue(app.descendants(matching: .any)["integrity.trusted"].firstMatch.waitForExistence(timeout: 30),
                      "trusting the build did not land")
        attach("s12-self-hosted-trusted")
        print("TRUSTED-LINE", app.descendants(matching: .any)["integrity.trusted"].firstMatch.label)

        let menu = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@ AND identifier CONTAINS %@",
                                  "signingPage.menu.", "localhost")).firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 5), "no menu on the self-hosted row")
        menu.tap()
        let rename = app.buttons["重命名"].firstMatch
        XCTAssertTrue(rename.waitForExistence(timeout: 5))
        rename.tap()
        let nameField = app.alerts.textFields.firstMatch
        XCTAssertTrue(nameField.waitForExistence(timeout: 5), "no name field")
        nameField.typeText("Test page")
        attach("s13-rename")
        app.alerts.buttons["保存"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["Test page"].waitForExistence(timeout: 10), "the name did not land")
        attach("s14-renamed")

        // The account's venue: the page cannot reach getvela.app keys — a
        // disabled row with the core's reason.
        let back = app.descendants(matching: .any)["关闭"].firstMatch
        if back.exists { back.tap() } else { app.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.1)).tap() }
        Thread.sleep(forTimeInterval: 1)
        let row = app.descendants(matching: .any)["settings.venue"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        scrollIntoReach(row, in: app)
        row.tap()
        XCTAssertTrue(app.descendants(matching: .any)["venue.reason"].firstMatch.waitForExistence(timeout: 10),
                      "the page that cannot reach the keys is not explained")
        attach("s15-venue-sheet-disabled-row")
        print("VENUE-REASON", app.descendants(matching: .any)["venue.reason"].firstMatch.label)
        app.swipeDown(velocity: .fast)
        Thread.sleep(forTimeInterval: 1)

        // Removed: the list as it was.
        openAdvancedRow("签名页", in: app)
        let again = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@ AND identifier CONTAINS %@",
                                  "signingPage.menu.", "localhost")).firstMatch
        XCTAssertTrue(again.waitForExistence(timeout: 5))
        again.tap()
        let remove = app.buttons["移除"].firstMatch
        XCTAssertTrue(remove.waitForExistence(timeout: 5))
        remove.tap()
        let confirm = app.buttons["移除"].firstMatch
        if confirm.waitForExistence(timeout: 5) { confirm.tap() }
        Thread.sleep(forTimeInterval: 2)
        XCTAssertFalse(app.staticTexts["Test page"].exists, "the page is still listed")
        attach("s16-removed")

        // Its trust went with it: added again, it is asked about again —
        // never "trusted on this device" from a ruling held from before.
        field.tap()
        field.typeText("http://localhost:8140/")
        app.buttons["signingPage.addSave"].tap()
        XCTAssertTrue(trust.waitForExistence(timeout: 30), "a page added again is not asked about")
        XCTAssertFalse(app.descendants(matching: .any)["integrity.trusted"].firstMatch.exists,
                       "a removed page's trust came back with it")
        attach("s17-added-again-asks")
        let last = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@ AND identifier CONTAINS %@",
                                  "signingPage.menu.", "localhost")).firstMatch
        if last.waitForExistence(timeout: 5) {
            last.tap()
            let again = app.buttons["移除"].firstMatch
            if again.waitForExistence(timeout: 5) { again.tap() }
            let sure = app.buttons["移除"].firstMatch
            if sure.waitForExistence(timeout: 5) { sure.tap() }
        }
        Thread.sleep(forTimeInterval: 2)
        attach("s18-removed-again")
        app.terminate()
    }

    // MARK: - 2. The account's 「在哪里预览并签名」 → the official page

    func test2VenueToTheOfficialPage() throws {
        let app = launchInSpace()
        openSettings(app)
        let row = app.descendants(matching: .any)["settings.venue"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 15), "no 在哪里预览并签名 row")
        scrollIntoReach(row, in: app)
        attach("20-venue-row")
        row.tap()
        let official = app.descendants(matching: .any)["venue.sign.getvela.app"].firstMatch
        XCTAssertTrue(official.waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["venue.inVela"].firstMatch.exists, "no In Vela row")
        waitForAVerdict(in: app, timeout: 30)
        attach("21-venue-sheet")
        let reasons = app.descendants(matching: .any).matching(identifier: "venue.reason")
        print("VENUE-REASONS", reasons.count, (0..<reasons.count).map { reasons.element(boundBy: $0).label })
        official.tap()
        Thread.sleep(forTimeInterval: 2)
        attach("22-venue-official-chosen")
        printTexts(app, "VENUE-CHOSEN")
        app.terminate()
    }

    // MARK: - 3. A send hands off — stopped at the confirm

    /// `VELA_TOKEN` (the picker's row text, e.g. `BNB`) picks the coin;
    /// `VELA_AMOUNT` the dust. The recipient is the account's own Safe.
    func test3SendHandsOff() throws {
        let env = ProcessInfo.processInfo.environment
        let token = env["VELA_TOKEN"] ?? "BNB"
        let amount = env["VELA_AMOUNT"] ?? "0.00001"
        let app = launchInSpace()
        chooseOfficialVenue(in: app)
        let wallet = app.buttons["钱包"].firstMatch
        if wallet.exists, wallet.isHittable { wallet.tap() } else { app.tabBars.firstMatch.buttons.element(boundBy: 0).tap() }
        let send = app.buttons["转账"].firstMatch
        XCTAssertTrue(send.waitForExistence(timeout: 20))
        send.tap()
        Thread.sleep(forTimeInterval: 3)
        attach("30-send-picker")
        printTexts(app, "PICKER")
        let row = app.staticTexts[token].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20), "no \(token) in the picker")
        row.tap()
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20), "the form did not open")
        XCTAssertEqual(app.textFields.count, 2, "the form's fields changed shape")
        let recipient = app.textFields.element(boundBy: 1)
        recipient.tap()
        recipient.typeText(Self.ownSafe)
        let amountField = app.textFields.element(boundBy: 0)
        amountField.tap()
        amountField.typeText(amount)
        Thread.sleep(forTimeInterval: 2)
        attach("31-send-form")
        // The keypad's own 完成 — the keyboard covers 继续 until it goes.
        let done = app.buttons["完成"].firstMatch
        if done.exists, done.isHittable { done.tap() }
        Thread.sleep(forTimeInterval: 1)
        let cont = app.buttons["继续"]
        XCTAssertTrue(cont.waitForExistence(timeout: 30))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: cont, handler: nil)
        waitForExpectations(timeout: 90)
        attach("32-send-form-armed")
        cont.tap()

        // The confirm, as it lands: the line may still be checking.
        let open = app.buttons["去签名页确认"].firstMatch
        XCTAssertTrue(open.waitForExistence(timeout: 20), "the confirm does not hand off")
        let card = app.descendants(matching: .any)["handoff.card"].firstMatch
        attach("33-confirm-first-look")
        let openTopFirst = open.frame.minY
        let cardBottomFirst = card.frame.maxY
        let firstState = integrityState(in: app)
        waitForAVerdict(in: app, timeout: 30)
        attach("34-confirm-checked")
        print("CONFIRM-STATES", firstState ?? "none", "→", integrityState(in: app) ?? "none")
        print("CONFIRM-LINE", verdictText(in: app) ?? "none")
        XCTAssertEqual(open.frame.minY, openTopFirst, accuracy: 0.5, "Open moved when the check landed")
        XCTAssertEqual(card.frame.maxY, cardBottomFirst, accuracy: 0.5, "the card grew when the check landed")
        XCTAssertLessThanOrEqual(card.frame.maxY, open.frame.minY, "the card runs under the pinned button")
        // The fee is said once: the confirm's own row, none on the card.
        let feeRows = app.staticTexts.matching(NSPredicate(format: "label == %@", "预估手续费"))
        XCTAssertEqual(feeRows.count, 1, "the fee is said \(feeRows.count) times")
        XCTAssertFalse(app.descendants(matching: .any)["handoff.fee"].exists, "the card said the fee again")
        let key = app.descendants(matching: .any)["handoff.key"].firstMatch
        XCTAssertTrue(key.exists && key.label.hasPrefix("确认方式"), "the key row says \(key.label)")
        print("KEY-ROW", key.label, "OPEN-TOP", open.frame.minY, "CARD-BOTTOM", card.frame.maxY,
              "LINE-STATE", integrityState(in: app) ?? "none")
        printTexts(app, "CONFIRM")

        if env["VELA_OPEN_PAGE"] == "1", open.isEnabled {
            open.tap()
            Thread.sleep(forTimeInterval: 8)
            attach("35-page-open")
            Thread.sleep(forTimeInterval: 4)
            attach("36-page-settled")
            // Closed, never signed: Safari's own Done.
            let done = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "完成", "Done")).firstMatch
            if done.waitForExistence(timeout: 10) { done.tap() }
            Thread.sleep(forTimeInterval: 3)
            attach("37-page-closed")
            printTexts(app, "CLOSED")
        }
        app.terminate()
    }

    // MARK: - 4. A site's message hands off — declined

    func test4DappMessageHandsOff() throws {
        let server = try LocalDappServer(html: try LocalDappServer.page())
        server.start()
        self.server = server
        let app = launchInSpace(url: LocalDappServer.url)
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 15) {
            chooseOfficialVenue(in: app)
            let tab = app.buttons["探索"].firstMatch
            if tab.exists, tab.isHittable { tab.tap() }
            let resume = app.buttons.matching(identifier: "explore.resume.row").firstMatch
            if resume.waitForExistence(timeout: 10) { resume.tap() }
        } else {
            let tab = app.buttons["探索"].firstMatch
            if tab.exists, tab.isHittable { tab.tap() }
            let resume = app.buttons.matching(identifier: "explore.resume.row").firstMatch
            if resume.waitForExistence(timeout: 10) { resume.tap() }
        }
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "连接", "批准")).firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        Thread.sleep(forTimeInterval: 3)
        app.webViews.buttons["Sign"].firstMatch.tap()
        let card = app.descendants(matching: .any)["handoff.card"].firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 30), "the sheet did not hand off")
        attach("40-dapp-sheet-first-look")
        waitForAVerdict(in: app, timeout: 30)
        attach("41-dapp-sheet-checked")
        app.swipeUp()
        Thread.sleep(forTimeInterval: 1)
        attach("42-dapp-sheet-lower")
        let key = app.descendants(matching: .any)["handoff.key"].firstMatch
        print("DAPP-KEY", key.exists ? key.label : "none", "LINE", verdictText(in: app) ?? "none")
        XCTAssertFalse(app.descendants(matching: .any)["handoff.fee"].exists, "the card said a fee")
        printTexts(app, "DAPP")
        // Declined: the sheet's own reject, never Open.
        let reject = app.buttons.matching(
            NSPredicate(format: "label == %@ OR label == %@ OR label == %@ OR identifier == %@", "拒绝", "取消", "关闭", "signing.close")
        ).firstMatch
        if reject.waitForExistence(timeout: 5) { reject.tap() }
        Thread.sleep(forTimeInterval: 3)
        attach("43-dapp-declined")
        app.terminate()
    }

    // MARK: - 9. The phone put back

    /// The venue back to In Vela (inside the space), then out of the space —
    /// and the phone photographed as `test0` found it.
    func test9RestoresThePhone() throws {
        var app = launchInSpace()
        openSettings(app)
        let row = app.descendants(matching: .any)["settings.venue"].firstMatch
        if row.waitForExistence(timeout: 15) {
            scrollIntoReach(row, in: app)
            row.tap()
            let inVela = app.descendants(matching: .any)["venue.inVela"].firstMatch
            if inVela.waitForExistence(timeout: 10) { inVela.tap() }
            Thread.sleep(forTimeInterval: 2)
            attach("90-venue-back-in-vela")
            printTexts(app, "RESTORED-VENUE")
        }
        app.terminate()

        app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launch()
        Thread.sleep(forTimeInterval: 8)
        attach("91-after-home")
        printTexts(app, "AFTER-HOME")
        openSettings(app)
        Thread.sleep(forTimeInterval: 2)
        attach("92-after-settings")
        printTexts(app, "AFTER-SETTINGS")
        // Explore: no tab the walk's test page left behind.
        let explore = app.buttons["探索"].firstMatch
        if explore.exists, explore.isHittable { explore.tap() } else { app.tabBars.firstMatch.buttons.element(boundBy: 2).tap() }
        Thread.sleep(forTimeInterval: 3)
        attach("93-after-explore")
        printTexts(app, "AFTER-EXPLORE")
        app.terminate()
    }

    /// The walk's test page opened a browser tab (`VELA_URL`): closed, so
    /// the owner's tabs are as they were.
    func test9bClosesTheWalksTab() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        Thread.sleep(forTimeInterval: 6)
        let explore = app.buttons["探索"].firstMatch
        if explore.exists, explore.isHittable { explore.tap() } else { app.tabBars.firstMatch.buttons.element(boundBy: 2).tap() }
        Thread.sleep(forTimeInterval: 2)
        let bar = app.buttons["explore.bar.tabs"].firstMatch
        if bar.waitForExistence(timeout: 3) {
            bar.tap()
        } else {
            let tabs = app.staticTexts["标签页"].firstMatch
            XCTAssertTrue(tabs.waitForExistence(timeout: 10), "no open tabs")
            tabs.tap()
        }
        Thread.sleep(forTimeInterval: 2)
        attach("94-tabs-before-close")
        let card = app.buttons.matching(NSPredicate(format: "label CONTAINS %@ OR label CONTAINS %@", "Vela test dApp", "127.0.0.1:8137")).firstMatch
        if card.waitForExistence(timeout: 5) {
            // The card's own ✕: the close under it, in its column.
            let frame = card.frame
            let closes = app.buttons.matching(NSPredicate(format: "label == %@", "关闭标签页"))
            for index in 0..<closes.count {
                let close = closes.element(boundBy: index)
                if close.frame.midX > frame.minX, close.frame.midX < frame.maxX {
                    close.tap()
                    break
                }
            }
        } else {
            print("NO-TEST-TAB")
        }
        Thread.sleep(forTimeInterval: 2)
        attach("95-tabs-after-close")
        printTexts(app, "TABS-AFTER")
        app.terminate()
    }

    // MARK: - Helpers

    /// Settings → 在哪里预览并签名 → Vela 官方签名页, in this launch.
    private func chooseOfficialVenue(in app: XCUIApplication) {
        openSettings(app)
        let row = app.descendants(matching: .any)["settings.venue"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 15), "no 在哪里预览并签名 row")
        scrollIntoReach(row, in: app)
        row.tap()
        let official = app.descendants(matching: .any)["venue.sign.getvela.app"].firstMatch
        XCTAssertTrue(official.waitForExistence(timeout: 10))
        official.tap()
        Thread.sleep(forTimeInterval: 2)
        let chosen = app.staticTexts["Vela 官方签名页"].firstMatch
        XCTAssertTrue(chosen.waitForExistence(timeout: 10), "the venue did not change")
    }

    private func launchInSpace(url: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        if let url { app.launchEnvironment["VELA_URL"] = url }
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 40), "not in the parallel space")
        Thread.sleep(forTimeInterval: 2)
        return app
    }

    private func openSettings(_ app: XCUIApplication) {
        for label in ["设置", "Settings", "設定"] {
            let button = app.buttons[label].firstMatch
            if button.exists, button.isHittable {
                button.tap()
                return
            }
        }
        // The icon-only bar: its last button.
        let bar = app.tabBars.firstMatch
        if bar.exists {
            bar.buttons.element(boundBy: max(0, bar.buttons.count - 1)).tap()
        }
    }

    private func openAdvancedRow(_ title: String, in app: XCUIApplication) {
        var label = app.staticTexts[title].firstMatch
        if !label.exists {
            let advanced = app.staticTexts["高级"].firstMatch
            scrollIntoReach(advanced, in: app)
            advanced.tap()
            Thread.sleep(forTimeInterval: 0.8)
            label = app.staticTexts[title].firstMatch
        }
        XCTAssertTrue(label.waitForExistence(timeout: 8), "no row called \(title)")
        scrollIntoReach(label, in: app)
        label.tap()
        Thread.sleep(forTimeInterval: 1)
    }

    private func scrollIntoReach(_ element: XCUIElement, in app: XCUIApplication) {
        var swipes = 0
        while !element.isHittable, swipes < 6 {
            app.swipeUp()
            swipes += 1
        }
    }

    private static let verdicts = ["matches", "trusted", "mismatch", "couldNotCheck", "noVersion", "blocked", "allBlocked", "askTrust", "unchecked"]

    private func waitForAVerdict(in app: XCUIApplication, timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if integrityState(in: app).map({ $0 != "checking" }) == true { return }
            Thread.sleep(forTimeInterval: 0.5)
        }
    }

    private func integrityState(in app: XCUIApplication) -> String? {
        for state in ["checking"] + Self.verdicts
        where app.descendants(matching: .any)["integrity.\(state)"].firstMatch.exists {
            return state
        }
        return nil
    }

    private func verdictText(in app: XCUIApplication) -> String? {
        for state in Self.verdicts {
            let line = app.descendants(matching: .any)["integrity.\(state)"].firstMatch
            if line.exists { return "\(state): \(line.label)" }
        }
        return nil
    }

    /// The screen's words, from one snapshot — a list read element by
    /// element fails when one goes while it is read (a sheet animating).
    private func printTexts(_ app: XCUIApplication, _ tag: String) {
        guard let root = try? app.snapshot() else { return }
        var texts: [String] = []
        func walk(_ node: XCUIElementSnapshot) {
            if node.elementType == .staticText, !node.label.isEmpty { texts.append(node.label) }
            node.children.forEach(walk)
        }
        walk(root)
        print("TEXTS-\(tag)", texts.prefix(80))
    }

    private func attach(_ name: String) {
        Thread.sleep(forTimeInterval: 0.6)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
