//
//  R12FixLiveUITests.swift
//  VelaWalletUITests
//
//  Round 12's iOS fixes, live against the multisig fork harness
//  (scripts/onchain/multisig-fork/up.sh), an Arc-chain-id fork with a proxy
//  in front of it that can be made dead or slow, and a host driver playing
//  the outside owner (it answers the request files written in
//  `VELA_R12_STATE`: propose with its approval, fund).
//
//      TEST_RUNNER_VELA_HARNESS_RPC / _RELAY / _GENESIS / _PEERCONNECT
//      TEST_RUNNER_VELA_R12_ARC_RPC=…    the proxy in front of the fork answering
//                                        as Arc Testnet (5042002): dead while
//                                        `$VELA_R12_FLAGS/dead` exists, slow
//                                        while `$VELA_R12_FLAGS/slow` does
//      TEST_RUNNER_VELA_R12_FLAGS=…      that proxy's flag directory
//      TEST_RUNNER_VELA_R12_STATE=…      the driver's request files
//      TEST_RUNNER_VELA_R12_SHOTS=…      where the screenshots go
//      TEST_RUNNER_VELA_R12_LANG / _THEME  zh / dark for the zh-dark pass
//
//  Skipped without them. On one install, in order:
//  test1 — S4: Activity says it is still looking while the first scan is
//          out (Arc slow), never "No activity yet"; then the empty line.
//  test2 — r12-3: Arc's RPC dead with the app warm: never "Updated now" over
//          failed reads, and the reach line within the core's time.
//  test3 — r12-1: an ERC-20 transfer from a multisig that holds none of it
//          says "would fail" in the token's own decimals (or its base units,
//          "decimals unknown") — never "needs <0.000001"; #0 says one note.
//  test4 — zh / dark: the multisig sheet, home, Activity and Settings
//          captured (run with _LANG=zh _THEME=dark).
//

import XCTest

final class R12FixLiveUITests: XCTestCase {

    static let parallelTwo = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    /// The fork's unlocked dev account #1: the outside owner the driver signs as.
    static let outside = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private var env: [String: String] { ProcessInfo.processInfo.environment }

    private func required() throws -> (rpc: String, relay: String, genesis: String, arc: String, flags: String, state: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"], let arc = env["VELA_R12_ARC_RPC"],
              let flags = env["VELA_R12_FLAGS"], let state = env["VELA_R12_STATE"]
        else { throw XCTSkip("no fork harness / Arc proxy / r12 driver named") }
        return (rpc, relay, genesis, arc, flags, state)
    }

    private var safeFile: String { (env["VELA_R12_STATE"] ?? "/tmp") + "/safe-addr.txt" }
    private var safe: String {
        ((try? String(contentsOfFile: safeFile, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func flag(_ name: String, _ on: Bool, _ content: String = "on") throws {
        let path = "\(try required().flags)/\(name)"
        if on { try content.write(toFile: path, atomically: true, encoding: .utf8) } else { try? FileManager.default.removeItem(atPath: path) }
    }

    private func launch(space: String) throws -> XCUIApplication {
        let h = try required()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = env["VELA_R12_LANG"] ?? "en"
        app.launchEnvironment["VELA_THEME"] = env["VELA_R12_THEME"] ?? "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_RPC_OVERRIDES"] =
            #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true},"5042002":{"rpcURL":"\#(h.arc)","onlyMine":true}}"#
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if let peer = env["VELA_HARNESS_PEERCONNECT"] { app.launchEnvironment["VELA_PEERCONNECT_RELAY"] = peer }
        app.launchArguments += ["-vela.networkSpace", space]
        app.launch()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        return app
    }

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
        if let dir = env["VELA_R12_SHOTS"] {
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

    /// Every label on screen, one per line (for the log and the evidence).
    private func labels(_ app: XCUIApplication) -> [String] {
        app.descendants(matching: .any).allElementsBoundByIndex.prefix(400).map(\.label).filter { !$0.isEmpty }
    }

    private func dump(_ app: XCUIApplication, _ name: String) {
        guard let dir = env["VELA_R12_SHOTS"] else { return }
        try? labels(app).joined(separator: "\n").write(toFile: "\(dir)/\(name).txt", atomically: true, encoding: .utf8)
    }

    /// The parallel space's own account in front (not a multisig).
    private func showTheVelaAccount(_ app: XCUIApplication) {
        let header = app.buttons["wallet.header.name"]
        sleep(2)
        guard !header.label.contains("Interleave") else { return }
        header.tap()
        let mine = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "Interleave")).firstMatch
        if mine.waitForExistence(timeout: 10) { mine.tap(); sleep(2) }
    }

    // MARK: - S4: Activity before the first scan

    /// Arc slow (every answer 25 s late) from launch: while the account's
    /// first incoming scan is out the home's Activity is loading — never "No
    /// activity yet"; once the network answers, the empty line.
    func test1ActivityIsLoadingUntilTheFirstScanIsBack() throws {
        try flag("dead", false)
        try flag("slow", true, "25")
        defer { try? flag("slow", false) }
        let app = try launch(space: "testnet")
        showTheVelaAccount(app)
        let started = Date()
        let empty = text(app, containing: "No activity yet")
        var sawEmptyEarly = false
        while Date().timeIntervalSince(started) < 15 {
            if empty.exists { sawEmptyEarly = true; break }
            sleep(1)
        }
        shot(app, "i-r12-s4-activity-0-scanning")
        dump(app, "i-r12-s4-activity-0-scanning")
        XCTAssertFalse(sawEmptyEarly, "\"No activity yet\" before the first scan came back")
        try flag("slow", false)
        XCTAssertTrue(empty.waitForExistence(timeout: 120), "never the empty line after the scan")
        print("R12-S4: the empty line after \(Int(Date().timeIntervalSince(started))) s")
        shot(app, "i-r12-s4-activity-1-scanned")
    }

    // MARK: - r12-3: a warm Arc outage

    /// Read once while Arc answers; then the RPC dies with the app warm —
    /// never "Updated now" over the failed reads, and the reach line (or the
    /// core's warm-outage word) within 150 s; back again after.
    func test2AWarmArcOutageIsSaid() throws {
        try flag("dead", false)
        try flag("slow", false)
        defer { try? flag("dead", false) }
        let app = try launch(space: "testnet")
        showTheVelaAccount(app)
        let fresh = app.buttons.matching(NSPredicate(format: "label == %@", "Updated now")).firstMatch
        XCTAssertTrue(fresh.waitForExistence(timeout: 90), "never read")
        shot(app, "i-r12-3-0-before")
        try flag("dead", true)
        let started = Date()
        let line = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", "Can't reach")).firstMatch
        var said = false
        var freshOverFailed = false
        var shot45 = false
        // The first read after the death is the earliest a stamp could lie.
        while Date().timeIntervalSince(started) < 150 {
            if line.exists { said = true; break }
            if Date().timeIntervalSince(started) > 70, fresh.exists { freshOverFailed = true }
            if !shot45, Date().timeIntervalSince(started) > 45 { shot(app, "i-r12-3-1-dead-45s"); dump(app, "i-r12-3-1-dead-45s"); shot45 = true }
            sleep(2)
        }
        let after = Int(Date().timeIntervalSince(started))
        print("R12-3: reach line after \(after) s: \(said ? line.label : "-")")
        shot(app, "i-r12-3-2-dead-said")
        dump(app, "i-r12-3-2-dead-said")
        try? "\(after) s: \(said ? line.label : "not said")".write(
            toFile: "\(env["VELA_R12_SHOTS"] ?? "/tmp")/i-r12-3-timing.txt", atomically: true, encoding: .utf8)
        XCTAssertFalse(freshOverFailed, "\"Updated now\" over failed reads")
        XCTAssertTrue(said, "no can't-reach line within 150 s of a warm outage")
        XCTAssertTrue(line.label.contains("Arc Testnet"), line.label)
        try flag("dead", false)
        XCTAssertTrue(fresh.waitForExistence(timeout: 150), "never read again after the network came back")
        shot(app, "i-r12-3-3-back")
    }

    // MARK: - r12-1: "would fail" in the token's decimals

    /// A 1-of-2 multisig on Gnosis (the fork) that holds no USDC; the
    /// outside owner proposes and approves a 0.01 USDC transfer, so it is
    /// ready and short. The home and the sheet say the shortfall in USDC's
    /// own decimals (or its base units, "decimals unknown") — never
    /// "needs <0.000001".
    func test3WouldFailSaysTheTokensDecimals() throws {
        let app = try launch(space: "mainnet")
        tap(app, "wallet.header.name")
        tap(app, "multisig.switcher.create")
        type(app, "multisig.create.name", "R12 Short")
        type(app, "multisig.create.address", Self.parallelTwo)
        tap(app, "multisig.create.addMember")
        type(app, "multisig.create.address", Self.outside)
        tap(app, "multisig.create.addMember")
        tap(app, "multisig.create.next")
        for _ in 0..<4 {
            let value = element(app, "multisig.create.thresholdValue")
            if value.waitForExistence(timeout: 5), value.label.hasPrefix("1") || value.label.contains(" 1 ") { break }
            let fewer = element(app, "multisig.create.fewer")
            guard fewer.exists, fewer.isEnabled else { break }
            fewer.tap()
        }
        let picker = app.buttons["multisig.create.network"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 10))
        picker.tap()
        let gnosis = app.buttons["Gnosis"].firstMatch
        XCTAssertTrue(gnosis.waitForExistence(timeout: 5), "no Gnosis in the mainnet space")
        gnosis.tap()
        tap(app, "multisig.create.toReview")
        let draft = element(app, "multisig.create.draftAddress")
        XCTAssertTrue(draft.waitForExistence(timeout: 20), "no address")
        let address = draft.label.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression).map { String(draft.label[$0]) } ?? ""
        try address.write(toFile: safeFile, atomically: true, encoding: .utf8)
        shot(app, "i-r12-1-0-review")
        tap(app, "multisig.create.create")
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 20) { allow.tap() }
        XCTAssertTrue(element(app, "multisig.create.created").waitForExistence(timeout: 90), "not created")
        tap(app, "multisig.create.done")
        XCTAssertTrue(element(app, "multisig.home").waitForExistence(timeout: 20))
        sleep(6)

        try driver("propose", "100 \(address) erc20 0")
        let row = element(app, "multisig.home.proposal")
        let seen = Date().addingTimeInterval(120)
        while !row.exists && Date() < seen { sleep(3) }
        XCTAssertTrue(row.exists, "the proposal never reached the home")
        // The look: ready (1 of 1 approved) and short — "Can't run now".
        let cannot = text(app, containing: "run now —")
        let looked = cannot.waitForExistence(timeout: 90)
        shot(app, "i-r12-1-1-home")
        dump(app, "i-r12-1-1-home")
        XCTAssertTrue(looked, "the home never said why it can't run")
        XCTAssertFalse(cannot.label.contains("<0.000001"), "18 decimals assumed: \(cannot.label)")
        XCTAssertTrue(cannot.label.contains("0.01 USDC") || cannot.label.contains("decimals unknown"), cannot.label)

        if !row.isHittable { app.swipeUp() }
        row.tap()
        let fails = text(app, containing: "would fail")
        var found = fails.waitForExistence(timeout: 40)
        for _ in 0..<4 where !found { app.swipeUp(); found = fails.waitForExistence(timeout: 5) }
        shot(app, "i-r12-1-2-sheet")
        dump(app, "i-r12-1-2-sheet")
        XCTAssertTrue(found, "the sheet never said it would fail")
        XCTAssertFalse(fails.label.contains("<0.000001"), "18 decimals assumed: \(fails.label)")
        let notes = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS[c] %@ OR label CONTAINS[c] %@", "would fail", "expected to fail"))
            .allElementsBoundByIndex.map(\.label)
        print("R12-1 #0 fail notes: \(Set(notes))")
    }

    // MARK: - zh / dark

    /// The multisig of test3 shown: its home, the transaction's sheet,
    /// Activity and Settings, captured as the language and theme the run
    /// names (`VELA_R12_LANG`, `VELA_R12_THEME`) — the evidence for the
    /// zh/dark pass, every label dumped beside it for untranslated words.
    func test4ZhDarkCapture() throws {
        let tag = "\(env["VELA_R12_LANG"] ?? "en")-\(env["VELA_R12_THEME"] ?? "light")"
        let app = try launch(space: "mainnet")
        sleep(8)
        shot(app, "i-\(tag)-0-home")
        dump(app, "i-\(tag)-0-home")
        let row = element(app, "multisig.home.proposal")
        if row.waitForExistence(timeout: 30) {
            if !row.isHittable { app.swipeUp() }
            row.tap()
            sleep(8)
            shot(app, "i-\(tag)-1-sheet")
            dump(app, "i-\(tag)-1-sheet")
            app.swipeUp()
            sleep(1)
            shot(app, "i-\(tag)-2-sheet-more")
            dump(app, "i-\(tag)-2-sheet-more")
            let done = element(app, "multisig.sheet.close")
            if done.exists { done.tap() } else { app.swipeDown(velocity: .fast) }
            sleep(2)
        }
        // Activity: the tab bar's second… the home's "All" opens History.
        app.swipeUp()
        sleep(1)
        shot(app, "i-\(tag)-3-home-lower")
        dump(app, "i-\(tag)-3-home-lower")
        let tabs = app.tabBars.buttons
        if tabs.count >= 4 {
            tabs.element(boundBy: 3).tap()
            sleep(3)
            shot(app, "i-\(tag)-4-settings")
            dump(app, "i-\(tag)-4-settings")
            app.swipeUp()
            sleep(1)
            shot(app, "i-\(tag)-5-settings-more")
            dump(app, "i-\(tag)-5-settings-more")
            tabs.element(boundBy: 0).tap()
            sleep(2)
        }
        let settings = element(app, "multisig.home.settings")
        if settings.waitForExistence(timeout: 10) {
            if !settings.isHittable { app.swipeUp() }
            settings.tap()
            sleep(4)
            shot(app, "i-\(tag)-6-multisig-settings")
            dump(app, "i-\(tag)-6-multisig-settings")
        }
    }
}
