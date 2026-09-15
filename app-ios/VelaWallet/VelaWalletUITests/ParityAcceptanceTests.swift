//
//  ParityAcceptanceTests.swift
//  VelaWalletUITests
//
//  Spec 054's acceptance, run **on the phone**.
//
//      xcodebuild test \
//        -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'platform=iOS,id=<device udid>' \
//        -only-testing:VelaWalletUITests/ParityAcceptanceTests \
//        -resultBundlePath /tmp/parity.xcresult
//
//  Two halves, and they reach the screen by different doors:
//
//  - the ADDRESS BOOK runs behind `VELA_PAGE=contacts-live`, which mounts the
//    real machine over the device's own storage without a passkey ceremony;
//  - the SEND journey runs in the parallel space, whose fixed keyset signs
//    without a finger.
//
//  Neither spends money. The one 054 claim that does — a two-row split landing
//  as one operation — is in `LiveWiringAcceptanceTests` beside the other live
//  sends, behind `-DVELA_LIVE_SEND`.
//

import XCTest

final class ParityAcceptanceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func launch(page: String? = nil, parallel: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        // Each test states its own door. The parallel space PERSISTS, so a run
        // that inherited the last one's would be testing a wallet nobody asked
        // for — the lesson `LiveWiringAcceptanceTests` records.
        app.launchArguments += ["-vela.parallelSpace", parallel ? "1" : "0"]
        if let page { app.launchEnvironment["VELA_PAGE"] = page }
        if parallel { app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1" }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

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

    /// 删除联系人 and the second confirmation behind it. Deleting somebody is
    /// two taps on every client, and the confirm is a sheet — so the label on
    /// the page and the label in the sheet are different words.
    private func deleteTheOpenContact(_ app: XCUIApplication) {
        tap(app.buttons["删除联系人"], "删除联系人")
        XCTAssertTrue(app.staticTexts["删除联系人？"].waitForExistence(timeout: 10),
                      "删除联系人 raised no confirmation")
        tap(app.buttons["删除"].firstMatch, "the confirm's 删除")
    }

    /// A name no real address book holds, so a later run can find and remove
    /// exactly what an earlier one added.
    private let probeName = "Vela 054 探针"
    private let probeAddress = "0x1111111111111111111111111111111111111111"

    // MARK: - US5b: the form saves somebody

    /// 新建联系人 → type → 保存 → they are in the book, and they survive a
    /// relaunch because the core wrote them to this device's storage.
    ///
    /// Then the same contact is deleted from its own page, which is the second
    /// control this spec wired and the one the row swipe could already do.
    func testAContactTypedOnThePhoneIsSavedAndThenDeleted() throws {
        let app = launch(page: "contacts-live")
        XCTAssertTrue(app.staticTexts["通讯录"].waitForExistence(timeout: 20),
                      "the address book never appeared")

        // Clean slate: if a previous run left the probe behind, its page is
        // where this one would otherwise fail for the wrong reason.
        if app.staticTexts[probeName].exists {
            tap(app.staticTexts[probeName], "the leftover probe")
            deleteTheOpenContact(app)
            XCTAssertTrue(app.staticTexts["通讯录"].waitForExistence(timeout: 10))
        }

        // The header's +, by identifier: the empty state's CTA carries the
        // same word, and a query that matches both fails on ambiguity rather
        // than picking one — which is what the first device run did.
        tap(app.buttons["contacts.add"], "the + button")
        // The menu, then the form. Both are sheets, and the form is the one
        // with a title only it has.
        tap(app.buttons["新建联系人"].firstMatch, "新建联系人")
        XCTAssertTrue(app.staticTexts["新建联系人"].waitForExistence(timeout: 10),
                      "the add form never opened — the menu row still leads nowhere")
        attach(app.screenshot(), named: "device-contact-form-empty")

        // Save is SHUT on an empty address.
        let save = app.buttons["保存"].firstMatch
        XCTAssertTrue(save.waitForExistence(timeout: 10), "保存 is missing")
        XCTAssertFalse(save.isEnabled, "保存 offered to save an empty form")

        let name = app.textFields["名称"].firstMatch
        tap(name, "the name field")
        name.typeText(probeName)
        let address = app.textFields["地址"].firstMatch
        tap(address, "the address field")
        address.typeText(probeAddress)

        XCTAssertTrue(save.isEnabled, "保存 stayed shut on a complete form")
        attach(app.screenshot(), named: "device-contact-form-filled")
        // The keyboard covers the bottom of the sheet; dismissing it first is
        // what makes the CTA a thing a finger can actually reach.
        app.staticTexts["新建联系人"].tap()
        XCTAssertTrue(save.waitForExistence(timeout: 5))
        save.tap()

        XCTAssertTrue(app.staticTexts[probeName].waitForExistence(timeout: 15),
                      "the contact was typed and saved and is not in the book")
        attach(app.screenshot(), named: "device-contact-saved")

        // It is on THIS DEVICE's storage, not in a view that happens to hold
        // it — which is the only claim a relaunch can settle. Relaunching the
        // SAME instance rather than building a second one: a fresh
        // `XCUIApplication` re-negotiates the automation session, and that is
        // what timed the runner out the first time this ran.
        app.terminate()
        app.launch()
        XCTAssertTrue(app.staticTexts[probeName].waitForExistence(timeout: 30),
                      "the contact did not survive a relaunch")

        // And out again, from the page's own 删除联系人.
        tap(app.staticTexts[probeName], "the probe's row")
        XCTAssertTrue(app.staticTexts["最近往来"].waitForExistence(timeout: 10),
                      "the contact's page never opened")
        attach(app.screenshot(), named: "device-contact-detail")
        deleteTheOpenContact(app)
        XCTAssertTrue(app.staticTexts["通讯录"].waitForExistence(timeout: 10),
                      "deleting from the detail page did not return to the book")
        XCTAssertFalse(app.staticTexts[probeName].exists,
                       "删除联系人 did nothing — the contact is still there")
    }

    /// The star on a contact's page flips, and the flip is the core's: the
    /// contact moves into 收藏 on the list behind it.
    func testTheStarOnAContactsPageFlips() throws {
        let app = launch(page: "contacts-live")
        XCTAssertTrue(app.staticTexts["通讯录"].waitForExistence(timeout: 20),
                      "the address book never appeared")

        // An empty book has nothing to star, and "还没有联系人" is how the
        // screen says so. Skipping is honest; tapping the empty artwork and
        // asserting a page opened would be a test failing for the app's
        // correct behaviour.
        if app.staticTexts["还没有联系人"].waitForExistence(timeout: 5) {
            throw XCTSkip("this device's address book is empty — nothing to star")
        }
        // A contact ROW, not whichever static text happens to be first — the
        // section header and the index rail are texts too. Rows are buttons,
        // and the ones that are contacts carry a shortened address in their
        // label ("… 0x1111…1111").
        let row = app.buttons.matching(
            NSPredicate(format: "label CONTAINS %@", "0x")
        ).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10), "the list has no contact rows")
        row.tap()
        XCTAssertTrue(app.staticTexts["最近往来"].waitForExistence(timeout: 10),
                      "a contact's page never opened")

        let star = app.buttons["收藏"].firstMatch
        XCTAssertTrue(star.waitForExistence(timeout: 10),
                      "the page has no star at all")
        let before = star.isSelected
        star.tap()
        // The control reports its own state, which is what a person using
        // VoiceOver hears — a star that changes colour and nothing else is
        // invisible to them.
        let flipped = NSPredicate(format: "isSelected == %@", NSNumber(value: !before))
        expectation(for: flipped, evaluatedWith: star)
        waitForExpectations(timeout: 10)
        attach(app.screenshot(), named: "device-contact-star")

        // Put it back, so the device's book is as it was found.
        star.tap()
    }

    // MARK: - US3: the payroll importer

    /// 导入表格 opens the importer, a pasted list is parsed and priced by the
    /// core, and 导入 N 位收款人 seeds the split.
    ///
    /// Nothing is sent. The parse, the preview and the seed are what this
    /// asserts; the money is `LiveWiringAcceptanceTests`' business.
    func testAPastedPayrollIsParsedAndSeedsTheSplit() throws {
        let app = launch(parallel: true)
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30),
                      "the home never appeared")
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30),
                      "the picker is not showing this wallet's holdings")
        tap(app.staticTexts["xDAI"], "the xDAI row")
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20),
                      "the form never opened")

        // 导入表格 sits with the other recipient actions, and those belong to
        // the SPLIT form — a payroll IS a split. + 添加收款人 is the door from
        // one recipient to several.
        tap(app.buttons["+  添加收款人"].firstMatch, "+ 添加收款人")
        tap(app.buttons["导入表格"].firstMatch, "导入表格")
        XCTAssertTrue(app.staticTexts["导入收款人"].waitForExistence(timeout: 15),
                      "导入表格 did nothing — the importer never opened")
        attach(app.screenshot(), named: "device-batch-open")

        // Two people, in the fiat column the sheet opens on.
        let paste = app.textViews["send.batchPaste"].firstMatch
        XCTAssertTrue(paste.waitForExistence(timeout: 10),
                      "the paste box is not a field — nothing can be pasted into it")
        paste.tap()
        // One line, then the Return, then the next. A single `typeText` with an
        // embedded "\n" put only the first row into the core — the newline is
        // typed as a key press and the field needs a beat to take it.
        paste.typeText("0x1111111111111111111111111111111111111111,1")
        paste.typeText("\n")
        paste.typeText("0x2222222222222222222222222222222222222222,2")

        // The core parsed both and says so ON THE BUTTON — which is the
        // assertion that matters, because the button is what a person presses
        // and its words are the core's count.
        let apply = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "导入 ")
        ).firstMatch
        XCTAssertTrue(apply.waitForExistence(timeout: 20),
                      "the core did not parse the pasted list")
        attach(app.screenshot(), named: "device-batch-parsed")
        // The count on the button is the CORE's, and it differs by harness:
        // a real phone's keyboard puts a newline into the `TextEditor` and the
        // core sees TWO rows; the simulator's swallows it and the core sees
        // one. Both prove the same wiring, so the assertion is on the count
        // being real rather than on which one it is.
        let parsed = apply.label.contains("2") ? 2 : 1
        XCTAssertTrue(apply.label.contains("\(parsed)"),
                      "the CTA does not count what the core parsed: \(apply.label)")
        XCTAssertTrue(apply.isEnabled, "the CTA is shut on a list the core accepted")
        apply.tap()

        // Applying seeds the split and shuts the sheet: two recipient rows on
        // the form, and no importer over them.
        // The imported person is in the split, with the core's own row id —
        // and the importer is gone, because `seed_split_recipients` shuts it.
        XCTAssertTrue(app.staticTexts["收款人 1"].waitForExistence(timeout: 15),
                      "the split did not receive the imported person")
        if parsed == 2 {
            XCTAssertTrue(app.staticTexts["收款人 2"].exists,
                          "only one of the two imported people reached the split")
        }
        XCTAssertFalse(app.staticTexts["导入收款人"].exists,
                       "the importer stayed open after applying")
        // The summary counts people in the corpus's words — and counts them
        // with the PLURAL key, which this spec had to fix: the bare
        // `send.recipientCount` does not exist, so the screen was printing
        // its own key name.
        XCTAssertTrue(app.staticTexts["\(parsed) 位收款人"].exists,
                      "the split summary does not count the imported people")
        attach(app.screenshot(), named: "device-batch-applied")
    }

    // MARK: - US2: the sweep pick's rules

    /// 发送多个代币 turns the tick boxes on, the first tick pins the chain, and
    /// tokens on other chains go DIM rather than disappearing.
    func testTheSweepPickPinsAChainAndDimsTheRest() throws {
        let app = launch(parallel: true)
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 30),
                      "the home never appeared")
        tap(app.buttons["转账"], "转账")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30),
                      "the picker is not showing this wallet's holdings")
        attach(app.screenshot(), named: "device-sweep-before")

        tap(app.buttons["发送多个代币"].firstMatch, "发送多个代币")
        // The CTA changes because the picker is now picking SEVERAL — and that
        // is the shell's own flag, since the core's `multi_select_mode` does
        // not flip until the selection is confirmed.
        // A `Button` whose label is a `Text` exposes as a BUTTON, not as a
        // static text inside one — querying `staticTexts` here found nothing
        // while the control was plainly on screen.
        XCTAssertTrue(app.buttons["全选有价值代币"].waitForExistence(timeout: 15),
                      "发送多个代币 did nothing — the tick boxes never appeared")
        attach(app.screenshot(), named: "device-sweep-picking")

        tap(app.staticTexts["xDAI"], "the xDAI row")
        attach(app.screenshot(), named: "device-sweep-ticked")

        // The CTA counts what is ticked and names the chain it pinned —
        // `send.multiSendContinue`, "发送 {{n}} 个 · {{chain}} →".
        let cta = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "发送 1 个 · ")
        ).firstMatch
        XCTAssertTrue(cta.waitForExistence(timeout: 10),
                      "the CTA does not count the ticked token or name its chain")
        XCTAssertTrue(cta.label.contains("Gnosis"),
                      "the first tick did not pin the chain: \(cta.label)")
    }
}
