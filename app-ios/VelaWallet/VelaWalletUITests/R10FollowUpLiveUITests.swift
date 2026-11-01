//
//  R10FollowUpLiveUITests.swift
//  VelaWalletUITests
//
//  The round-10 follow-ups (docs/multisig-machines.md §10) on a running app,
//  against the multisig fork harness (scripts/onchain/multisig-fork/up.sh):
//
//      TEST_RUNNER_VELA_HARNESS_RPC / _RELAY / _GENESIS
//      TEST_RUNNER_VELA_R10B_SHOTS=…   where the screenshots go
//      TEST_RUNNER_VELA_R10B_DEAD_RPC=http://127.0.0.1:<nothing listening>
//      TEST_RUNNER_VELA_R10B_PAGE_PORT=<a free port for the test page>
//
//  Skipped without them. `test1` wants a FRESH install (no cached holdings):
//  with every network dead, Send's picker says the list could not be read and
//  offers "Try again" — no alert. `test2`: the keys in Settings go by their
//  own names, never the account's ("Interleave"). `test3`: a page's switch
//  to the other space's network is refused naming that space; after a
//  space switch the site's next sheet says it was moved.
//

import XCTest

final class R10FollowUpLiveUITests: XCTestCase {

    private var env: [String: String] { ProcessInfo.processInfo.environment }
    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    private func required() throws -> (rpc: String, relay: String, genesis: String) {
        guard let rpc = env["VELA_HARNESS_RPC"], let relay = env["VELA_HARNESS_RELAY"],
              let genesis = env["VELA_HARNESS_GENESIS"]
        else { throw XCTSkip("no fork harness named") }
        return (rpc, relay, genesis)
    }

    /// Every built-in network (`ChainCatalog`), for a launch that kills them all.
    private static let everyChain = [
        1, 56, 137, 42161, 10, 8453, 43114, 100, 130, 4217, 143, 480, 5042, 196, 988, 1868, 4326, 4663, 5000,
        8217, 42220, 57073, 98866, 1440000, 11155111, 84532, 421614, 11155420, 80002, 97, 43113, 5042002, 42431,
    ]

    /// `dead`: every network answers from nowhere (the overrides persist:
    /// run it on a fresh install, and uninstall after).
    private func launch(dead: String? = nil, url: String? = nil, space: String = "mainnet") throws -> XCUIApplication {
        let h = try required()
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        if let dead {
            let all = Self.everyChain.map { #""\#($0)":{"rpcURL":"\#(dead)","onlyMine":true}"# }
            app.launchEnvironment["VELA_RPC_OVERRIDES"] = "{" + all.joined(separator: ",") + "}"
        } else {
            app.launchEnvironment["VELA_RPC_OVERRIDES"] = #"{"100":{"rpcURL":"\#(h.rpc)","onlyMine":true}}"#
        }
        app.launchEnvironment["VELA_SERVICE_ENDPOINTS"] =
            #"{"bundlerServiceURL":"\#(h.relay)","genesisServiceURL":"\#(h.genesis)"}"#
        if let url { app.launchEnvironment["VELA_URL"] = url }
        app.launchArguments += ["-AppleLanguages", "(en)", "-vela.networkSpace", space]
        app.launch()
        if url == nil {
            XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 60), "the wallet never opened")
        }
        return app
    }

    @discardableResult
    private func shot(_ app: XCUIApplication, _ name: String) -> XCUIScreenshot {
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = env["VELA_R10B_SHOTS"] {
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

    private func tapTab(_ label: String, in app: XCUIApplication) {
        let button = app.buttons[label]
        if button.exists { button.tap(); return }
        let matches = app.staticTexts.matching(identifier: label)
        if matches.count > 0 { matches.element(boundBy: matches.count - 1).tap(); return }
        XCTFail("no tab called \(label)")
    }

    // MARK: - Send: a list that could not be read

    func test1ATokenListThatFailedIsThePickersToRetry() throws {
        guard let dead = env["VELA_R10B_DEAD_RPC"] else { throw XCTSkip("no dead RPC named") }
        let app = try launch(dead: dead)
        app.buttons["Send"].firstMatch.tap()
        let empty = element(app, "send.pick.empty")
        let said = empty.waitForExistence(timeout: 120)
        if !said { shot(app, "r10b-send-debug") }
        XCTAssertTrue(said, "the picker never said the list failed")
        XCTAssertTrue(empty.label.contains("Couldn"), empty.label)
        let retry = element(app, "send.pick.retry")
        XCTAssertTrue(retry.exists, "no Try again")
        XCTAssertEqual(app.alerts.count, 0, "the core's alert was drawn over the state")
        shot(app, "r10b-send-tokens-failed")
        retry.tap()
        // Asked again: the state stands (every network is still dead), and no alert.
        sleep(2)
        XCTAssertEqual(app.alerts.count, 0)
        XCTAssertTrue(element(app, "send.pick.empty").waitForExistence(timeout: 120))
        shot(app, "r10b-send-tokens-failed-retried")
        // Left by the back arrow: nothing rises over the home afterwards.
        app.buttons["Back"].firstMatch.tap()
        XCTAssertTrue(app.buttons["wallet.header.name"].waitForExistence(timeout: 10))
        sleep(5)
        XCTAssertEqual(app.alerts.count, 0, "a late answer raised something over the home")
        shot(app, "r10b-send-left-home")
    }

    // MARK: - Keys are not accounts

    func test2TheKeysGoByTheirOwnNames() throws {
        let app = try launch()
        let header = app.buttons["wallet.header.name"]
        let named = NSPredicate(format: "label CONTAINS %@", "Interleave")
        expectation(for: named, evaluatedWith: header)
        waitForExpectations(timeout: 30)
        tapTab("Settings", in: app)
        let first = text(app, containing: "Parallel One")
        for _ in 0..<6 where !(first.exists && first.isHittable) { app.swipeUp() }
        XCTAssertTrue(first.waitForExistence(timeout: 30), "key 1 is not named by its own name")
        shot(app, "r10b-keys-own-names")
        // The registry's answer (the fork's Gnosis) names them too.
        let checking = text(app, containing: "Checking")
        let deadline = Date().addingTimeInterval(30)
        while checking.exists, Date() < deadline { sleep(1) }
        sleep(2)
        XCTAssertTrue(first.exists, "the registry's answer took key 1's own name away")
        shot(app, "r10b-keys-own-names-registry")
        // The account's row is not a key's: R10FixLiveUITests.test10 reads it so.
        XCTAssertFalse(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Parallel One")).firstMatch.exists)
        // No key row carries the account's name: "Interleave" is the
        // account's, named once at the top of Settings.
        let keyRows = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", "Interleave"))
        XCTAssertLessThanOrEqual(keyRows.count, 2, "the account's name stands on more than its own rows")
    }

    // MARK: - The in-app browser and the network space

    private static let page = """
    <!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">
    <style>body{font:16px -apple-system;margin:16px}button{font-size:18px;padding:10px 14px;margin:6px 6px 6px 0}pre{white-space:pre-wrap}</style>
    </head><body>
    <pre id="last">#verdict boot waiting</pre>
    <pre id="events">#event none</pre>
    <button id="connect">Connect</button>
    <button id="sepolia">Switch to Sepolia</button>
    <button id="sign">Sign</button>
    <script>
    const last = document.getElementById('last');
    function say(s){ last.textContent = '#verdict ' + s; }
    async function ask(method, params){
      try { const r = await window.ethereum.request({method, params}); say(method + ' ok ' + JSON.stringify(r)); }
      catch (e) { say(method + ' err ' + e.code + ' ' + e.message); }
    }
    const events = document.getElementById('events');
    window.ethereum && window.ethereum.on && window.ethereum.on('chainChanged', c => { events.textContent = '#event chainChanged ' + c; });
    document.getElementById('connect').onclick = () => ask('eth_requestAccounts', []);
    document.getElementById('sepolia').onclick = () => ask('wallet_switchEthereumChain', [{chainId: '0xaa36a7'}]);
    document.getElementById('sign').onclick = async () => {
      const [a] = await window.ethereum.request({method: 'eth_accounts'});
      ask('personal_sign', ['0x68656c6c6f', a]);
    };
    </script></body></html>
    """

    private func verdict(_ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30) -> Bool {
        let line = app.webViews.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "#verdict")).firstMatch
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if line.exists, line.label.contains(fragment) { return true }
            Thread.sleep(forTimeInterval: 0.5)
        }
        return false
    }

    func test3AMovedSiteSaysSoAndTheOtherSpaceIsNamed() throws {
        guard let portText = env["VELA_R10B_PAGE_PORT"], let port = UInt16(portText) else {
            throw XCTSkip("no page port named")
        }
        let server = try LocalDappServer(html: Self.page, port: port)
        server.start()
        self.server = server
        let app = try launch(url: server.pageUrl)
        XCTAssertTrue(app.buttons["explore.bar.back"].waitForExistence(timeout: 30), "the page never opened")
        let connect = app.webViews.buttons["Connect"].firstMatch
        XCTAssertTrue(connect.waitForExistence(timeout: 30))
        connect.tap()
        // The sheet's Connect, not the page's: the sheet stands once its
        // title names the asking site.
        if element(app, "explore.connection.title").waitForExistence(timeout: 12) {
            let buttons = app.buttons.matching(NSPredicate(format: "label == %@", "Connect"))
            let sheet = (0..<buttons.count).map { buttons.element(boundBy: $0) }
                .last { $0.isHittable && $0.frame.minY > 400 }
            sheet?.tap()
        }
        let connected = verdict(app, containing: "eth_requestAccounts ok")
        if !connected { shot(app, "r10b-browser-connect-debug") }
        XCTAssertTrue(connected, "never connected")

        // The other space's network: refused naming that space.
        app.webViews.buttons["Switch to Sepolia"].firstMatch.tap()
        XCTAssertTrue(verdict(app, containing: "4902"), "not refused")
        XCTAssertTrue(verdict(app, containing: "testnet"), "the refusal does not name the space")
        XCTAssertFalse(verdict(app, containing: "not in Vela's networks", timeout: 1))
        shot(app, "r10b-browser-other-space-refused")

        // The person switches to the testnet space in Vela.
        tapTab("Wallet", in: app)
        let space = element(app, "network-space")
        XCTAssertTrue(space.waitForExistence(timeout: 15), "no space label on the home")
        space.tap()
        let toTestnet = app.buttons["Switch to the testnet space"].firstMatch
        XCTAssertTrue(toTestnet.waitForExistence(timeout: 10))
        toTestnet.tap()
        sleep(3)
        shot(app, "r10b-home-testnet-space")
        tapTab("Explore", in: app)
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        if row.waitForExistence(timeout: 5) { row.tap() }
        XCTAssertTrue(app.webViews.buttons["Sign"].firstMatch.waitForExistence(timeout: 20))

        // The site's next sheet says the space moved it.
        app.webViews.buttons["Sign"].firstMatch.tap()
        let note = element(app, "sheet.spaceNote")
        let said = note.waitForExistence(timeout: 30)
        if !said { shot(app, "r10b-browser-debug") }
        XCTAssertTrue(said, "the sheet does not say the site moved")
        XCTAssertTrue(note.label.contains("The network changed with the space"), note.label)
        shot(app, "r10b-browser-space-note-signing")
        let close = app.buttons["Close"].firstMatch
        if close.exists { close.tap() }
        XCTAssertTrue(verdict(app, containing: "personal_sign err"), "the sheet's close did not answer the page")

        // Back to the mainnet space for whoever runs next.
        tapTab("Wallet", in: app)
        if element(app, "network-space").waitForExistence(timeout: 10) {
            element(app, "network-space").tap()
            let back = app.buttons["Switch to the mainnet space"].firstMatch
            if back.waitForExistence(timeout: 10) { back.tap() }
        }
    }
}
