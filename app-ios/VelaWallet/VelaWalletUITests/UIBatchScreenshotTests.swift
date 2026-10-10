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
//  - `testTheTwoRefusals`: a network refused for no P-256 verifier vs for
//    missing contracts, in Settings and on a dApp's sheet.
//
//  - `testTheBoards`: the home's three activity rows (#469), the recipient
//    rows' icons and the contacts-only picker (#471), a contact's page
//    (#479) and Add passkeys with no key, some and seven (#475).
//
//  - `testTheLiveHomeDrawsTheNewestThree`, `testAColdStartNeverShowsDollarsFirst`,
//    `testAHiddenTransferDetailKeepsItsUnit`: the LIVE app over a read-only
//    account with five seeded transfers — the home's three and History's
//    five (#469), a cold start with CNY stored (item 10), and a hidden
//    transfer's detail (item 12). These reach the network.
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

    /// Issues #459 × #480, live: the content-sized sheet still lets a person
    /// leave — Cancel, a swipe down ON the sheet, and a tap on the page
    /// above it each take the code away, and Welcome works again.
    func testTheScanSheetLeavesThreeWays() {
        let app = launch(env: [:], args: ["-vela.parallelSpace", "0"], lang: "zh", theme: "light")
        let signIn = app.buttons["我已有钱包"]
        XCTAssertTrue(signIn.waitForExistence(timeout: 40), "no Welcome")
        for exit in ["cancel", "swipe", "tap-outside"] {
            signIn.tap()
            let phone = app.staticTexts["手机或平板"]
            XCTAssertTrue(phone.waitForExistence(timeout: 10), "no sign-in sheet (\(exit))")
            phone.tap()
            let cancel = app.buttons["cable.cancel"]
            XCTAssertTrue(cancel.waitForExistence(timeout: 15), "the phone's code never came up (\(exit))")
            settle(1.5)
            attach(app, "480-live-code-\(exit)")
            switch exit {
            case "cancel":
                cancel.tap()
            case "swipe":
                // From the sheet's own title, down past the bottom edge.
                let title = app.staticTexts["手机或平板"].firstMatch
                title.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
                    .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.99)))
            default:
                // The page above the sheet: what a content-sized sheet leaves.
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap()
            }
            XCTAssertTrue(cancel.waitForNonExistence(timeout: 6), "the code stayed up after \(exit)")
            settle(2)
            XCTAssertTrue(signIn.isHittable, "Welcome is not back after \(exit)")
        }
        app.terminate()
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

    // MARK: - A refused network: no P-256 verifier vs missing contracts

    /// The two refusals, in Settings → Add network and on a dApp's sheet:
    /// no P-256 says so plainly and offers no Chain Setup; missing contracts
    /// keeps the button.
    func testTheTwoRefusals() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tool = zh ? "打开链配置工具" : "Open Chain Setup Tool"
            for surface in ["settings", "dapp"] {
                for kind in ["none", "missing"] {
                    let tag = "\(surface)-\(kind)-\(look.lang)-\(look.theme)"
                    let app = launch(env: ["VELA_PAGE": "pr3", "VELA_STATE": "p256-\(surface)-\(kind)"],
                                     lang: look.lang, theme: look.theme)
                    let badge = app.staticTexts[zh ? "不兼容" : "Incompatible"].firstMatch
                    XCTAssertTrue(badge.waitForExistence(timeout: 20), "no refusal (\(tag))")
                    let line = NSPredicate(format: "label BEGINSWITH %@", kind == "none"
                        ? (zh ? "这个网络无法验证通行密钥签名" : "This network can't check passkey signatures")
                        : (zh ? "这个网络上还缺少 Vela 需要的部分合约" : "Some contracts Vela needs aren't on this network yet"))
                    XCTAssertTrue(app.staticTexts.matching(line).firstMatch.exists, "the refusal does not say why (\(tag))")
                    XCTAssertEqual(app.buttons[tool].exists, kind == "missing",
                                   kind == "none" ? "a button to a tool that cannot help (\(tag))" : "no Chain Setup (\(tag))")
                    settle(0.8)
                    attach(app, "p256-\(tag)")
                    app.swipeUp()
                    settle(0.8)
                    attach(app, "p256-\(tag)-scrolled")
                    app.terminate()
                }
            }
        }
    }

    // MARK: - The boards: home, recipient rows, Add passkeys, a contact's page

    /// The galleries' boards for the issues with nothing to walk: each one
    /// asserts the thing it shows.
    func testTheBoards() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            func board(_ env: [String: String], _ name: String, _ check: (XCUIApplication) -> Void) {
                let app = launch(env: env, lang: look.lang, theme: look.theme)
                settle(2.5)
                check(app)
                attach(app, "\(name)-\(tag)")
                app.terminate()
            }
            let scan = zh ? "扫描二维码" : "Scan a QR code"

            // Issue #469: the home's Activity is three rows, under "All".
            board(["VELA_PAGE": "gallery", "VELA_STATE": "h1s"], "469-home-three") { app in
                // A board is a picture: its links are words, not buttons.
                XCTAssertTrue(app.staticTexts[zh ? "全部" : "All"].firstMatch.waitForExistence(timeout: 20), "no All link (\(tag))")
                XCTAssertFalse(app.staticTexts["+50"].exists, "the fourth activity row is on the home (\(tag))")
                XCTAssertTrue(app.staticTexts["+120"].exists, "the newest rows are not on the home (\(tag))")
            }

            // Issue #471: the recipient row — the address book and a scan
            // icon, on the single field and on every split row; the picker
            // is contacts only.
            board(["VELA_PAGE": "flows-gallery", "VELA_STATE": "sd2"], "471-recipient-single") { app in
                XCTAssertTrue(app.buttons[scan].firstMatch.waitForExistence(timeout: 20), "no scan icon (\(tag))")
                XCTAssertEqual(app.buttons.matching(identifier: scan).count, 1)
            }
            board(["VELA_PAGE": "flows-gallery", "VELA_STATE": "sd2b"], "471-recipient-split") { app in
                XCTAssertTrue(app.buttons[scan].firstMatch.waitForExistence(timeout: 20), "no scan icon on a split row (\(tag))")
                XCTAssertEqual(app.buttons.matching(identifier: scan).count, 3, "one scan icon per row (\(tag))")
                let picks = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "send.row.pick."))
                XCTAssertEqual(picks.count, 3, "one address-book icon per row (\(tag))")
            }
            board(["VELA_PAGE": "flows-gallery", "VELA_STATE": "sd2e"], "471-picker-contacts-only") { app in
                XCTAssertTrue(app.staticTexts["Alice"].firstMatch.waitForExistence(timeout: 20), "no picker (\(tag))")
                XCTAssertFalse(app.staticTexts[zh ? "扫码填写地址" : "Scan to fill the address"].exists,
                               "the picker still offers a scan row (\(tag))")
            }

            // Issue #479: a contact's page offers Send, and only Send.
            board(["VELA_PAGE": "contacts-gallery", "VELA_STATE": "c2"], "479-contact-send-only") { app in
                XCTAssertTrue(app.staticTexts[zh ? "转账" : "Send"].firstMatch.waitForExistence(timeout: 20), "no Send (\(tag))")
                for gone in zh ? ["收款", "二维码"] : ["Receive", "QR code"] {
                    XCTAssertFalse(app.staticTexts[gone].exists || app.buttons[gone].exists, "\(gone) is back (\(tag))")
                }
            }

            // Issue #475: Add passkeys with no key, one, two, and at the cap.
            for (fixture, name) in [
                ("keys · none", "475-keys-none"),
                ("keys · signing page offered", "475-keys-none-page-offered"),
                ("keys · one, needs a second", "475-keys-one"),
                ("keys · two, ready", "475-keys-two"),
                ("keys · at the cap", "475-keys-cap"),
            ] {
                board(["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": fixture], name) { app in
                    let heading = app.descendants(matching: .any)["create.addHeading"].firstMatch
                    XCTAssertTrue(heading.waitForExistence(timeout: 20), "no heading over the places (\(fixture), \(tag))")
                    let place = app.staticTexts[zh ? "手机或平板" : "Phone or tablet"]
                    switch fixture {
                    case "keys · none", "keys · signing page offered":
                        XCTAssertEqual(heading.label, zh ? "添加通行密钥" : "Add a passkey")
                        XCTAssertTrue(place.exists, "the three places are not open with no key (\(tag))")
                    case "keys · at the cap":
                        XCTAssertEqual(heading.label, zh ? "已达上限 7 把" : "Limit of 7 reached")
                        XCTAssertFalse(place.exists)
                    default:
                        XCTAssertEqual(heading.label, zh ? "再添加一把" : "Add another")
                        XCTAssertFalse(place.exists, "the places are not folded with a key (\(tag))")
                    }
                }
            }
            // …and "Add another" unfolded.
            let app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · two, ready"],
                             lang: look.lang, theme: look.theme)
            let heading = app.descendants(matching: .any)["create.addHeading"].firstMatch
            XCTAssertTrue(heading.waitForExistence(timeout: 20))
            heading.tap()
            XCTAssertTrue(app.staticTexts[zh ? "手机或平板" : "Phone or tablet"].waitForExistence(timeout: 5),
                          "Add another did not unfold the places (\(tag))")
            settle(0.8)
            attach(app, "475-keys-two-unfolded-\(tag)")
            app.terminate()
        }
    }

    /// The sign-in sheet lists the same three rows the create screen does
    /// (issue #475: Settings metrics, a hairline between rows, one line under
    /// each title), with the signing-page entry apart.
    func testTheSignInSheet() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: [:], args: ["-vela.parallelSpace", "0"], lang: look.lang, theme: look.theme)
            let signIn = app.buttons[zh ? "我已有钱包" : "I already have a wallet"]
            XCTAssertTrue(signIn.waitForExistence(timeout: 40), "no Welcome (\(tag))")
            signIn.tap()
            for place in zh ? ["这台设备", "手机或平板", "USB 安全密钥"] : ["This device", "Phone or tablet", "USB security key"] {
                XCTAssertTrue(app.staticTexts[place].waitForExistence(timeout: 10), "no \(place) (\(tag))")
            }
            settle(1)
            attach(app, "475-signin-sheet-\(tag)")
            app.terminate()
        }
    }

    // MARK: - The live home: the newest three, the person's currency, a hidden detail

    /// A read-only account over real chains, with five transfers seeded.
    private static let me = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"

    private static let seeded = ["11.17", "22.27", "33.37", "44.47", "55.57"]

    private static func history(now: Int) -> [[String: Any]] {
        func transfer(_ index: Int, _ value: String, _ symbol: String, sent: Bool) -> [String: Any] {
            let other = "0x" + String(repeating: String(format: "%x", 10 + index), count: 40)
            return [
                "id": "pr3-\(index)", "userOpHash": sent ? "0x" + String(repeating: "5\(index)", count: 32) : "",
                "txHash": "0x" + String(repeating: "a\(index)", count: 32),
                "from": sent ? me : other, "to": sent ? other : me,
                "value": value, "symbol": symbol, "decimals": 6, "chainId": 1,
                "timestamp": now - index * 900, "status": "confirmed",
                "type": sent ? "send" : "receive", "usd": value,
            ]
        }
        // Newest first: the first three are what the home draws; the last
        // two are History's alone. Figures no balance on the page repeats.
        return zip(1..., seeded).map { index, value in
            transfer(index, value, index % 2 == 1 ? "USDC" : "USDT", sent: index % 2 == 0)
        }
    }

    private static func argument(_ records: [[String: Any]]) -> String {
        let json = String(data: try! JSONSerialization.data(withJSONObject: records), encoding: .utf8)!
        return "\"" + json.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    private func launchLive(lang: String, theme: String, args: [String] = []) -> XCUIApplication {
        launch(
            env: ["VELA_ACCOUNT": Self.me],
            args: ["-vela.parallelSpace", "0", "-vela.transactionHistory",
                   Self.argument(Self.history(now: Int(Date().timeIntervalSince1970)))] + args,
            lang: lang, theme: theme
        )
    }

    /// Issue #469, live: five transfers recorded, three on the home under
    /// "All", and all five in History — the same rows, in the same order.
    func testTheLiveHomeDrawsTheNewestThree() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launchLive(lang: look.lang, theme: look.theme)
            XCTAssertTrue(app.staticTexts[zh ? "资产" : "Assets"].firstMatch.waitForExistence(timeout: 40),
                          "the home never appeared (\(tag))")
            // The feed is the store's: it does not wait for the chains.
            func figure(_ digits: String) -> XCUIElement {
                app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", digits)).firstMatch
            }
            XCTAssertTrue(figure(Self.seeded[0]).waitForExistence(timeout: 20), "the newest transfer is not on the home (\(tag))")
            settle(6)
            for shown in Self.seeded.prefix(3) { XCTAssertTrue(figure(shown).exists, "\(shown) is not on the home (\(tag))") }
            for held in Self.seeded.suffix(2) { XCTAssertFalse(figure(held).exists, "\(held) is on the home: more than three (\(tag))") }
            attach(app, "469-live-home-\(tag)")

            // "All" (Activity's — the upper of the two) opens History, whole.
            let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", zh ? "全部" : "All"))
                .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
            XCTAssertNotNil(all, "no All link over Activity (\(tag))")
            all?.tap()
            settle(2)
            for every in Self.seeded {
                XCTAssertTrue(figure(every).exists, "\(every) is missing from History (\(tag))")
            }
            attach(app, "469-live-history-\(tag)")
            app.terminate()
        }
    }

    /// Item 10, live: a cold start with CNY stored. No frame names the
    /// dollar or draws a dollar total; the hero waits under "CNY" and the
    /// figure arrives once, in yuan.
    func testAColdStartNeverShowsDollarsFirst() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launchLive(lang: look.lang, theme: look.theme, args: ["-vela.displayCurrency", "CNY"])
            let label = app.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", zh ? "总余额" : "Total balance")
            ).firstMatch
            var sawYuan = false
            for (index, wait) in [0.2, 0.4, 0.6, 1.0, 1.5, 2.5, 4.0, 8.0, 12.0].enumerated() {
                settle(wait)
                attach(app, "item10-cold-start-\(index)-\(tag)")
                guard label.exists else { continue }
                XCTAssertFalse(label.label.contains("USD"), "frame \(index) names the dollar: \(label.label) (\(tag))")
                let dollars = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "$")).count
                XCTAssertEqual(dollars, 0, "frame \(index) draws a dollar figure (\(tag))")
                if app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "¥")).count > 0 { sawYuan = true }
            }
            XCTAssertTrue(label.exists, "the hero never appeared (\(tag))")
            XCTAssertTrue(label.label.hasSuffix("CNY"), "the hero is not named by the stored choice: \(label.label) (\(tag))")
            XCTAssertTrue(sawYuan, "no yuan figure arrived in 30 s (\(tag))")
            app.terminate()
        }
    }

    /// Item 12, live: the balance hidden, a transfer opened from History.
    /// Its figure is the mask AND its unit — "•••• USDC".
    func testAHiddenTransferDetailKeepsItsUnit() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launchLive(lang: look.lang, theme: look.theme, args: ["-vela.balanceHidden", "1"])
            XCTAssertTrue(app.staticTexts[zh ? "资产" : "Assets"].firstMatch.waitForExistence(timeout: 40),
                          "the home never appeared (\(tag))")
            settle(6)
            attach(app, "item12-hidden-home-\(tag)")
            let row = app.staticTexts[zh ? "已收到" : "Received"].firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 10), "no received row on the home (\(tag))")
            row.tap()
            let masked = app.staticTexts.matching(NSPredicate(format: "label == %@", "•••• USDC")).firstMatch
            XCTAssertTrue(masked.waitForExistence(timeout: 10), "the hidden detail does not read \"•••• USDC\" (\(tag))")
            XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", Self.seeded[0])).firstMatch.exists,
                           "the amount is drawn while hidden (\(tag))")
            settle(1)
            attach(app, "item12-hidden-transfer-detail-\(tag)")
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
