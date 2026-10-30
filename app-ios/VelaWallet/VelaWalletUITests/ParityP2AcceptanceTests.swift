//
//  ParityP2AcceptanceTests.swift
//  VelaWalletUITests
//
//  Native parity Phase 2 on a device: the app in the parallel space, pointed
//  at the multisig fork harness through the Debug launch inputs (F0.5), with
//  Gnosis read through the harness's node as the person's own ("Only use my
//  node" on). Each scenario photographs what the web's same page shows, for
//  the side-by-side (light/dark is the simulator's appearance; the language
//  is `TEST_RUNNER_VELA_P2_LANG`, `en` or `zh`).
//
//      FORK_PORT=9445 RELAY_PORT=9490 GENESIS_PORT=9487 … up.sh
//      TEST_RUNNER_VELA_HARNESS_RPC=http://127.0.0.1:9445 \
//      TEST_RUNNER_VELA_HARNESS_RELAY=http://127.0.0.1:9490 \
//      TEST_RUNNER_VELA_HARNESS_GENESIS=http://127.0.0.1:9487 \
//      TEST_RUNNER_VELA_P2_LANG=en \
//        xcodebuild test … -only-testing:VelaWalletUITests/ParityP2AcceptanceTests
//
//  Skipped without a harness, so CI's UI leg is not failed by it. Nothing is
//  ever slid or signed.
//

import XCTest

final class ParityP2AcceptanceTests: XCTestCase {

    private var lang: String { ProcessInfo.processInfo.environment["VELA_P2_LANG"] ?? "en" }

    /// The words these scenarios look for, in the two languages they run in.
    private func w(_ en: String, _ zh: String) -> String { lang == "zh" ? zh : en }

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func launch() throws -> XCUIApplication {
        let env = ProcessInfo.processInfo.environment
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"] else {
            throw XCTSkip("no fork harness named (TEST_RUNNER_VELA_HARNESS_RPC / _RELAY)")
        }
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] = #"{"100":{"rpcURL":"\#(rpc)","onlyMine":true}}"#
        var endpoints = #"{"bundlerServiceURL":"\#(relay)""#
        if let genesis = env["VELA_HARNESS_GENESIS"], !genesis.isEmpty {
            endpoints += #","genesisServiceURL":"\#(genesis)""#
        }
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] = endpoints + "}"
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
        app.launch()
        let name = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH %@", "Parallel")).firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 60), "the wallet never opened")
        settle(3)
        return app
    }

    // MARK: - P2.1 the network pages

    /// Settings → Networks: the mainnet heading names the Vela networks out of
    /// the list; Gnosis, read through the person's own node, offers "Only use
    /// my node" and shows it on.
    func testTheNetworkPagesSayWhatTheWebSays() throws {
        let app = try launch()
        tapTab(w("Settings", "设置"), in: app)
        let advanced = app.staticTexts[w("Advanced", "高级")]
        XCTAssertTrue(advanced.waitForExistence(timeout: 15), "Settings did not open")
        settle(1)
        if !app.staticTexts[w("Networks", "网络")].exists { advanced.tap(); settle(1) }
        open(row: w("Networks", "网络"), in: app)
        settle(2)

        // The heading's hint, with the networks out of the list by name.
        let hint = app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "Stable")).firstMatch
        XCTAssertTrue(hint.waitForExistence(timeout: 15), "the networks out of the list are not named")
        XCTAssertTrue(hint.label.contains(w("Also supported, not in your list", "同样支持、但不在你列表中的网络")),
                      "the line is not the corpus's: \(hint.label)")
        attach(app.screenshot(), named: "p2-networks-\(lang)")

        open(row: "Gnosis", in: app)
        let onlyMine = app.descendants(matching: .any)["only-mine"].firstMatch
        XCTAssertTrue(onlyMine.waitForExistence(timeout: 15), "Only use my node is not offered on the person's own RPC")
        let toggle = app.switches[w("Only use my node", "只使用我的节点")].firstMatch
        XCTAssertTrue(toggle.exists, "no switch")
        XCTAssertEqual(toggle.value as? String, "1", "the saved switch is not shown on")
        attach(app.screenshot(), named: "p2-network-gnosis-\(lang)")
        app.terminate()
    }

    // MARK: - P2.5 a contact's note

    /// A contact saved with a note shows it on its page.
    func testAContactsNoteIsSavedAndShown() throws {
        let app = try launch()
        tapTab(w("Contacts", "通讯录"), in: app)
        let add = app.buttons["contacts.add"].firstMatch
        XCTAssertTrue(add.waitForExistence(timeout: 15), "no add button")
        add.tap()
        settle(1)
        // The add button opens its menu: new contact, import, export.
        let new = app.staticTexts[w("New contact", "新建联系人")].firstMatch
        XCTAssertTrue(new.waitForExistence(timeout: 8), "no menu")
        new.tap()
        settle(1)
        func field(_ placeholder: String) -> XCUIElement {
            app.textFields.matching(NSPredicate(format: "placeholderValue == %@", placeholder)).firstMatch
        }
        let name = field(w("e.g. Alice", "例如 Alice"))
        XCTAssertTrue(name.waitForExistence(timeout: 10), "the form did not open")
        name.tap()
        name.typeText("Parity Bob")
        let address = field(w("Paste address or .eth", "粘贴地址或 .eth"))
        address.tap()
        address.typeText("0x031d7D57c99CAF891e1C250554691Fd12D84772b")
        let note = app.textFields["contacts.form.note"].exists
            ? app.textFields["contacts.form.note"] : app.textViews["contacts.form.note"]
        XCTAssertTrue(note.waitForExistence(timeout: 5), "the form has no note")
        note.tap()
        note.typeText("Met at ETHGlobal")
        attach(app.screenshot(), named: "p2-contact-form-\(lang)")
        app.buttons[w("Save", "保存")].firstMatch.tap()
        settle(2)

        let row = app.staticTexts["Parity Bob"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10), "the contact was not saved")
        row.tap()
        let shown = app.descendants(matching: .any)["contacts.detail.note"].firstMatch
        XCTAssertTrue(shown.waitForExistence(timeout: 10), "the note is not on the contact's page")
        XCTAssertTrue(app.staticTexts["Met at ETHGlobal"].exists)
        attach(app.screenshot(), named: "p2-contact-note-\(lang)")
        app.terminate()
    }

    // MARK: - P2.2 the confirm

    /// A send to the account's own address reaches the confirm, which names
    /// the person's own account and quotes a fee — photographed, never slid.
    func testTheConfirmIsReachedAndSaysWhoIsPaid() throws {
        let app = try launch()
        app.buttons[w("Send", "转账")].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 60), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        let recipient = app.textFields["send.recipient"]
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "the form never opened")
        recipient.tap()
        recipient.typeText("0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        let amount = app.textFields["send.amount"]
        amount.tap()
        amount.typeText("0.01")
        if app.buttons["keyboard.done"].firstMatch.waitForExistence(timeout: 3) {
            app.buttons["keyboard.done"].firstMatch.tap()
        }
        let next = app.buttons[w("Continue", "继续")].firstMatch
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()
        XCTAssertTrue(app.buttons[w("Confirm & Send", "确认并发送")].firstMatch.waitForExistence(timeout: 60),
                      "the send never reached the confirm")
        XCTAssertTrue(app.staticTexts["Parallel One"].firstMatch.exists || app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "Parallel")).firstMatch.exists)
        settle(4)
        attach(app.screenshot(), named: "p2-send-confirm-\(lang)")
        app.terminate()
    }

    // MARK: - Helpers

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

    private func open(row: String, in app: XCUIApplication) {
        let label = app.staticTexts[row].firstMatch
        XCTAssertTrue(label.waitForExistence(timeout: 10), "no row called \(row)")
        var swipes = 0
        while !label.isHittable, swipes < 5 {
            app.swipeUp()
            swipes += 1
        }
        label.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(1.5)
    }

    private func settle(_ seconds: TimeInterval = 2) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
