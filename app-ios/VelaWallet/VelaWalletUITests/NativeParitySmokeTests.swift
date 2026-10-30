//
//  NativeParitySmokeTests.swift
//  VelaWalletUITests
//
//  The I1 smoke (native parity): the app, in the parallel space, pointed at
//  the multisig fork harness through the Debug launch inputs (F0.5) — the
//  home renders, and a send to the account's own address reaches the confirm.
//  Nothing is ever slid: the confirm is reached and photographed.
//
//  Needs the harness (scripts/onchain/multisig-fork/up.sh) and its addresses,
//  handed to the runner with xcodebuild's `TEST_RUNNER_` prefix:
//
//      FORK_PORT=9045 RELAY_PORT=9090 … scripts/onchain/multisig-fork/up.sh
//      TEST_RUNNER_VELA_HARNESS_RPC=http://127.0.0.1:9045 \
//      TEST_RUNNER_VELA_HARNESS_RELAY=http://127.0.0.1:9090 \
//      TEST_RUNNER_VELA_HARNESS_GENESIS=http://127.0.0.1:9087 \
//      TEST_RUNNER_VELA_HARNESS_PEERCONNECT=ws://127.0.0.1:9092 \
//        xcodebuild test … -only-testing:VelaWalletUITests/NativeParitySmokeTests
//
//  Skipped without them, so CI's UI leg (no harness) is not failed by it.
//  The simulator shares the host's loopback, so 127.0.0.1 is the harness.
//

import XCTest

final class NativeParitySmokeTests: XCTestCase {

    /// The golden multi-key Safe the parallel space enters (docs/PARALLEL-SPACE.md),
    /// funded on the fork by up.sh.
    private let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func harness() throws -> [String: String] {
        let env = ProcessInfo.processInfo.environment
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"] else {
            throw XCTSkip("no fork harness named (TEST_RUNNER_VELA_HARNESS_RPC / _RELAY)")
        }
        return [
            "rpc": rpc, "relay": relay,
            "genesis": env["VELA_HARNESS_GENESIS"] ?? "",
            "peerconnect": env["VELA_HARNESS_PEERCONNECT"] ?? "",
        ]
    }

    private func launch() throws -> XCUIApplication {
        let h = try harness()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        // F0.5: Gnosis read through the fork alone, the relay the harness runs.
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h["rpc"]!)","onlyMine":true}}"#
        var endpoints = #"{"bundlerServiceURL":"\#(h["relay"]!)""#
        if let genesis = h["genesis"], !genesis.isEmpty { endpoints += #","genesisServiceURL":"\#(genesis)""# }
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] = endpoints + "}"
        if let relay = h["peerconnect"], !relay.isEmpty {
            app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = relay
        }
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
    }

    func testTheHomeRendersAgainstTheFork() throws {
        let app = try launch()
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 60), "the wallet never opened")
        XCTAssertTrue(app.buttons["转账"].firstMatch.exists, "the home has no send")
        sleep(6)
        attach(app.screenshot(), named: "i1-smoke-home")
        app.terminate()
    }

    func testASendToSelfReachesTheConfirm() throws {
        let app = try launch()
        XCTAssertTrue(app.buttons["收款"].waitForExistence(timeout: 60), "the wallet never opened")
        sleep(3)
        app.buttons["转账"].firstMatch.tap()
        // The fork funded the account with xDAI on Gnosis.
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 60), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["收款人"].waitForExistence(timeout: 20), "the form never opened")

        let recipient = app.textFields["send.recipient"]
        recipient.tap()
        recipient.typeText(account)
        let amount = app.textFields["send.amount"]
        amount.tap()
        amount.typeText("0.01")
        if app.buttons["keyboard.done"].firstMatch.waitForExistence(timeout: 3) {
            app.buttons["keyboard.done"].firstMatch.tap()
        }
        attach(app.screenshot(), named: "i1-smoke-form")

        let next = app.buttons["继续"].firstMatch
        let enabled = NSPredicate(format: "isEnabled == true")
        expectation(for: enabled, evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()

        // The confirm: its own action, and a fee the harness relay quoted.
        XCTAssertTrue(app.buttons["确认并发送"].firstMatch.waitForExistence(timeout: 60),
                      "the send never reached the confirm")
        XCTAssertTrue(app.staticTexts["预估手续费"].firstMatch.exists, "the confirm has no fee line")
        XCTAssertTrue(app.staticTexts["Gnosis"].firstMatch.exists, "the confirm is not on Gnosis")
        // vis-24 / r4a-18: the person's own account is named as they named it.
        XCTAssertTrue(app.staticTexts["Parallel One"].firstMatch.exists)
        sleep(4)
        attach(app.screenshot(), named: "i1-smoke-confirm")
        app.terminate()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
