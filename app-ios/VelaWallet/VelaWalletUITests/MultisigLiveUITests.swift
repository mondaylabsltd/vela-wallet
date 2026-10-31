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

    // MARK: - M2: propose a send from the multisig (funded by the driver)

    func test2ProposeASend() throws {
        let app = try launch()
        XCTAssertTrue(app.descendants(matching: .any)["multisig.home"].waitForExistence(timeout: 30), "the multisig is not shown")
        sleep(4)
        app.buttons["Send"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 60), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        let recipient = app.textFields["send.recipient"].firstMatch
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "the form never opened")
        recipient.tap()
        recipient.typeText(Self.parallelThree)
        let amount = app.textFields["send.amount"].firstMatch
        amount.tap()
        amount.typeText("1")
        if app.buttons["keyboard.done"].exists { app.buttons["keyboard.done"].tap() }
        shot(app, "send-form")
        let next = app.buttons["Continue"].firstMatch
        XCTAssertTrue(next.waitForExistence(timeout: 20))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()
        let propose = app.buttons["Propose and approve"].firstMatch
        XCTAssertTrue(propose.waitForExistence(timeout: 60), "no propose on the confirm")
        shot(app, "send-confirm")
        propose.tap()
        // The review first (§3.6), then one passkey.
        XCTAssertTrue(app.descendants(matching: .any)["multisig.draft.propose"].waitForExistence(timeout: 60), "no review before the passkey")
        sleep(3)
        shot(app, "draft-review")
        tap(app, "multisig.draft.propose")
        // Proposed: the transaction's own sheet takes over.
        XCTAssertTrue(app.descendants(matching: .any)["multisig.proposal.title"].waitForExistence(timeout: 90), "the proposal sheet never opened")
        sleep(4)
        shot(app, "proposal-proposed")
        let status = app.descendants(matching: .any)["multisig.proposal.status"].firstMatch
        XCTAssertTrue(status.waitForExistence(timeout: 30))
    }

    // MARK: - M2: execute (the second member approved on the web)

    func test3ExecuteWhenApproved() throws {
        let app = try launch()
        let row = app.descendants(matching: .any)["multisig.home.proposal"].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 60), "nothing waiting on the home")
        shot(app, "home-ready")
        row.tap()
        let execute = app.buttons["multisig.proposal.execute"].firstMatch
        XCTAssertTrue(execute.waitForExistence(timeout: 60), "no Execute — not ready?")
        // The machine's look: the fee and that it would run.
        let fee = app.descendants(matching: .any)["multisig.proposal.fee"].firstMatch
        XCTAssertTrue(fee.waitForExistence(timeout: 30))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: execute)
        waitForExpectations(timeout: 60)
        sleep(2)
        shot(app, "proposal-ready")
        if !execute.isHittable { app.swipeUp() }
        execute.tap()
        let executed = app.descendants(matching: .any)["multisig.proposal.executed"].firstMatch
        XCTAssertTrue(executed.waitForExistence(timeout: 180), "never executed")
        sleep(2)
        shot(app, "proposal-executed")
    }
}
