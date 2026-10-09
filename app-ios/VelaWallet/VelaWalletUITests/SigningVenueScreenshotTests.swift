//
//  SigningVenueScreenshotTests.swift
//  VelaWalletUITests
//
//  Spec 102's surfaces, walked and photographed on the simulator — the visual
//  proof of Phase 2 on iOS. Skipped in the scheme (like the screenshot sweep):
//  it reaches the network for the official page's integrity check and walks
//  the parallel space's wallet, which a CI leg should not.
//
//      xcodebuild test … -only-testing:VelaWalletUITests/SigningVenueScreenshotTests
//      (with the skip removed from a copy of the .xctestrun)
//
//  Every board is a screenshot attachment (`xcrun xcresulttool export
//  attachments`), and each one asserts the thing it shows: no board here is a
//  picture of a screen that did not render.
//

import XCTest

final class SigningVenueScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    // MARK: - The hand-off (D4) and the page's own sheet

    func testTheHandoffBoards() throws {
        let boards: [(state: String, id: String)] = [
            ("sheet", "handoff.card"),
            ("sheet-checking", "integrity.checking"),
            ("sheet-refused", "integrity.mismatch"),
            ("card", "handoff.open"),
            ("checking", "integrity.checking"),
            ("refused", "integrity.mismatch"),
            ("couldNotCheck", "handoff.recheck"),
            ("waiting", "trustedSigner.waiting"),
            ("down", "trustedSigner.signerDown"),
        ]
        for (state, id) in boards {
            let app = launch(env: ["VELA_PAGE": "handoff", "VELA_STATE": state])
            XCTAssertTrue(app.descendants(matching: .any)[id].firstMatch.waitForExistence(timeout: 15),
                          "\(state): no \(id)")
            attach("handoff-\(state)-zh")
            app.terminate()
        }
        for (state, id) in [("sheet", "handoff.card"), ("card", "handoff.open")] {
            let app = launch(env: ["VELA_PAGE": "handoff", "VELA_STATE": state], lang: "en", theme: "dark")
            XCTAssertTrue(app.descendants(matching: .any)[id].firstMatch.waitForExistence(timeout: 15))
            attach("handoff-\(state)-en-dark")
            app.terminate()
        }
    }

    // MARK: - The choosers

    func testTheCreateChooserBoards() throws {
        var app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · own page offered"])
        let entry = app.descendants(matching: .any)["chooser.ownPage"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 15))
        attach("create-chooser")
        entry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
        attach("create-picker")
        app.terminate()

        app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · on my own page"])
        XCTAssertTrue(app.descendants(matching: .any)["signingPage.chosen"].firstMatch.waitForExistence(timeout: 15))
        XCTAssertTrue(app.staticTexts["钥匙在 sign.example.com"].exists)
        attach("create-on-my-own-page")
        app.terminate()
    }

    /// The live sign-in sheet: the official page checked by THIS phone, for
    /// real, and chosen.
    func testTheSignInChooserLive() throws {
        let app = launch(env: ["VELA_PARALLEL_SPACE": "0"], args: ["-vela.parallelSpace", "0"])
        XCTAssertTrue(app.buttons["我已有钱包"].waitForExistence(timeout: 30))
        app.buttons["我已有钱包"].tap()
        let entry = app.descendants(matching: .any)["chooser.ownPage"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 10))
        attach("signin-chooser")
        entry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
        waitForAVerdict(in: app, timeout: 25)
        attach("signin-picker-live")
        app.descendants(matching: .any)["signingPage.sign.getvela.app"].firstMatch.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPage.chosen"].firstMatch.waitForExistence(timeout: 10))
        attach("signin-on-official-page")
        app.terminate()
    }

    // MARK: - Settings: where you review and sign, and signing pages

    func testSettingsVenueAndSigningPages() throws {
        let app = launch(env: ["VELA_PARALLEL_SPACE": "1"])
        let name = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH %@", "Parallel")).firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 40), "no wallet to work in")
        Thread.sleep(forTimeInterval: 1.5)
        tapTab("设置", in: app)
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 10), "Settings did not open")

        let venueRow = app.descendants(matching: .any)["settings.venue"].firstMatch
        XCTAssertTrue(venueRow.waitForExistence(timeout: 15), "no \"在哪里预览并签名\" row")
        scrollIntoReach(venueRow, in: app)
        attach("settings-venue-row")

        // The sheet: In Vela, the official page with its line.
        venueRow.tap()
        let official = app.descendants(matching: .any)["venue.sign.getvela.app"].firstMatch
        XCTAssertTrue(official.waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["venue.domain"].firstMatch.exists)
        waitForAVerdict(in: app, timeout: 25)
        attach("settings-venue-sheet")

        // Choose the official page; the row says so.
        official.tap()
        let chosen = app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "在可信签名页")).firstMatch
        XCTAssertTrue(chosen.waitForExistence(timeout: 10), "the row does not say the page")
        attach("settings-venue-chosen")

        // Settings → Signing pages.
        openAdvancedRow("签名页", in: app)
        let officialRow = app.descendants(matching: .any)["signingPage.row.sign.getvela.app"].firstMatch
        XCTAssertTrue(officialRow.waitForExistence(timeout: 10), "Signing pages did not open")
        waitForAVerdict(in: app, timeout: 25)
        attach("settings-signing-pages")

        // A refused address stores nothing and says why.
        let field = app.textFields["signingPage.addField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("http://192.168.1.4/")
        app.buttons["signingPage.addSave"].tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPage.addError"].firstMatch.waitForExistence(timeout: 5))
        attach("settings-signing-pages-refused")

        // A page on somebody's own domain.
        field.tap()
        field.typeText("https://sign.example.com/")
        app.buttons["signingPage.addSave"].tap()
        let own = app.descendants(matching: .any)["signingPage.row.sign.example.com"].firstMatch
        XCTAssertTrue(own.waitForExistence(timeout: 10), "the page was not added")
        dismissKeyboard(in: app)
        Thread.sleep(forTimeInterval: 4)
        attach("settings-signing-pages-added")

        // Back on the account: that page cannot reach getvela.app keys, and
        // the sheet says so under a disabled row.
        back(in: app)
        let row = app.descendants(matching: .any)["settings.venue"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        scrollIntoReach(row, in: app)
        row.tap()
        XCTAssertTrue(app.descendants(matching: .any)["venue.reason"].firstMatch.waitForExistence(timeout: 10),
                      "a page that cannot reach the keys is not explained")
        app.swipeUp()
        attach("settings-venue-sheet-blocked")

        // Back to In Vela, and the page removed: the simulator as it was.
        app.descendants(matching: .any)["venue.inVela"].firstMatch.tap()
        Thread.sleep(forTimeInterval: 1.5)
        openAdvancedRow("签名页", in: app)
        let menu = app.descendants(matching: .any)["signingPage.menu.sign.example.com"].firstMatch
        if menu.waitForExistence(timeout: 5) {
            menu.tap()
            let remove = app.buttons["移除"].firstMatch
            if remove.waitForExistence(timeout: 5) { remove.tap() }
            let confirm = app.buttons["移除"].firstMatch
            if confirm.waitForExistence(timeout: 5) { confirm.tap() }
        }
        app.terminate()
    }

    // MARK: - Helpers

    private func launch(
        env: [String: String], args: [String] = [], lang: String = "zh", theme: String = "light"
    ) -> XCUIApplication {
        let app = XCUIApplication()
        for (key, value) in env { app.launchEnvironment[key] = value }
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += args + ["-AppleLanguages", lang == "zh" ? "(zh)" : "(en)"]
        app.launch()
        return app
    }

    /// The official page's check reaches the network; give it a moment to
    /// come back with any verdict before the photograph.
    private func waitForAVerdict(in app: XCUIApplication, timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        let verdicts = ["matches", "trusted", "mismatch", "couldNotCheck", "noVersion", "blocked", "allBlocked"]
        while Date() < deadline {
            if verdicts.contains(where: { app.descendants(matching: .any)["integrity.\($0)"].firstMatch.exists }) {
                return
            }
            Thread.sleep(forTimeInterval: 0.5)
        }
    }

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

    private func back(in app: XCUIApplication) {
        let chevron = app.descendants(matching: .any)["关闭"].firstMatch
        if chevron.exists {
            chevron.tap()
        } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.1)).tap()
        }
        Thread.sleep(forTimeInterval: 1)
    }

    private func dismissKeyboard(in app: XCUIApplication) {
        if app.keyboards.count > 0 {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap()
        }
    }

    private func attach(_ name: String) {
        Thread.sleep(forTimeInterval: 0.8)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
