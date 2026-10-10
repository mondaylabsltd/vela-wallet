//
//  UIBatchScreenshotTests.swift
//  VelaWalletUITests
//
//  PR 3's screens (the UI batch), in zh and en, light and dark — walked on a
//  simulator and photographed, each board asserting the thing it shows:
//
//  - `testReportAProblemKeyboard` (issue #478): "Report a problem" with the
//    keyboard up and its Done bar; Done, a tap outside a field and a drag of
//    the sheet each put the keyboard away; Return is a newline; Send is
//    reached with the keyboard still up.
//
//  - `testScanACodeSheet` (issue #480): the "Phone or tablet · scan a code"
//    sheet, as tall as its content, Cancel at the bottom.
//
//  - `testInsertYourSecurityKey`: "Insert your security key" with "Use
//    Apple's security-key sheet" and its hint.
//
//  - `testCopyTheRecordToEthereum`: the Keys block's row in each state the
//    core words, and the sheet it opens.
//
//  Simulator only; skipped in the scheme (a copy of the .xctestrun with the
//  skip removed runs it).
//

import XCTest

final class UIBatchScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    private static let looks: [(lang: String, theme: String)] = [
        ("zh", "light"), ("en", "light"), ("zh", "dark"), ("en", "dark"),
    ]

    // MARK: - Issue #478: "Report a problem" and the keyboard

    func testReportAProblemKeyboard() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: ["VELA_PAGE": "settings-live", "VELA_STATE": "st15"],
                             lang: look.lang, theme: look.theme)
            let what = app.textViews["feedback.what"]
            XCTAssertTrue(what.waitForExistence(timeout: 30), "no report sheet (\(tag))")
            let keyboard = app.keyboards.firstMatch
            let done = app.buttons["keyboard.done"].firstMatch

            // 1. Typing: the keyboard is up, with a Done bar over it, and
            // Return starts a new line rather than closing anything.
            what.tap()
            XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "no keyboard (\(tag))")
            what.typeText("The fee row jumped\nwhen I changed the coin")
            settle(0.8)
            XCTAssertTrue(done.waitForExistence(timeout: 5), "no Done over the keys (\(tag))")
            XCTAssertEqual(done.label, look.lang == "zh" ? "完成" : "Done")
            XCTAssertTrue((what.value as? String ?? "").contains("\n"), "Return did not start a new line (\(tag))")
            XCTAssertTrue(keyboard.exists, "Return closed the keyboard (\(tag))")
            attach(app, "478-report-keyboard-up-\(tag)")

            // 2. Send is reached with the keyboard still up: the sheet scrolls
            // above the keys.
            let send = app.buttons["feedback.send"]
            var drags = 0
            while !send.isHittable, drags < 8 {
                slowDrag(app, from: 0.42, to: 0.16)
                drags += 1
            }
            XCTAssertTrue(keyboard.exists, "scrolling up to Send dropped the keyboard (\(tag))")
            XCTAssertTrue(send.isHittable, "Send is not reachable above the keyboard (\(tag))")
            attach(app, "478-report-send-above-keyboard-\(tag)")

            // 3. Done puts the keyboard away.
            done.tap()
            XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "Done left the keyboard up (\(tag))")
            settle(0.6)
            attach(app, "478-report-keyboard-dismissed-\(tag)")

            // 4. A tap outside a field does too — here, on the sheet's title.
            let title = app.staticTexts[look.lang == "zh" ? "反馈问题" : "Report a problem"].firstMatch
            slowDrag(app, from: 0.2, to: 0.45)
            what.tap()
            XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "the field did not take the keyboard back (\(tag))")
            XCTAssertTrue(title.isHittable, "the sheet's title is not on screen to tap (\(tag))")
            title.tap()
            XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "a tap outside left the keyboard up (\(tag))")
            XCTAssertTrue((what.value as? String ?? "").contains("changed the coin"), "the tap lost the draft (\(tag))")

            // 5. And a drag of the sheet's page down into the keys. The page
            // is scrolled up first, so the drag moves the page and not the
            // sheet (a pull on a sheet at its top is the system's dismiss).
            what.tap()
            XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
            slowDrag(app, from: 0.42, to: 0.16)
            slowDrag(app, from: 0.42, to: 0.16)
            XCTAssertTrue(keyboard.exists, "scrolling up dropped the keyboard (\(tag))")
            slowDrag(app, from: 0.3, to: 0.72)
            XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "a drag left the keyboard up (\(tag))")
            XCTAssertTrue(send.exists, "the drag closed the sheet (\(tag))")
            app.terminate()
        }
    }

    // MARK: - Issue #480: the "Phone or tablet · scan a code" sheet

    /// One sheet for create, add-key, sign-in and the switcher's sign-in.
    /// It is as tall as its content — the page it rose over shows above it —
    /// with the whole code on screen and Cancel at the bottom.
    func testScanACodeSheet() {
        for look in Self.looks {
            for board in ["cable-create", "cable-signin"] {
                let tag = "\(board)-\(look.lang)-\(look.theme)"
                let app = launch(env: ["VELA_PAGE": "pr3", "VELA_STATE": board],
                                 lang: look.lang, theme: look.theme)
                let cancel = app.buttons["cable.cancel"]
                XCTAssertTrue(cancel.waitForExistence(timeout: 20), "no scan-a-code sheet (\(tag))")
                settle(1.2)
                let title = app.staticTexts[look.lang == "zh" ? "手机或平板" : "Phone or tablet"].firstMatch
                XCTAssertTrue(title.exists, "the sheet is not titled (\(tag))")
                XCTAssertTrue(cancel.isHittable, "Cancel is off the sheet (\(tag))")
                XCTAssertEqual(cancel.label, look.lang == "zh" ? "取消" : "Cancel")
                let screen = app.frame.height
                // Content-sized: its title starts well below the top of the
                // screen (a `.large` sheet's sat at about 9%)…
                XCTAssertGreaterThan(title.frame.minY, screen * 0.2, "the sheet is still near-full-height (\(tag))")
                // …and Cancel is the last thing, at the bottom.
                XCTAssertGreaterThan(cancel.frame.maxY, screen * 0.86, "Cancel is not at the bottom (\(tag))")
                attach(app, "480-\(tag)")
                app.terminate()
            }
        }
    }

    // MARK: - The security key's way to Apple's sheet

    /// "Insert your security key" offers Apple's security-key sheet, says
    /// which keys that is for, and keeps Close at the bottom.
    func testInsertYourSecurityKey() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            let zh = look.lang == "zh"
            let app = launch(env: ["VELA_PAGE": "pr3", "VELA_STATE": "insert-key"],
                             lang: look.lang, theme: look.theme)
            let option = app.buttons["insertKey.appleSheet"]
            XCTAssertTrue(option.waitForExistence(timeout: 20), "no Apple-sheet option (\(tag))")
            settle(1.2)
            XCTAssertEqual(option.label, zh ? "改用 Apple 的安全密钥面板" : "Use Apple's security-key sheet")
            XCTAssertTrue(option.isHittable, "the option is off the sheet (\(tag))")
            XCTAssertTrue(app.staticTexts[zh ? "插入安全密钥" : "Insert your security key"].exists)
            XCTAssertTrue(
                app.staticTexts[zh ? "适用于 NFC、Lightning 或较旧的密钥" : "For NFC, Lightning or older keys"].exists,
                "the option does not say which keys it is for (\(tag))"
            )
            let close = app.buttons["insertKey.close"]
            XCTAssertTrue(close.isHittable, "Close is off the sheet (\(tag))")
            XCTAssertGreaterThan(close.frame.minY, option.frame.maxY, "Close is not the last thing (\(tag))")
            attach(app, "securitykey-insert-\(tag)")
            app.terminate()
        }
    }

    /// The LIVE sign-in, "USB security key" chosen, with no key anywhere.
    ///
    /// - On a phone with a USB-C port the insert sheet comes up and waits;
    ///   its option hands the ceremony to the system's security-key provider
    ///   (the insert sheet goes, nothing is left waiting).
    /// - On a Lightning iPhone (an iPhone SE simulator) the insert sheet never
    ///   comes up at all: the ceremony goes straight to the system provider.
    ///
    /// What the system provider then DRAWS is not asserted: an unsigned
    /// simulator build carries no `webcredentials` association, so Apple's
    /// sheet refuses instead of appearing. A key on a real phone is the
    /// owner's to try.
    func testTheSecurityKeyRouteLive() {
        let model = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] ?? ""
        let numbers = model.dropFirst("iPhone".count).split(separator: ",").compactMap { Int($0) }
        let lightning = model.hasPrefix("iPhone") && numbers.count == 2
            && (numbers[0] < 15 || (numbers[0] == 15 && numbers[1] < 4))
        let app = launch(env: [:], args: ["-vela.parallelSpace", "0"], lang: "zh", theme: "light")
        let signIn = app.buttons["我已有钱包"]
        XCTAssertTrue(signIn.waitForExistence(timeout: 40), "no Welcome")
        signIn.tap()
        let method = app.staticTexts["USB 安全密钥"]
        XCTAssertTrue(method.waitForExistence(timeout: 10), "no USB security key row")
        method.tap()
        let insert = app.staticTexts["插入安全密钥"]
        if lightning {
            XCTAssertFalse(insert.waitForExistence(timeout: 6),
                           "a Lightning iPhone (\(model)) was made to wait for a USB-C key")
            settle(2)
            attach(app, "securitykey-live-lightning-\(model)")
        } else {
            XCTAssertTrue(insert.waitForExistence(timeout: 15), "no insert sheet on \(model)")
            settle(1)
            attach(app, "securitykey-live-insert-\(model)")
            let option = app.buttons["insertKey.appleSheet"]
            XCTAssertTrue(option.isHittable, "the option is off the sheet")
            option.tap()
            // What follows the tap, frame by frame: the hold between the two
            // sheets, then whatever the system provider answers.
            for (index, wait) in [0.4, 0.8, 1.5, 3.0, 4.0].enumerated() {
                settle(wait)
                attach(app, "securitykey-live-after-option-\(index)-\(model)")
            }
            XCTAssertFalse(insert.exists, "the option left the insert sheet waiting")
        }
        app.terminate()
    }

    // MARK: - "Copy this wallet's record to Ethereum"

    /// The Keys block's row in every state the core words, and the sheet a
    /// tap on "not copied yet" opens.
    func testCopyTheRecordToEthereum() {
        let states: [(board: String, zh: String, en: String)] = [
            ("checking", "正在检查…", "Checking…"),
            ("not-copied", "尚未复制（可选）", "Not copied yet (optional)"),
            ("copied", "已复制到以太坊", "Copied to Ethereum"),
            ("could-not-check", "无法检查，点按重试", "Couldn't check. Tap to try again."),
            ("cannot-copy", "这个较早创建的钱包无法复制", "This older wallet can't be copied"),
        ]
        for look in Self.looks {
            let zh = look.lang == "zh"
            for state in states {
                let tag = "\(state.board)-\(look.lang)-\(look.theme)"
                let app = launch(env: ["VELA_PAGE": "pr3", "VELA_STATE": "backup-\(state.board)"],
                                 lang: look.lang, theme: look.theme)
                let title = app.staticTexts[zh ? "把钱包记录复制到以太坊" : "Copy this wallet's record to Ethereum"].firstMatch
                var scrolls = 0
                while !(title.exists && title.isHittable), scrolls < 6 {
                    if !title.waitForExistence(timeout: scrolls == 0 ? 20 : 1) || !title.isHittable { app.swipeUp() }
                    scrolls += 1
                }
                XCTAssertTrue(title.exists, "no record row (\(tag))")
                XCTAssertTrue(app.staticTexts[zh ? state.zh : state.en].exists, "the row does not read its state (\(tag))")
                settle(0.8)
                attach(app, "backup-row-\(tag)")
                app.terminate()
            }
            // The sheet: the wallet's own request, with its name as a row.
            let app = launch(env: ["VELA_PAGE": "signing", "VELA_STATE": "cs36"], lang: look.lang, theme: look.theme)
            XCTAssertTrue(app.staticTexts[zh ? "复制钱包记录" : "Copy this wallet's record"].firstMatch
                .waitForExistence(timeout: 20), "the sheet is not headed by its intent")
            XCTAssertTrue(app.staticTexts[zh ? "钱包名称" : "Wallet name"].exists, "the wallet's name is not a row")
            XCTAssertTrue(app.staticTexts[zh ? "包含的钥匙" : "Keys included"].exists)
            settle(0.8)
            attach(app, "backup-sheet-\(look.lang)-\(look.theme)")
            app.terminate()
        }
    }

    // MARK: - Helpers

    private func launch(
        env: [String: String], args: [String] = [], lang: String, theme: String
    ) -> XCUIApplication {
        let app = XCUIApplication()
        for (key, value) in env { app.launchEnvironment[key] = value }
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += args + ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
        app.launch()
        return app
    }

    /// A finger-speed drag: a flick does not scroll a sheet with the keyboard
    /// up.
    private func slowDrag(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: from))
            .press(forDuration: 0.15, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: to)),
                   withVelocity: .slow, thenHoldForDuration: 0.15)
        settle(0.7)
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    /// The whole screen, not the app's frame: on an iPad the iPhone layout
    /// runs in a window, and the app's own screenshot is a crop of it.
    private func attach(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
