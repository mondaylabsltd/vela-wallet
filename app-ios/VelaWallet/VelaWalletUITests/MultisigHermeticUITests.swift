//
//  MultisigHermeticUITests.swift
//  VelaWalletUITests
//
//  A 2-of-3 multisig created — and a send proposed from it — on any
//  simulator, with no fork harness and no public network:
//  `LocalMultisigWorld`, inside this runner, plays the genesis service and
//  its proposals mailbox in front of a local anvil (Safe's real code; see its
//  header); the app is pointed at it by its Debug launch inputs; the parallel
//  space signs every passkey. Everything between is the app's own: the
//  create sheet, the core's accounts and proposals machines, the send flow,
//  the draft's review, the proposal's sheet.
//
//  The review is checked on that chain: the trace of Safe's execTransaction
//  and the simulation run and answer, so the proposal goes with a plain
//  press. And on a node that offers neither, the review says so and proposes
//  only when held (`testAReviewThatCannotBeChecked…`).
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
        // The chain is required, never skipped: without it this suite would
        // read green while checking nothing.
        let chain = try XCTUnwrap(
            LocalMultisigWorld.chainURL,
            "no chain: start one with app-ios/scripts/ui-anvil.sh and run with TEST_RUNNER_VELA_UI_ANVIL_URL (CI's ios job does)"
        )
        let world = try LocalMultisigWorld(chain: chain)
        XCTAssertTrue(world.start(), "the local world never listened, or its chain (\(chain)) was not prepared")
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
        // Return, as a person ends a field: the field's own submit only puts
        // the keyboard away (`MsField`). r16-9's action row rides above the
        // keyboard, so while it is up the next field or "Add" can sit under
        // "Next" — a tap there would press Next instead.
        field.typeText(text + "\n")
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

    /// Create a 2-of-3 with Parallel Two and Three on Gnosis, from the
    /// create sheet, recorded by the local service; the home shows it.
    /// Answers its address. (The walk of MultisigLiveUITests.test1CreateATwoOfThree.)
    private func createTwoOfThree(_ app: XCUIApplication, _ world: LocalMultisigWorld) throws -> String {
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
        return address
    }

    /// A send of 0.5 xDAI to Parallel Three from the multisig, filled and
    /// continued to its confirm: answers the confirm's "Propose and approve".
    private func toSendConfirm(_ app: XCUIApplication) -> XCUIElement {
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
        return propose
    }

    /// The draft's review, read on the chain: the state-diff guard traced
    /// Safe's real execTransaction and found nothing about who controls the
    /// multisig changing, the simulation says what leaves it (−0.5 xDAI),
    /// nothing is said unread, and the propose is a plain press. Both checks
    /// were ANSWERED by the chain — not merely asked.
    private func assertReviewChecked(_ app: XCUIApplication, _ world: LocalMultisigWorld) {
        XCTAssertTrue(element(app, "multisig.review.state.ok").waitForExistence(timeout: 60),
                      "the state-diff guard did not read the multisig as unchanged")
        let delta = element(app, "multisig.review.delta")
        XCTAssertTrue(delta.waitForExistence(timeout: 30), "no balance change from the simulation")
        XCTAssertTrue(delta.label.contains("0.5"), "not the 0.5 xDAI that leaves: \(delta.label)")
        XCTAssertFalse(element(app, "multisig.review.state.unread").exists, "the state check said unread")
        XCTAssertFalse(element(app, "multisig.review.unchecked").exists, "the review said unchecked")
        XCTAssertEqual(element(app, "multisig.draft.propose").label, "Propose and approve", "not a plain propose")
        XCTAssertGreaterThan(world.rpcAnswered("debug_traceCall"), 0, "the chain never answered the trace")
        XCTAssertGreaterThan(world.rpcAnswered("eth_simulateV1"), 0, "the chain never answered the simulation")
    }

    func testCreateATwoOfThreeAndProposeASendAgainstALocalWorld() throws {
        let world = try XCTUnwrap(world)
        let app = try launch()
        let address = try createTwoOfThree(app, world)

        // MARK: Propose a send (MultisigLiveUITests.proposeSend)
        let propose = toSendConfirm(app)
        propose.tap()
        // The review first (§3.6), then one passkey.
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 60), "no review before the passkey")
        // §12: shut until its review is read (the core's `review.door`).
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: element(app, "multisig.draft.propose"))
        waitForExpectations(timeout: 60)
        assertReviewChecked(app, world)
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

    /// A proposal's review swiped down is withdrawn at once, as "Don't
    /// propose": the review goes, the send's confirm is back with "Propose
    /// and approve" in seconds (before, the propose waited out its 600 s
    /// limit), nothing reached the mailbox, nothing is said — and proposed
    /// again, it goes.
    func testAReviewSwipedDownIsWithdrawnAtOnceAndTheSendCanBeProposedAgain() throws {
        let world = try XCTUnwrap(world)
        let app = try launch()
        _ = try createTwoOfThree(app, world)

        toSendConfirm(app).tap()
        let title = element(app, "multisig.draft.title")
        XCTAssertTrue(title.waitForExistence(timeout: 60), "no review before the passkey")
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 30))
        shot(app, "swipe-review-up")

        // Swiped down, from its title to the bottom of the screen.
        title.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98)))
        XCTAssertTrue(title.waitForNonExistence(timeout: 10), "the review did not go")
        let again = app.buttons["Propose and approve"].firstMatch
        XCTAssertTrue(again.waitForExistence(timeout: 10), "the send's confirm did not come back at once")
        expectation(for: NSPredicate(format: "isEnabled == true AND isHittable == true"), evaluatedWith: again)
        waitForExpectations(timeout: 10)
        XCTAssertFalse(element(app, "multisig.draft.failed").exists, "a withdrawal is not a failure")
        XCTAssertTrue(world.mailbox.isEmpty, "a swiped review sent something: \(world.mailbox)")
        shot(app, "swipe-review-withdrawn")

        // Proposed again, on its review this time: it goes.
        again.tap()
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 60), "no review the second time")
        // §12: shut until its review is read (the core's `review.door`).
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: element(app, "multisig.draft.propose"))
        waitForExpectations(timeout: 60)
        assertReviewChecked(app, world)
        XCTAssertTrue(world.mailbox.isEmpty)
        tap(app, "multisig.draft.propose")
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "the proposal sheet never opened")
        XCTAssertEqual(world.mailbox.count, 1, "mailbox: \(world.mailbox)")
        shot(app, "swipe-review-proposed")
    }

    /// A node that offers none of the review's checks (no trace, no
    /// simulation, no state overrides — as many public ones): the review says
    /// both are unread, in the danger style, and its propose is "… anyway",
    /// held. A tap proposes nothing; held for the core's hold (1.5 s) it asks
    /// once more, naming why; confirmed there, it goes.
    func testAReviewThatCannotBeCheckedSaysSoAndProposesOnlyWhenHeld() throws {
        let world = try XCTUnwrap(world)
        world.checks = .notOffered
        let app = try launch()
        let address = try createTwoOfThree(app, world)

        toSendConfirm(app).tap()
        let propose = element(app, "multisig.draft.propose")
        XCTAssertTrue(propose.waitForExistence(timeout: 60), "no review before the passkey")
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: propose)
        waitForExpectations(timeout: 60)

        // Said: neither check was read, and nothing claims they were.
        XCTAssertTrue(element(app, "multisig.review.state.unread").waitForExistence(timeout: 30),
                      "the review does not say the state check was unread")
        XCTAssertFalse(element(app, "multisig.review.state.ok").exists, "a check that never ran is said as passed")
        XCTAssertFalse(element(app, "multisig.review.delta").exists, "a balance change nobody simulated is shown")
        XCTAssertEqual(propose.label, "Propose and approve anyway", "the unread review's propose is not \"… anyway\"")
        // The app asked; the node refused; nothing was answered.
        XCTAssertGreaterThan(world.rpcCalls("debug_traceCall"), 0, "the trace was never asked")
        XCTAssertGreaterThan(world.rpcCalls("eth_simulateV1"), 0, "the simulation was never asked")
        XCTAssertEqual(world.rpcAnswered("debug_traceCall"), 0)
        XCTAssertEqual(world.rpcAnswered("eth_simulateV1"), 0)
        shot(app, "unchecked-review")

        // A tap is no confirm.
        propose.tap()
        XCTAssertFalse(element(app, "multisig.proposal.title").waitForExistence(timeout: 5), "a tap proposed an unread review")
        XCTAssertTrue(world.mailbox.isEmpty, "a tap sent something: \(world.mailbox)")

        // Held past the core's 1.5 s: the "… anyway" step (r8-1) names why,
        // and still nothing is sent until the person confirms it.
        propose.press(forDuration: 2.5)
        let confirm = element(app, "multisig.anyway.confirm")
        XCTAssertTrue(confirm.waitForExistence(timeout: 10), "held, no \"… anyway\" step")
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "couldn't check")).firstMatch.exists,
                      "the step does not say what was not checked")
        XCTAssertTrue(world.mailbox.isEmpty, "the hold alone sent something: \(world.mailbox)")
        shot(app, "unchecked-anyway-step")
        confirm.tap()
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "confirmed, the proposal sheet never opened")
        XCTAssertEqual(world.mailbox.count, 1, "mailbox: \(world.mailbox)")
        XCTAssertEqual((world.mailbox.first?["safe"] as? String)?.lowercased(), address.lowercased())
        shot(app, "unchecked-proposed")
    }
}
