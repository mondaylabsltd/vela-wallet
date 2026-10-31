//
//  R9FixLiveUITests.swift
//  VelaWalletUITests
//
//  Round 9's iOS fixes, live against the multisig fork harness
//  (scripts/onchain/multisig-fork/up.sh) with a host driver playing the
//  other members — it answers the request files this test writes in
//  `VELA_R9_STATE` (fund, deploy, receive, propose as the outside owner).
//
//      TEST_RUNNER_VELA_HARNESS_RPC=http://127.0.0.1:11545 … (as MultisigLiveUITests)
//      TEST_RUNNER_VELA_R9_STATE=/tmp/harness-r9ios
//      TEST_RUNNER_VELA_R9_OUTSIDE=0x…   the driver's outside owner (a plain account,
//                                          its kind never said: the r9 "Eve")
//      TEST_RUNNER_VELA_R9_SHOTS=…       where the screenshots go
//
//  Skipped without them. In order, on one fresh install: create (the app
//  asks for notifications once, then), propose from the multisig's send
//  (r9-20 form, r9-7 every member asked at once, history times), a coin
//  received while the multisig is shown (r9-17), a new proposal from the
//  outside owner while the app is away (r9-8).
//

import XCTest

final class R9FixLiveUITests: XCTestCase {

    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func required() throws -> (rpc: String, relay: String, genesis: String, state: String, outside: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"], let state = env["VELA_R9_STATE"],
              let outside = env["VELA_R9_OUTSIDE"]
        else { throw XCTSkip("no fork harness / r9 driver named") }
        return (rpc, relay, genesis, state, outside)
    }

    private var safeFile: String { (env["VELA_R9_STATE"] ?? "/tmp") + "/safe-addr.txt" }
    private var safe: String {
        ((try? String(contentsOfFile: safeFile, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func launch(extra: [String: String] = [:], flaky: String? = nil, genesis: String? = nil) throws -> XCUIApplication {
        let h = try required()
        let service = genesis ?? h.genesis
        // `flaky`: BNB Chain's reads through the host's flaky proxy — one of
        // the networks the member split reads a signer's configuration on.
        let more = flaky.map { #","56":{"rpcURL":"\#($0)","onlyMine":true}"# } ?? ""
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true},"8453":{"rpcURL":"\#(env["VELA_HARNESS_UNREAD"] ?? "http://127.0.0.1:11546")","onlyMine":true}\#(more)}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(service)"}"#
        if let peer = env["VELA_HARNESS_PEERCONNECT"] { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = peer }
        for (k, v) in extra { app.launchEnvironment[k] = v }
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        return app
    }

    /// Ask the host driver for `verb` on the multisig, and wait for it.
    private func driver(_ verb: String, timeout: TimeInterval = 120) throws {
        let h = try required()
        let done = "\(h.state)/done-\(verb)"
        try? FileManager.default.removeItem(atPath: done)
        try safe.write(toFile: "\(h.state)/req-\(verb)", atomically: true, encoding: .utf8)
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let answer = try? String(contentsOfFile: done, encoding: .utf8) {
                XCTAssertEqual(answer, "ok", "the driver's \(verb): \(answer)")
                return
            }
            Thread.sleep(forTimeInterval: 0.5)
        }
        XCTFail("the driver never answered \(verb)")
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = env["VELA_R9_SHOTS"] {
            try? app.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: "\(dir)/\(name).png"))
        }
    }

    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id].firstMatch
    }

    private func text(_ app: XCUIApplication, containing fragment: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", fragment)).firstMatch
    }

    private func tap(_ app: XCUIApplication, _ id: String, timeout: TimeInterval = 15) {
        let e = element(app, id)
        XCTAssertTrue(e.waitForExistence(timeout: timeout), "no \(id)")
        if !e.isHittable { app.swipeUp() }
        e.tap()
    }

    private func type(_ app: XCUIApplication, _ id: String, _ value: String) {
        let field = app.textFields[id].firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10), "no field \(id)")
        field.tap()
        if let kept = field.value as? String, !kept.isEmpty, kept != field.placeholderValue {
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: kept.count))
        }
        field.typeText(value)
    }

    /// Scroll the sheet until `id` shows (or give up after a few swipes).
    @discardableResult
    private func reveal(_ app: XCUIApplication, _ id: String, swipes: Int = 6) -> Bool {
        let e = element(app, id)
        for _ in 0..<swipes {
            if e.exists && e.isHittable { return true }
            app.swipeUp()
            sleep(1)
        }
        return e.exists
    }

    /// From the multisig's home: Send → xDAI → a recipient and an amount →
    /// the confirm → the review → one passkey (scripted in the parallel space).
    private func proposeSend(_ app: XCUIApplication, amount value: String, form: Bool = false) {
        app.buttons["Send"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 60), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        let recipient = app.textFields["send.recipient"].firstMatch
        XCTAssertTrue(recipient.waitForExistence(timeout: 20))
        recipient.tap()
        recipient.typeText("0x000000000000000000000000000000000000bEEF")
        let amount = app.textFields["send.amount"].firstMatch
        amount.tap()
        amount.typeText(value)
        if app.buttons["keyboard.done"].exists { app.buttons["keyboard.done"].tap() }
        sleep(3)
        if form {
            // r9-20: no fee and no speed for the proposer — who must approve, and who pays.
            XCTAssertTrue(text(app, containing: "Needs 2 of 3").waitForExistence(timeout: 20), "the form does not say who must approve")
            XCTAssertTrue(text(app, containing: "paid by the executing member").exists, "the form does not say who pays")
            XCTAssertFalse(app.staticTexts["Speed"].exists, "a speed on a multisig's proposal")
            XCTAssertFalse(app.staticTexts["Network fee"].exists, "a network fee row on a multisig's proposal")
            shot(app, "r9-20-form")
        }
        let next = app.buttons["Continue"].firstMatch
        XCTAssertTrue(next.waitForExistence(timeout: 20))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()
        let propose = app.buttons["Propose and approve"].firstMatch
        XCTAssertTrue(propose.waitForExistence(timeout: 60), "no propose on the confirm")
        propose.tap()
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 60), "no review before the passkey")
        sleep(2)
        tap(app, "multisig.draft.propose")
    }

    // MARK: - r9-8: the first multisig made here asks for notifications

    func test1CreatingTheFirstMultisigAsksForNotifications() throws {
        let h = try required()
        let app = try launch()
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        type(app, "multisig.create.name", "R9 iOS")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
        // The outside owner, its kind never said (the r9 "Eve").
        type(app, "multisig.create.address", h.outside)
        tap(app, "multisig.create.addMember")
        tap(app, "multisig.create.next")
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 10))
        picker.tap()
        let gnosis = app.buttons["Gnosis"].firstMatch
        if gnosis.waitForExistence(timeout: 5) { gnosis.tap() }
        tap(app, "multisig.create.toReview")
        let draft = element(app, "multisig.create.draftAddress")
        XCTAssertTrue(draft.waitForExistence(timeout: 20), "no address")
        // The row reads "Address, 0x…": the address alone.
        let address = draft.label.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression).map { String(draft.label[$0]) }
        XCTAssertNotNil(address, "no address in \(draft.label)")
        try (address ?? "").write(toFile: safeFile, atomically: true, encoding: .utf8)
        tap(app, "multisig.create.create")
        // The system's question, once there is a multisig to say things about.
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons["Allow"]
        XCTAssertTrue(allow.waitForExistence(timeout: 60), "the app never asked for notifications")
        sleep(1)
        shot(app, "r9-8-asked-on-create")
        allow.tap()
        XCTAssertTrue(element(app, "multisig.create.created").waitForExistence(timeout: 60), "not created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 20), "the home does not show the multisig")
    }

    // MARK: - r9-20, r9-7, history times: propose from the multisig's send

    func test2TheProposalFormAndEveryMemberAskedAtOnce() throws {
        try driver("fund")
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        sleep(4)
        proposeSend(app, amount: "0.1", form: true)
        // Right after this device's own proposal lands: its sheet.
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "the proposal sheet never opened")
        sleep(3)
        shot(app, "r9-7-proposed-top")
        // r9-7: the outside owner, its kind never said, is asked — its own
        // link and "Approve with another wallet" — without reopening anything.
        let found = reveal(app, "multisig.proposal.outsideLink", swipes: 8)
        shot(app, found ? "r9-7-proposed-outside" : "r9-7-proposed-no-outside")
        XCTAssertTrue(found, "the outside owner is not asked right after proposing")
        XCTAssertTrue(reveal(app, "multisig.proposal.outside"), "no Approve with another wallet")
        shot(app, "r9-7-proposed-outside-wallet")
        // History: each step with its time.
        XCTAssertTrue(reveal(app, "multisig.proposal.history.when"), "the history has no times")
        shot(app, "r9-s4-history-times")
    }

    // MARK: - r9-17: a coin received while the multisig is shown

    func test3AReceiptToTheMultisigIsInActivity() throws {
        try driver("deploy", timeout: 180)
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        sleep(6)
        try driver("receive")
        let row = app.buttons.matching(NSPredicate(format: "label CONTAINS %@ AND label CONTAINS %@", "Received", "0.25")).firstMatch
        let deadline = Date().addingTimeInterval(120)
        while !row.exists && Date() < deadline { sleep(3) }
        if !row.exists { shot(app, "r9-17-no-received") }
        XCTAssertTrue(row.exists, "no Received row in the multisig's Activity")
        for _ in 0..<3 where !row.isHittable { app.swipeUp(); sleep(1) }
        sleep(1)
        shot(app, "r9-17-received")
    }

    // MARK: - r9-8: a new proposal from another member, while away

    func test4ANewProposalFromAnotherMemberWhileAwayIsNotified() throws {
        let state = try required().state
        let app = try launch(extra: ["VELA_DEV_MULTISIG_POLL": "1"])
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        sleep(8) // the views on screen record what waits now
        XCUIDevice.shared.press(.home)
        sleep(1)
        try driver("propose")
        // The round runs by itself while the app is away (the Debug poll),
        // and on the trigger a tester posts.
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName("app.getvela.VelaWallet.dev.multisigPoll" as CFString), nil, nil, true
        )
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let banner = springboard.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS[c] %@", "your approval")).firstMatch
        let seen = banner.waitForExistence(timeout: 40)
        let attachment = XCTAttachment(screenshot: springboard.screenshot())
        attachment.name = "r9-8-banner"
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = env["VELA_R9_SHOTS"] {
            try? springboard.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: "\(dir)/r9-8-banner.png"))
        }
        _ = state
        XCTAssertTrue(seen, "no notification for the new proposal")
        banner.tap()
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 30), "the notification did not open the transaction")
        sleep(3)
        shot(app, "r9-8-opened")
    }

    // MARK: - r9-7: a read refused right after proposing is asked again

    /// The r9 failure, made on purpose: while this device proposes, one of
    /// the networks the member split reads a signer's configuration on
    /// answers 429 (the host's flaky proxy, `VELA_R9_FLAKY`); the outside
    /// owner stays unplaced. Once that network answers again, the sheet —
    /// left open, nothing reopened — asks the machine again and the owner is
    /// asked: its own link and "Approve with another wallet".
    func test5AReadRefusedRightAfterProposingIsAskedAgain() throws {
        let h = try required()
        guard let flaky = env["VELA_R9_FLAKY"] else { throw XCTSkip("no flaky proxy named (VELA_R9_FLAKY)") }
        let flag = "\(h.state)/fail-56"
        let app = try launch(flaky: flaky)
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        sleep(4)
        try "on".write(toFile: flag, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(atPath: flag) }
        proposeSend(app, amount: "0.05")
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "the proposal sheet never opened")
        sleep(4)
        let link = element(app, "multisig.proposal.outsideLink")
        let early = link.exists
        reveal(app, "multisig.proposal.history", swipes: 4)
        shot(app, "r9-7-refused-while-429")
        // Past the core's bound on one read (8 s): that read surely failed.
        sleep(8)
        try? FileManager.default.removeItem(atPath: flag)
        let deadline = Date().addingTimeInterval(60)
        while !link.exists && Date() < deadline { sleep(2) }
        let found = reveal(app, "multisig.proposal.outsideLink", swipes: 8)
        shot(app, found ? "r9-7-asked-again-outside" : "r9-7-never-asked-again")
        XCTAssertFalse(early, "the refused read did not leave the owner unplaced (the case is not made)")
        XCTAssertTrue(found, "the outside owner is never asked once the network answers again")
        XCTAssertTrue(reveal(app, "multisig.proposal.outside"), "no Approve with another wallet")
        shot(app, "r9-7-asked-again-wallet")
    }

    // MARK: - r9-8: the service seconds away, as on Arc Testnet

    /// The r9 failure, made on purpose: once the app is away, the multisig
    /// service answers 5 s late (the host's slow proxy, `VELA_R9_SLOW`) —
    /// a round that said what it knew 2 s in said nothing. The round now
    /// waits for its own read, and a read landing later while away is said.
    func test6ANewProposalIsNotifiedWhenTheServiceIsSlow() throws {
        let h = try required()
        guard let slow = env["VELA_R9_SLOW"] else { throw XCTSkip("no slow proxy named (VELA_R9_SLOW)") }
        let flag = "\(h.state)/slow-service"
        try? FileManager.default.removeItem(atPath: flag)
        let app = try launch(extra: ["VELA_DEV_MULTISIG_POLL": "1"], genesis: slow)
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        sleep(8)
        XCUIDevice.shared.press(.home)
        sleep(1)
        try "on".write(toFile: flag, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(atPath: flag) }
        try driver("propose")
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName("app.getvela.VelaWallet.dev.multisigPoll" as CFString), nil, nil, true
        )
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let banner = springboard.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS[c] %@", "your approval")).firstMatch
        let seen = banner.waitForExistence(timeout: 28)
        if let dir = env["VELA_R9_SHOTS"] {
            try? springboard.screenshot().pngRepresentation.write(
                to: URL(fileURLWithPath: "\(dir)/r9-8-slow-\(seen ? "banner" : "no-banner").png"))
        }
        XCTAssertTrue(seen, "no notification while the service was slow")
    }
}
