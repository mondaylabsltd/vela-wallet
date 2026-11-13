//
//  R11FixLiveUITests.swift
//  VelaWalletUITests
//
//  Round 11's iOS fixes, live against the multisig fork harness
//  (scripts/onchain/multisig-fork/up.sh) with an Arc-chain-id fork beside it
//  and a host driver answering the request files written in `VELA_R11_STATE`
//  (fund the multisig, propose as the outside owner).
//
//      TEST_RUNNER_VELA_HARNESS_RPC / _RELAY / _GENESIS / _PEERCONNECT
//      TEST_RUNNER_VELA_R11_ARC_RPC=…    the fork answering as Arc Testnet (5042002)
//      TEST_RUNNER_VELA_R11_DEAD_ARC=…   a proxy in front of it, dead while
//                                        `$VELA_R11_STATE/dead-arc` exists
//      TEST_RUNNER_VELA_R11_STATE=…      the driver's request files
//      TEST_RUNNER_VELA_R11_SHOTS=…      where the screenshots go
//
//  Skipped without them. In order, on one install: a multisig made on Arc
//  Testnet, shown from the mainnet space — the testnet-space note and its
//  switch, never "0.00", "Updated now", "No activity yet" (r11-3); an outside
//  owner's send on it, its sheet headed "Balance changes" with a U+2212
//  minus (S4); Arc's RPC dead while the app is warm — the reason is said
//  (r11-7).
//

import XCTest

final class R11FixLiveUITests: XCTestCase {

    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    /// The fork's unlocked dev account #1: the outside owner the driver signs as.
    static let outside = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func required() throws -> (rpc: String, relay: String, genesis: String, arc: String, state: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"], let arc = env["VELA_R11_ARC_RPC"],
              let state = env["VELA_R11_STATE"]
        else { throw XCTSkip("no fork harness / Arc fork / r11 driver named") }
        return (rpc, relay, genesis, arc, state)
    }

    private var safeFile: String { (env["VELA_R11_STATE"] ?? "/tmp") + "/safe-addr.txt" }
    private var safe: String {
        ((try? String(contentsOfFile: safeFile, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// `arc`: the Arc Testnet RPC this launch reads (the dead proxy for r11-7).
    private func launch(space: String, arc: String? = nil) throws -> XCUIApplication {
        let h = try required()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true},"5042002":{"rpcURL":"\#(arc ?? h.arc)","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if let peer = env["VELA_HARNESS_PEERCONNECT"] { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = peer }
        app.launchArguments += ["-vela.networkSpace", space]
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        return app
    }

    /// Ask the host driver for `verb` with `args`, and wait for its answer.
    private func driver(_ verb: String, _ args: String, timeout: TimeInterval = 120) throws {
        let h = try required()
        let done = "\(h.state)/done-\(verb)"
        try? FileManager.default.removeItem(atPath: done)
        try args.write(toFile: "\(h.state)/req-\(verb)", atomically: true, encoding: .utf8)
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
        if let dir = env["VELA_R11_SHOTS"] {
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

    /// The space switched as the person does: the home's label, then the prompt.
    private func switchSpace(_ app: XCUIApplication, to space: String) {
        let label = element(app, "network-space")
        XCTAssertTrue(label.waitForExistence(timeout: 15), "no space label on the home")
        label.tap()
        let button = app.buttons["Switch to the \(space) space"].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 10), "no switch to \(space)")
        button.tap()
        sleep(3)
    }

    // MARK: - The Vela account shown (for R10FollowUpLiveUITests/test3)

    /// The parallel space's own account in front, in the mainnet space: the
    /// in-app browser's space note is the signing sheet's (a multisig's dApp
    /// request is its own sheet).
    func test9ShowTheVelaAccount() throws {
        let app = try launch(space: "mainnet")
        let header = app.buttons["wallet.header.name"]
        sleep(3)
        if !header.label.contains("Interleave") {
            header.tap()
            let mine = app.descendants(matching: .any)
                .matching(NSPredicate(format: "label BEGINSWITH %@", "Interleave")).firstMatch
            XCTAssertTrue(mine.waitForExistence(timeout: 10), "no Vela account in the switcher")
            mine.tap()
            sleep(2)
        }
        expectation(for: NSPredicate(format: "label CONTAINS %@", "Interleave"), evaluatedWith: header)
        waitForExpectations(timeout: 30)
    }

    // MARK: - r11-3: a multisig of the other space

    /// A FRESH clone, in the testnet space: a multisig made on Arc Testnet;
    /// then the mainnet space in front — its note and switch, and no false
    /// zero, stamp or empty state; the switch takes it back.
    func test1AMultisigOfTheOtherSpaceSaysWhereItLives() throws {
        let app = try launch(space: "testnet")
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        type(app, "multisig.create.name", "R11 Arc")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
        type(app, "multisig.create.address", Self.outside)
        tap(app, "multisig.create.addMember")
        tap(app, "multisig.create.next")
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 10))
        picker.tap()
        let arcRow = app.buttons["Arc Testnet"].firstMatch
        XCTAssertTrue(arcRow.waitForExistence(timeout: 5), "no Arc Testnet in the testnet space")
        arcRow.tap()
        tap(app, "multisig.create.toReview")
        let draft = element(app, "multisig.create.draftAddress")
        XCTAssertTrue(draft.waitForExistence(timeout: 20), "no address")
        let address = draft.label.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression).map { String(draft.label[$0]) } ?? ""
        try address.write(toFile: safeFile, atomically: true, encoding: .utf8)
        tap(app, "multisig.create.create")
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 20) { allow.tap() }
        XCTAssertTrue(element(app, "multisig.create.created").waitForExistence(timeout: 60), "not created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 20))
        XCTAssertFalse(element(app, "multisig.home.otherSpace").exists, "a note in its own space")
        sleep(8)
        shot(app, "r11-3-0-testnet-home")

        switchSpace(app, to: "mainnet")
        let note = element(app, "multisig.home.otherSpace")
        XCTAssertTrue(note.waitForExistence(timeout: 20), "no testnet-space note in the mainnet space")
        XCTAssertTrue(note.label.contains("Arc Testnet"), note.label)
        XCTAssertTrue(note.label.contains("Testnet space"), note.label)
        sleep(15)
        shot(app, "r11-3-1-mainnet-home")
        XCTAssertTrue(text(app, containing: "Testnet space").exists)
        for falseWords in ["0.00", "Updated now", "No activity yet", "Deposit your first asset", "Live · listening"] {
            XCTAssertFalse(text(app, containing: falseWords).exists, "a false \"\(falseWords)\" for a multisig of the other space")
        }
        // Its switch: back where it lives.
        tap(app, "multisig.home.otherSpaceSwitch")
        sleep(4)
        XCTAssertFalse(element(app, "multisig.home.otherSpace").exists, "the note stayed after the switch")
        shot(app, "r11-3-2-switched-back")
    }

    // MARK: - S4: the sheet's heading and minus

    /// After test1: the outside owner proposes a 0.01 send from the funded
    /// multisig; its sheet heads what moves "Balance changes" and writes the
    /// outflow with U+2212, never a hyphen.
    func test2TheSheetSaysBalanceChangesWithTheMinusSign() throws {
        let app = try launch(space: "testnet")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60), "the Arc multisig is not shown")
        try driver("fund", safe)
        try driver("propose", "5042002 \(safe) native 0")
        let row = element(app, "multisig.home.proposal")
        let seen = Date().addingTimeInterval(90)
        while !row.exists && Date() < seen { sleep(3) }
        if !row.exists { shot(app, "r11-s4-debug-home") }
        XCTAssertTrue(row.exists, "the proposal never reached the home")
        if !row.isHittable { app.swipeUp() }
        row.tap()
        let heading = text(app, containing: "Balance changes")
        var found = heading.waitForExistence(timeout: 30)
        for _ in 0..<4 where !found { app.swipeUp(); found = heading.waitForExistence(timeout: 5) }
        shot(app, "r11-s4-sheet")
        XCTAssertTrue(found, "no \"Balance changes\" heading")
        XCTAssertFalse(text(app, containing: "Simulation result").exists)
        let delta = element(app, "multisig.review.delta")
        XCTAssertTrue(delta.waitForExistence(timeout: 10), "no balance change drawn")
        XCTAssertTrue(delta.label.hasPrefix("\u{2212}"), "not U+2212: \(delta.label)")
        XCTAssertFalse(delta.label.hasPrefix("-"), "a hyphen: \(delta.label)")
    }

    // MARK: - r11-7: a dead network, warm

    /// After test1: the Arc multisig shown with Arc's RPC through the dead
    /// proxy, read once while it answers; then the network dies with the app
    /// warm — within 150 s the home says it cannot reach Arc Testnet.
    func test3AWarmOutageIsSaid() throws {
        let h = try required()
        guard let dead = env["VELA_R11_DEAD_ARC"] else { throw XCTSkip("no dead proxy named (VELA_R11_DEAD_ARC)") }
        let flag = "\(h.state)/dead-arc"
        try? FileManager.default.removeItem(atPath: flag)
        defer { try? FileManager.default.removeItem(atPath: flag) }
        let app = try launch(space: "testnet", arc: dead)
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 60))
        let fresh = app.buttons.matching(NSPredicate(format: "label == %@", "Updated now")).firstMatch
        XCTAssertTrue(fresh.waitForExistence(timeout: 90), "never read")
        shot(app, "r11-7-0-before")
        try "on".write(toFile: flag, atomically: true, encoding: .utf8)
        let started = Date()
        let reach = NSPredicate(format: "label CONTAINS %@", "Can't reach")
        let line = app.descendants(matching: .any).matching(reach).firstMatch
        var said = false
        var shot45 = false
        while Date().timeIntervalSince(started) < 150 {
            if line.exists { said = true; break }
            if !shot45, Date().timeIntervalSince(started) > 45 { shot(app, "r11-7-1-dead-45s"); shot45 = true }
            sleep(3)
        }
        print("R11-7: reason said after \(Int(Date().timeIntervalSince(started))) s: \(said ? line.label : "-")")
        shot(app, "r11-7-2-dead-said")
        XCTAssertTrue(said, "no can't-reach line within 150 s of a warm outage")
        XCTAssertTrue(line.label.contains("Arc Testnet"), line.label)
        try? FileManager.default.removeItem(atPath: flag)
        let back = Date()
        XCTAssertTrue(fresh.waitForExistence(timeout: 120), "never read again after the network came back")
        print("R11-7: read again \(Int(Date().timeIntervalSince(back))) s after the network came back")
        shot(app, "r11-7-3-back")
    }
}
