//
//  IssueFixesDeviceTests.swift
//  VelaWalletUITests
//
//  Issues #444, #445, #446, #449, #450 and #459, looked at on a real phone —
//  each one a thing a person SEES, which no hermetic test can say.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/IssueFixesDeviceTests \
//        -resultBundlePath /tmp/issues.xcresult
//
//  Nothing here writes: the galleries are fixtures, the address book is only
//  exported (and the share sheet dismissed), and the sign-in ceremonies are
//  cancelled or left to time out. The phone is left as it was found.
//

import UIKit
import XCTest

final class IssueFixesDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// #444 — "submitted" with no time estimate: the ring circles instead of a
    /// still clock. Two frames ~0.6 s apart show it moving.
    func testASubmittedTransactionWithoutAnEstimateKeepsMoving() {
        let app = launch(page: "flows-gallery", state: "sd4b")
        XCTAssertTrue(app.staticTexts["交易已提交至网络"].waitForExistence(timeout: 20))
        settle()
        attach(app.screenshot(), named: "444-submitted-a")
        settle(0.6)
        attach(app.screenshot(), named: "444-submitted-b")
        app.terminate()
    }

    /// #445 — Add member on a book of eight: the sheet can be searched, says
    /// so when nothing matches, and keeps the ticks it hides.
    func testTheMemberPickerCanBeSearched() {
        let app = launch(page: "contacts-gallery", state: "c10")
        XCTAssertTrue(app.staticTexts["添加成员"].waitForExistence(timeout: 20))
        settle()
        attach(app.screenshot(), named: "445-open")

        let field = app.textFields["搜索名字、ENS 或地址"]
        XCTAssertTrue(field.waitForExistence(timeout: 6), "the picker of eight has no search field")
        field.tap()
        field.typeText("Ali")
        settle(1)
        attach(app.screenshot(), named: "445-query-ali")
        XCTAssertTrue(app.staticTexts["Alice"].exists, "Alice should match \"Ali\"")
        XCTAssertFalse(app.staticTexts["Charlie"].exists, "Charlie should be filtered out")

        field.typeText("zzz")
        settle(1)
        attach(app.screenshot(), named: "445-no-match")
        XCTAssertTrue(
            app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "没有匹配")).firstMatch.exists,
            "no \"no matches\" line"
        )
        app.terminate()
    }

    /// #449 — Contacts → menu → Export contacts opens the share sheet.
    func testExportContactsOpensTheShareSheet() {
        let app = launch(page: "contacts-live")
        let add = app.buttons["contacts.add"]
        XCTAssertTrue(add.waitForExistence(timeout: 20))
        settle()
        add.tap()
        settle(1.2)
        attach(app.screenshot(), named: "449-menu")

        tap(element(labelled: "导出通讯录", in: app))
        // The menu has to finish leaving before the share sheet is asked for —
        // that ordering is the fix — so give it the dismissal and the sheet's
        // own rise.
        settle(3)
        attach(app.screenshot(), named: "449-after-export")
        let share = app.otherElements["ActivityListView"]
        let shareShown = share.waitForExistence(timeout: 8)
            || app.navigationBars["UIActivityContentView"].exists
            || app.buttons["Close"].exists || app.buttons["关闭"].exists
        attach(app.screenshot(), named: "449-share-sheet")
        XCTAssertTrue(shareShown, "no share sheet after Export")

        // Leave without sending the file anywhere.
        app.swipeDown(velocity: .fast)
        settle(1.5)
        if share.exists {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.08)).tap()
            settle(1)
        }
        attach(app.screenshot(), named: "449-dismissed")
        app.terminate()
    }

    /// #450 — USB security key with none plugged in: "insert your security
    /// key", never "biometric authentication is not available". Closed again.
    func testASecurityKeyWithNonePluggedInAsksForIt() {
        let app = launch(page: nil)
        toSignInMethods(app)
        tap(element(labelled: "USB 安全密钥", in: app))
        settle(3)
        attach(app.screenshot(), named: "450-after-pick")
        XCTAssertTrue(
            app.staticTexts["插入安全密钥"].waitForExistence(timeout: 10),
            "no insert-your-key sheet"
        )
        XCTAssertFalse(app.staticTexts["不支持"].exists, "the not-supported sheet came up")
        XCTAssertFalse(app.staticTexts["Not Supported"].exists, "an ENGLISH not-supported sheet came up")
        attach(app.screenshot(), named: "450-insert-sheet")

        tap(element(labelled: "关闭", in: app))
        settle(2)
        attach(app.screenshot(), named: "450-closed")
        app.terminate()
    }

    /// #446 — Phone or tablet, and no phone ever scans: when the 90 s scan
    /// gives up, the sheet says the link failed, not "set up Face ID".
    /// Leave the code UNSCANNED while this runs — a phone that answers it
    /// signs that wallet in (the success path, which this test does not
    /// wait for).
    func testAPhoneThatNeverAnswersIsSaidAsTheLink() throws {
        // A phone scan needs the device's own Bluetooth and a 90 s wait; a
        // simulator answers neither honestly.
        #if targetEnvironment(simulator)
        let simulator = true
        #else
        let simulator = false
        #endif
        try XCTSkipIf(simulator, "the phone-link sheet is looked at on a real device")
        let app = launch(page: nil)
        toSignInMethods(app)
        tap(element(labelled: "手机或平板", in: app))
        settle(4)
        attach(app.screenshot(), named: "446-qr")

        let link = app.staticTexts
            .matching(NSPredicate(format: "label BEGINSWITH %@", "另一台设备")).firstMatch
        XCTAssertTrue(link.waitForExistence(timeout: 150), "no phone-link sheet after the scan timed out")
        attach(app.screenshot(), named: "446-link-failed")
        XCTAssertFalse(
            app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "面容")).firstMatch.exists,
            "the sheet still talks about Face ID"
        )
        // Put the sheet away if it is still up; the sentence above is the
        // finding, the close is only tidying.
        let close = element(labelled: "关闭", in: app)
        if close.waitForExistence(timeout: 3) { close.tap() }
        settle(1.5)
        app.terminate()
    }

    /// #459 — Phone or tablet, then the person changes their mind: Cancel,
    /// and separately a swipe down, takes the code away at once, Welcome's
    /// buttons work again, and no "your other device didn't connect" sheet
    /// follows. Leave the code UNSCANNED while this runs.
    func testADismissedPhoneCodeCancelsQuietly() throws {
        let app = launch(page: nil)
        for exit in ["cancel", "swipe"] {
            toSignInMethods(app)
            tap(element(labelled: "手机或平板", in: app))
            let cancel = app.buttons["cable.cancel"]
            XCTAssertTrue(cancel.waitForExistence(timeout: 15), "the phone's code has no Cancel")
            settle(1.5)
            attach(app.screenshot(), named: "459-code-\(exit)")

            if exit == "cancel" {
                cancel.tap()
            } else {
                // From the sheet's own title: since issue #480 the sheet is
                // as tall as its content, so a point near the top of the
                // screen is the page behind it (a tap there, not a swipe on
                // the sheet) — and a drag that starts low on a short sheet
                // has too little room left to carry it away.
                element(labelled: "手机或平板", in: app).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
                    .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98)))
            }
            XCTAssertTrue(cancel.waitForNonExistence(timeout: 5), "the code stayed up after \(exit)")
            // Long enough for a phone-link sheet to have come back, were the
            // scan still running into one.
            settle(5)
            attach(app.screenshot(), named: "459-after-\(exit)")
            XCTAssertFalse(
                app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "另一台设备")).firstMatch.exists,
                "a dismissed code was blamed on the link"
            )
            if app.buttons["我已有钱包"].exists {
                XCTAssertTrue(app.buttons["我已有钱包"].isEnabled, "Welcome is still busy after \(exit)")
            }
        }
        app.terminate()
    }

    // MARK: - Helpers

    private func launch(page: String?, state: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        if let page { app.launchEnvironment["VELA_PAGE"] = page }
        if let state { app.launchEnvironment["VELA_STATE"] = state }
        app.launch()
        return app
    }

    /// The sign-in method sheet: from Welcome when no wallet is here, or via
    /// Settings → 切换账户 → 登录已有账户 when one is — which signs nobody out.
    private func toSignInMethods(_ app: XCUIApplication) {
        let already = app.buttons["我已有钱包"]
        if !already.waitForExistence(timeout: 15) {
            attach(app.screenshot(), named: "root")
            tap(element(labelled: "设置", in: app))
            settle(1.5)
            tap(element(labelled: "切换账户", in: app))
            settle(1.5)
            attach(app.screenshot(), named: "accounts")
            // From the account list the method sheet comes up directly —
            // there is no Welcome in between.
            tap(element(labelled: "登录已有账户", in: app))
            settle(2)
        } else {
            tap(already)
            settle(1.5)
        }
        XCTAssertTrue(
            element(labelled: "USB 安全密钥", in: app).waitForExistence(timeout: 10),
            "the sign-in method sheet did not come up"
        )
        attach(app.screenshot(), named: "sign-in-methods")
    }

    private func element(labelled text: String, in app: XCUIApplication) -> XCUIElement {
        let button = app.buttons[text]
        if button.exists { return button }
        let prefixed = app.buttons
            .matching(NSPredicate(format: "label BEGINSWITH %@", text)).firstMatch
        if prefixed.exists { return prefixed }
        return app.staticTexts[text]
    }

    private func tap(_ element: XCUIElement) {
        XCTAssertTrue(element.waitForExistence(timeout: 10), "nothing to tap: \(element.description)")
        element.tap()
    }

    private func settle(_ seconds: TimeInterval = 2.5) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
