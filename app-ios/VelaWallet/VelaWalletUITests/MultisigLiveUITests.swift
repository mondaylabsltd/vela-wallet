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

    private func launch(lang: String = "en", link: String? = nil, url: String? = nil) throws -> XCUIApplication {
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
        if let url { app.launchEnvironment["VELA_URL"] = url }
        app.launch()
        if url == nil {
            XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        }
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
        // A kept draft (M1) may hold an earlier run's text: start from empty.
        if let kept = field.value as? String, !kept.isEmpty, kept != field.placeholderValue {
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: kept.count))
        }
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
        // `VELA_MS_THIRD_MEMBER`: an outside wallet as the third member, so
        // test5 runs on a fresh device without the member change first.
        type(app, "multisig.create.address", env["VELA_MS_THIRD_MEMBER"] ?? Self.parallelThree)
        tap(app, "multisig.create.addMember")
        shot(app, "create-members")
        tap(app, "multisig.create.next")
        // The network: Gnosis (the fork).
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 10))
        picker.tap()
        let gnosis = app.buttons["Gnosis"].firstMatch
        if gnosis.waitForExistence(timeout: 5) { gnosis.tap() }
        // The machine's stepper (core-ms-2 `step_threshold`): up to 3 of 3,
        // no further, then back to the 2 of 3 this test creates.
        let rule = app.descendants(matching: .any)["multisig.create.thresholdValue"].firstMatch
        XCTAssertTrue(rule.waitForExistence(timeout: 10))
        let start = rule.label
        tap(app, "multisig.create.more")
        XCTAssertTrue(rule.label.contains("3") && rule.label != start, "the stepper did not step up: \(rule.label)")
        let top = rule.label
        tap(app, "multisig.create.more")
        XCTAssertEqual(rule.label, top, "the stepper went past the members")
        tap(app, "multisig.create.fewer")
        XCTAssertEqual(rule.label, start)
        shot(app, "create-threshold")
        tap(app, "multisig.create.toReview")
        let draft = app.descendants(matching: .any)["multisig.create.draftAddress"].firstMatch
        XCTAssertTrue(draft.waitForExistence(timeout: 20), "no address")
        // For the driver that funds it (`VELA_MS_ADDRESS_FILE`).
        if let file = env["VELA_MS_ADDRESS_FILE"] { try draft.label.write(toFile: file, atomically: true, encoding: .utf8) }
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
        proposeSend(app, amount: "1")
    }

    // MARK: - M4: an outside owner approves with its own wallet (PeerConnect)

    func test5OutsideWalletApproves() throws {
        let linkFile = env["VELA_MS_LINK_FILE"] ?? "/private/tmp/ios-ms-outside-link.txt"
        try? FileManager.default.removeItem(atPath: linkFile)
        let app = try launch()
        proposeSend(app, amount: "0.5")
        tap(app, "multisig.proposal.outside", timeout: 30)
        let link = app.staticTexts["multisig.outside.link"].firstMatch
        XCTAssertTrue(link.waitForExistence(timeout: 60), "no connect link")
        sleep(2)
        // A wallet paired on an earlier run is asked in its old room (no
        // invite in the link); the scripted wallet is a fresh browser, so
        // pair anew ("another wallet").
        if !link.label.contains("ws%3A") {
            tap(app, "multisig.outside.unpair")
            let fresh = NSPredicate(format: "label CONTAINS 'ws%3A'")
            expectation(for: fresh, evaluatedWith: link)
            waitForExpectations(timeout: 60)
        }
        shot(app, "outside-sheet")
        try link.label.write(toFile: linkFile, atomically: true, encoding: .utf8)
        // The scripted wallet joins the room, connects and signs; the core
        // verifies its signature before the proposals machine counts it.
        // The session sheet goes with its outcome (as on the web): its
        // "signed by" note, or the proposal it counted toward, whichever shows.
        let approved = app.descendants(matching: .any)["multisig.outside.approved"].firstMatch
        let counted = app.buttons["multisig.proposal.execute"].firstMatch
        let either = NSPredicate { _, _ in approved.exists || counted.exists }
        expectation(for: either, evaluatedWith: NSNull())
        waitForExpectations(timeout: 240)
        XCTAssertTrue(approved.exists || counted.exists, "the outside wallet never approved")
        shot(app, "outside-approved")
        // The session sheet goes with its outcome; the proposal's is under it.
        if app.descendants(matching: .any)["multisig.outside.link"].exists { tap(app, "multisig.sheet.close") }
        let execute = app.buttons["multisig.proposal.execute"].firstMatch
        XCTAssertTrue(execute.waitForExistence(timeout: 90), "not ready after the outside approval")
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: execute)
        waitForExpectations(timeout: 60)
        if !execute.isHittable { app.swipeUp() }
        shot(app, "outside-ready")
        execute.tap()
        XCTAssertTrue(app.descendants(matching: .any)["multisig.proposal.executed"].waitForExistence(timeout: 180), "never executed")
        sleep(2)
        shot(app, "outside-executed")
    }

    // MARK: - M1: the invite link adds it (removed first, added back by its link)

    func test8InviteLinkAddsIt() throws {
        var app = try launch()
        tap(app, "multisig.home.settings", timeout: 60)
        tap(app, "multisig.settings.invite")
        let linkText = app.staticTexts["multisig.invite.link"].firstMatch
        XCTAssertTrue(linkText.waitForExistence(timeout: 15))
        let link = linkText.label
        XCTAssertTrue(link.contains("#msa="), "not an invite link: \(link)")
        shot(app, "invite")
        tap(app, "multisig.sheet.close")
        tap(app, "multisig.home.settings", timeout: 30)
        tap(app, "multisig.settings.remove")
        let confirm = app.alerts.buttons.element(boundBy: 0)
        XCTAssertTrue(confirm.waitForExistence(timeout: 10))
        confirm.tap()
        XCTAssertTrue(app.descendants(matching: .any)["multisig.home"].waitForNonExistence(timeout: 20), "still shown after remove")
        app.terminate()
        // The invite opens the app: the native flow, not the web wallet.
        app = try launch(link: link)
        XCTAssertTrue(app.descendants(matching: .any)["multisig.add.invite"].waitForExistence(timeout: 60), "the invite did not open the add sheet")
        sleep(2)
        shot(app, "invite-add")
        tap(app, "multisig.add.add")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.add.invite"].waitForNonExistence(timeout: 90), "not added")
        tap(app, "wallet.header.name", timeout: 30)
        XCTAssertTrue(app.descendants(matching: .any)["multisig.switcher.row"].waitForExistence(timeout: 20), "not in the switcher")
        shot(app, "invite-added")
    }

    // MARK: - Screens beside the web's (VELA_MS_LANG / VELA_MS_THEME)

    func test7Screens() throws {
        let lang = env["VELA_MS_LANG"] ?? "en"
        let theme = env["VELA_MS_THEME"] ?? "light"
        let tag = "ios-\(lang)-\(theme)"
        let app = try launch(lang: lang)
        XCTAssertTrue(app.descendants(matching: .any)["multisig.home"].waitForExistence(timeout: 60))
        sleep(5)
        shot(app, "\(tag)-home")
        tap(app, "wallet.header.name")
        sleep(2)
        app.swipeUp()
        shot(app, "\(tag)-switcher")
        app.swipeDown(velocity: .fast)
        app.swipeDown(velocity: .fast)
        sleep(1)
        let row = app.descendants(matching: .any)["multisig.home.proposal"].firstMatch
        if row.waitForExistence(timeout: 30) {
            row.tap()
            XCTAssertTrue(app.descendants(matching: .any)["multisig.proposal.title"].waitForExistence(timeout: 30))
            sleep(4)
            shot(app, "\(tag)-proposal")
            app.swipeUp()
            sleep(1)
            shot(app, "\(tag)-proposal-2")
            tap(app, "multisig.sheet.close")
            sleep(1)
        }
        tap(app, "multisig.home.settings")
        sleep(4)
        shot(app, "\(tag)-settings")
        tap(app, "multisig.sheet.close")
        sleep(1)
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        sleep(2)
        shot(app, "\(tag)-create")
    }

    // MARK: - M5: a dApp asks the multisig for a transaction

    func test6DappRequestBecomesAProposal() throws {
        let server = try LocalDappServer(html: try LocalDappServer.page(), port: 8139)
        server.start()
        defer { server.stop() }
        let app = try launch(url: server.pageUrl)
        if !app.buttons["explore.bar.back"].waitForExistence(timeout: 20) {
            let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
            if row.waitForExistence(timeout: 10) { row.tap() }
        }
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "Connect", "Approve")).element(boundBy: 0)
        if approve.waitForExistence(timeout: 12), approve.isHittable { approve.tap() }
        XCTAssertTrue(waitForVerdict(app, "#verdict eth_requestAccounts ok", timeout: 30), "never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, "#verdict wallet_switchEthereumChain ok", timeout: 30))
        app.webViews.buttons["Send dust"].firstMatch.tap()
        // Held by the proposals machine: its review first.
        XCTAssertTrue(app.descendants(matching: .any)["multisig.draft.site"].waitForExistence(timeout: 60), "no held request")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.draft.propose"].waitForExistence(timeout: 60))
        sleep(3)
        shot(app, "dapp-review")
        tap(app, "multisig.draft.propose")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.proposal.site"].waitForExistence(timeout: 90), "not proposed")
        sleep(3)
        shot(app, "dapp-proposed")
        // Closed before it is executed: the site is told it was proposed.
        tap(app, "multisig.sheet.close")
        XCTAssertTrue(waitForVerdict(app, "#verdict eth_sendTransaction", timeout: 60), "the site was never answered")
        shot(app, "dapp-answered")
    }

    private func waitForVerdict(_ app: XCUIApplication, _ fragment: String, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "#verdict")).allElementsBoundByIndex
            if let hit = lines.first(where: { $0.label.contains(fragment) }) {
                print("VERDICT: \(hit.label)")
                return true
            }
            sleep(1)
        }
        return false
    }

    /// From the home: Send → xDAI → a recipient and an amount → Propose and
    /// approve → the review → one passkey → the transaction's sheet.
    private func proposeSend(_ app: XCUIApplication, amount value: String) {
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
        amount.typeText(value)
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

    // MARK: - M3: a member change (an outside wallet joins: 2 of 3 → 2 of 4)

    /// anvil's dev account #1: an outside owner whose wallet signs through the fork.
    static let outsideOwner = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

    func test4ProposeAMemberChange() throws {
        let app = try launch()
        tap(app, "multisig.home.settings", timeout: 60)
        sleep(3)
        shot(app, "settings")
        tap(app, "multisig.settings.members")
        tap(app, "multisig.members.add")
        let entry = app.textFields["multisig.members.entry"].firstMatch
        XCTAssertTrue(entry.waitForExistence(timeout: 15))
        entry.tap()
        entry.typeText((env["VELA_MS_NEW_MEMBER"] ?? Self.outsideOwner) + "\n")
        let rule = app.descendants(matching: .any)["multisig.members.plan.100"].firstMatch
        XCTAssertTrue(rule.waitForExistence(timeout: 30), "no plan for Gnosis")
        sleep(1)
        shot(app, "members-plan")
        tap(app, "multisig.members.propose")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.draft.propose"].waitForExistence(timeout: 60), "no review before the passkey")
        sleep(3)
        shot(app, "members-review")
        XCTAssertTrue(app.descendants(matching: .any)["multisig.review.rule"].exists, "the rule the change leaves is not shown")
        tap(app, "multisig.draft.propose")
        let proposed = app.descendants(matching: .any)["multisig.members.proposing.100"].firstMatch
        XCTAssertTrue(proposed.waitForExistence(timeout: 90))
        sleep(3)
        shot(app, "members-proposed")
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

    // MARK: - core-ms-2: one press approves and executes

    /// An outside owner proposed and approved (the driver, through the
    /// mailbox); this device's approval reaches the threshold, so the
    /// machine offers Approve and execute — one press, one passkey prompt.
    func test9ApproveAndExecute() throws {
        guard let link = env["VELA_MS_LINK"] else { throw XCTSkip("VELA_MS_LINK names the proposal") }
        let app = try launch(link: link)
        let button = app.buttons["multisig.proposal.approveExecute"].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 60), "no Approve and execute")
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: button)
        waitForExpectations(timeout: 60)
        sleep(2)
        shot(app, "approve-execute")
        if !button.isHittable { app.swipeUp() }
        button.tap()
        let executed = app.descendants(matching: .any)["multisig.proposal.executed"].firstMatch
        XCTAssertTrue(executed.waitForExistence(timeout: 180), "never executed")
        sleep(2)
        shot(app, "approve-executed")
    }
}
