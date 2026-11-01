//
//  R10FixLiveUITests.swift
//  VelaWalletUITests
//
//  Round 10's iOS fixes and the first real-device pass, live against the
//  multisig fork harness (scripts/onchain/multisig-fork/up.sh) with a host
//  driver playing the other members — it answers the request files this
//  test writes in `VELA_R10_STATE` (fund, deploy, propose a contract call as
//  the outside owner, drain or fund this device's member).
//
//      TEST_RUNNER_VELA_HARNESS_RPC / _RELAY / _GENESIS / _PEERCONNECT / _UNREAD
//      TEST_RUNNER_VELA_R10_STATE=/tmp/harness-r10ios
//      TEST_RUNNER_VELA_R10_OUTSIDE=0x…   the driver's outside owner
//      TEST_RUNNER_VELA_R10_SHOTS=…       where the screenshots go
//      TEST_RUNNER_VELA_R10_DEAD_RPC=…    the dead-RPC proxy in front of the fork (test5)
//
//  Skipped without them. In order, on one install: create a 2-of-3 (r10
//  QR), a contract call proposed by the outside owner titled as the core
//  reads it (r10-8) with why only "Approve" while this member cannot pay
//  (S4), then approved and executed in one press — "Executed" once (S4);
//  a dApp connecting to the multisig (r10-9); the balance read again as
//  soon as a dead network answers (r10-1). `test9` needs a FRESH install
//  (no account): a phone's sign-in no phone answers is said (S3).
//

import CoreImage
import XCTest

final class R10FixLiveUITests: XCTestCase {

    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func required() throws -> (rpc: String, relay: String, genesis: String, state: String, outside: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"], let state = env["VELA_R10_STATE"],
              let outside = env["VELA_R10_OUTSIDE"]
        else { throw XCTSkip("no fork harness / r10 driver named") }
        return (rpc, relay, genesis, state, outside)
    }

    private var safeFile: String { (env["VELA_R10_STATE"] ?? "/tmp") + "/safe-addr.txt" }
    private var safe: String {
        ((try? String(contentsOfFile: safeFile, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func launch(extra: [String: String] = [:], rpc: String? = nil, url: String? = nil) throws -> XCUIApplication {
        let h = try required()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(rpc ?? h.rpc)","onlyMine":true},"8453":{"rpcURL":"\#(env["VELA_HARNESS_UNREAD"] ?? "http://127.0.0.1:11846")","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if let peer = env["VELA_HARNESS_PEERCONNECT"] { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = peer }
        if let url { app.launchEnvironment["VELA_URL"] = url }
        for (k, v) in extra { app.launchEnvironment[k] = v }
        app.launch()
        if url == nil {
            XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        }
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

    @discardableResult
    private func shot(_ app: XCUIApplication, _ name: String) -> XCUIScreenshot {
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = env["VELA_R10_SHOTS"] {
            try? screenshot.pngRepresentation.write(to: URL(fileURLWithPath: "\(dir)/\(name).png"))
        }
        return screenshot
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

    /// The QR codes in a screenshot, read by Core Image's standard decoder.
    private func codes(in screenshot: XCUIScreenshot) -> [String] {
        guard let image = screenshot.image.cgImage else { return [] }
        let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil,
                                  options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])
        return (detector?.features(in: CIImage(cgImage: image)) ?? []).compactMap { ($0 as? CIQRCodeFeature)?.messageString }
    }

    // MARK: - Create, and the invite's code read off a screenshot (S3)

    func test1CreateAndTheInviteCodeDecodesFromAScreenshot() throws {
        let h = try required()
        let app = try launch()
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        type(app, "multisig.create.name", env["VELA_R10_NAME"] ?? "R10 iOS")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
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
        let address = draft.label.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression).map { String(draft.label[$0]) }
        XCTAssertNotNil(address, "no address in \(draft.label)")
        try (address ?? "").write(toFile: safeFile, atomically: true, encoding: .utf8)
        tap(app, "multisig.create.create")
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 30) { allow.tap() }
        XCTAssertTrue(element(app, "multisig.create.created").waitForExistence(timeout: 60), "not created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 20), "the home does not show the multisig")
    }

    /// S3: the invite's code, drawn on whole pixels, decodes from a plain
    /// screenshot with a standard decoder; an unnamed member is its address once.
    func test1bTheInviteCodeDecodesFromAScreenshot() throws {
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        tap(app, "multisig.home.settings")
        sleep(2)
        XCTAssertFalse(text(app, containing: "772b, 0x031d").exists, "an unnamed member's address said twice")
        shot(app, "r10-settings-members")
        tap(app, "multisig.settings.invite")
        let opened = element(app, "multisig.invite.qr").waitForExistence(timeout: 20)
            || element(app, "multisig.invite.link").waitForExistence(timeout: 5)
        if !opened { shot(app, "r10-qr-invite-not-open") }
        XCTAssertTrue(opened, "no invite code")
        sleep(2)
        let read = codes(in: shot(app, "r10-qr-invite"))
        print("R10-QR: \(read)")
        XCTAssertFalse(read.isEmpty, "no code a decoder can read on the invite")
        let link = element(app, "multisig.invite.link")
        if link.exists { XCTAssertTrue(read.contains { link.label.contains($0) || $0.contains(link.label) }, "\(read) vs \(link.label)") }
    }

    // MARK: - r10-8, S4: a contract call's title; why only Approve; Executed once

    func test2AContractCallIsTitledAsTheCoreReadsItAndExecutedOnce() throws {
        try driver("fund")
        try driver("deploy", timeout: 180)
        try driver("drain")
        try driver("proposeApprove")
        let app = try launch()
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the multisig is not shown")
        // r10-8: the home's row and the sheet's title are the core's reading.
        let row = text(app, containing: "Approve · USDC")
        XCTAssertTrue(row.waitForExistence(timeout: 90), "the row is not titled Approve · USDC")
        shot(app, "r10-8-home-row")
        row.tap()
        let title = element(app, "multisig.proposal.title")
        XCTAssertTrue(title.waitForExistence(timeout: 30), "the sheet never opened")
        XCTAssertEqual(title.label, "Approve · USDC")
        // S4: theirs completes it, but this member holds nothing for the fee:
        // said, before the press.
        let why = element(app, "multisig.proposal.approveOnly")
        XCTAssertTrue(why.waitForExistence(timeout: 60), "only Approve, and nothing says why")
        XCTAssertTrue(why.label.contains("network fee") || why.label.contains("can’t pay") || why.label.contains("can't pay"), why.label)
        XCTAssertFalse(element(app, "multisig.proposal.approveExecute").exists)
        reveal(app, "multisig.proposal.approve", swipes: 3)
        shot(app, "r10-s4-why-only-approve")
        // Funded: the one press is offered — and once it ran, "Executed" once.
        try driver("fundMember")
        let one = element(app, "multisig.proposal.approveExecute")
        let deadline = Date().addingTimeInterval(90)
        while !one.exists && Date() < deadline { sleep(2) }
        XCTAssertTrue(one.exists, "the one press never came once the member could pay")
        XCTAssertFalse(why.exists, "the reason stayed after the member could pay")
        shot(app, "r10-s4-one-press")
        reveal(app, "multisig.proposal.approveExecute", swipes: 3)
        one.tap()
        let executed = element(app, "multisig.proposal.executed")
        XCTAssertTrue(executed.waitForExistence(timeout: 120), "never executed")
        app.swipeDown()
        sleep(3)
        XCTAssertFalse(element(app, "multisig.proposal.note.executed").exists, "\"Executed\" said twice")
        shot(app, "r10-s4-executed-once")
        // r10-8: its Activity row says what it was, too.
        tap(app, "multisig.sheet.close")
        let activity = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Approve · USDC")).firstMatch
        let seen = activity.waitForExistence(timeout: 30)
        shot(app, "r10-8-activity-row")
        XCTAssertTrue(seen, "the executed call's Activity row is not titled as the core reads it")
    }

    // MARK: - r10-9: a dApp connecting to the multisig

    func test3TheConnectSheetNamesTheMultisigAndSaysWhatIsTrue() throws {
        let page = """
        <!doctype html><html><head><meta name="viewport" content="width=device-width"></head><body>
        <button id="c" onclick="ethereum.request({method:'eth_requestAccounts'}).then(a=>document.getElementById('v').textContent='#verdict ok '+a)">Connect</button>
        <p id="v">#verdict none</p></body></html>
        """
        let server = try LocalDappServer(html: page, port: 8141)
        server.start()
        defer { server.stop() }
        let app = try launch(url: server.pageUrl)
        if !app.buttons["explore.bar.back"].waitForExistence(timeout: 20) {
            let resume = app.buttons.matching(identifier: "explore.resume.row").firstMatch
            if resume.waitForExistence(timeout: 10) { resume.tap() }
        }
        let connect = app.webViews.buttons["Connect"].firstMatch
        XCTAssertTrue(connect.waitForExistence(timeout: 30))
        connect.tap()
        XCTAssertTrue(element(app, "explore.connection.title").waitForExistence(timeout: 20), "no connect sheet")
        sleep(2)
        let name = env["VELA_R10_NAME"] ?? "R10 iOS"
        XCTAssertTrue(text(app, containing: name).exists, "the multisig's name is not on the sheet")
        XCTAssertTrue(text(app, containing: "This is a multisig").exists, "the sheet does not say what a multisig can do")
        XCTAssertFalse(text(app, containing: "ask you to sign").exists, "a multisig told it can be asked to sign")
        shot(app, "r10-9-connect-multisig")
    }

    // MARK: - r10-1: the balance read again as soon as the network answers

    func test5TheStampIsReadAgainSoonAfterTheNetworkComesBack() throws {
        let h = try required()
        guard let dead = env["VELA_R10_DEAD_RPC"] else { throw XCTSkip("no dead-RPC proxy named (VELA_R10_DEAD_RPC)") }
        let flag = "\(h.state)/dead-100"
        try? FileManager.default.removeItem(atPath: flag)
        defer { try? FileManager.default.removeItem(atPath: flag) }
        let app = try launch(rpc: dead)
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        let fresh = app.buttons.matching(NSPredicate(format: "label == %@", "Updated now")).firstMatch
        XCTAssertTrue(fresh.waitForExistence(timeout: 60), "never read")
        shot(app, "r10-1-0-before")
        try "on".write(toFile: flag, atomically: true, encoding: .utf8)
        // Long enough for the core's recovery ladder to be minutes out.
        let outage = TimeInterval(env["VELA_R10_OUTAGE_S"].flatMap(Double.init) ?? 240)
        sleep(UInt32(outage))
        shot(app, "r10-1-1-dead")
        let stale = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Updated")).firstMatch
        XCTAssertTrue(stale.exists)
        XCTAssertNotEqual(stale.label, "Updated now", "the stamp said now over a dead network")
        try? FileManager.default.removeItem(atPath: flag)
        let back = Date()
        XCTAssertTrue(fresh.waitForExistence(timeout: 60), "the stamp was not refreshed within a minute of the network coming back")
        print("R10-1: read again \(Int(Date().timeIntervalSince(back))) s after the network came back")
        shot(app, "r10-1-2-back")
    }

    // MARK: - S3: the receive code decodes off a screenshot (no harness)

    /// The parallel space's own account: Receive, a screenshot, a standard
    /// decoder. Runs on a simulator or on a phone (the device-test build).
    func test8TheReceiveCodeDecodesFromAScreenshot() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        let receive = app.buttons["Receive"].firstMatch
        XCTAssertTrue(receive.waitForExistence(timeout: 20))
        receive.tap()
        // The networks first: a network's code.
        let showQr = app.buttons.matching(NSPredicate(format: "label == %@", "Show QR code")).firstMatch
        if showQr.waitForExistence(timeout: 15) { showQr.tap() }
        sleep(3)
        var read: [String] = []
        for attempt in 0..<4 {
            read = codes(in: shot(app, "r10-qr-receive-\(attempt)"))
            if !read.isEmpty { break }
            sleep(2)
        }
        print("R10-QR-RECEIVE: \(read)")
        XCTAssertTrue(read.contains { $0.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression) != nil },
                      "no code a decoder can read on Receive: \(read)")
    }

    // MARK: - S3: a phone's sign-in that no phone answers is said

    /// Needs a FRESH install (Welcome). No phone is near a simulator: the
    /// code goes after the scan, and the reason must take its place —
    /// never a sheet that closes saying nothing.
    func test9APhoneSignInNoPhoneAnswersIsSaid() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        let have = app.buttons["I already have a wallet"].firstMatch
        guard have.waitForExistence(timeout: 30) else { throw XCTSkip("not on Welcome: a fresh install is needed") }
        have.tap()
        let phone = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Phone or tablet")).firstMatch
        XCTAssertTrue(phone.waitForExistence(timeout: 15), "no Phone or tablet")
        phone.tap()
        // A phone asks for Bluetooth once: allowed, so the scan really runs.
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for label in ["Allow", "允许"] {
            let allow = springboard.buttons[label]
            if allow.waitForExistence(timeout: 4) { allow.tap(); break }
        }
        XCTAssertTrue(element(app, "cable.cancel").waitForExistence(timeout: 30), "no code")
        sleep(2)
        shot(app, "r10-s3-hybrid-code")
        // The scan's 90 s, then the code goes by itself.
        let gone = NSPredicate(format: "exists == false")
        expectation(for: gone, evaluatedWith: element(app, "cable.cancel"))
        waitForExpectations(timeout: 150)
        sleep(2)
        shot(app, "r10-s3-hybrid-after")
        // The core's reason, on the sheet the code was on: "Sign In Failed —
        // Your other device didn't connect, or the link dropped. …"
        let said = text(app, containing: "didn't connect")
        XCTAssertTrue(said.waitForExistence(timeout: 10), "the sheet closed and nothing was said")
        XCTAssertTrue(app.buttons["Close"].exists, "the reason has no way out")
    }
}
