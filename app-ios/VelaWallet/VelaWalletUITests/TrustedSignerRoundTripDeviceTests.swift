//
//  TrustedSignerRoundTripDeviceTests.swift
//  VelaWalletUITests
//
//  Issue #318, "the trusted signer does not work on iOS": the whole round
//  trip, on a simulator, against the REAL published page. The page opening
//  was already covered (`TrustedSignerChooserDeviceTests`); what had never
//  run on iOS is the way back — the page navigating to
//  `velawallet://sign-result?…` from inside the app's Safari tab after an
//  asynchronous passkey ceremony, and the wallet taking the answer. Android
//  proved its half on a device (229b4a5b).
//
//  Create a wallet → name → 可信签名器 → the page creates a passkey, then
//  confirms it joins the wallet → both answers come back → the key list
//  holds the new key. It stops THERE, before 创建钱包: nothing is registered
//  and nothing is signed. Network needed (the published page).
//
//  Needs an iOS 18 simulator (an iOS 26 simulator offers no local passkey
//  provider, only "Other Devices") with Face ID enrolled, and Face ID matched
//  from the host while it runs:
//
//      U=<simulator udid>
//      xcrun simctl spawn $U notifyutil -s com.apple.BiometricKit.enrollmentChanged 1
//      xcrun simctl spawn $U notifyutil -p com.apple.BiometricKit.enrollmentChanged
//      while sleep 1.5; do xcrun simctl spawn $U notifyutil -p com.apple.BiometricKit_Sim.pearl.match; done &
//      TEST_RUNNER_VELA_SIGNER_ROUND_TRIP=1 xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet -destination "id=$U" \
//        -only-testing:VelaWalletUITests/TrustedSignerRoundTripDeviceTests
//

import XCTest

final class TrustedSignerRoundTripDeviceTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipIf(ProcessInfo.processInfo.environment["VELA_SIGNER_ROUND_TRIP"] == nil,
                      "set TEST_RUNNER_VELA_SIGNER_ROUND_TRIP=1 and match Face ID from the host (see the header)")
    }

    func testAKeyMadeOnThePageComesBackToTheWallet() throws {
        let app = XCUIApplication()
        // `0` LEAVES the parallel space: this wants Welcome, not a fixture wallet.
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()

        XCTAssertTrue(app.buttons["创建钱包"].waitForExistence(timeout: 30), "Welcome did not appear")
        app.buttons["创建钱包"].tap()
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 20))
        field.tap()
        field.typeText("往返318")
        let acks = app.buttons.matching(NSPredicate(
            format: "label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@",
            "公钥和钱包名字", "私钥保存在", "我已阅读并同意"))
        for index in 0..<acks.count { acks.element(boundBy: index).tap() }
        app.buttons["继续"].tap()
        XCTAssertTrue(app.staticTexts["可信签名器"].waitForExistence(timeout: 30))
        app.staticTexts["可信签名器"].firstMatch.tap()

        // The page speaks the browser's language, not the app's.
        let sliders = app.staticTexts.matching(NSPredicate(
            format: "label BEGINSWITH %@ OR label BEGINSWITH %@", "滑动以确认", "Slide to confirm"))
        XCTAssertTrue(sliders.firstMatch.waitForExistence(timeout: 90), "the signer page did not draw its slider")
        attach("page")

        // Two ceremonies on one page: create the key, then confirm it joins.
        // Each is a slide, then the system sheet's Continue (reached by
        // position — its buttons are not queryable from here), then Face ID.
        let auth = XCUIApplication(bundleIdentifier: "com.apple.AuthenticationServicesUI")
        let origin = app.coordinate(withNormalizedOffset: .zero)
        var slid: Set<String> = []
        let keyAdded = app.staticTexts["1 / 7"]
        let deadline = Date().addingTimeInterval(150)
        var tick = 0
        while Date() < deadline, !keyAdded.exists {
            if auth.state == .runningForeground {
                if tick % 4 == 1 {
                    origin.withOffset(CGVector(dx: app.frame.width / 2, dy: app.frame.height - 84)).tap()
                }
            } else if sliders.count > 0 {
                let next = sliders.firstMatch
                if next.exists, next.isHittable, !slid.contains(next.label) {
                    slid.insert(next.label)
                    attach("slide-\(slid.count)")
                    let f = next.frame
                    origin.withOffset(CGVector(dx: f.minX - 40, dy: f.midY)).press(
                        forDuration: 0.3,
                        thenDragTo: origin.withOffset(CGVector(dx: f.maxX + 110, dy: f.midY)),
                        withVelocity: .slow, thenHoldForDuration: 0.2)
                }
            }
            tick += 1
            Thread.sleep(forTimeInterval: 1)
        }
        attach(keyAdded.exists ? "key-list" : "never-came-back")
        XCTAssertEqual(slid.count, 2, "expected two slides: create the key, confirm it joins")
        XCTAssertTrue(keyAdded.exists, "the page's answer never reached the wallet's key list")
        XCTAssertTrue(app.staticTexts["往返318"].exists, "the key list does not name the new key")
        app.terminate()
    }

    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
