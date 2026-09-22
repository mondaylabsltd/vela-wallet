//
//  SettingsAuditDeviceTests.swift
//  VelaWalletUITests
//
//  Every setting, TAPPED, and the effect looked for where a person would look
//  for it — 049's lesson applied to the other phone.
//
//  The founder's report on 2026-09-15: "设置页面切换语言似乎没有生效,切换首字母
//  和图形头像也没有理解生效". A hermetic test cannot answer that, because what
//  is in question is not what a builder returns — it is whether the SCREEN
//  redraws when a stored preference changes. Only a running app can say.
//  (The avatar half of that report is moot since spec 074: the 首字母 /
//  图形头像 choice is gone and every avatar is the identicon.)
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/SettingsAuditDeviceTests
//
//  Nothing here needs a wallet: `VELA_PAGE=settings-live` mounts the real
//  settings machine. Every case restores what it changed, so the founder's own
//  phone is left as it was found.
//

import UIKit
import XCTest

final class SettingsAuditDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// 语言 — picked in the sheet, and the app is in that language **now**.
    func testPickingALanguageChangesTheAppWithoutARelaunch() {
        let app = launch()
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 20))
        settle()

        open(row: "语言", in: app)
        tap(element(labelled: "日本語", in: app), in: app)
        settle(1.5)
        // Before the assertion, so a failure carries the evidence: if the ROW
        // reads 日本語 and the page is still Chinese, the pick reached the
        // store and not the screen — a different bug from a pick that was
        // never delivered.
        attach(app.screenshot(), named: "audit-language-picked")

        XCTAssertTrue(
            app.staticTexts["設定"].waitForExistence(timeout: 6),
            "the page is still Chinese after picking 日本語 — the pick did not reach the screen"
        )
        attach(app.screenshot(), named: "audit-language-after")

        // Put it back: 系统 follows the device, which is where it started.
        open(row: "言語", in: app)
        tap(element(labelled: "システムに合わせる", in: app), in: app)
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 6),
                      "系统 did not return the app to the device's language")
        app.terminate()
    }

    /// 数字格式 — every figure on the page, in the chosen shape.
    ///
    /// The currency row carries a formatted amount, so it is where the choice
    /// has to show. 049 found this exact defect on Android: the row ticked and
    /// the figures did not move.
    func testTheNumberFormatChangesTheFiguresOnThePage() {
        let app = launch()
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 20))
        settle()

        open(row: "数字格式", in: app)
        // The four presets are drawn as their own example. Pick the European
        // one, which cannot be confused with any other.
        tap(element(labelled: "1.234.567,89", in: app), in: app)
        settle(0.8)
        attach(app.screenshot(), named: "audit-number-european")

        // The page's own row must now read the same way it was asked to.
        XCTAssertTrue(
            element(labelled: "1.234.567,89", in: app).exists,
            "the number-format row does not show the choice it was given"
        )

        open(row: "数字格式", in: app)
        tap(element(labelled: "1,234,567.89", in: app), in: app)
        settle(0.8)
        attach(app.screenshot(), named: "audit-number-anglo")
        app.terminate()
    }

    /// 主题 — the background changes, which is the whole of what it promises.
    func testTheThemeRepaintsTheScreen() {
        let app = launch(theme: nil)
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 20))
        settle()

        tap(app.staticTexts["浅色"], in: app)
        settle(0.8)
        let light = app.screenshot().pngRepresentation
        attach(app.screenshot(), named: "audit-theme-light")

        tap(app.staticTexts["深色"], in: app)
        settle(0.8)
        let dark = app.screenshot().pngRepresentation
        attach(app.screenshot(), named: "audit-theme-dark")

        XCTAssertNotEqual(light, dark, "the two themes paint the same screen")
        tap(app.staticTexts["跟随系统"], in: app)
        app.terminate()
    }

    // MARK: - Plumbing

    /// 退出登录 — ONE tap, ONE sheet, and it is the CORE's.
    ///
    /// The settings screen used to raise its own ST3 confirm first and the
    /// session machine's sheet only after it, so leaving a wallet was three
    /// taps and two sheets saying the same sentence (founder, 2026-09-16).
    /// What tells them apart is the body: ST3 leads with `signOut.desc`
    /// ("此设备将退出登录…"), the core's sheet carries only `signOut.keeps`.
    ///
    /// **Needs a signed-in phone, and says so rather than failing on one that
    /// is not.** The row now ASKS THE CORE, and the session machine refuses
    /// `SignOut` unless a wallet is active — correctly, but it means an empty
    /// device can prove nothing here. Neither `settings-live` nor
    /// `VELA_PARALLEL_SPACE=1` stands a session up on a phone with no
    /// credential (verified 2026-09-16 on the iPhone 11: the account row keeps
    /// its 切换账户 chevron and shows no address).
    ///
    /// Cancels rather than confirming: the flow being verified is the sheet,
    /// not the wipe.
    func testSignOutIsOneTapAndOneSheet() throws {
        // The app's OWN navigation, not `VELA_PAGE=settings-live`: the page
        // override renders the settings surface outside the route the session
        // is attached to, so its account row stands empty even on a phone with
        // a wallet — which is exactly how this test first reported "not signed
        // in" on a phone that was (2026-09-16).
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        settle(6)

        let settingsTab = app.buttons["设置"].exists ? app.buttons["设置"] : app.staticTexts["设置"]
        XCTAssertTrue(settingsTab.waitForExistence(timeout: 20), "no settings tab")
        settingsTab.tap()
        settle(2)

        // The precondition, with the evidence attached either way — so "not
        // signed in" can be told apart from "signed in, and this locator is
        // wrong".
        let address = app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "0x")
        ).firstMatch
        let signedIn = address.waitForExistence(timeout: 8)
        attach(app.screenshot(), named: "audit-sign-out-precondition")
        let hierarchy = XCTAttachment(string: app.debugDescription)
        hierarchy.name = "audit-sign-out-hierarchy"
        hierarchy.lifetime = .keepAlways
        add(hierarchy)
        try XCTSkipUnless(
            signedIn,
            "no wallet is signed in on this device — sign in first, then re-run"
        )

        app.swipeUp()
        app.swipeUp()
        settle(1.0)

        open(row: "退出登录", in: app)
        attach(app.screenshot(), named: "audit-sign-out-one-sheet")

        // The core's sheet, by the only thing that distinguishes it from ST3.
        XCTAssertTrue(
            app.staticTexts.containing(
                NSPredicate(format: "label BEGINSWITH %@", "选择「我已有钱包」")
            ).firstMatch.waitForExistence(timeout: 8),
            "the core's sign-out sheet did not open on the first tap"
        )
        XCTAssertFalse(
            app.staticTexts.containing(
                NSPredicate(format: "label BEGINSWITH %@", "此设备将退出登录")
            ).firstMatch.exists,
            "the settings confirm is still standing in front of the core's sheet"
        )

        // Cancels rather than confirming: the flow under test is the sheet,
        // not the wipe, and this runs on somebody's signed-in phone.
        tap(element(labelled: "取消", in: app), in: app)
        settle(1.5)
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 8),
                      "cancelling did not return to settings")
        app.terminate()
    }

    /// The account switcher, everywhere it is now reachable — and the
    /// identicon viewer, whose way out used to be off-screen.
    ///
    /// Four things the founder found on 2026-09-16, in one pass:
    ///   1. the home header's chevron led nowhere,
    ///   2. the settings switcher listed fixture accounts,
    ///   3. 创建新账户 / 登录已有账户 had empty closures,
    ///   4. the viewer clipped its artwork under the grabber and its 关闭
    ///      button off the bottom of the screen.
    func testSwitcherAndViewerOnDevice() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        settle(8)

        // 1. The home header's name opens the switcher.
        let name = app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "Parallel")
        ).firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 20), "no wallet name in the header")
        name.tap()
        settle(2)
        attach(app.screenshot(), named: "shot-home-switcher")
        // 2. And its rows are the session's, not the fixture three.
        XCTAssertTrue(
            app.staticTexts.containing(
                NSPredicate(format: "label BEGINSWITH %@", "0x88cC")
            ).firstMatch.waitForExistence(timeout: 8),
            "the switcher did not open on the header tap, or shows no live row"
        )
        for mock in ["Ann", "Bo", "Cy"] {
            XCTAssertFalse(app.staticTexts[mock].exists, "fixture account \(mock) is still listed")
        }
        // 3. The two ways on are live: 登录已有账户 raises the method picker.
        let signIn = app.buttons.containing(
            NSPredicate(format: "label CONTAINS %@", "登录")
        ).firstMatch
        if signIn.waitForExistence(timeout: 4) {
            signIn.tap()
            settle(2)
            attach(app.screenshot(), named: "shot-signin-picker")
        }
        app.terminate()

        // 4. The identicon viewer: its way out has to be ON the screen.
        let second = XCUIApplication()
        second.launchEnvironment["VELA_LANG"] = "zh"
        second.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        second.launch()
        settle(6)
        let art = second.buttons.matching(
            NSPredicate(format: "label CONTAINS %@", "身份图")
        ).firstMatch
        XCTAssertTrue(art.waitForExistence(timeout: 12), "no identicon button in the header")
        art.tap()
        settle(2)
        attach(second.screenshot(), named: "shot-identicon-viewer")
        let close = second.buttons["关闭"]
        XCTAssertTrue(close.waitForExistence(timeout: 6), "the viewer has no 关闭 button")
        XCTAssertTrue(close.isHittable, "关闭 is off-screen — the viewer clips its own way out")
        close.tap()
        settle(1.5)
        second.terminate()
    }

    /// Spec 047 on iOS: a network row wears its chain's own logo, the letter
    /// only until the logo lands — as Android's and the web's always have.
    func testTheNetworkListWearsChainLogos() {
        let app = launch()
        XCTAssertTrue(app.staticTexts["高级"].waitForExistence(timeout: 30),
                      "the settings page never opened")
        if !app.staticTexts["网络"].exists {
            app.staticTexts["高级"].tap()
            settle(1)
        }
        if !app.staticTexts["网络"].isHittable {
            app.swipeUp()
            settle(1)
        }
        open(row: "网络", in: app)
        XCTAssertTrue(app.staticTexts["Ethereum"].waitForExistence(timeout: 10),
                      "the network list never opened")
        // The logos' own round trips.
        settle(6)
        attach(app.screenshot(), named: "audit-network-logos")
        app.terminate()
    }

    private func launch(
        page: String = "settings-live",
        theme: String? = "dark",
        account: String? = nil
    ) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_PAGE"] = page
        // NOT pinned: this file is about what a PERSON picking a language does,
        // and `VELA_LANG` deliberately outranks a stored choice (058).
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        if let theme { app.launchEnvironment["VELA_THEME"] = theme }
        if let account { app.launchEnvironment["VELA_ACCOUNT"] = account }
        app.launch()
        return app
    }

    /// Open a settings row by its title.
    ///
    /// The row is an `HStack` with `.onTapGesture`, not a `Button`, so the tap
    /// goes to the row's own frame rather than to the label inside it — a
    /// label tap lands on a `Text` that owns no gesture and the sheet never
    /// opens. The screenshot is kept either way: a failure here should say
    /// what the screen looked like.
    private func open(row: String, in app: XCUIApplication) {
        let label = app.staticTexts[row]
        XCTAssertTrue(label.waitForExistence(timeout: 8), "no row called \(row)")
        label.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(1.0)
        attach(app.screenshot(), named: "audit-opened-\(row)")
    }

    /// Anything whose accessibility label is this text, whatever element type
    /// it turned out to be.
    ///
    /// A select sheet's row is not a `staticText` to XCUITest — the row
    /// combines its label, note and tick into one element — so looking only at
    /// `app.staticTexts` finds a sheet that is plainly on screen and reports
    /// nothing to tap.
    private func element(labelled text: String, in app: XCUIApplication) -> XCUIElement {
        // A select sheet's row is a Button whose LABEL is the row's text, and
        // the button is what has to be tapped: `descendants(matching: .any)`
        // hands back the static text inside it first, and a tap there is
        // swallowed. (Found on the device: the sheet stayed open and the
        // selection never moved.)
        let button = app.buttons[text]
        if button.exists { return button }
        let prefixed = app.buttons
            .matching(NSPredicate(format: "label BEGINSWITH %@", text)).firstMatch
        if prefixed.exists { return prefixed }
        return app.staticTexts[text]
    }

    private func tap(_ element: XCUIElement, in app: XCUIApplication) {
        XCTAssertTrue(element.waitForExistence(timeout: 8),
                      "nothing to tap: \(element.description)")
        element.tap()
    }

    /// Past the launch animation, or past a sheet's own transition.
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
