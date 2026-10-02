//
//  ParallelSpaceDeviceTests.swift
//  VelaWalletUITests
//
//  The wallet with a wallet in it — on the phone, in the parallel space.
//
//  Everything in `DeviceParityTests` is reachable without a session. These are
//  not: real balances, the marks beside them, the receive list, and a dApp
//  asking to connect. The parallel space is the real app with one substitution
//  — `vela-core`'s fixed keyset signs where a passkey would — so a script can
//  reach screens that otherwise need a finger on a sensor.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/ParallelSpaceDeviceTests \
//        -resultBundlePath /tmp/space.xcresult
//
//  **Nothing here spends.** The send is opened and photographed and never
//  confirmed: a slider at the end of it moves real money on a real chain, and
//  that is the founder's to pull.
//

import UIKit
import XCTest

final class ParallelSpaceDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The home, with the golden Safe's own holdings behind it.
    ///
    /// What is being looked at is the MARKS: 058 gave every token and network
    /// its real logo from the chain-data endpoint, with the lettermark as the
    /// fallback. Android and the web have drawn them since 047 and this client
    /// drew three letters in a coloured circle.
    func testTheWalletHomeDrawsItsHoldings() {
        let app = launch()
        // The three action buttons, not the balance label: the label carries
        // the DISPLAY CURRENCY ("总余额 · CNY" on this phone), so asserting one
        // spelling of it asserts a preference.
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened in the parallel space")
        // The reads take a moment; the marks arrive with them.
        settle(8)
        attach(app.screenshot(), named: "space-wallet-home")
        app.terminate()
    }

    /// 收款 — the network list, where every row is a chain's own logo.
    func testTheReceiveListDrawsEveryNetwork() {
        let app = launch()
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened")
        settle(4)
        app.buttons["收款"].tap()
        XCTAssertTrue(app.staticTexts["收款"].waitForExistence(timeout: 12),
                      "the receive screen never opened")
        settle(6)
        attach(app.screenshot(), named: "space-receive-list")
        app.terminate()
    }

    // MARK: - The send form's keypad (087 F28)

    /// On an iPhone 11 the amount's decimal pad could not be put away — no
    /// Done key, and neither a tap outside nor a drag of the form did
    /// anything — and it sat over 继续 until the form was scrolled by hand.
    /// Nothing here spends; the form is filled in and left.
    func testTheSendKeypadCanBePutAway() throws {
        let app = launch()
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40), "the wallet never opened")
        settle(4)
        app.buttons["转账"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 40), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20), "the form never opened")
        settle(2)
        attach(app.screenshot(), named: "f28-1-form")

        let amount = app.textFields["send.amount"]
        let keyboard = app.keyboards.firstMatch
        let cta = app.buttons["继续"].firstMatch

        // The keypad up: 继续 is above it, not under it.
        amount.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5), "the amount field raised no keypad")
        amount.typeText("0.01")
        settle(1)
        attach(XCUIScreen.main.screenshot(), named: "f28-2-keypad-up")
        XCTAssertLessThanOrEqual(cta.frame.maxY, keyboard.frame.minY + 1,
                                 "继续 is under the keypad")

        // 完成 on the keypad puts it away.
        let done = app.buttons["keyboard.done"].firstMatch
        XCTAssertTrue(done.waitForExistence(timeout: 3), "the keypad has no 完成")
        XCTAssertEqual(done.label, "完成")
        done.tap()
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 3), "完成 left the keypad up")

        // A drag of the form puts it away.
        amount.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        app.scrollViews.firstMatch.swipeDown()
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 3), "dragging the form left the keypad up")

        // A tap on the form, away from every control, puts it away.
        amount.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        app.staticTexts["收款人"].firstMatch.tap()
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 3), "a tap outside the field left the keypad up")

        // The recipient's keyboard goes with its own return key.
        let recipient = app.textFields["send.recipient"]
        recipient.tap()
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5), "the recipient field raised no keyboard")
        attach(XCUIScreen.main.screenshot(), named: "f28-3-recipient-keyboard")
        // The return key, in whatever language the keyboard is in ("done" on
        // the simulator's English one).
        app.keyboards.buttons.matching(NSPredicate(format: "label ==[c] 'done' OR label == '完成'")).firstMatch.tap()
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 3), "the recipient's return key left the keyboard up")
        attach(app.screenshot(), named: "f28-4-put-away")
        app.terminate()
    }

    /// 探索 — a real dApp, loaded, in the app's own browser.
    ///
    /// Uniswap is the founder's own test case. What this asserts is that the
    /// page LOADS and the wallet's chrome is around it; connecting and
    /// swapping need decisions a person makes, and the screenshot is what
    /// those decisions get read from.
    func testTheBrowserLoadsUniswap() {
        let app = launch(openUrl: "https://app.uniswap.org/")
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40),
                      "the wallet never opened")
        // `VELA_URL` loads the page into a TAB; it does not change which tab
        // the person is looking at, so the harness has to walk over to it —
        // found here, with a screenshot of the wallet home labelled uniswap.
        // The tab bar is at the bottom of the window; tapping the button by
        // its own coordinate is what survives a label that matches more than
        // one element.
        let explore = app.buttons["探索"].firstMatch
        XCTAssertTrue(explore.waitForExistence(timeout: 8), "no 探索 tab")
        explore.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(3)
        attach(app.screenshot(), named: "space-explore-tab")
        // The browser tab takes the page's own time; give it the same patience
        // a person would — and then some: app.uniswap.org is a large SPA and
        // twenty seconds was not enough on this phone (058's first run showed
        // an empty address bar and a white page).
        settle(60)
        attach(app.screenshot(), named: "space-uniswap")
        // Either the page is there, or the app SAYS WHY it is not. Silence is
        // the one outcome this rejects — a white rectangle under an empty
        // address bar is what 058 found, and what the failure panel exists to
        // make impossible.
        let loaded = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", "uniswap")).firstMatch.exists
        let saidWhy = app.staticTexts["无法加载此页面"].exists
        XCTAssertTrue(
            loaded || saidWhy,
            "the browser showed neither the page nor a reason — it said nothing at all"
        )
        app.terminate()
    }

    /// The same door, with a page that cannot be slow.
    ///
    /// Separates "the browser is broken" from "that dApp is heavy": if this
    /// one renders and Uniswap does not, the finding is about Uniswap.
    func testTheBrowserLoadsAPlainPage() {
        let app = launch(openUrl: "https://example.com/")
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 40))
        let explore = app.buttons["探索"].firstMatch
        XCTAssertTrue(explore.waitForExistence(timeout: 8))
        explore.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(10)
        attach(app.screenshot(), named: "space-plain-page")
        app.terminate()
    }

    // MARK: - Leaving Send (087 F27)

    /// Back out of Send, then 转账 again: a NEW send — never the journey left.
    ///
    /// On an iPhone 11 the back arrow only popped the shell's stack: from the
    /// form it landed on the home, and after a pay link scoped to Base the next
    /// 转账 came up on 选择代币 · 收款人 0xD400…130b · 仅支持 Base 网络付款.
    /// Nothing here spends; the form is opened and left.
    func testLeavingSendEndsTheJourney() throws {
        let app = launch()
        let home = app.buttons["收款"]
        XCTAssertTrue(home.waitForExistence(timeout: 40), "the wallet never opened")
        settle(4)
        let back = app.buttons["返回"].firstMatch
        let picker = app.staticTexts["选择代币"]

        // 转账 → pick → the form.
        app.buttons["转账"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 40), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20), "the form never opened")
        attach(app.screenshot(), named: "f27-1-form")

        // ‹ on the form is the MACHINE's back: the picker, not the home.
        back.tap()
        XCTAssertTrue(picker.waitForExistence(timeout: 10), "Back from the form did not land on the picker")
        XCTAssertFalse(home.exists, "Back from the form left Send altogether")
        attach(app.screenshot(), named: "f27-2-back-to-picker")

        // ‹ on the picker ends the journey.
        back.tap()
        XCTAssertTrue(home.waitForExistence(timeout: 10), "Back from the picker did not leave Send")

        // A pay link scoped to Base, to 0xD400…130b — then Back until home.
        let link = "velawallet://pay?to=0xD400866e00B055B20752a826CD5C89b811de130b&chain=8453"
        app.open(try XCTUnwrap(URL(string: link)))
        XCTAssertTrue(back.waitForExistence(timeout: 30), "the pay link never opened Send")
        settle(4)
        attach(app.screenshot(), named: "f27-3-pay-link")
        // On the simulator, main's pay link can end in 加载代币失败 — found
        // here, and not this defect: it is put away, and the Back after it
        // is what is being tested.
        let gotIt = app.alerts.buttons["知道了"].firstMatch
        if gotIt.exists { gotIt.tap() }
        for _ in 0..<3 where !home.exists {
            back.tap()
            _ = home.waitForExistence(timeout: 5)
        }
        XCTAssertTrue(home.exists, "Back never left the pay link's send")

        // The home's 转账: a fresh picker on this wallet's own holdings — not
        // the pay link's journey, its recipient or its scope.
        app.buttons["转账"].firstMatch.tap()
        XCTAssertTrue(picker.waitForExistence(timeout: 20), "转账 did not open the picker")
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 30),
                      "the new send is still the pay link's journey: no holdings listed")
        settle(2)
        attach(app.screenshot(), named: "f27-4-fresh-send")
        XCTAssertFalse(hasText(app, containing: "D400"), "the last pay link's recipient came back on a new send")
        XCTAssertFalse(app.alerts.firstMatch.exists, "the new send opened on the old journey's alert")
        app.terminate()
    }

    private func hasText(_ app: XCUIApplication, containing fragment: String) -> Bool {
        app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", fragment)).count > 0
            || app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", fragment)).count > 0
    }

    // MARK: - Plumbing

    private func launch(openUrl: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        // IN the space: `vela-core`'s fixed keyset, the golden Safe, real
        // chains. The flag persists, so every other test in this suite states
        // `0` to stay out of it.
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        // `VELA_URL` is the harness's own door into the browser (053).
        if let openUrl { app.launchEnvironment["VELA_URL"] = openUrl }
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
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
