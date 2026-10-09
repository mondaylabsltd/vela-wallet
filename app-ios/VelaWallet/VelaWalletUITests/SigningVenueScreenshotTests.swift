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

    // MARK: - Phase 2b: hermetic boards, in both languages and both themes

    private static let looks: [(lang: String, theme: String)] = [
        ("zh", "light"), ("en", "light"), ("zh", "dark"), ("en", "dark"),
    ]

    /// The hand-off card as the core round left it: "Confirm with …" (the
    /// plan's key label), the fee and speed the page will sign, "checked
    /// {{time}}"; while checking (no fee row, Open shut); a self-hosted build
    /// asking to be trusted, with its answer; a sign-time venue refusal said
    /// in the person's language; the page's own sheet, and a ceremony's own
    /// title.
    func testTheHandoffBoardsAfterTheCoreRound() throws {
        let boards: [(state: String, id: String)] = [
            ("sheet", "handoff.fee"),
            ("sheet-checking", "integrity.checking"),
            ("sheet-ask", "handoff.trust"),
            ("sheet-blocked", "handoff.none"),
            ("card", "handoff.open"),
            ("ask", "handoff.trust"),
            ("ceremony", "handoff.title"),
        ]
        for look in Self.looks {
            for (state, id) in boards {
                let app = launch(env: ["VELA_PAGE": "handoff", "VELA_STATE": state], lang: look.lang, theme: look.theme)
                if state == "sheet-blocked" {
                    // The failure's reason, from the core's notice: both
                    // domains, in the board's language.
                    let reason = app.staticTexts.containing(
                        NSPredicate(format: "label CONTAINS %@ AND label CONTAINS %@", "getvela.app", "example.com")
                    ).firstMatch
                    XCTAssertTrue(reason.waitForExistence(timeout: 15), "\(look): the venue refusal is not said")
                } else {
                    XCTAssertTrue(app.descendants(matching: .any)[id].firstMatch.waitForExistence(timeout: 15),
                                  "\(state) \(look): no \(id)")
                }
                if state == "sheet" {
                    let mine = look.lang == "zh" ? "用 YubiKey 5C 确认" : "Confirm with YubiKey 5C"
                    XCTAssertTrue(app.staticTexts[mine].exists, "the key is not named: \(mine)")
                }
                if state == "sheet-blocked", look.lang == "zh" {
                    let reason = app.staticTexts.containing(
                        NSPredicate(format: "label CONTAINS %@", "这个页面在 getvela.app")
                    ).firstMatch
                    XCTAssertTrue(reason.exists, "the venue refusal is not said in Chinese")
                }
                if state == "sheet-checking" {
                    XCTAssertFalse(app.buttons["signing.openSigner"].isEnabled, "Open was on while checking")
                }
                attach("2b-handoff-\(state)-\(look.lang)-\(look.theme)")
                app.terminate()
            }
        }
    }

    /// Settings → Signing pages: 「Vela 官方签名页」, a self-hosted page whose
    /// build asks to be trusted with its button, a named one; then its menu
    /// (rename / remove) and the rename field (name).
    func testTheSigningPagesBoards() throws {
        for look in Self.looks {
            let app = launch(env: ["VELA_PAGE": "settings", "VELA_STATE": "signing-pages"],
                             lang: look.lang, theme: look.theme)
            XCTAssertTrue(app.descendants(matching: .any)["signingPage.row.sign.getvela.app"].firstMatch
                .waitForExistence(timeout: 15), "Signing pages did not open")
            XCTAssertTrue(app.descendants(matching: .any)["signingPage.trust.sign.example.com"].firstMatch.exists,
                          "the self-hosted page's question has no answer")
            let official = look.lang == "zh" ? "Vela 官方签名页" : "Vela's official signing page"
            XCTAssertTrue(app.staticTexts[official].exists, "the official page is not named \(official)")
            attach("2b-signing-pages-\(look.lang)-\(look.theme)")

            let menu = app.descendants(matching: .any)["signingPage.menu.sign.work.example"].firstMatch
            XCTAssertTrue(menu.exists)
            menu.tap()
            let rename = app.buttons[look.lang == "zh" ? "重命名" : "Rename"].firstMatch
            XCTAssertTrue(rename.waitForExistence(timeout: 5), "no rename")
            XCTAssertTrue(app.buttons[look.lang == "zh" ? "移除" : "Remove"].firstMatch.exists, "no remove")
            attach("2b-signing-pages-menu-\(look.lang)-\(look.theme)")
            rename.tap()
            XCTAssertTrue(app.alerts.firstMatch.waitForExistence(timeout: 5), "no rename field")
            attach("2b-signing-pages-rename-\(look.lang)-\(look.theme)")
            app.terminate()
        }
    }

    /// The account's 「在哪里预览并签名」: its row, and the sheet — In Vela, the
    /// official page chosen, and the self-hosted pages disabled, each with the
    /// core's reason under it.
    func testTheVenueBoards() throws {
        for look in Self.looks {
            let app = launch(env: ["VELA_PAGE": "settings", "VELA_STATE": "signing-venue"],
                             lang: look.lang, theme: look.theme)
            let row = app.descendants(matching: .any)["settings.venue"].firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 15), "no venue row")
            scrollIntoReach(row, in: app)
            attach("2b-venue-row-\(look.lang)-\(look.theme)")
            row.tap()
            XCTAssertTrue(app.descendants(matching: .any)["venue.reason"].firstMatch.waitForExistence(timeout: 10),
                          "a page that cannot reach the keys is not explained")
            attach("2b-venue-sheet-\(look.lang)-\(look.theme)")
            app.swipeUp()
            attach("2b-venue-sheet-reasons-\(look.lang)-\(look.theme)")
            app.terminate()
        }
    }

    /// The create chooser's 「使用可信签名页」 and its list — Vela's own sheet,
    /// 「Vela 官方签名页」 and a self-hosted page — and a chosen self-hosted page.
    func testTheChooserBoards() throws {
        for look in Self.looks {
            var app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · signing page offered"],
                             lang: look.lang, theme: look.theme)
            let entry = app.descendants(matching: .any)["chooser.signingPage"].firstMatch
            XCTAssertTrue(entry.waitForExistence(timeout: 15))
            let words = look.lang == "zh" ? "使用可信签名页" : "Use a trusted signing page"
            XCTAssertTrue(app.staticTexts[words].exists, "the entry is not worded \(words)")
            attach("2b-create-chooser-\(look.lang)-\(look.theme)")
            entry.tap()
            XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
            let selfHosted = look.lang == "zh" ? "自己部署的签名页 · sign.example.com" : "Self-hosted · sign.example.com"
            XCTAssertTrue(app.staticTexts[selfHosted].waitForExistence(timeout: 5), "no \(selfHosted)")
            attach("2b-create-picker-\(look.lang)-\(look.theme)")
            app.terminate()

            app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · on a self-hosted page"],
                         lang: look.lang, theme: look.theme)
            XCTAssertTrue(app.descendants(matching: .any)["signingPage.chosen"].firstMatch.waitForExistence(timeout: 15))
            attach("2b-create-on-a-self-hosted-page-\(look.lang)-\(look.theme)")
            app.terminate()
        }
    }

    // MARK: - The choosers

    func testTheCreateChooserBoards() throws {
        var app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · signing page offered"])
        let entry = app.descendants(matching: .any)["chooser.signingPage"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 15))
        attach("create-chooser")
        entry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
        attach("create-picker")
        app.terminate()

        app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · on a self-hosted page"])
        XCTAssertTrue(app.descendants(matching: .any)["signingPage.chosen"].firstMatch.waitForExistence(timeout: 15))
        XCTAssertTrue(app.staticTexts["钥匙在 sign.example.com"].exists)
        attach("create-on-a-self-hosted-page")
        app.terminate()
    }

    /// The live sign-in sheet: the official page checked by THIS phone, for
    /// real, and chosen.
    func testTheSignInChooserLive() throws {
        let app = launch(env: ["VELA_PARALLEL_SPACE": "0"], args: ["-vela.parallelSpace", "0"])
        XCTAssertTrue(app.buttons["我已有钱包"].waitForExistence(timeout: 30))
        app.buttons["我已有钱包"].tap()
        let entry = app.descendants(matching: .any)["chooser.signingPage"].firstMatch
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
        let chosen = app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "Vela 官方签名页")).firstMatch
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
