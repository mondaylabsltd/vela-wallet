//
//  PR3DeviceTests.swift
//  VelaWalletUITests
//
//  The UI batch's walk on a phone (PR 3 device round). Every test is
//  read-only towards the person's real wallet: account-touching steps run in
//  the parallel space's test wallet, nothing is ever confirmed, signed or
//  sent — every dApp request and every sheet is closed with its ✕ or its
//  Cancel — no wallet is created, no key registered, no report submitted,
//  and no setting is moved.
//
//  Two of the steps put something on the screen that must not be recorded:
//  the scanner's live viewfinder (`testScannerOpens`) and the phone-link
//  code, which carries session material (`testScanACodeSheet`). Run those
//  two from an .xctestrun with automatic screen capture OFF
//  (`SystemAttachmentLifetime = keepNever`): they take no picture of their
//  own while it is up — the viewfinder is verified by its accessibility
//  elements, and the code is painted over on the phone before the picture is
//  kept. Every other test's recording is worth keeping (`keepAlways`): the
//  cold start is read from it frame by frame. A launch recording also holds
//  a few frames of the phone's own home screen, as the system opens the app.
//
//  - `testColdStart` — the person's own wallet, no pins: "Checking…" first,
//    never a dollar before the stored currency. Read-only.
//  - `testHomeThreeAndHistory` (#469), `testSendRecipientRow` and
//    `testScannerOpens` (#471), `testAddPasskeysScreen` (#475, no key added),
//    `testReportAProblemKeyboard` (#478, nothing sent, nothing kept),
//    `testContactPage` (#479, an existing contact), `testScanACodeSheet`
//    (#480), `testSecurityKeyFallsToApplesSheet` (a Lightning iPhone reaches
//    Apple's sheet; cancelled), `testCopyTheRecordRowAndSheet`.
//  - `testSigningSheets` — the local test dApp's message, send and approval:
//    the confirm, the fee row, the line under the confirm and the verdict's
//    place, read from one snapshot of the tree per tick while the fee and
//    the verdict land. Every request is declined.
//  - `testVerdictBoards` — `VELA_PAGE=pr3c`: a tall verdict (four balance
//    rows and the unverified-token warning) whole on the phone's own screen,
//    landing late, with the ✕ and the confirm where they were.
//  - `testHiddenBalanceKeepsUnits` — `KEPT_HIDDEN` is the person's own
//    setting, read before the walk; it is put back.
//  - `testLeaveSpace` — `VELA_PARALLEL_SPACE=0`: back to the wallet that was
//    in front. `PR2PolishDeviceTests/testState` before and after must match.
//
//  The three last fixes (2026-10-11):
//  - `testReceiptsStandUnderTheirOwnDay` — LOOK ONLY, the person's own
//    wallet: the home's Activity read per tick while the stored receipts'
//    times are repaired, then History. Opens no sheet, starts no request.
//  - `testConfirmWaitsForTheVerdict` — the space: the test dApp's message
//    and its "Send dust", the confirm and the line under it sampled from the
//    first frame. Both DECLINED.
//  - `testConfirmWaitBoards` — `VELA_PAGE=pr3c`: the held confirm and the
//    waited-out verdict on the phone's own screen. Nothing is tapped.
//
//  Skipped in the scheme (they need the phone, the network and the space);
//  run them by name from a copy of the .xctestrun with the skips removed.
//

import UIKit
import XCTest

final class PR3DeviceTests: XCTestCase {

    private var server: LocalDappServer?
    /// The app as a test in the space launched it — so that a test which
    /// stops half-way never leaves a request's sheet open behind it.
    private var spaceApp: XCUIApplication?

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    override func tearDownWithError() throws {
        // A request still on the sheet is REFUSED before anything else can
        // happen to the app: the test dApp's "Send dust" pays the person's
        // real address, and a sheet must never be left open unattended.
        if let app = spaceApp, app.state == .runningForeground {
            let close = app.buttons["signing.close"].firstMatch
            if close.exists {
                close.tap()
                XCTFail("a signing sheet was still open when the test ended — it was closed by its ✕ (refused)")
            }
        }
        spaceApp = nil
        server?.stop()
        server = nil
    }

    // MARK: - 1. Cold start: "Checking…" first, never a dollar

    /// The person's own wallet, exactly as they launch it: no pins. The
    /// hero's caption, figure and status line are read as fast as the tree
    /// can be asked, and the screen recording carries every frame between.
    func testColdStart() throws {
        for skipAnimation in [false, true] {
            let tag = skipAnimation ? "skip" : "real"
            let app = XCUIApplication()
            app.terminate()
            if skipAnimation { app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1" }
            let started = Date()
            app.launch()
            var lines: [String] = []
            var index = 0
            while Date().timeIntervalSince(started) < 28 {
                let at = Date().timeIntervalSince(started)
                let caption = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "总余额")).firstMatch
                let checking = app.descendants(matching: .any)["balance-checking"].firstMatch
                // A dollar FIGURE ("$…", "≈$…") or the hero naming the dollar.
                // Not a coin's own name: "USDT" in the holdings is not money
                // drawn in the wrong currency.
                let dollars = app.staticTexts.matching(NSPredicate(
                    format: "label BEGINSWITH %@ OR label CONTAINS %@ OR (label BEGINSWITH %@ AND label CONTAINS %@)",
                    "$", "≈$", "总余额", "USD"
                ))
                let yuan = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "¥")).firstMatch
                let status = app.staticTexts.matching(NSPredicate(
                    format: "label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@",
                    "正在检查", "已更新", "连不上", "实时"
                )).firstMatch
                let line = String(
                    format: "t=%.2f caption=%@ checking=%@ status=%@ yuan=%@ dollars=%d",
                    at,
                    caption.exists ? caption.label : "-",
                    checking.exists ? checking.label : "-",
                    status.exists ? status.label : "-",
                    yuan.exists ? yuan.label : "-",
                    dollars.count
                )
                lines.append(line)
                if dollars.count > 0 {
                    XCTFail("a dollar on a CNY phone at \(line)")
                    shoot(app, "cold-\(tag)-DOLLAR-\(index)")
                }
                if index < 12 || index % 4 == 0 { shoot(app, String(format: "cold-%@-%02d", tag, index)) }
                index += 1
                if at > 6 { settle(1.2) }
            }
            note("cold-\(tag)-timeline", lines.joined(separator: "\n"))
            dump(app, "cold-\(tag)-end")
            app.terminate()
        }
    }

    // MARK: - 2. Issue #469: three on the home, all in History

    func testHomeThreeAndHistory() throws {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(8)
        shoot(app, "469-1-home")
        dump(app, "469-1-home")
        swipePage(app, from: 0.75, to: 0.4)
        settle(1)
        shoot(app, "469-2-home-lower")
        dump(app, "469-2-home-lower")
        for _ in 0..<2 { swipePage(app, from: 0.35, to: 0.85) }
        settle(1)
        // "全部" over Activity is the upper of the two.
        let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
            .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        XCTAssertNotNil(all, "no 全部 over Activity")
        all?.tap()
        settle(3)
        shoot(app, "469-3-history")
        dump(app, "469-3-history")
        swipePage(app, from: 0.8, to: 0.3)
        settle(1)
        shoot(app, "469-4-history-lower")
        dump(app, "469-4-history-lower")
        closeTop(app)
        app.terminate()
    }

    // MARK: - 3. Issue #471: the recipient row's icons; the picker has no scan row

    func testSendRecipientRow() throws {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(6)
        app.buttons["转账"].firstMatch.tap()
        settle(3)
        shoot(app, "471-0-token-picker")
        dump(app, "471-0-token-picker")
        // The coin first (the space's wallet holds BNB), then the form.
        let coin = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "BNB")).firstMatch
        XCTAssertTrue(coin.waitForExistence(timeout: 8), "no coin to send")
        coin.tap()
        settle(3)
        shoot(app, "471-1-send")
        dump(app, "471-1-send")
        let scanLabel = NSPredicate(format: "label == %@ OR identifier BEGINSWITH %@", "扫描二维码", "send.row.scan.")
        let scans = app.buttons.matching(scanLabel)
        let picks = app.buttons.matching(NSPredicate(
            format: "label == %@ OR label == %@ OR identifier BEGINSWITH %@", "通讯录", "从通讯录选择", "send.row.pick."
        ))
        note("471-1-icons", "pick \(picks.count) scan \(scans.count)")
        XCTAssertEqual(scans.count, 1, "the recipient row has one scan icon")
        XCTAssertEqual(picks.count, 1, "the recipient row has one contacts icon")
        if scans.count > 0, picks.count > 0 {
            let scan = scans.element(boundBy: 0).frame, pick = picks.element(boundBy: 0).frame
            note("471-1-frames", "pick \(pick) scan \(scan)")
            XCTAssertEqual(scan.midY, pick.midY, accuracy: 1, "the two icons are not on one line")
        }

        // The contacts icon: the picker, with no "scan to fill" row.
        if picks.count > 0 {
            picks.element(boundBy: 0).tap()
            settle(2.5)
            shoot(app, "471-2-picker")
            dump(app, "471-2-picker")
            XCTAssertFalse(app.staticTexts["扫码填写地址"].exists, "the picker still offers a scan row")
            XCTAssertFalse(app.buttons["扫码填写地址"].exists, "the picker still offers a scan row")
            closeTop(app)
            settle(1)
            shoot(app, "471-2-picker-closed")
        }
        // A second recipient: its row has both icons too.
        let add = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "添加收款人")).firstMatch
        note("471-3-add", add.exists ? add.label : "no add-recipient control found")
        if add.exists {
            reachElement(add, in: app)
            add.tap()
            settle(2.5)
            shoot(app, "471-3-multi")
            dump(app, "471-3-multi")
            let rowPicks = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "send.row.pick."))
            let rowScans = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "send.row.scan."))
            note("471-3-icons", "pick \(rowPicks.count) scan \(rowScans.count)")
            XCTAssertGreaterThanOrEqual(rowScans.count, 2, "a recipient row has no scan icon")
            XCTAssertEqual(rowPicks.count, rowScans.count, "a row is missing one of its icons")
            swipePage(app, from: 0.7, to: 0.4)
            settle(1)
            shoot(app, "471-4-multi-lower")
        }
        // Stop here: nothing typed, nothing confirmed. Out by back / close.
        var backs = 0
        while backs < 6, !app.buttons["设置"].exists {
            closeTop(app)
            backs += 1
        }
        settle(1)
        shoot(app, "471-9-left")
        app.terminate()
    }

    /// The scan icon opens the scanner. NO picture and NO recording while it
    /// is up (run with screen capture off): it is checked by what the
    /// accessibility tree says is on screen, then closed.
    func testScannerOpens() throws {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(6)
        app.buttons["转账"].firstMatch.tap()
        settle(3)
        let coin = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "BNB")).firstMatch
        XCTAssertTrue(coin.waitForExistence(timeout: 8), "no coin to send")
        coin.tap()
        settle(3)
        let scans = app.buttons.matching(NSPredicate(format: "label == %@ OR identifier BEGINSWITH %@", "扫描二维码", "send.row.scan."))
        XCTAssertGreaterThanOrEqual(scans.count, 1, "no scan icon")
        guard scans.count > 0 else { return }
        let before = app.debugDescription
        scans.element(boundBy: 0).tap()
        settle(3)
        note("471-scan-tree-before", before)
        // A system prompt for the camera is the person's to answer.
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let prompt = springboard.alerts.firstMatch
        if prompt.exists {
            note("471-scan-permission-prompt", prompt.debugDescription)
            XCTFail("the camera permission prompt is up: left for the owner")
            return
        }
        // Words only — never a picture.
        note("471-scan-tree", app.debugDescription)
        let closes = app.buttons.matching(NSPredicate(
            format: "label == %@ OR label == %@ OR label == %@ OR label == %@", "关闭", "取消", "Close", "Cancel"
        ))
        note("471-scan-closes", "\(closes.count)")
        if closes.count > 0 {
            closes.allElementsBoundByIndex.first { $0.isHittable }?.tap()
        } else {
            dismissSheet(app)
        }
        settle(2)
        // The viewfinder is gone: pictures are allowed again.
        let stillUp = app.descendants(matching: .any)["scan.fixtureFrame"].exists
        note("471-scan-after-close", "fixtureFrame \(stillUp)")
        closeTop(app)
        settle(1)
        shoot(app, "471-scan-closed")
        app.terminate()
    }

    // MARK: - 4. Issue #475: Add passkeys

    func testAddPasskeysScreen() throws {
        let app = launchInSpace(page: false)
        openSettings(app)
        let row = app.staticTexts["切换账户"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 切换账户")
        row.tap()
        settle(2)
        shoot(app, "475-1-switcher")
        dump(app, "475-1-switcher")
        let create = app.buttons["创建新账户"].firstMatch
        XCTAssertTrue(create.waitForExistence(timeout: 5), "no 创建新账户")
        create.tap()
        settle(3)
        shoot(app, "475-2-create")
        dump(app, "475-2-create")
        // Whatever comes first (a name, an intro): photographed; the keys
        // screen is the one with the heading.
        let heading = app.descendants(matching: .any)["create.addHeading"].firstMatch
        // The name step: a throwaway name and its three acknowledgements.
        // Nothing is created by them — a wallet exists only once a key is
        // added, and none is.
        let field = app.textFields.firstMatch
        if !heading.exists, field.waitForExistence(timeout: 5) {
            field.tap()
            field.typeText("PR3 walk")
            settle(0.8)
            // The keyboard away, so the boxes under it take taps.
            let title = app.staticTexts["给钱包起个名字"].firstMatch
            if title.exists { title.tap() }
            settle(0.8)
            for prefix in ["我的公钥", "我的私钥", "我已阅读"] {
                let box = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", prefix)).firstMatch
                if box.exists, (box.value as? String) != "1" { box.tap() }
            }
            settle(0.8)
            shoot(app, "475-2-create-named")
            let next = app.buttons["继续"].firstMatch
            XCTAssertTrue(next.isEnabled, "Continue did not open")
            if next.isEnabled { next.tap() }
            settle(3)
        }
        XCTAssertTrue(heading.exists, "the Add passkeys screen was not reached")
        if heading.exists {
            note("475-heading", heading.label)
            XCTAssertEqual(heading.label, "选择存放位置")
            let count = app.descendants(matching: .any)["create.keyCount"].firstMatch
            note("475-keyCount", count.exists ? count.label : "absent")
            XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "0 / 7")).firstMatch.exists,
                           "\"0 / 7\" is on the screen")
            let page = app.staticTexts["使用可信签名页"].firstMatch
            note("475-signing-page-row", page.exists ? "\(page.frame)" : "absent")
            shoot(app, "475-3-keys")
            dump(app, "475-3-keys")
            swipePage(app, from: 0.7, to: 0.4)
            settle(1)
            shoot(app, "475-4-keys-lower")
            dump(app, "475-4-keys-lower")
        }
        // Out, with no key added: back / close until the tab bar is back.
        var backs = 0
        while backs < 6, !app.buttons["设置"].exists {
            closeTop(app)
            backs += 1
        }
        settle(1)
        shoot(app, "475-9-left")
        dump(app, "475-9-left")
        app.terminate()
    }

    // MARK: - 5. Issue #478: "Report a problem" and the keyboard

    func testReportAProblemKeyboard() throws {
        let app = launchInSpace(page: false)
        openSettings(app)
        let row = app.staticTexts["反馈"].firstMatch
        reachElement(row, in: app)
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 反馈 row")
        row.tap()
        settle(2.5)
        shoot(app, "478-1-sheet")
        dump(app, "478-1-sheet")
        let what = app.textViews["feedback.what"]
        guard what.waitForExistence(timeout: 10) else {
            XCTFail("no report sheet")
            return
        }
        let keyboard = app.keyboards.firstMatch
        let done = app.buttons["keyboard.done"].firstMatch
        let send = app.buttons["feedback.send"]
        let before = what.value as? String ?? ""
        note("478-draft-before", before)

        // Typing: the keyboard, a Done bar over it, Return a new line.
        what.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "no keyboard")
        what.typeText("device walk\nnot to be sent")
        settle(1)
        shoot(app, "478-2-keyboard-up")
        XCTAssertTrue(done.waitForExistence(timeout: 5), "no Done over the keys")
        note("478-done-label", done.exists ? done.label : "absent")

        // Send is reached with the keyboard still up.
        var drags = 0
        while !send.isHittable, drags < 8 {
            slowDrag(app, from: 0.42, to: 0.16)
            drags += 1
        }
        XCTAssertTrue(keyboard.exists, "scrolling up to Send dropped the keyboard")
        XCTAssertTrue(send.isHittable, "Send is not reachable above the keyboard")
        shoot(app, "478-3-send-above-keyboard")

        // Done puts the keyboard away.
        done.tap()
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "Done left the keyboard up")
        settle(0.6)
        shoot(app, "478-4-after-done")

        // A tap outside a field does too.
        slowDrag(app, from: 0.2, to: 0.45)
        what.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "the field did not take the keyboard back")
        let title = app.staticTexts["反馈问题"].firstMatch
        if title.exists, title.isHittable { title.tap() } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap()
        }
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "a tap outside left the keyboard up")
        shoot(app, "478-5-after-tap-outside")

        // And a scroll of the page.
        what.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
        slowDrag(app, from: 0.42, to: 0.16)
        slowDrag(app, from: 0.42, to: 0.16)
        XCTAssertTrue(keyboard.exists, "scrolling up dropped the keyboard")
        slowDrag(app, from: 0.3, to: 0.72)
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5), "a drag left the keyboard up")
        XCTAssertTrue(send.exists, "the drag closed the sheet")
        shoot(app, "478-6-after-scroll")

        // Leave WITHOUT sending.
        closeTop(app)
        settle(1.5)
        shoot(app, "478-9-left")
        XCTAssertFalse(app.buttons["feedback.send"].exists, "the report sheet is still up")

        // Opened again, it is empty: the words of the walk were kept nowhere.
        let again = app.staticTexts["反馈"].firstMatch
        reachElement(again, in: app)
        if again.exists { again.tap() }
        let field = app.textViews["feedback.what"]
        if field.waitForExistence(timeout: 8) {
            let kept = field.value as? String ?? ""
            note("478-draft-on-reopen", kept.isEmpty ? "(empty)" : kept)
            XCTAssertFalse(kept.contains("device walk"), "the unsent words are still in the report sheet")
            shoot(app, "478-10-reopened-empty")
            closeTop(app)
            settle(1.5)
        } else {
            XCTFail("the report sheet did not open a second time")
        }
        app.terminate()
    }

    // MARK: - 6. Issue #479: a contact's page is Send only

    func testContactPage() throws {
        let app = launchInSpace(page: false)
        tapTab(["通讯录", "Contacts"], in: app)
        settle(3)
        shoot(app, "479-1-contacts")
        dump(app, "479-1-contacts")
        // A contact that is already there (none is written by the walk).
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "hzhs")).firstMatch
        let fallback = app.staticTexts["hzhs"].firstMatch
        guard row.exists || fallback.exists else {
            note("479-no-contact", "the space has no contact: the board is the evidence")
            app.terminate()
            return
        }
        (row.exists ? row : fallback).tap()
        settle(3)
        shoot(app, "479-2-contact-page")
        dump(app, "479-2-contact-page")
        let actions = app.buttons.allElementsBoundByIndex.filter { $0.isHittable }.map { $0.label }
        note("479-2-buttons", actions.joined(separator: " | "))
        XCTAssertTrue(app.buttons["转账"].firstMatch.exists || app.staticTexts["转账"].firstMatch.exists, "no 转账")
        for gone in ["收款", "二维码"] {
            XCTAssertFalse(app.staticTexts[gone].exists || app.buttons[gone].exists, "\(gone) is on a contact's page")
        }
        swipePage(app, from: 0.7, to: 0.35)
        settle(1)
        shoot(app, "479-3-contact-page-lower")
        // Read only: nothing on the page was tapped but the page itself, so
        // it is left by ending the app.
        app.terminate()
    }

    /// The board, when the space has no contact of its own (nothing is
    /// written to the phone's contacts).
    func testContactPageBoard() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PAGE"] = "contacts-gallery"
        app.launchEnvironment["VELA_STATE"] = "c2"
        app.launch()
        settle(4)
        shoot(app, "479-2-contact-board")
        dump(app, "479-2-contact-board")
        XCTAssertTrue(app.staticTexts["转账"].firstMatch.exists || app.buttons["转账"].firstMatch.exists, "no 转账")
        for gone in ["收款", "二维码"] {
            XCTAssertFalse(app.staticTexts[gone].exists || app.buttons[gone].exists, "\(gone) is on a contact's page")
        }
        app.terminate()
    }

    // MARK: - 7. Issue #480: the scan-a-code sheet (NO recording; the code is painted over)

    func testScanACodeSheet() throws {
        let app = launchInSpace(page: false)
        openSettings(app)
        let row = app.staticTexts["切换账户"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 切换账户")
        row.tap()
        settle(2)
        let signIn = app.buttons["登录已有账户"].firstMatch
        XCTAssertTrue(signIn.waitForExistence(timeout: 5), "no 登录已有账户")
        signIn.tap()
        settle(2.5)
        shoot(app, "480-1-methods")
        dump(app, "480-1-methods")
        let phone = app.staticTexts["手机或平板"].firstMatch
        XCTAssertTrue(phone.waitForExistence(timeout: 10), "no 手机或平板")
        phone.tap()
        let cancel = app.buttons["cable.cancel"]
        XCTAssertTrue(cancel.waitForExistence(timeout: 20), "the code never came up")
        settle(1.5)
        // The tree first (frames, no pixels), then a picture with everything
        // between the title and Cancel painted over.
        note("480-2-sheet-tree", app.debugDescription)
        let title = app.staticTexts["手机或平板"].firstMatch
        let screen = app.frame
        note("480-2-frames", "screen \(screen) title \(title.frame) cancel \(cancel.frame) label \(cancel.label)")
        XCTAssertEqual(cancel.label, "取消")
        XCTAssertTrue(cancel.isHittable, "Cancel is off the sheet")
        XCTAssertGreaterThan(title.frame.minY, screen.height * 0.2, "the sheet is still near-full-height")
        XCTAssertGreaterThan(cancel.frame.maxY, screen.height * 0.86, "Cancel is not at the bottom")
        shootRedacted(app, "480-2-sheet-code-redacted", hide: CGRect(
            x: 0, y: title.frame.maxY + 4, width: screen.width, height: max(0, cancel.frame.minY - title.frame.maxY - 8)
        ))
        cancel.tap()
        XCTAssertTrue(cancel.waitForNonExistence(timeout: 6), "Cancel left the code up")
        settle(2)
        shoot(app, "480-3-after-cancel")
        dump(app, "480-3-after-cancel")
        var backs = 0
        while backs < 4, !app.buttons["设置"].exists {
            closeTop(app)
            backs += 1
        }
        shoot(app, "480-9-left")
        app.terminate()
    }

    // MARK: - 8. Note 29: a Lightning iPhone reaches Apple's security-key sheet

    func testSecurityKeyFallsToApplesSheet() throws {
        let app = launchInSpace(page: false)
        openSettings(app)
        let row = app.staticTexts["切换账户"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 切换账户")
        row.tap()
        settle(2)
        let signIn = app.buttons["登录已有账户"].firstMatch
        XCTAssertTrue(signIn.waitForExistence(timeout: 5), "no 登录已有账户")
        signIn.tap()
        settle(2.5)
        let method = app.staticTexts["USB 安全密钥"].firstMatch
        XCTAssertTrue(method.waitForExistence(timeout: 10), "no USB 安全密钥 row")
        method.tap()
        let insert = app.staticTexts["插入安全密钥"].firstMatch
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        // What comes up, second by second: the app's tree, and the whole
        // screen (the system's sheet is not the app's to photograph).
        for (index, wait) in [0.6, 1.0, 1.5, 2.0, 3.0, 4.0].enumerated() {
            settle(wait)
            shootScreen("sk-\(index)-screen")
            note("sk-\(index)-app", "insert sheet \(insert.exists) · app state \(app.state.rawValue)")
        }
        XCTAssertFalse(insert.exists, "a Lightning iPhone was made to wait for a USB-C key")
        note("sk-springboard-tree", springboard.debugDescription)
        for bundle in ["com.apple.AuthenticationServicesUI", "com.apple.AuthKitUIService", "com.apple.CTKUIService",
                       "com.apple.PasswordManagerUIService", "com.apple.CoreAuthUI"] {
            let other = XCUIApplication(bundleIdentifier: bundle)
            if other.state != .notRunning, other.state != .unknown {
                note("sk-tree-\(bundle)", other.debugDescription)
            }
        }
        // CANCEL it: never a key, never a ceremony.
        var cancelled = false
        for host in [springboard, app] + ["com.apple.AuthenticationServicesUI", "com.apple.AuthKitUIService"].map({
            XCUIApplication(bundleIdentifier: $0)
        }) {
            let cancel = host.buttons.matching(NSPredicate(
                format: "label == %@ OR label == %@ OR label == %@ OR label == %@", "取消", "Cancel", "关闭", "Close"
            )).firstMatch
            if cancel.exists {
                note("sk-cancel-host", "\(host) \(cancel.label) \(cancel.frame)")
                cancel.tap()
                cancelled = true
                break
            }
        }
        note("sk-cancelled", "\(cancelled)")
        settle(3)
        shootScreen("sk-9-after-cancel")
        dump(app, "sk-9-after-cancel")
        var backs = 0
        while backs < 4, !app.buttons["设置"].exists {
            closeTop(app)
            backs += 1
        }
        shootScreen("sk-9-left")
        app.terminate()
    }

    // MARK: - 9. "Copy this wallet's record to Ethereum"

    func testCopyTheRecordRowAndSheet() throws {
        let app = launchInSpace(page: false)
        openSettings(app)
        let title = app.staticTexts["把钱包记录复制到以太坊"].firstMatch
        XCTAssertTrue(title.waitForExistence(timeout: 10), "no record row")
        reachElement(title, in: app)
        shoot(app, "copy-1-row-early")
        dump(app, "copy-1-row-early")
        // The row's state settles (checking → …).
        settle(12)
        shoot(app, "copy-2-row-settled")
        dump(app, "copy-2-row-settled")
        title.tap()
        settle(4)
        shoot(app, "copy-3-after-tap")
        dump(app, "copy-3-after-tap")
        let close = app.buttons["signing.close"]
        if close.waitForExistence(timeout: 15) {
            // The fee, as it lands.
            for index in 0..<5 {
                settle(3)
                shoot(app, "copy-4-sheet-\(index)")
            }
            dump(app, "copy-4-sheet")
            swipePage(app, from: 0.7, to: 0.4)
            settle(1)
            shoot(app, "copy-5-sheet-lower")
            // Close WITHOUT confirming.
            close.tap()
            XCTAssertTrue(close.waitForNonExistence(timeout: 8), "the sheet did not close")
        } else {
            note("copy-3-no-sheet", "the row opened no signing sheet (its state may not offer one)")
            closeTop(app)
        }
        settle(1.5)
        shoot(app, "copy-9-left")
        app.terminate()
    }

    // MARK: - 10. The signing sheet: the verdict's place, the fee, nothing moving

    func testSigningSheets() throws {
        startServer()
        let app = launchInSpace()
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30), "the test dApp did not load")
        connect(app)

        // personal_sign: a message has no verdict and no fee.
        app.webViews.buttons["Sign"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet for personal_sign")
        frames(app, "sign-message", seconds: 6)
        shoot(app, "sign-1-message")
        dump(app, "sign-1-message")
        app.buttons["signing.close"].tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict personal_sign err 4001"), "closing did not refuse personal_sign")

        // Send dust: the verdict's place from the first frame, the fee.
        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet for the send")
        frames(app, "sign-send", seconds: 24)
        shoot(app, "sign-2-send")
        dump(app, "sign-2-send")
        swipePage(app, from: 0.7, to: 0.45)
        settle(1)
        shoot(app, "sign-3-send-lower")
        dump(app, "sign-3-send-lower")
        app.buttons["signing.close"].tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001"), "closing did not refuse the send")

        // An approval: what it moves is nothing — the verdict says so, if a node can check.
        app.webViews.buttons["Approve unlimited"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet for the approval")
        frames(app, "sign-approve", seconds: 20)
        shoot(app, "sign-4-approve")
        dump(app, "sign-4-approve")
        swipePage(app, from: 0.7, to: 0.4)
        settle(1)
        shoot(app, "sign-5-approve-lower")
        app.buttons["signing.close"].tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001"), "closing did not refuse the approval")
        settle(1)
        shoot(app, "sign-9-left")
        app.terminate()
    }

    /// The confirm and the simulation's verdict, live, in the space: the test
    /// dApp's message (no simulation: never held) and its "Send dust" (a
    /// transaction: the confirm is shut, with one line, until the verdict is
    /// in its place). Both are DECLINED by the ✕, and each refusal is read
    /// back from the page (4001) before the next step. Nothing is confirmed:
    /// the confirm is never tapped, and "Send dust" pays the person's real
    /// address, so it is refused whatever else happens (`tearDownWithError`).
    ///
    /// Read from one snapshot of the tree per tick (`frames`): whether the
    /// confirm is enabled, the line under it, the verdict's place, and where
    /// the confirm is — which must be one position from the sheet's first
    /// frame to its last.
    func testConfirmWaitsForTheVerdict() throws {
        startServer()
        let app = launchInSpace()
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30), "the test dApp did not load")
        connect(app)

        // personal_sign: no simulation is started for a message.
        app.webViews.buttons["Sign"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet for personal_sign")
        frames(app, "wait-message", seconds: 7)
        shoot(app, "wait-1-message")
        dump(app, "wait-1-message")
        app.buttons["signing.close"].tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict personal_sign err 4001"), "closing did not refuse personal_sign")

        // Send dust: sampled from the sheet's first frame, past the core's
        // four seconds and the fee's own arrival.
        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet for the send")
        frames(app, "wait-send", seconds: 22)
        shoot(app, "wait-2-send")
        dump(app, "wait-2-send")
        app.buttons["signing.close"].tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001"), "closing did not refuse the send")
        settle(1)
        shoot(app, "wait-9-left")
        XCTAssertFalse(app.buttons["signing.close"].exists, "a sheet is still up")
        app.terminate()
    }

    /// Part 1 item 1 on the phone's own screen (`VELA_PAGE=pr3c`): the
    /// presented sheet under "Checking…", "No asset changes", a tall verdict
    /// (four balance rows and the unverified-token warning), and the tall one
    /// landing late — on a plain request and on one long enough to scroll.
    ///
    /// On each: the ✕ and the confirm are on screen and take taps' worth of
    /// room (hittable), every balance row and the warning can be brought
    /// fully between them, and the confirm is where it was after the body
    /// has been scrolled to its end and back.
    func testVerdictBoards() throws {
        let boards = (ProcessInfo.processInfo.environment["VERDICT_BOARDS"]
            ?? "sheet-out,sheet-nothing,sheet-send,sheet-tall,sheet-rich-tall,sheet-lands,sheet-rich-lands")
            .split(separator: ",").map(String.init)
        for board in boards {
            let app = XCUIApplication()
            app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
            app.launchEnvironment["VELA_PAGE"] = "pr3c"
            app.launchEnvironment["VELA_STATE"] = board
            app.launch()
            let confirm = app.buttons["signing.confirm"].firstMatch
            let close = app.buttons["signing.close"].firstMatch
            guard confirm.waitForExistence(timeout: 20) else {
                XCTFail("no confirm on \(board)")
                shoot(app, "verdict-\(board)-MISSING")
                dump(app, "verdict-\(board)-MISSING")
                app.terminate()
                continue
            }
            // As it opens, and while anything lands.
            frames(app, "verdict-\(board)", seconds: board.hasSuffix("lands") ? 9 : 3)
            let rested = confirm.frame
            let screen = app.frame
            shoot(app, "verdict-\(board)-1-rest")
            dump(app, "verdict-\(board)-1-rest")
            XCTAssertTrue(confirm.isHittable, "the confirm is not on screen (\(board))")
            XCTAssertLessThanOrEqual(rested.maxY, screen.maxY, "the confirm runs off the screen (\(board))")
            XCTAssertTrue(close.exists && close.isHittable, "the ✕ is not on screen (\(board))")

            // What the verdict's place holds, and where each line is.
            func lines() -> [(String, CGRect)] {
                app.staticTexts.allElementsBoundByIndex.compactMap { text in
                    let label = text.label
                    let mine = label.hasPrefix("+") || label.hasPrefix("\u{2212}") || label.hasPrefix("-")
                        || label.contains("余额变化") || label.contains("未验证") || label.contains("无资产变动")
                        || label.contains("正在检查") || label.contains("未能检查") || label.contains("无法在链上核实")
                    return mine ? (label, text.frame) : nil
                }
            }
            let top = close.frame.maxY, bottom = rested.minY
            var seen = Set<String>()
            var report: [String] = []
            for pass in 0..<6 {
                for (label, frame) in lines() where frame.minY >= top - 1 && frame.maxY <= bottom + 1 {
                    if seen.insert(label).inserted {
                        report.append(String(format: "pass %d whole: %@ %.0f..%.0f", pass, label, frame.minY, frame.maxY))
                    }
                }
                if pass < 5 {
                    slowDrag(app, from: 0.62, to: 0.38)
                    XCTAssertEqual(confirm.frame.minY, rested.minY, accuracy: 0.5,
                                   "the confirm moved when the body was scrolled (\(board), pass \(pass))")
                    XCTAssertTrue(close.isHittable, "the ✕ scrolled away (\(board), pass \(pass))")
                }
            }
            shoot(app, "verdict-\(board)-2-end")
            dump(app, "verdict-\(board)-2-end")
            let all = Set(lines().map { $0.0 }).union(seen)
            report.append("lines in the place: \(all.sorted().joined(separator: " | "))")
            report.append("never whole between the ✕ and the confirm: \(all.subtracting(seen).sorted().joined(separator: " | "))")
            report.append(String(format: "screen %.0fx%.0f ✕ %.1f..%.1f confirm %.1f..%.1f",
                                 screen.width, screen.height, close.frame.minY, close.frame.maxY, rested.minY, rested.maxY))
            note("verdict-\(board)-lines", report.joined(separator: "\n"))
            if board.contains("tall") || board.hasSuffix("lands") {
                XCTAssertTrue(all.subtracting(seen).isEmpty,
                              "part of the verdict could not be brought into view (\(board)): \(all.subtracting(seen))")
            }
            for _ in 0..<6 { slowDrag(app, from: 0.38, to: 0.62) }
            XCTAssertEqual(confirm.frame.minY, rested.minY, accuracy: 0.5, "the confirm is not back where it was (\(board))")
            shoot(app, "verdict-\(board)-3-back")
            app.terminate()
        }
    }

    /// The confirm waiting for the simulation's verdict, on the phone's own
    /// screen (`VELA_PAGE=pr3c`). The live sheet cannot show it on this
    /// phone — Gnosis nodes answer "not offered" at once, and the space's
    /// wallet holds no coin for a fee — so the boards hold the two moments
    /// still: `sheet-held` (the simulation is out: the confirm is shut and
    /// says so in one line) and `sheet-waited-out` (the core's deadline
    /// passed: "could not check" in the verdict's place, the confirm open).
    /// `sheet-out` is the measure: a sheet with nothing to wait for.
    ///
    /// Nothing is tapped. The confirm's frame, whether it is enabled, the
    /// line under it and the verdict's place are read from the tree, and the
    /// confirm must be at ONE position on all three.
    func testConfirmWaitBoards() throws {
        let boards = (ProcessInfo.processInfo.environment["WAIT_BOARDS"] ?? "sheet-out,sheet-held,sheet-waited-out")
            .split(separator: ",").map(String.init)
        var confirmY: [String: CGFloat] = [:]
        var report: [String] = []
        for board in boards {
            let app = XCUIApplication()
            app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
            app.launchEnvironment["VELA_PAGE"] = "pr3c"
            app.launchEnvironment["VELA_STATE"] = board
            app.launch()
            let confirm = app.buttons["signing.confirm"].firstMatch
            guard confirm.waitForExistence(timeout: 20) else {
                XCTFail("no confirm on \(board)")
                shoot(app, "wait-\(board)-MISSING")
                dump(app, "wait-\(board)-MISSING")
                app.terminate()
                continue
            }
            // Longer than the core's four seconds: a board that says "held"
            // must still be held after them, and nothing may move meanwhile.
            frames(app, "wait-\(board)", seconds: 7)
            shoot(app, "wait-\(board)")
            dump(app, "wait-\(board)")
            let footer = app.descendants(matching: .any)["signing.confirmBlock"].firstMatch
            let line = footer.exists ? footer.label : ""
            let verdict = app.staticTexts.allElementsBoundByIndex.map { $0.label }.filter { label in
                ["正在检查", "未能检查", "无资产变动", "预计会失败", "余额变化", "Checking", "couldn’t check", "No asset changes"]
                    .contains { label.contains($0) }
            }
            confirmY[board] = confirm.frame.minY
            report.append(String(
                format: "%@: confirm y=%.1f h=%.1f enabled=%@ line=[%@]%@ verdict=[%@]",
                board, confirm.frame.minY, confirm.frame.height, "\(confirm.isEnabled)", line,
                footer.exists ? String(format: "@%.1f", footer.frame.minY) : "",
                verdict.joined(separator: " | ")
            ))
            if board.hasSuffix("held") {
                XCTAssertFalse(confirm.isEnabled, "the confirm is open while the verdict is out (\(board))")
                XCTAssertTrue(line.contains("正在检查这笔交易") || line.contains("Checking what this transaction does"),
                              "the held confirm does not say what it waits for (\(board)): [\(line)]")
            }
            if board.hasSuffix("waited-out") {
                XCTAssertTrue(confirm.isEnabled, "the confirm is still shut after the deadline (\(board))")
                XCTAssertTrue(verdict.contains { $0.contains("未能检查") || $0.contains("couldn’t check") },
                              "the verdict's place does not say it could not check (\(board)): \(verdict)")
            }
            app.terminate()
        }
        note("wait-boards", report.joined(separator: "\n"))
        if let first = confirmY.values.first {
            for (board, y) in confirmY {
                XCTAssertEqual(y, first, accuracy: 0.5, "the confirm is elsewhere on \(board): \(confirmY)")
            }
        }
    }

    /// Any gallery board, on the phone's own screen (`BOARD_PAGE`,
    /// `BOARD_STATE`, `BOARD_NAME`): a picture, its tree, and one scrolled.
    func testBoard() throws {
        let env = ProcessInfo.processInfo.environment
        let name = env["BOARD_NAME"] ?? "board"
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PAGE"] = env["BOARD_PAGE"] ?? "signing"
        app.launchEnvironment["VELA_STATE"] = env["BOARD_STATE"] ?? "cs36"
        app.launch()
        settle(4)
        shoot(app, "\(name)-1")
        dump(app, "\(name)-1")
        swipePage(app, from: 0.7, to: 0.35)
        settle(1)
        shoot(app, "\(name)-2-scrolled")
        dump(app, "\(name)-2-scrolled")
        app.terminate()
    }

    /// The confirm, the fee row and the verdict's place, read from ONE
    /// snapshot of the tree per tick — what moved, and when, as the verdict
    /// and the worth land. (Asking the live tree for an element's label a
    /// moment after asking whether it exists fails the test when the row
    /// has been redrawn in between: the fee does exactly that as it lands.)
    private func frames(_ app: XCUIApplication, _ name: String, seconds: TimeInterval) {
        let started = Date()
        var lines: [String] = []
        var index = 0
        while Date().timeIntervalSince(started) < seconds {
            let at = Date().timeIntervalSince(started)
            guard let root = try? app.snapshot() else {
                lines.append(String(format: "t=%.2f (no snapshot)", at))
                settle(0.2)
                continue
            }
            var all: [XCUIElementSnapshot] = []
            func walk(_ node: XCUIElementSnapshot) {
                all.append(node)
                node.children.forEach(walk)
            }
            walk(root)
            func named(_ identifier: String) -> XCUIElementSnapshot? { all.first { $0.identifier == identifier } }
            let texts = all.filter { $0.elementType == .staticText }
            let button = named("signing.confirm") ?? named("signing.openSigner")
            let close = named("signing.close")
            let footer = named("signing.confirmBlock")
            let fee = texts.first { text in
                text.label.hasPrefix("~") || ["估算中", "点击重试", "无网络费", "换一种币"].contains { text.label.contains($0) }
            }
            let feeLines = texts.filter { text in
                ["不够付", "不足以支付", "不额外收费", "已改用", "正在重试", "点费用重试"].contains { text.label.contains($0) }
            }.map { "\($0.label.prefix(14))@\(Int($0.frame.minY))" }.joined(separator: " | ")
            let verdict = texts.filter { text in
                ["正在检查", "未能检查", "无资产变动", "预计会失败", "余额变化", "无法在链上核实"].contains { text.label.contains($0) }
            }.map { "\($0.label.prefix(12))@\(Int($0.frame.minY))" }.joined(separator: " | ")
            lines.append(String(
                format: "t=%.2f close.y=%.1f confirm=%@ enabled=%@ fee=%@ feeLines=[%@] footer=%@ verdict=[%@]",
                at,
                close?.frame.minY ?? -1,
                button.map { "\($0.frame.minY),\($0.frame.height)" } ?? "-",
                button.map { "\($0.isEnabled)" } ?? "-",
                fee.map { "\($0.label)@\(Int($0.frame.minY))" } ?? "-",
                feeLines,
                footer.map { "\($0.label)@\(Int($0.frame.minY))" } ?? "-",
                verdict
            ))
            if index < 6 || index % 5 == 0 { shoot(app, String(format: "%@-f%02d", name, index)) }
            index += 1
        }
        note("\(name)-frames", lines.joined(separator: "\n"))
    }

    // MARK: - 11. Hide balance: every figure masked, every unit kept

    /// The space's wallet, hidden: the home, the assets list, a token's
    /// detail, History, a transfer's detail and the switcher. Hide balance is
    /// put back to what the person had (`KEPT_HIDDEN`, read before the walk).
    func testHiddenBalanceKeepsUnits() throws {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(8)
        let keptHidden = ProcessInfo.processInfo.environment["KEPT_HIDDEN"] == "1"
        if isHidden(app) { toggleHero(app); settle(2) }
        shoot(app, "hide-0-shown")
        toggleHero(app)
        settle(2)
        XCTAssertTrue(isHidden(app), "the hero is not hidden")
        shoot(app, "hide-1-home")
        dump(app, "hide-1-home")
        swipePage(app, from: 0.75, to: 0.4)
        settle(1)
        shoot(app, "hide-1-home-lower")
        dump(app, "hide-1-home-lower")

        // Assets → all, then a token's detail.
        let alls = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
            .allElementsBoundByIndex.filter { $0.isHittable }.sorted { $0.frame.minY < $1.frame.minY }
        if let assets = alls.last {
            assets.tap()
            settle(2.5)
            shoot(app, "hide-2-assets")
            dump(app, "hide-2-assets")
            let holding = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "BNB")).firstMatch
            if holding.exists {
                holding.tap()
                settle(3)
                shoot(app, "hide-3-token-detail")
                dump(app, "hide-3-token-detail")
                closeTop(app)
            } else {
                XCTFail("no holding row in the assets list")
            }
            closeTop(app)
            settle(1)
        } else {
            XCTFail("no 全部 over Assets")
        }
        for _ in 0..<3 { swipePage(app, from: 0.35, to: 0.85) }
        settle(1)

        // Activity → all (History), then a sent transfer's detail.
        let upper = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
            .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        if let upper {
            upper.tap()
            settle(3)
            shoot(app, "hide-4-history")
            dump(app, "hide-4-history")
            let sent = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "已发送")).firstMatch
            var swipes = 0
            while swipes < 6, !(sent.exists && sent.isHittable) {
                swipePage(app, from: 0.75, to: 0.45)
                settle(0.8)
                swipes += 1
            }
            if sent.exists, sent.isHittable {
                sent.tap()
                settle(3)
                shoot(app, "hide-5-transfer-detail")
                dump(app, "hide-5-transfer-detail")
                closeTop(app)
            } else {
                XCTFail("no sent transfer in History")
            }
            closeTop(app)
            settle(1)
        } else {
            XCTFail("no 全部 over Activity")
        }
        for _ in 0..<3 { swipePage(app, from: 0.35, to: 0.85) }
        settle(1)

        // The switcher: rows and total.
        let account = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Parallel One")).firstMatch
        if account.exists { account.tap() } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.1)).tap()
        }
        settle(2.5)
        shoot(app, "hide-6-switcher")
        dump(app, "hide-6-switcher")
        closeTop(app)
        settle(1)

        // Back to what the person had.
        tapTab(["钱包", "Wallet"], in: app)
        settle(1.5)
        for _ in 0..<3 { swipePage(app, from: 0.35, to: 0.85) }
        if isHidden(app) != keptHidden { toggleHero(app) }
        settle(2)
        shoot(app, "hide-9-restored")
        XCTAssertEqual(isHidden(app), keptHidden, "hide balance was not put back")
        app.terminate()
    }

    /// Hide balance put back to the person's own setting, and nothing else.
    func testRestoreHidden() throws {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(4)
        let keptHidden = ProcessInfo.processInfo.environment["KEPT_HIDDEN"] == "1"
        if isHidden(app) != keptHidden { toggleHero(app) }
        settle(2)
        shoot(app, "hide-9-restored")
        XCTAssertEqual(isHidden(app), keptHidden, "hide balance was not put back")
        app.terminate()
    }

    private func isHidden(_ app: XCUIApplication) -> Bool {
        app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@ OR label == %@", "显示余额", "Show balance")
        ).firstMatch.exists
    }

    /// The hero's figure reads "¥1.49 CNY"; its caption "总余额 · CNY" is not it.
    private static let heroFigure = NSPredicate(
        format: "label ENDSWITH %@ AND NOT (label CONTAINS %@)", " CNY", "·"
    )

    /// The hero figure is the hide-balance toggle.
    private func toggleHero(_ app: XCUIApplication) {
        let hidden = app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@ OR label == %@", "显示余额", "Show balance")
        ).firstMatch
        if hidden.exists {
            hidden.tap()
            return
        }
        let shown = app.descendants(matching: .any).matching(Self.heroFigure).firstMatch
        if shown.exists {
            shown.tap()
        } else {
            XCTFail("no hero figure to toggle")
        }
    }

    // MARK: - 12. A receipt's time is its block's time: the repair, watched

    /// LOOK ONLY, on the person's own wallet: nothing is tapped but the Wallet
    /// tab, Activity's「全部」and History's back. No sheet is opened and no
    /// request is started.
    ///
    /// Three receipts of 2026-09-29 stood under「今天」on 2026-10-10: their
    /// block's time had not been read and the clock stood in, for good. This
    /// build re-reads each stored receive's block time in the background and
    /// rewrites the record. The home's Activity is read from one snapshot of
    /// the tree per tick — its day headers and rows, top to bottom — from the
    /// first frame (the stored times, as they were) until the rows stand
    /// under their own day, and History is read the same way after.
    ///
    /// `REPAIR_WAIT` (seconds, default 150) is how long the home is watched;
    /// `REPAIR_ROW` (default "Interleave") names the rows to follow.
    func testReceiptsStandUnderTheirOwnDay() throws {
        let env = ProcessInfo.processInfo.environment
        let wait = TimeInterval(env["REPAIR_WAIT"] ?? "") ?? 150
        let followed = env["REPAIR_ROW"] ?? "Interleave"
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        let started = Date()
        app.launch()

        var timeline: [String] = []
        var last: [String]? = nil
        var changes = 0
        var movedAt: TimeInterval? = nil
        var final: [String] = []
        while Date().timeIntervalSince(started) < wait {
            let at = Date().timeIntervalSince(started)
            guard let outline = activityOutline(app, from: "活动", to: "资产") else {
                settle(0.3)
                continue
            }
            if outline != last {
                timeline.append(String(format: "t=%.2f %@", at, outline.joined(separator: " | ")))
                shoot(app, String(format: "repair-home-%02d", changes))
                if changes < 4 { dump(app, String(format: "repair-home-%02d", changes)) }
                changes += 1
                last = outline
            }
            final = outline
            // The rows followed are on the home and none of them is under「今天」.
            let rows = Self.rows(of: outline, containing: followed)
            if !rows.isEmpty, rows.allSatisfy({ $0.day != "今天" && $0.day != "Today" }) {
                if movedAt == nil { movedAt = at }
                // Watch a little longer: what moved must stay.
                if let movedAt, at - movedAt > 25 { break }
            } else {
                movedAt = nil
            }
            settle(0.5)
        }
        note("repair-home-timeline", timeline.joined(separator: "\n"))
        XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "this walk is the person's own wallet, not the space")
        let rows = Self.rows(of: final, containing: followed)
        XCTAssertFalse(rows.isEmpty, "no \(followed) row on the home: \(final)")
        XCTAssertTrue(rows.allSatisfy { $0.day != "今天" && $0.day != "Today" },
                      "a \(followed) receipt still stands under today after \(Int(wait)) s: \(final)")
        shoot(app, "repair-home-final")
        dump(app, "repair-home-final")

        // History: the same rows, under the same day.
        let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
            .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        XCTAssertNotNil(all, "no 全部 over Activity")
        all?.tap()
        settle(3)
        shoot(app, "repair-history-1")
        dump(app, "repair-history-1")
        let history = activityOutline(app, from: nil, to: nil) ?? []
        note("repair-history-outline", history.joined(separator: "\n"))
        let listed = Self.rows(of: history, containing: followed)
        XCTAssertFalse(listed.isEmpty, "no \(followed) row in History: \(history)")
        XCTAssertTrue(listed.allSatisfy { $0.day != "今天" && $0.day != "Today" },
                      "History still files a \(followed) receipt under today: \(history)")
        XCTAssertEqual(Set(listed.map { $0.day }), Set(rows.map { $0.day }), "the home and History name different days")
        swipePage(app, from: 0.8, to: 0.3)
        settle(1)
        shoot(app, "repair-history-2-lower")
        let back = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "返回", "Back"))
            .allElementsBoundByIndex.first { $0.isHittable }
        XCTAssertNotNil(back, "no back on History")
        back?.tap()
        settle(1.5)
        shoot(app, "repair-home-after-history")
        app.terminate()
    }

    /// The day headers and rows of an activity list, top to bottom, from ONE
    /// snapshot: `"# <day>"` for a header and the row's own label for a row.
    /// `from` / `to` are the section titles that bound it on the home (the
    /// whole page when `nil`). `nil` when the tree could not be read or the
    /// bounds are not on screen yet.
    private func activityOutline(_ app: XCUIApplication, from: String?, to: String?) -> [String]? {
        guard let root = try? app.snapshot() else { return nil }
        var all: [XCUIElementSnapshot] = []
        func walk(_ node: XCUIElementSnapshot) {
            all.append(node)
            node.children.forEach(walk)
        }
        walk(root)
        let texts = all.filter { $0.elementType == .staticText }
        var top = -CGFloat.infinity, bottom = CGFloat.infinity
        if let from {
            guard let title = texts.first(where: { $0.label == from }) else { return nil }
            top = title.frame.maxY
        }
        if let to, let title = texts.first(where: { $0.label == to && $0.frame.minY > top }) {
            bottom = title.frame.minY
        }
        // A row is a wide button whose label reads as a row ("…、…"); a day
        // header is a text that is no row's child, at the list's leading edge.
        let rows = all.filter {
            $0.elementType == .button && $0.frame.width > 250 && $0.label.contains("、")
                && $0.frame.minY >= top && $0.frame.maxY <= bottom + 1
        }
        guard let leading = rows.map({ $0.frame.minX }).min() else {
            // No row at all: the list's own words (empty, loading), if any.
            let words = texts.filter { $0.frame.minY >= top && $0.frame.maxY <= bottom + 1 }.map { $0.label }
            return words.isEmpty ? [] : ["(no rows: \(words.joined(separator: " / ")))"]
        }
        let headers = texts.filter { text in
            abs(text.frame.minX - leading) < 2
                && text.frame.minY >= top && text.frame.maxY <= bottom + 1
                && !rows.contains { row in row.frame.contains(text.frame) }
        }
        let lines = rows.map { ($0.frame.minY, $0.label) } + headers.map { ($0.frame.minY, "# " + $0.label) }
        return lines.sorted { $0.0 < $1.0 }.map { $0.1 }
    }

    /// The rows of an outline whose label contains `fragment`, each with the
    /// day header it stands under.
    private static func rows(of outline: [String], containing fragment: String) -> [(day: String, row: String)] {
        var day = "(no header)"
        var out: [(day: String, row: String)] = []
        for line in outline {
            if line.hasPrefix("# ") {
                day = String(line.dropFirst(2))
            } else if line.contains(fragment) {
                out.append((day: day, row: line))
            }
        }
        return out
    }

    // MARK: - Out of the space

    func testLeaveSpace() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launch()
        settle(6)
        shoot(app, "leave-home")
        dump(app, "leave-home")
        app.terminate()
    }

    // MARK: - Plumbing

    private func startServer() {
        guard server == nil else { return }
        do {
            let server = try LocalDappServer(html: try LocalDappServer.page())
            server.start()
            self.server = server
        } catch {
            XCTFail("the local dApp server: \(error)")
        }
    }

    private func launchInSpace(extraArguments: [String] = [], page: Bool = true) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        if page { app.launchEnvironment["VELA_URL"] = LocalDappServer.url }
        app.launchArguments += extraArguments
        app.launch()
        spaceApp = app
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30), "not in the parallel space")
        return app
    }

    private func openExplore(_ app: XCUIApplication) {
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 3) { return }
        tapTab(["探索", "Explore"], in: app)
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        if row.waitForExistence(timeout: 8) { row.tap() }
    }

    private func connect(_ app: XCUIApplication) {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(
            NSPredicate(format: "label == %@ OR label == %@ OR label == %@ OR label == %@",
                        "连接", "批准", "Connect", "Approve")
        ).firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_requestAccounts ok"),
                      "the page was never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")
    }

    private func waitForVerdict(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if lines.contains(where: { $0.label.contains(fragment) }) { return true }
            settle(1)
        }
        return false
    }

    private func openSettings(_ app: XCUIApplication) {
        tapTab(["设置", "Settings"], in: app)
        settle(2)
    }

    private func reachElement(_ element: XCUIElement, in app: XCUIApplication) {
        let screen = app.frame.height
        var swipes = 0
        while swipes < 8 {
            if element.exists {
                let frame = element.frame
                if frame.maxY > screen * 0.75 {
                    swipePage(app, from: 0.55, to: 0.3)
                } else if frame.minY < screen * 0.2 {
                    swipePage(app, from: 0.3, to: 0.55)
                } else {
                    break
                }
            } else {
                swipePage(app, from: 0.6, to: 0.35)
            }
            swipes += 1
            settle(0.8)
        }
    }

    /// Close whatever is on top: a sheet's ✕ (「关闭」), else a page's back
    /// (「返回」), else the sheet's grabber — never a row.
    private func closeTop(_ app: XCUIApplication) {
        let close = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "关闭", "Close"))
            .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        if let close {
            close.tap()
            settle(1.5)
            return
        }
        let back = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "返回", "Back"))
            .allElementsBoundByIndex.first { $0.isHittable }
        if let back {
            back.tap()
            settle(1.5)
            return
        }
        dismissSheet(app)
    }

    /// A sheet goes down by its grabber — never a tap on a row (a row's ✕
    /// removes an account from the device).
    private func dismissSheet(_ app: XCUIApplication) {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.07))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98))
        start.press(forDuration: 0.05, thenDragTo: end)
        settle(1.5)
    }

    private func tapTab(_ labels: [String], in app: XCUIApplication) {
        for label in labels {
            let button = app.buttons[label]
            if button.exists {
                button.tap()
                return
            }
            let matches = app.staticTexts.matching(identifier: label)
            if matches.count > 0 {
                matches.element(boundBy: matches.count - 1).tap()
                return
            }
        }
        XCTFail("no tab called any of \(labels)")
    }

    private func swipePage(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)))
    }

    /// A drag slow enough to scroll rather than fling.
    private func slowDrag(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
            .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)),
                   withVelocity: .slow, thenHoldForDuration: 0.2)
        settle(0.6)
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func shoot(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The whole screen, system sheets included.
    private func shootScreen(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// A picture with `hide` (in points) painted over before it is kept.
    private func shootRedacted(_ app: XCUIApplication, _ name: String, hide: CGRect) {
        let image = app.screenshot().image
        let scale = image.size.width / max(app.frame.width, 1)
        let format = UIGraphicsImageRendererFormat()
        format.scale = image.scale
        let painted = UIGraphicsImageRenderer(size: image.size, format: format).image { context in
            image.draw(at: .zero)
            UIColor.gray.setFill()
            context.fill(CGRect(x: hide.minX * scale, y: hide.minY * scale,
                                width: hide.width * scale, height: hide.height * scale))
        }
        let attachment = XCTAttachment(image: painted)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func dump(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(string: app.debugDescription)
        attachment.name = name + "-tree"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func note(_ name: String, _ text: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
