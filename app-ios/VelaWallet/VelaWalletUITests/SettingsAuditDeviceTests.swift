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

    /// 头像样式 — 首字母 and 图形头像 draw DIFFERENT art for the same address.
    ///
    /// Read on the CONTACTS page, not on the settings page that owns the
    /// control: the settings account row has no address unless a wallet is
    /// signed in, so its avatar is an empty circle under both choices and a
    /// comparison there proves nothing. (It passed that way once, which is how
    /// this defect survived 057.)
    ///
    /// The choice is made in one launch and read in the next, so what this
    /// asserts is the part a person actually complained about: **the style
    /// reaches avatars on other screens.** Same-session redraw is the
    /// environment's job (`\.avatarStyle`) and needs a signed-in wallet to
    /// watch happen.
    func testTheAvatarStyleReachesEveryAvatar() throws {
        func avatarBytes(after style: String) -> Data? {
            let settings = launch()
            XCTAssertTrue(settings.staticTexts["设置"].waitForExistence(timeout: 20))
            settle()
            tap(settings.staticTexts[style], in: settings)
            settle(0.8)
            settings.terminate()

            let contacts = launch(page: "contacts")
            XCTAssertTrue(contacts.staticTexts["Alice"].waitForExistence(timeout: 20),
                          "the fixture address book never appeared")
            settle()
            attach(contacts.screenshot(), named: "audit-avatar-\(style)")
            let row = contacts.staticTexts["Alice"].frame
            // The row's leading edge, where every contact avatar is drawn.
            let art = CGRect(x: 16, y: row.midY - 24, width: 56, height: 48)
            let bytes = crop(contacts.screenshot(), to: art)
            contacts.terminate()
            return bytes
        }

        let initials = avatarBytes(after: "首字母")
        let identicon = avatarBytes(after: "图形头像")
        XCTAssertNotNil(initials)
        XCTAssertNotEqual(
            initials, identicon,
            "a contact's avatar is identical under both styles — the choice does not reach the artwork"
        )
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
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PAGE"] = "settings-live"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 20))
        settle()

        // The precondition, stated where a reader will see it fail.
        let signedIn = app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "0x")
        ).firstMatch
        try XCTSkipUnless(
            signedIn.waitForExistence(timeout: 6),
            "no wallet is signed in on this device — sign in first, then re-run"
        )

        app.swipeUp()
        app.swipeUp()
        settle(1.0)

        open(row: "退出登录", in: app)
        attach(app.screenshot(), named: "audit-sign-out-one-sheet")

        // The core's sheet, by the only thing that distinguishes it.
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

        // A way back out, and it leaves the session alone.
        tap(element(labelled: "取消", in: app), in: app)
        settle(1.5)
        XCTAssertTrue(app.staticTexts["设置"].waitForExistence(timeout: 8),
                      "cancelling did not return to settings")
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

    /// The bytes of one region of a screenshot, for comparing artwork rather
    /// than screens.
    private func crop(_ screenshot: XCUIScreenshot, to rect: CGRect) -> Data? {
        let image = screenshot.image
        let scale = image.scale
        let scaled = CGRect(x: rect.minX * scale, y: rect.minY * scale,
                            width: rect.width * scale, height: rect.height * scale)
        guard let cg = image.cgImage?.cropping(to: scaled) else { return nil }
        return UIImage(cgImage: cg).pngData()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
