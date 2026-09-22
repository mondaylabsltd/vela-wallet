//
//  ClearSignerChooserDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 075 SC-001 on the simulator: the Clear Signer is on the CREATE and
//  SIGN-IN choosers, beside 这台设备 / 手机或平板 / USB 安全密钥, and choosing
//  it asks where the signer is instead of raising the OS passkey sheet.
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/ClearSignerChooserDeviceTests
//
//  This is the owner's whole complaint, checked where a person would see it:
//  「创建/登录/转账/dapp签名/公钥备份 等 都只有 这台设备 手机或平板 USB 安全密钥」.
//  A unit test can pin `KeyMethod.allCases`; only this can say the row is
//  drawn, is reachable, and leads somewhere.
//
//  **Nothing here signs, and nothing needs a network.** Both tests stop at the
//  where-choice and cancel. The create flow reaches its key list without a
//  ceremony (the machine mints no key until a method is picked), which is
//  exactly what makes the picker reachable on a simulator with no passkeys.
//
//  Launched OUTSIDE the parallel space (`VELA_PARALLEL_SPACE=0`): these two
//  want the Welcome screen, and an earlier class in the same run enters the
//  space — which writes a fixture wallet. Clearing the flag alone leaves that
//  record on disk and this class then opens on a wallet, which is the trap
//  `VelaWalletUITests` is skipped for. `0` LEAVES, and leaving removes it.
//

import XCTest

final class ClearSignerChooserDeviceTests: XCTestCase {

    private enum Words {
        static let create = "创建钱包"
        static let signIn = "我已有钱包"
        static let clearSigner = "清晰签名器"
        static let whereIsIt = "清晰签名器在哪里？"
        static let thisDevice = "在这台设备上"
        static let otherDevice = "在另一台设备上"
        static let thisOne = "这台设备"
        static let phone = "手机或平板"
        static let securityKey = "USB 安全密钥"
        static let nameTitle = "给钱包起个名字"
        static let next = "继续"
        static let cancel = "取消"
    }

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// 我已有钱包 → the method sheet lists FOUR routes, the fourth is the Clear
    /// Signer, and tapping it asks where the signer is.
    func testTheSignInChooserOffersTheClearSignerAndAsksWhere() throws {
        let app = launch()
        XCTAssertTrue(app.buttons[Words.signIn].waitForExistence(timeout: 30),
                      "Welcome did not appear — is a session left over from another class?")
        app.buttons[Words.signIn].tap()

        for route in [Words.thisOne, Words.phone, Words.securityKey, Words.clearSigner] {
            XCTAssertTrue(app.staticTexts[route].waitForExistence(timeout: 10),
                          "the sign-in chooser does not offer \(route)")
        }
        attach(XCUIScreen.main.screenshot(), named: "sign-in-chooser")

        app.staticTexts[Words.clearSigner].firstMatch.tap()
        assertWhereChoice(in: app, named: "sign-in-where")
        cancel(app)
        app.terminate()
    }

    /// 创建钱包 → name → the key list's picker lists FOUR routes, and choosing
    /// the Clear Signer asks where it is rather than opening the OS sheet.
    func testTheCreateChooserOffersTheClearSignerAndAsksWhere() throws {
        let app = launch()
        XCTAssertTrue(app.buttons[Words.create].waitForExistence(timeout: 30),
                      "Welcome did not appear — is a session left over from another class?")
        app.buttons[Words.create].tap()

        XCTAssertTrue(app.staticTexts[Words.nameTitle].waitForExistence(timeout: 20))
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText("清晰")

        // The three acknowledgements. Each checkbox carries its whole sentence
        // as its label, so the rows are addressable without a fixture id.
        let acks = app.buttons.matching(
            NSPredicate(format: "label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@",
                        "公钥和钱包名字", "私钥保存在", "我已阅读并同意")
        )
        XCTAssertEqual(acks.count, 3, "the name screen no longer has three acknowledgements")
        for index in 0..<acks.count { acks.element(boundBy: index).tap() }

        let next = app.buttons[Words.next]
        XCTAssertTrue(next.waitForExistence(timeout: 5))
        XCTAssertTrue(next.isEnabled, "继续 is still disabled — an acknowledgement did not take")
        next.tap()

        // The key list opens with the picker already open, because there is no
        // key yet. Every founding key is minted from this choice, the first one
        // included (the core stopped choosing for the shell in 062).
        for route in [Words.thisOne, Words.phone, Words.securityKey, Words.clearSigner] {
            XCTAssertTrue(app.staticTexts[route].waitForExistence(timeout: 30),
                          "the create chooser does not offer \(route)")
        }
        attach(XCUIScreen.main.screenshot(), named: "create-chooser")

        app.staticTexts[Words.clearSigner].firstMatch.tap()
        assertWhereChoice(in: app, named: "create-where")
        cancel(app)
        app.terminate()
    }

    // MARK: - Helpers

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        // `VELA_PARALLEL_SPACE=0` LEAVES the space, and leaving is the point:
        // an earlier class in the same run enters it, and entering writes a
        // fixture wallet into `vela.accounts`. Clearing only the flag (the
        // argument domain) would leave that record behind and this class would
        // open on a wallet rather than on Welcome — which is exactly why
        // `VelaWalletUITests` is skipped in the scheme.
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    /// The Clear Signer's own sheet: where is it — this device, or another one.
    /// No OS passkey sheet, and nothing signed.
    private func assertWhereChoice(in app: XCUIApplication, named: String) {
        XCTAssertTrue(app.staticTexts[Words.whereIsIt].waitForExistence(timeout: 20),
                      "choosing the Clear Signer did not ask where it is")
        XCTAssertTrue(app.staticTexts[Words.thisDevice].exists, "no \(Words.thisDevice)")
        XCTAssertTrue(app.staticTexts[Words.otherDevice].exists, "no \(Words.otherDevice)")
        attach(XCUIScreen.main.screenshot(), named: named)
    }

    private func cancel(_ app: XCUIApplication) {
        let cancel = app.buttons[Words.cancel].firstMatch
        if cancel.waitForExistence(timeout: 5) { cancel.tap() }
    }

    private func attach(_ shot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
