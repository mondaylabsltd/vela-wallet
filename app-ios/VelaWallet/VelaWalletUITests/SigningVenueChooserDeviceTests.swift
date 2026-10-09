//
//  SigningVenueChooserDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 102 on the simulator: the create and sign-in choosers list where a key
//  LIVES — 这台设备 / 手机或平板 / USB 安全密钥 — and nothing else; the trusted
//  page is no longer a fourth row. Apart from them, "使用我自己的签名页" opens
//  the list of signing pages (Vela's own first), each with its domain.
//
//  The owner's complaint this answers (2026-10-09): tapping 可信签名器 showed
//  this device / scan a code / USB key AGAIN, and nothing said what was
//  trusted. A unit test can pin `KeyMethod.allCases`; only this can say what
//  is drawn and that the entry leads somewhere.
//
//  Hermetic: nothing signs, no page opens, and no assertion waits on the
//  network (the integrity lines may still be "checking" — that is fine here).
//
//      xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim>' \
//        -only-testing:VelaWalletUITests/SigningVenueChooserDeviceTests
//
//  Launched OUTSIDE the parallel space (`VELA_PARALLEL_SPACE=0`): these want
//  the Welcome screen, and leaving the space removes its fixture wallet.
//

import XCTest

final class SigningVenueChooserDeviceTests: XCTestCase {

    private enum Words {
        static let signIn = "我已有钱包"
        static let places = ["这台设备", "手机或平板", "USB 安全密钥"]
        static let ownPage = "使用我自己的签名页"
        static let fourth = "可信签名器"
        static let inVela = "在 Vela 里"
        static let official = "官方"
    }

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// 我已有钱包 → three places, no fourth; "使用我自己的签名页" apart from
    /// them, opening Vela's own sheet and the official page with its domain.
    func testTheSignInChooserListsThreePlacesAndOwnPageApart() throws {
        let app = launch()
        XCTAssertTrue(app.buttons[Words.signIn].waitForExistence(timeout: 30),
                      "Welcome did not appear — is a session left over from another class?")
        app.buttons[Words.signIn].tap()

        for place in Words.places {
            XCTAssertTrue(app.staticTexts[place].waitForExistence(timeout: 10), "no \(place)")
        }
        XCTAssertFalse(app.staticTexts[Words.fourth].exists, "the fourth key method came back")
        let entry = app.descendants(matching: .any)["chooser.ownPage"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 10), "no \"\(Words.ownPage)\" entry")
        attach(app, "signin-chooser")

        entry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts[Words.inVela].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts[Words.official].exists, "the official page is not listed")
        XCTAssertTrue(app.staticTexts["钥匙在 getvela.app"].exists, "the official page's domain is not said")
        attach(app, "signin-picker")
        app.terminate()
    }

    /// The create flow's key list: three places, and — before the first key —
    /// the same entry, apart.
    func testTheCreateChooserListsThreePlacesAndOwnPageApart() throws {
        let app = launch()
        XCTAssertTrue(app.buttons["创建钱包"].waitForExistence(timeout: 30))
        app.buttons["创建钱包"].tap()
        XCTAssertTrue(app.staticTexts["给钱包起个名字"].waitForExistence(timeout: 20))
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText("签名页")
        let acks = app.buttons.matching(
            NSPredicate(format: "label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@",
                        "公钥和钱包名字", "私钥保存在", "我已阅读并同意")
        )
        XCTAssertEqual(acks.count, 3, "the name screen no longer has three acknowledgements")
        for index in 0..<acks.count { acks.element(boundBy: index).tap() }
        app.buttons["继续"].tap()

        for place in Words.places {
            XCTAssertTrue(app.staticTexts[place].waitForExistence(timeout: 30), "no \(place)")
        }
        XCTAssertFalse(app.staticTexts[Words.fourth].exists, "the fourth key method came back")
        let entry = app.descendants(matching: .any)["chooser.ownPage"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 10), "no \"\(Words.ownPage)\" entry before the first key")
        attach(app, "create-chooser")
        entry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["signingPagePicker"].firstMatch.waitForExistence(timeout: 10))
        attach(app, "create-picker")
        app.terminate()
    }

    // MARK: - Helpers

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    private func attach(_ app: XCUIApplication, _ name: String) {
        Thread.sleep(forTimeInterval: 0.6)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
