//
//  R8FixLiveUITests.swift
//  VelaWalletUITests
//
//  Round 8's iOS fixes, live against the multisig fork harness
//  (scripts/onchain/multisig-fork/up.sh) with a host driver playing the
//  other members (the r8-fix-ios driver: it funds the multisig, executes as
//  an outside owner, proposes as one) — it answers the request files this
//  test writes in `VELA_R8_STATE`.
//
//      TEST_RUNNER_VELA_HARNESS_RPC=http://127.0.0.1:10945 … (as MultisigLiveUITests)
//      TEST_RUNNER_VELA_R8_STATE=/tmp/harness-r8ios
//      TEST_RUNNER_VELA_R8_SAFE=0x…            the 2-of-3 MultisigLiveUITests/test1 created,
//                                              its third member the driver's outside owner
//
//  Skipped without them. In order: the confirm and the proposal's outside
//  path (r8-9, r8-8), the outside owner executing it (r8-17, r8-19), a new
//  proposal from that owner noticed in the background (the notifications'
//  Debug trigger).
//

import XCTest

final class R8FixLiveUITests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func required() throws -> (rpc: String, relay: String, genesis: String, state: String, safe: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"], let state = env["VELA_R8_STATE"], let safe = env["VELA_R8_SAFE"]
        else { throw XCTSkip("no fork harness / r8 driver named") }
        return (rpc, relay, genesis, state, safe)
    }

    private func launch(extra: [String: String] = [:]) throws -> XCUIApplication {
        let h = try required()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true},"8453":{"rpcURL":"\#(env["VELA_HARNESS_UNREAD"] ?? "http://127.0.0.1:10946")","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
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
        try h.safe.write(toFile: "\(h.state)/req-\(verb)", atomically: true, encoding: .utf8)
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
        if let dir = env["VELA_R8_SHOTS"] {
            try? app.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: "\(dir)/\(name).png"))
        }
    }

    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id].firstMatch
    }

    private func tap(_ app: XCUIApplication, _ id: String, timeout: TimeInterval = 15) {
        let e = element(app, id)
        XCTAssertTrue(e.waitForExistence(timeout: timeout), "no \(id)")
        if !e.isHittable { app.swipeUp() }
        e.tap()
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

    private func text(_ app: XCUIApplication, containing fragment: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", fragment)).firstMatch
    }

    // MARK: - r8-9 and r8-8: the confirm, and the outside owner's way in

    func test1ConfirmNamesTheMultisigAndTheSheetAsksTheOutsideOwner() throws {
        try driver("fund")
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        sleep(4)
        app.buttons["Send"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["xDAI"].waitForExistence(timeout: 60), "the picker never listed xDAI")
        app.staticTexts["xDAI"].firstMatch.tap()
        let recipient = app.textFields["send.recipient"].firstMatch
        XCTAssertTrue(recipient.waitForExistence(timeout: 20))
        recipient.tap()
        recipient.typeText("0x000000000000000000000000000000000000bEEF")
        let amount = app.textFields["send.amount"].firstMatch
        amount.tap()
        amount.typeText("0.1")
        if app.buttons["keyboard.done"].exists { app.buttons["keyboard.done"].tap() }
        let next = app.buttons["Continue"].firstMatch
        XCTAssertTrue(next.waitForExistence(timeout: 20))
        expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: next)
        waitForExpectations(timeout: 60)
        next.tap()
        let propose = app.buttons["Propose and approve"].firstMatch
        XCTAssertTrue(propose.waitForExistence(timeout: 60), "no propose on the confirm")
        // r8-9: the multisig is the source, its rule where the fee would be.
        XCTAssertTrue(text(app, containing: "iOS Team").waitForExistence(timeout: 5), "the confirm does not name the multisig")
        XCTAssertFalse(text(app, containing: "Parallel One").exists, "the confirm names the member, not the multisig")
        XCTAssertTrue(text(app, containing: "Needs 2 of 3").exists, "no rule on the confirm")
        sleep(1)
        shot(app, "r8-9-confirm")
        propose.tap()
        XCTAssertTrue(element(app, "multisig.draft.propose").waitForExistence(timeout: 60), "no review before the passkey")
        sleep(3)
        shot(app, "r8-1-draft-review")
        tap(app, "multisig.draft.propose")
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 90), "the proposal sheet never opened")
        // The app asks for notifications once there is something to say.
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 5) { allow.tap() }
        sleep(6)
        shot(app, "r8-8-proposal-top")
        // r8-8: the outside owner's own link (and its code), the Vela link
        // for the Vela member, and "Approve with another wallet".
        let found = reveal(app, "multisig.proposal.outsideLink")
        if !found { shot(app, "r8-8-proposal-scrolled") }
        XCTAssertTrue(found, "no outside-wallet link")
        let link = element(app, "multisig.proposal.outsideLink").label
        XCTAssertTrue(link.contains("/connect"), "not the connect page: \(link)")
        XCTAssertTrue(element(app, "multisig.proposal.outsideQr").exists, "no code for the outside link")
        shot(app, "r8-8-proposal-outside")
        XCTAssertTrue(reveal(app, "multisig.proposal.outside"), "no Approve with another wallet")
        XCTAssertTrue(element(app, "multisig.proposal.invite").exists || element(app, "multisig.proposal.share").exists,
                      "no Vela link for the Vela member")
        shot(app, "r8-8-proposal-bottom")
        // r8-19: the review is in — no endless "Checking…".
        XCTAssertFalse(element(app, "multisig.review.checking").exists, "the review never came")
    }

    // MARK: - r8-17 and r8-19: the outside owner approves by executing

    func test2AnOutsideOwnerExecutingIsShownAsItRan() throws {
        try driver("execute", timeout: 180)
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        // The execution found on chain is written to Activity; its row opens
        // the multisig's transaction (never "Delete record").
        // Activity's row, not a waiting proposal's on the home.
        let row = app.buttons.matching(NSPredicate(
            format: "label CONTAINS %@ AND identifier != %@", "0.1", "multisig.home.proposal"
        )).firstMatch
        let deadline = Date().addingTimeInterval(120)
        while !row.exists && Date() < deadline {
            app.swipeDown()
            sleep(3)
        }
        XCTAssertTrue(row.exists, "no executed row in Activity")
        shot(app, "r8-17-activity")
        if !row.isHittable { app.swipeUp() }
        row.tap()
        XCTAssertTrue(element(app, "multisig.proposal.title").waitForExistence(timeout: 30), "the row did not open the multisig transaction")
        XCTAssertFalse(app.buttons["txDetail.delete"].exists, "a plain record's detail opened")
        let executed = element(app, "multisig.proposal.executedFacts")
        XCTAssertTrue(executed.waitForExistence(timeout: 60), "how it ran is not said")
        sleep(3)
        XCTAssertTrue(executed.label.contains("Executed by"), "who executed it: \(executed.label)")
        XCTAssertTrue(executed.label.contains("2 of 2"), "whose approvals counted: \(executed.label)")
        // The sheet's own chip and member rows (the home under it lists the
        // other proposals, still waiting).
        let status = element(app, "multisig.proposal.status")
        XCTAssertEqual(status.label, "Executed", "re-counted against today's rule: \(status.label)")
        let members = app.descendants(matching: .any).matching(identifier: "multisig.proposal.member").allElementsBoundByIndex
        XCTAssertFalse(members.isEmpty)
        for member in members {
            XCTAssertFalse(member.label.contains("Waiting"), "an owner that executed is still Waiting: \(member.label)")
        }
        XCTAssertTrue(members.contains { $0.label.contains("Executed") }, "the executing owner is not said to have executed")
        XCTAssertFalse(element(app, "multisig.review.checking").exists, "r8-19: a spinner on an executed sheet")
        shot(app, "r8-17-executed-sheet")
        app.swipeUp()
        sleep(1)
        shot(app, "r8-17-executed-sheet-2")
    }

    // MARK: - r8-8: an outside owner opens a member's Vela link

    /// A device with no Vela account (the host uninstalled the app first):
    /// the transaction, read only, with the outside owner's way to sign —
    /// never the plain Welcome.
    func test4AnOutsideOwnerOpeningAVelaLinkSeesTheTransaction() throws {
        let h = try required()
        guard let link = try? String(contentsOfFile: "\(h.state)/p2-link.txt", encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines), !link.isEmpty
        else { throw XCTSkip("no proposal link written by the driver") }
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] = #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] = #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if let peer = env["VELA_HARNESS_PEERCONNECT"] { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = peer }
        app.launchEnvironment["VELA_OPEN_LINK"] = link
        app.launch()
        let title = element(app, "multisig.proposal.title")
        let opened = title.waitForExistence(timeout: 90)
        sleep(4)
        shot(app, opened ? "r8-8-link-no-account" : "r8-8-link-no-account-failed")
        XCTAssertTrue(opened, "the link landed on Welcome")
        XCTAssertTrue(reveal(app, "multisig.proposal.outside"), "no way for the outside owner to sign")
        shot(app, "r8-8-link-no-account-outside")
    }

    // MARK: - Notifications: a new proposal from another member, while away

    func test3ANewProposalWhileAwayIsNotified() throws {
        let state = try required().state
        let app = try launch(extra: [
            "VELA_DEV_MULTISIG_POLL": "1", "VELA_DEV_MULTISIG_POLL_TRACE": "\(state)/poll-trace.log",
        ])
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        // The app asks once, after a multisig exists; answer it if it is up.
        let allow = springboard.buttons["Allow"]
        if allow.waitForExistence(timeout: 5) { allow.tap() }
        sleep(8) // the on-screen views record what is waiting now
        XCUIDevice.shared.press(.home)
        sleep(1)
        try driver("propose")
        // The refresh's own round, on the Debug trigger (iOS never runs a
        // BGAppRefreshTask on a simulator by itself).
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName("app.getvela.VelaWallet.dev.multisigPoll" as CFString), nil, nil, true
        )
        let banner = springboard.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS[c] %@", "your approval")).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 40), "no notification for the new proposal")
        let attachment = XCTAttachment(screenshot: springboard.screenshot())
        attachment.name = "notification-banner"
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = env["VELA_R8_SHOTS"] {
            try? springboard.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: "\(dir)/notification-banner.png"))
        }
        // Tapping it opens the transaction's sheet.
        banner.tap()
        let opened = element(app, "multisig.proposal.title").waitForExistence(timeout: 30)
        if !opened { shot(app, "notification-tap-failed") }
        XCTAssertTrue(opened, "the notification did not open the transaction")
        sleep(3)
        shot(app, "notification-opened")
    }
}
