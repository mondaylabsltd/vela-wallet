//
//  SplitRowTouchDeviceTests.swift
//  VelaWalletUITests
//
//  Issue #331 on the simulator: on a split send, a tap meant for a
//  recipient's amount never removes the recipient. It was a one-line figure
//  with an 18pt ✕ 12pt to its right, and SwiftUI gives a touch NEAR a button to
//  the button (its touch radius — ~20pt on the simulator), so a tap just past
//  the "0" dropped the row. The amount is now a well a full control tall, and
//  the field runs on through the gap to the ✕'s own 44pt target: no point short
//  of the ✕ is left for the ✕ to claim.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/SplitRowTouchDeviceTests
//
//  **Nothing here spends.** The split form is opened and tapped, never
//  continued. It needs the parallel space's wallet (its home loads over the
//  network), like the other device tests.
//

import XCTest

final class SplitRowTouchDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testATapMeantForTheAmountNeverRemovesTheRecipient() {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        tap(app.buttons["转账"], "转账")
        let picker = app.textFields["搜索代币..."]
        let form = app.textFields["send.amount"]
        let deadline = Date().addingTimeInterval(30)
        while !form.exists && !picker.exists && Date() < deadline { settle(0.5) }
        if picker.exists {
            // The combined label's separator follows the locale ("," in
            // English, "，" or "、" under zh-Hans), so the row is the button
            // that names both the coin and its network. The parallel space's
            // golden Safe is a real account: the picker lists only what it
            // holds, so either of the coins it is kept funded in will do.
            let row = app.buttons.matching(NSPredicate(
                format: "(label BEGINSWITH %@ AND label CONTAINS %@) OR (label BEGINSWITH %@ AND label CONTAINS %@)",
                "xDAI", "Gnosis", "BNB", "BNB Chain"
            )).firstMatch
            tap(row, "the picker's xDAI or BNB row", timeout: 30)
        }
        tap(app.buttons["+  添加收款人"].firstMatch, "+ 添加收款人", timeout: 30)
        XCTAssertTrue(app.staticTexts["收款人 2"].waitForExistence(timeout: 15), "the split never opened")
        settle(1)

        let amounts = app.textFields.matching(NSPredicate(format: "placeholderValue == %@", "0"))
        let removes = app.buttons.matching(NSPredicate(format: "label == %@", "移除"))
        XCTAssertGreaterThanOrEqual(amounts.count, 2, "the split rows have no amount fields")
        let rows = removes.count
        let field = amounts.element(boundBy: 0).frame
        let cross = removes.element(boundBy: 0).frame
        attach(app.screenshot(), named: "split-rows")

        XCTAssertGreaterThanOrEqual(field.height, 44, "the amount is shorter than a control (\(field.height)pt)")
        XCTAssertLessThanOrEqual(cross.minX - field.maxX, 1, "a gap between the amount and the ✕ is the ✕'s to claim")

        // The visible well's right edge (12pt short of the field's), a thumb's
        // overshoot just past it, and the last point before the ✕ — from the
        // top of the well to its bottom.
        let wellEdge = field.maxX - 12
        let origin = app.coordinate(withNormalizedOffset: .zero)
        let ordinal = origin.withOffset(CGVector(dx: field.minX - 60, dy: field.minY + 6))
        for x in [wellEdge - 2, wellEdge + 2, field.maxX - 2] {
            for y in stride(from: field.minY + 2, through: field.maxY - 2, by: (field.height - 4) / 4) {
                origin.withOffset(CGVector(dx: x, dy: y)).tap()
                settle(0.6)
                // A second tap on a field in hand raises the edit menu, which
                // hides the rest of the screen from the query; an inert tap on
                // the row's ordinal puts it away.
                ordinal.tap()
                settle(0.4)
            }
        }
        XCTAssertEqual(removes.count, rows, "a tap meant for the amount removed a recipient")
        XCTAssertTrue(app.staticTexts["收款人 2"].exists, "a tap meant for the amount removed a recipient")

        // And the well is the field: its bottom right corner, where the ✕ used
        // to be within reach, puts the caret in it.
        origin.withOffset(CGVector(dx: wellEdge - 4, dy: field.maxY - 4)).tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5), "the amount never took the tap")
        attach(app.screenshot(), named: "split-amount-in-hand")
        app.terminate()
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
