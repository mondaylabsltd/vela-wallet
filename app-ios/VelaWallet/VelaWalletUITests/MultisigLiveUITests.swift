//
//  MultisigLiveUITests.swift
//  VelaWalletUITests
//
//  Native multisig (M1–M6), live against the multisig fork harness
//  (scripts/onchain/multisig-fork/up.sh): a Gnosis fork, the genesis service
//  with its mailbox, the relay, a PeerConnect relay. The parallel space signs
//  (the golden multi-key Safe), so every passkey is scripted; everything else
//  is real — the core's machines, the chain, the service.
//
//      TEST_RUNNER_VELA_HARNESS_RPC=http://127.0.0.1:10345 \
//      TEST_RUNNER_VELA_HARNESS_RELAY=http://127.0.0.1:10390 \
//      TEST_RUNNER_VELA_HARNESS_GENESIS=http://127.0.0.1:10387 \
//      TEST_RUNNER_VELA_HARNESS_PEERCONNECT=ws://127.0.0.1:10392 \
//      xcodebuild test … -only-testing:VelaWalletUITests/MultisigLiveUITests
//
//  Skipped without a harness, so a CI leg without one is not failed by it.
//  The tests run in order (`test1…`, `test2…`): each continues where the last
//  left the device — the multisig created, then funded, proposed, approved,
//  executed. A second member approves on the web wallet (the driver script
//  waits for it: `TEST_RUNNER_VELA_MS_SECOND_MEMBER=1`).
//

import XCTest

final class MultisigLiveUITests: XCTestCase {

    /// Parallel Two and Three (docs/PARALLEL-SPACE.md): the other members.
    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    static let parallelThree = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func harness() throws -> (rpc: String, relay: String, genesis: String, peer: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"]
        else { throw XCTSkip("no fork harness named (TEST_RUNNER_VELA_HARNESS_RPC / _RELAY / _GENESIS)") }
        return (rpc, relay, genesis, env["VELA_HARNESS_PEERCONNECT"] ?? "")
    }

    private func launch(lang: String = "en", link: String? = nil) throws -> XCUIApplication {
        let h = try harness()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = env["VELA_MS_THEME"] ?? "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true},"8453":{"rpcURL":"\#(env["VELA_HARNESS_UNREAD"] ?? "http://127.0.0.1:10346")","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if !h.peer.isEmpty { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = h.peer }
        if let link { app.launchEnvironment["VELA_OPEN_LINK"] = link }
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        return app
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func type(_ app: XCUIApplication, _ id: String, _ text: String) {
        let field = app.textFields[id].firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10), "no field \(id)")
        field.tap()
        field.typeText(text)
    }

    private func tap(_ app: XCUIApplication, _ id: String, timeout: TimeInterval = 15) {
        let element = app.descendants(matching: .any)[id].firstMatch
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "no \(id)")
        if !element.isHittable { app.swipeUp() }
        element.tap()
    }

    // MARK: - M1: create a 2-of-3

    func test1CreateATwoOfThree() throws {
        let app = try launch()
        tap(app, "wallet.header.name")
        shot(app, "switcher")
        tap(app, "multisig.switcher.create")
        type(app, "multisig.create.name", "iOS Team")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
        type(app, "multisig.create.address", Self.parallelThree)
        tap(app, "multisig.create.addMember")
        shot(app, "create-members")
        tap(app, "multisig.create.next")
        // The network: Gnosis (the fork).
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 10))
        picker.tap()
        let gnosis = app.buttons["Gnosis"].firstMatch
        if gnosis.waitForExistence(timeout: 5) { gnosis.tap() }
        shot(app, "create-threshold")
        tap(app, "multisig.create.toReview")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.create.draftAddress"].waitForExistence(timeout: 20), "no address")
        shot(app, "create-review")
        tap(app, "multisig.create.create")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.create.created"].waitForExistence(timeout: 60), "not created")
        let shared = app.descendants(matching: .any)["multisig.create.shared"].firstMatch
        XCTAssertTrue(shared.waitForExistence(timeout: 5))
        shot(app, "create-created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.home"].waitForExistence(timeout: 20), "the home does not show the multisig")
        shot(app, "home-multisig")
    }
}
