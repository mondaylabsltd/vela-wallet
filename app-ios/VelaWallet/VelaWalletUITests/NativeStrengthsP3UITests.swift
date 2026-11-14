//
//  NativeStrengthsP3UITests.swift
//  VelaWalletUITests
//
//  Native parity Phase 3 on a simulator: the doors from outside the app and
//  the links it claims, opened the way iOS opens them.
//
//  - The Home Screen's quick actions: the app icon, pressed and held on the
//    SpringBoard, and its Receive / Scan / Send.
//  - A widget's button: `velawallet://scan`, opened by the system.
//  - A universal link: an unsigned simulator build has no associated
//    domains, so iOS never hands it one; `VELA_OPEN_LINK` (Debug only) puts
//    the same URL through the same handler. With the AASA hosted and a
//    signed build, `xcrun simctl openurl booted <link>` does the same.
//
//  In the parallel space, so a wallet is open; the language is
//  `TEST_RUNNER_VELA_P3_LANG` (`en` or `zh`). Screenshots are attached.
//

import XCTest

final class NativeStrengthsP3UITests: XCTestCase {

    private var lang: String { ProcessInfo.processInfo.environment["VELA_P3_LANG"] ?? "en" }
    private func w(_ en: String, _ zh: String) -> String { lang == "zh" ? zh : en }

    private let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private let txHash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipUnless(DeviceSafety.onSimulator, "drives the SpringBoard and the parallel space: simulator only")
    }

    private func launch(link: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        if let link { app.launchEnvironment["VELA_OPEN_LINK"] = link }
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
        app.launch()
        // The home's header, by its identifier: the name in it is the device's
        // ("Parallel One") until the registry answers, then the record's ("Interleave").
        let name = app.descendants(matching: .any)["wallet.header.name"].firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 90), "the wallet never opened")
        return app
    }

    // MARK: - Universal links

    /// The sheet a multisig link opened: the multisig host's, titled by the
    /// link (native multisig M1/M2 — a proposal, an invite or a damaged link
    /// goes to the accounts machine's one link flow; only the connect page
    /// keeps the link sheet with "Open in the web wallet").
    private func linkSheet(_ app: XCUIApplication, titled title: String, timeout: TimeInterval = 30) -> XCUIElement {
        let bar = app.navigationBars[title]
        XCTAssertTrue(bar.waitForExistence(timeout: timeout), "the link's sheet never opened")
        return bar
    }

    /// A proposal link: the transaction's sheet opens and says where the
    /// transaction stands. Here the mailbox has none at that hash, so it says
    /// so in the core's words. Nothing is approved here.
    func testAProposalLinkShowsWhatItNames() throws {
        let app = launch(link: "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(txHash)")
        _ = linkSheet(app, titled: w("Multisig transaction", "多签交易"))
        let stand = app.staticTexts["multisig.link.failed"]
        XCTAssertTrue(stand.waitForExistence(timeout: 30), "the sheet does not say where the transaction stands")
        XCTAssertEqual(stand.label, w("This transaction hasn't been shared with the members yet.", "信箱里还没有这笔交易。"))
        XCTAssertTrue(app.buttons["multisig.sheet.close"].exists)
        attach(app.screenshot(), named: "link-proposal-\(lang)")
        app.terminate()
    }

    /// At the largest accessibility text size (AX5) the sheet's line WRAPS
    /// inside the screen — whole, never cut.
    func testAProposalLinkAtTheLargestTextSize() throws {
        let app = XCUIApplication()
        app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_OPEN_LINK"] = "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(txHash)"
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
        app.launch()
        _ = linkSheet(app, titled: w("Multisig transaction", "多签交易"), timeout: 90)
        let stand = app.staticTexts["multisig.link.failed"]
        XCTAssertTrue(stand.waitForExistence(timeout: 30), "the sheet does not say where the transaction stands")
        let screen = app.windows.firstMatch.frame
        XCTAssertLessThanOrEqual(stand.frame.maxX, screen.maxX + 1, "the line runs off the screen")
        XCTAssertGreaterThan(stand.frame.height, stand.frame.width / 8, "the line is on one cut line")
        attach(app.screenshot(), named: "link-proposal-ax5-\(lang)")
        XCTAssertTrue(app.buttons["multisig.sheet.close"].isHittable, "the sheet cannot be closed")
        app.terminate()
    }

    /// An invite: the add sheet, its address filled in from the link, and the
    /// members as the invite names them.
    func testAnInviteNamesItsMembers() throws {
        let link = "https://wallet.getvela.app/en/wallet#msa=100:\(safe)&a=88cca0eedbf2c4426110bbfc998f048689266894pAlice"
        let app = launch(link: link)
        _ = linkSheet(app, titled: w("Add a multisig", "添加多签账户"))
        let member = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", "Alice")).firstMatch
        XCTAssertTrue(member.waitForExistence(timeout: 10), "the invite's member is not named")
        let address = app.textFields["multisig.add.address"]
        XCTAssertTrue(address.exists, "no address field")
        XCTAssertTrue((address.value as? String)?.lowercased().hasPrefix(String(safe.prefix(12)).lowercased()) ?? false,
                      "the invite's multisig is not filled in: \(String(describing: address.value))")
        attach(app.screenshot(), named: "link-invite-\(lang)")
        app.terminate()
    }

    /// A damaged link is said, in the web's words — never dropped.
    func testADamagedLinkIsSaid() throws {
        let app = launch(link: "https://wallet.getvela.app/en/wallet#ms=100.\(safe)")
        _ = linkSheet(app, titled: w("Multisig transaction", "多签交易"))
        let body = app.staticTexts["multisig.link.broken"]
        XCTAssertTrue(body.waitForExistence(timeout: 30), "a damaged link was dropped in silence")
        XCTAssertTrue(body.label.contains(w("incomplete or damaged", "不完整")), "not the web's words: \(body.label)")
        attach(app.screenshot(), named: "link-damaged-\(lang)")
        app.terminate()
    }

    // MARK: - A widget's button

    /// `velawallet://scan`, as a widget's button opens it: the scanner.
    func testAWidgetsScanButtonOpensTheScanner() throws {
        let app = launch()
        XCUIDevice.shared.system.open(URL(string: "velawallet://scan")!)
        let scanner = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS %@", w("camera", "相机"))
        ).firstMatch
        XCTAssertTrue(scanner.waitForExistence(timeout: 20) || app.otherElements["scan.fixtureFrame"].exists,
                      "the scanner did not open")
        attach(app.screenshot(), named: "door-scan-\(lang)")
        app.terminate()
    }

    // MARK: - Quick actions

    /// The icon, held on the Home Screen: Receive opens Receive.
    func testAQuickActionOpensItsDoor() throws {
        let app = launch()
        settle(2)
        XCUIDevice.shared.press(.home)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let icon = springboard.icons["Vela Wallet"]
        XCTAssertTrue(icon.waitForExistence(timeout: 20), "no app icon on the Home Screen")
        icon.press(forDuration: 1.6)
        let action = springboard.buttons[w("Receive", "收款")]
        XCTAssertTrue(action.waitForExistence(timeout: 10), "the quick action is not offered")
        attach(springboard.screenshot(), named: "quick-actions-\(lang)")
        action.tap()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20), "the app did not come forward")
        let receive = app.staticTexts[w("Receive", "收款")].firstMatch
        XCTAssertTrue(receive.waitForExistence(timeout: 20), "Receive did not open")
        let line = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS %@", w("One address", "同一地址"))
        ).firstMatch
        XCTAssertTrue(line.waitForExistence(timeout: 10), "the receive list is not up")
        attach(app.screenshot(), named: "quick-action-receive-\(lang)")
        app.terminate()
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
