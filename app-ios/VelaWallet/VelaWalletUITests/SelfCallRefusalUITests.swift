//
//  SelfCallRefusalUITests.swift
//  VelaWalletUITests
//
//  Spec 081 on a simulator: a page asks for something that would change who
//  controls the account — `addOwnerWithThreshold` or `enableModule` on the
//  account itself, a `SafeTx` typed-data signature, the Bybit shape (an
//  `execTransaction` carrying a DELEGATECALL) — and the wallet refuses it
//  where the person can see: the signing sheet opens REFUSED, says why and
//  names the function, offers no signature; closed, the page is told the
//  WALLET refused (-32603, the function inside the sentence), never that the
//  person rejected it (4001).
//
//  The harness page is `testdapp.html`, served by `LocalDappServer` inside
//  this runner (the `BrowserAcceptanceTests` pattern). Hermetic: the refusal
//  is the core's reading of the calldata, made before any network read, and
//  connecting needs none. Its own class, not `BrowserAcceptanceTests`, which
//  the scheme skips by name (it was written for hardware and reads real
//  chains); these four never touch a chain, so they run where the scheme's
//  UI leg runs.
//

import XCTest

final class SelfCallRefusalUITests: XCTestCase {

    private var server: LocalDappServer?

    /// Its own port: a run beside the browser's suites must not meet theirs.
    private static let port: UInt16 = 8141

    override func setUpWithError() throws {
        continueAfterFailure = false
        let server = try LocalDappServer(html: try LocalDappServer.page(), port: Self.port)
        server.start()
        self.server = server
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    /// The app in the parallel space (its first key), the page open.
    private func launchBrowsing() throws -> XCUIApplication {
        let server = try XCTUnwrap(server)
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = server.pageUrl
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    /// A launch with `VELA_URL` lands on the page; should the app be
    /// elsewhere, 探索 and its resume row bring it back.
    private func openExplore(_ app: XCUIApplication) {
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 15) { return }
        let tab = app.buttons["探索"].firstMatch
        if tab.exists, tab.isHittable { tab.tap() }
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        if row.waitForExistence(timeout: 10) { row.tap() }
    }

    /// Connected (whether or not this origin was granted before), on Gnosis.
    private func connect(_ app: XCUIApplication) {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "连接", "批准")).firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        XCTAssertNotNil(verdictLine(app, containing: "#verdict eth_requestAccounts ok"), "the page was never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertNotNil(verdictLine(app, containing: "#verdict wallet_switchEthereumChain ok"),
                        "the page could not switch itself to Gnosis")
    }

    /// The page's verdict line that contains `fragment`, once one does.
    private func verdictLine(_ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30) -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "#verdict"))
                .allElementsBoundByIndex
            if let hit = lines.first(where: { $0.label.contains(fragment) }) {
                let note = XCTAttachment(string: hit.label)
                note.name = "verdict"
                note.lifetime = .keepAlways
                add(note)
                return hit.label
            }
            _ = XCTWaiter.wait(for: [expectation(description: "poll")], timeout: 1.0)
        }
        return nil
    }

    private func attach(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The page's `button` asks with `method`; the sheet opens refused, its
    /// body containing `body`; closed, the page is answered -32603 naming
    /// `function`.
    private func assertRefused(button: String, method: String, function: String, body: String, shot: String) throws {
        let app = try launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30), "the space is not open")
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30), "the page never rendered")
        connect(app)

        app.webViews.buttons[button].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["Vela 不会签这笔请求"].waitForExistence(timeout: 30),
                      "\(button): the sheet did not open refused")
        let why = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", body)).firstMatch
        XCTAssertTrue(why.waitForExistence(timeout: 5), "\(button): the refusal does not say why (\(body))")
        attach(app, shot)
        // Nothing to sign with: no confirm, or one that stays shut.
        let confirm = app.buttons["signing.confirm"]
        if confirm.waitForExistence(timeout: 3) {
            XCTAssertFalse(confirm.isEnabled, "\(button): a refused request offers a signature")
        }

        let close = app.buttons["signing.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 10), "\(button): the refused sheet has no ✕")
        close.tap()
        let answered = verdictLine(app, containing: "#verdict \(method) err -32603")
        XCTAssertNotNil(answered, "\(button): the page was not told the wallet refused it")
        XCTAssertTrue(answered?.contains(function) == true,
                      "\(button): the page's answer does not name \(function): \(answered ?? "")")
        app.terminate()
    }

    /// `addOwnerWithThreshold` on the account itself: a takeover in one call.
    func testAnAddOwnerOnTheAccountIsRefusedAndSaid() throws {
        try assertRefused(
            button: "Add owner (blocked)", method: "eth_sendTransaction", function: "addOwnerWithThreshold",
            body: "（addOwnerWithThreshold）", shot: "refused-add-owner"
        )
    }

    /// `enableModule` on the account itself: a module acts without the keys.
    func testAnEnableModuleOnTheAccountIsRefusedAndSaid() throws {
        try assertRefused(
            button: "Enable module (blocked)", method: "eth_sendTransaction", function: "enableModule",
            body: "（enableModule）", shot: "refused-enable-module"
        )
    }

    /// A `SafeTx` typed-data signature: an instruction anyone can execute later.
    func testASafeTxTypedDataSignatureIsRefusedAndSaid() throws {
        try assertRefused(
            button: "SafeTx typed data (blocked)", method: "eth_signTypedData_v4", function: "SafeTx",
            body: "授权一笔以后再执行的 Safe 交易", shot: "refused-safetx"
        )
    }

    /// The Bybit shape: `execTransaction` on a Safe the account holds a key
    /// of, carrying a DELEGATECALL (operation 1) of `transfer(attacker, 0)`
    /// to the contract that rewrote the Safe's slot 0. Refused for the
    /// delegatecall, whatever it targets.
    func testABybitShapedDelegatecallIsRefusedAndSaid() throws {
        try assertRefused(
            button: "Bybit execTransaction (blocked)", method: "eth_sendTransaction", function: "delegatecall",
            body: "（delegatecall）", shot: "refused-bybit"
        )
    }
}
