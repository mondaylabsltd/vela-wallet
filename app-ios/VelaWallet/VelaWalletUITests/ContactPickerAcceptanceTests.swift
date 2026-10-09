//
//  ContactPickerAcceptanceTests.swift
//  VelaWalletUITests
//
//  Issue #467, on a screen: the person icon beside Send's recipient opens the
//  address book, a contact can be picked, and the icon still works after the
//  sheet is closed with ✕ or a drag. Before the fix the icon pushed a screen
//  the live router overrode, so it opened nothing, ever.
//
//  It reads a real account's holdings (`VELA_ACCOUNT`, the golden Safe, read
//  only — it cannot sign) to reach the live form, and it ADDS two probe
//  contacts, which it removes again at the end. So it runs only when asked:
//
//      TEST_RUNNER_VELA_LIVE_READS=1 xcodebuild test \
//        -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'platform=iOS Simulator,name=<sim>' \
//        -only-testing:VelaWalletUITests/ContactPickerAcceptanceTests
//
//  Nothing is sent: the test never reaches the confirm page, and the account
//  has no key on this device.
//

import UIKit
import XCTest

final class ContactPickerAcceptanceTests: XCTestCase {

    /// Names no real address book holds, so a later run finds and removes
    /// exactly what an earlier one added.
    private let probes = [
        (name: "Vela 467 甲", address: "0x4670000000000000000000000000000000000a01"),
        (name: "Vela 467 乙", address: "0x4670000000000000000000000000000000000b02"),
    ]

    override func setUpWithError() throws {
        // Its account is a seeded VELA_ACCOUNT: never on somebody's phone.
        try skipOnDeviceForSeededAccount()
        continueAfterFailure = false
        try XCTSkipUnless(
            ProcessInfo.processInfo.environment["VELA_LIVE_READS"] == "1",
            "reads a live account and writes probe contacts; run with TEST_RUNNER_VELA_LIVE_READS=1"
        )
    }

    func testTheContactIconPicksAContactAndOpensAgainAfterAClose() throws {
        let app = launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")

        // Two people in the book, through the book's own form.
        tap(app.buttons["通讯录"].firstMatch, "the contacts tab")
        XCTAssertTrue(app.buttons["contacts.add"].waitForExistence(timeout: 20), "the address book never appeared")
        for probe in probes where !app.staticTexts[probe.name].exists {
            save(probe, in: app)
        }

        // A fresh launch, straight to Send: the book must be there without a
        // visit to the contacts page first — the sheet of nobody (or of the
        // drawing's people) that a book opened by the tap itself would be.
        app.terminate()
        app.launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never came back")
        let recipient = toTheForm(app)

        // 1. The icon opens the picker, with the person's own book in it.
        // The person icon names the pick alone (`send.recipientPickAria`,
        // #468): scanning has its own door.
        let icon = app.buttons["从通讯录选择"]
        tap(icon, "the person icon")
        let title = app.staticTexts["选择联系人"]
        XCTAssertTrue(title.waitForExistence(timeout: 10), "the person icon opened nothing (#467)")
        settle(1)
        attach(app.screenshot(), named: "467-picker-open")
        XCTAssertTrue(app.staticTexts[probes[0].name].exists, "the picker is not the person's book")
        XCTAssertFalse(app.staticTexts["阿豪"].exists, "the picker drew the fixture's people")

        // 2. A tap picks THAT person: their address fills the field.
        tap(app.staticTexts[probes[1].name], "the second probe's row")
        XCTAssertTrue(title.waitForNonExistence(timeout: 10), "a pick did not close the picker")
        XCTAssertEqual((recipient.value as? String)?.lowercased(), probes[1].address,
                       "the field does not hold the picked person's address")
        attach(app.screenshot(), named: "467-picked")

        // 3. Closed with ✕, the icon opens it again — the core's flag went down.
        tap(icon, "the person icon, again")
        XCTAssertTrue(title.waitForExistence(timeout: 10), "the icon opened nothing after a pick")
        tap(app.buttons["关闭"].firstMatch, "the picker's ✕")
        XCTAssertTrue(title.waitForNonExistence(timeout: 10), "✕ did not close the picker")
        settle(1)
        tap(icon, "the person icon, after ✕")
        XCTAssertTrue(title.waitForExistence(timeout: 10), "the icon is dead after ✕ (#467)")
        attach(app.screenshot(), named: "467-reopened-after-close")

        // 4. Dragged away, the same.
        app.swipeDown(velocity: .fast)
        XCTAssertTrue(title.waitForNonExistence(timeout: 10), "a drag did not close the picker")
        settle(1)
        tap(icon, "the person icon, after a drag")
        XCTAssertTrue(title.waitForExistence(timeout: 10), "the icon is dead after a drag (#467)")

        // 5. And the other person, picked from the reopened sheet.
        tap(app.staticTexts[probes[0].name], "the first probe's row")
        XCTAssertTrue(title.waitForNonExistence(timeout: 10))
        XCTAssertEqual((recipient.value as? String)?.lowercased(), probes[0].address)
        attach(app.screenshot(), named: "467-picked-again")

        // 6. The picker's scan row opens the scanner — which stays open.
        tap(icon, "the person icon, for the scan row")
        XCTAssertTrue(title.waitForExistence(timeout: 10))
        tap(app.staticTexts["扫码填写地址"], "the scan row")
        settle(2)
        attach(app.screenshot(), named: "467-scan-row")
        // The scanner is a whole screen: neither the picker nor the form.
        XCTAssertFalse(title.exists, "the scan row left the picker up")
        XCTAssertFalse(recipient.exists, "the scanner closed as it opened — back on the form")

        // Clean up: the probes leave the book.
        app.terminate()
        app.launch()
        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40))
        tap(app.buttons["通讯录"].firstMatch, "the contacts tab")
        XCTAssertTrue(app.buttons["contacts.add"].waitForExistence(timeout: 20))
        for probe in probes where app.staticTexts[probe.name].waitForExistence(timeout: 5) {
            tap(app.staticTexts[probe.name], "the probe \(probe.name)")
            tap(app.buttons["删除联系人"], "删除联系人")
            XCTAssertTrue(app.staticTexts["删除联系人？"].waitForExistence(timeout: 10))
            tap(app.buttons["删除"].firstMatch, "the confirm's 删除")
            XCTAssertTrue(app.buttons["contacts.add"].waitForExistence(timeout: 10))
        }
        app.terminate()
    }

    // MARK: - Steps

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0", "-AppleLanguages", "(zh)"]
        app.launchEnvironment["VELA_ACCOUNT"] = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        return app
    }

    /// 新建联系人 → name, address → 保存.
    private func save(_ probe: (name: String, address: String), in app: XCUIApplication) {
        tap(app.buttons["contacts.add"], "the + button")
        tap(app.buttons["新建联系人"].firstMatch, "新建联系人")
        XCTAssertTrue(app.staticTexts["新建联系人"].waitForExistence(timeout: 10))
        let name = app.textFields["名称"].firstMatch
        tap(name, "the name field")
        name.typeText(probe.name)
        let address = app.textFields["地址"].firstMatch
        tap(address, "the address field")
        address.typeText(probe.address)
        app.staticTexts["新建联系人"].tap()
        tap(app.buttons["保存"].firstMatch, "保存")
        XCTAssertTrue(app.staticTexts[probe.name].waitForExistence(timeout: 15), "\(probe.name) was not saved")
    }

    /// 转账 → the account's BNB (or xDAI) → the live single form.
    private func toTheForm(_ app: XCUIApplication) -> XCUIElement {
        tap(app.buttons["转账"], "转账")
        let picker = app.textFields["搜索代币..."]
        let form = app.textFields["send.amount"]
        let deadline = Date().addingTimeInterval(30)
        while !form.exists && !picker.exists && Date() < deadline { settle(0.5) }
        if picker.exists {
            let row = app.buttons.matching(NSPredicate(
                format: "(label BEGINSWITH %@ AND label CONTAINS %@) OR (label BEGINSWITH %@ AND label CONTAINS %@)",
                "xDAI", "Gnosis", "BNB", "BNB Chain"
            )).firstMatch
            tap(row, "the picker's BNB or xDAI row", timeout: 30)
        }
        let recipient = app.textFields["send.recipient"]
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "the live form never opened")
        return recipient
    }

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
