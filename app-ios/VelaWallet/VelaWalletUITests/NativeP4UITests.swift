//
//  NativeP4UITests.swift
//  VelaWalletUITests
//
//  iOS lane 4 on a simulator: the words Phase 3 owed, and a refused payment
//  link said in the web's words.
//
//  - The multisig link sheet says where the link is acted on: "Multisig
//    links open in the web wallet for now."
//  - A `/pay` link the core refuses is said — "Invalid payment link", and
//    the core's reason (`pay_refusal`) in the web's sentence — never dropped.
//
//  Links go through `VELA_OPEN_LINK` (Debug only), the same handler a
//  universal link reaches (see `NativeStrengthsP3UITests`). In the parallel
//  space, so a wallet is open; the language is `TEST_RUNNER_VELA_P3_LANG`
//  (`en` or `zh`). Screenshots are attached.
//

import XCTest

final class NativeP4UITests: XCTestCase {

    private var lang: String { ProcessInfo.processInfo.environment["VELA_P3_LANG"] ?? "en" }
    private func w(_ en: String, _ zh: String) -> String { lang == "zh" ? zh : en }

    private let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private let txHash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipUnless(DeviceSafety.onSimulator, "drives the parallel space: simulator only")
    }

    private func launch(link: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_OPEN_LINK"] = link
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
        app.launch()
        return app
    }

    /// A proposal's link opens the native flow (native multisig M2): the
    /// accounts machine's link stand — this device has none of the
    /// multisig's members, so the transaction is read only, or not found —
    /// never "opens in the web wallet".
    func testAProposalLinkOpensTheNativeFlow() throws {
        let app = launch(link: "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(txHash)")
        let native = app.descendants(matching: .any).matching(NSPredicate(
            format: "identifier IN %@",
            ["multisig.link.opening", "multisig.link.failed", "multisig.link.broken", "multisig.link.missingNetwork",
             "multisig.proposal.opening", "multisig.proposal.openFailed", "multisig.proposal.notInMailbox", "multisig.proposal.title"]
        )).firstMatch
        XCTAssertTrue(native.waitForExistence(timeout: 90), "the link's native sheet never opened")
        XCTAssertFalse(app.staticTexts["applink.onWeb"].exists, "the sheet still sends multisig to the web wallet")
        attach(app.screenshot(), named: "p4-link-native-\(lang)")
        app.terminate()
    }

    /// A payment link whose amount cannot be read exactly: the core refuses
    /// it (`pay_refusal: amount`) and the app says so, in the web's words —
    /// and opens no Send.
    func testARefusedPayLinkSaysWhy() throws {
        let app = launch(link: "https://wallet.getvela.app/pay?to=\(me)&chain=100&amount=1e3")
        let alert = app.alerts.firstMatch
        XCTAssertTrue(alert.waitForExistence(timeout: 90), "a refused payment link was dropped in silence")
        XCTAssertEqual(alert.label, w("Invalid payment link", "无效的收款链接"), "not the title the web says")
        let body = alert.staticTexts.containing(
            NSPredicate(format: "label CONTAINS %@", w("amount in this link", "金额"))
        ).firstMatch
        XCTAssertTrue(body.exists, "not the core's reason in the web's words")
        attach(app.screenshot(), named: "p4-pay-refused-amount-\(lang)")
        alert.buttons.firstMatch.tap()
        XCTAssertTrue(alert.waitForNonExistence(timeout: 10), "the notice does not go")
        // Nothing was opened for a link that is not a request.
        XCTAssertFalse(app.staticTexts[w("Send", "转账")].firstMatch.exists && app.textFields.firstMatch.exists,
                       "a refused link opened Send")
        app.terminate()
    }

    /// A token that is not an address says that — not the general sentence.
    func testARefusedPayLinksTokenIsNamed() throws {
        let app = launch(link: "velawallet://pay?to=\(me)&chain=100&token=usdc")
        let alert = app.alerts.firstMatch
        XCTAssertTrue(alert.waitForExistence(timeout: 90), "a refused payment link was dropped in silence")
        let body = alert.staticTexts.containing(
            NSPredicate(format: "label CONTAINS %@", w("isn't a contract address", "合约地址"))
        ).firstMatch
        XCTAssertTrue(body.exists, "the token's refusal is not said")
        attach(app.screenshot(), named: "p4-pay-refused-token-\(lang)")
        app.terminate()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
