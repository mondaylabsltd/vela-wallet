//
//  MultisigHermeticUITests.swift
//  VelaWalletUITests
//
//  A 2-of-3 multisig created — and a send proposed from it — on any
//  simulator, with no fork harness: `LocalMultisigWorld`, inside this runner,
//  plays the genesis service, its proposals mailbox and Gnosis's JSON-RPC;
//  the app is pointed at it by its Debug launch inputs; the parallel space
//  signs every passkey. Everything between is the app's own: the create
//  sheet, the core's accounts and proposals machines, the send flow, the
//  draft's review, the proposal's sheet.
//
//  `MultisigLiveUITests` is the same walk against a real fork (and goes on to
//  fund, approve and execute); it skips without that harness. This one never
//  skips.
//
//  The launch inputs persist (they are the person's settings), and so does
//  the multisig: `tearDown` removes the multisig and puts both inputs back
//  to the defaults, so no suite after this one inherits a dead node.
//

import XCTest

final class MultisigHermeticUITests: XCTestCase {

    /// Parallel Two and Three (docs/PARALLEL-SPACE.md): the other members.
    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    static let parallelThree = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"

    private var world: LocalMultisigWorld?

    override func setUpWithError() throws {
        continueAfterFailure = false
        let world = try LocalMultisigWorld()
        XCTAssertTrue(world.start(), "the local world never listened")
        self.world = world
    }

    override func tearDownWithError() throws {
        defer {
            world?.stop()
            world = nil
        }
        // Back to the defaults, and the multisig gone (the world still up,
        // so the app it relaunches is not waiting on a dead node).
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] = #"{"100":null}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] = #"{"genesisServiceURL":null}"#
        app.launch()
        let home = element(app, "multisig.home")
        if home.waitForExistence(timeout: 20) {
            tap(app, "multisig.home.settings", timeout: 30)
            tap(app, "multisig.settings.remove")
            let confirm = app.alerts.buttons.element(boundBy: 0)
            if confirm.waitForExistence(timeout: 10) { confirm.tap() }
            _ = home.waitForNonExistence(timeout: 20)
        }
        app.terminate()
    }

    private func launch() throws -> XCUIApplication {
        let world = try XCTUnwrap(world)
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] = #"{"100":{"rpcURL":"\#(world.rpcURL)","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] = #"{"genesisServiceURL":"\#(world.serviceURL)"}"#
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        return app
    }

    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id].firstMatch
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
        if let kept = field.value as? String, !kept.isEmpty, kept != field.placeholderValue {
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: kept.count))
        }
        field.typeText(text)
    }

    private func tap(_ app: XCUIApplication, _ id: String, timeout: TimeInterval = 15) {
        let target = element(app, id)
        XCTAssertTrue(target.waitForExistence(timeout: timeout), "no \(id)")
        if !target.isHittable { app.swipeUp() }
        target.tap()
    }

    /// The notifications question the first multisig asks (r9-8), answered.
    private func allowNotificationsIfAsked() {
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 5) { allow.tap() }
    }

    func testCreateATwoOfThreeAndProposeASendAgainstALocalWorld() throws {
        let world = try XCTUnwrap(world)
        let app = try launch()

        // MARK: Create (the walk of MultisigLiveUITests.test1CreateATwoOfThree)
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        // A draft kept from an earlier run starts over.
        let discard = element(app, "multisig.create.discard")
        if discard.waitForExistence(timeout: 3) { discard.tap() }
        type(app, "multisig.create.name", "Hermetic Team")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
        type(app, "multisig.create.address", Self.parallelThree)
        tap(app, "multisig.create.addMember")
        shot(app, "hermetic-create-members")
        tap(app, "multisig.create.next")
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 20), "no network to pick — Gnosis's Safe contracts were not found")
        picker.tap()
        let gnosis = app.buttons["Gnosis"].firstMatch
        if gnosis.waitForExistence(timeout: 5) { gnosis.tap() }
        let rule = element(app, "multisig.create.thresholdValue")
        XCTAssertTrue(rule.waitForExistence(timeout: 10))
        XCTAssertTrue(rule.label.contains("2"), "a 2 of 3 is not where the stepper starts: \(rule.label)")
        tap(app, "multisig.create.toReview")
        let draft = element(app, "multisig.create.draftAddress")
        XCTAssertTrue(draft.waitForExistence(timeout: 20), "no address")
        let address = try XCTUnwrap(
            draft.label.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression).map { String(draft.label[$0]) },
            "no address in \(draft.label)"
        )
        shot(app, "hermetic-create-review")
        tap(app, "multisig.create.create")
        allowNotificationsIfAsked()
        XCTAssertTrue(element(app, "multisig.create.created").waitForExistence(timeout: 60), "not created")
        // Shared: the service took its record — on the index chain, for the
        // address the review showed, with the creator's passkey proof.
        let shared = element(app, "multisig.create.shared")
        XCTAssertTrue(shared.waitForExistence(timeout: 10))
        let imported = world.imports
        XCTAssertEqual(imported.count, 1, "imports: \(imported)")
        XCTAssertEqual((imported.first?["chainId"] as? NSNumber)?.intValue, 100, "not recorded on the index chain")
        XCTAssertEqual((imported.first?["safe"] as? String)?.lowercased(), address.lowercased())
        XCTAssertEqual((imported.first?["genesis"] as? [String: Any])?["name"] as? String, "Hermetic Team")
        let proof = imported.first?["ownerProof"] as? [String: Any]
        XCTAssertTrue((proof?["clientDataJSON"] as? String)?.hasPrefix(#"{"type":"webauthn.get""#) == true,
                      "no passkey proof with the record: \(String(describing: proof))")
        XCTAssertGreaterThan(world.rpcCalls("eth_getCode"), 0, "Gnosis's Safe contracts were never read from the local node")
        shot(app, "hermetic-created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 20), "the home does not show the multisig")
        allowNotificationsIfAsked()
        shot(app, "hermetic-home")

        // MARK: Propose a send (MultisigLiveUITests.proposeSend)
        app.buttons["Send"].firstMatch.tap()
        let coin = app.staticTexts["xDAI"].firstMatch
        XCTAssertTrue(coin.waitForExistence(timeout: 60), "the picker never listed xDAI")
        coin.tap()
        let recipient = app.textFields["send.recipient"].firstMatch
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "the form never opened")
        recipient.tap()
        recipient.typeText(Self.parallelThree)
        let amount = app.textFields["send.amount"].firstMatch
        amount.tap()
        amount.typeText("0.5")
        if app.buttons["keyboard.done"].exists { app.buttons["keyboard.done"].tap() }
        shot(app, "hermetic-send-form")
        let next = app.buttons["Continue"].firstMatch
        XCTAssertTrue(next.waitForExistence(timeout: 20))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()
        let propose = app.buttons["Propose and approve"].firstMatch
        XCTAssertTrue(propose.waitForExistence(timeout: 60), "no propose on the confirm")
        shot(app, "hermetic-send-confirm")
        propose.tap()
        // The review first (§3.6), then one passkey.
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 60), "no review before the passkey")
        XCTAssertTrue(world.mailbox.isEmpty, "something reached the mailbox before the review was confirmed")
        shot(app, "hermetic-draft-review")
        tap(app, "multisig.draft.propose")
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "the proposal sheet never opened")
        shot(app, "hermetic-proposed")

        // The mailbox has it: 0.5 xDAI to Three, from the multisig, at its
        // first number, approved once (this member's passkey).
        let mailbox = world.mailbox
        XCTAssertEqual(mailbox.count, 1, "mailbox: \(mailbox)")
        let proposal = try XCTUnwrap(mailbox.first)
        XCTAssertEqual((proposal["safe"] as? String)?.lowercased(), address.lowercased())
        XCTAssertEqual(proposal["chainId"] as? Int, 100)
        let tx = try XCTUnwrap(proposal["safeTx"] as? [String: Any])
        XCTAssertEqual((tx["to"] as? String)?.lowercased(), Self.parallelThree.lowercased())
        XCTAssertEqual(SafeTxHash.word(tx["value"]), SafeTxHash.word("500000000000000000"), "not 0.5 xDAI: \(tx["value"] ?? "")")
        XCTAssertEqual(SafeTxHash.word(tx["nonce"]), SafeTxHash.word(0), "not at its first number")
        XCTAssertEqual((proposal["approvals"] as? [Any])?.count, 1, "not approved once by its proposer")

        // Read back from the mailbox: on its sheet, and waiting on the home.
        let status = element(app, "multisig.proposal.status")
        XCTAssertTrue(status.waitForExistence(timeout: 30), "the proposal's sheet says nothing of where it stands")
        tap(app, "multisig.sheet.close")
        XCTAssertTrue(element(app, "multisig.home.proposal").waitForExistence(timeout: 60), "the home does not list it as waiting")
        XCTAssertTrue(world.requests.contains { $0.hasPrefix("GET /api/proposals/") }, "the mailbox was never read back")
        shot(app, "hermetic-home-waiting")
    }
}
