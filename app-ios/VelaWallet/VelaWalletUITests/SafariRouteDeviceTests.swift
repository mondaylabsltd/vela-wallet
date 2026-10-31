//
//  SafariRouteDeviceTests.swift
//  VelaWalletUITests
//
//  Spec 102 R8 on a simulator: the Safari Web Extension creates a wallet,
//  connects, signs and sends with a REAL simulator passkey, every ceremony and
//  every signature on the trusted signing page (WebKit refuses the
//  extension's own pages a passkey).
//
//  Simulator only, and opt-in (`TEST_RUNNER_VELA_SAFARI_ROUTE_E2E=1`): it
//  drives Safari and Settings. Inputs, all `TEST_RUNNER_`-prefixed:
//
//    VELA_SIGNER_PAGE   the signing page under test, on this machine's
//                       loopback (`samples/serve-dist.mjs`), e.g.
//                       http://localhost:10194/
//    VELA_PROBE_PAGE    a page that makes a passkey for its own host (step 0)
//    VELA_HARNESS_*     the fork harness's ports (as SafariExtensionDeviceTests)
//    VELA_SAFARI_LOCALE en | zh
//    VELA_SAFARI_SHOTS  where screenshots go
//
//  Face ID: the simulator's is enrolled, and a host-side loop posts a match
//  while the test runs (`notifyutil -p com.apple.BiometricKit_Sim.pearl.match`).
//

import XCTest

final class SafariRouteDeviceTests: XCTestCase {

    private let env = ProcessInfo.processInfo.environment
    private let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipUnless(DeviceSafety.onSimulator, "drives Safari: simulator only")
        try XCTSkipUnless(env["VELA_SAFARI_ROUTE_E2E"] == "1", "set TEST_RUNNER_VELA_SAFARI_ROUTE_E2E=1")
    }

    // MARK: - Evidence

    private var shots: URL? {
        env["VELA_SAFARI_SHOTS"].map { URL(fileURLWithPath: $0, isDirectory: true) }
    }

    func capture(_ name: String, _ app: XCUIApplication? = nil) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = shots else { return }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? shot.pngRepresentation.write(to: dir.appendingPathComponent("\(name).png"))
        let tree = (app ?? safari).debugDescription
        try? tree.write(to: dir.appendingPathComponent("\(name).tree.txt"), atomically: true, encoding: .utf8)
    }

    // MARK: - Step 0: can this simulator's Safari make a passkey at all?

    /// A page on the loopback makes a passkey for its own host — the
    /// simulator's own authenticator, Face ID matched by the host.
    func test00_passkeyOnTheLoopback() throws {
        let probe = try XCTUnwrap(env["VELA_PROBE_PAGE"].flatMap(URL.init(string:)), "VELA_PROBE_PAGE")
        openInSafari(probe)
        let create = safari.buttons["Create passkey"]
        XCTAssertTrue(create.waitForExistence(timeout: 20))
        tapWhenReady(create)
        sleep(2)
        capture("00a-asked")
        approvePasskeySheet(shot: "00b")
        let done = pageText(containing: "pk:")
        _ = done.waitForExistence(timeout: 20)
        sleep(1)
        capture("00c-result")
        XCTAssertTrue(pageText(containing: "pk:ok").exists, "the passkey was made")
    }

    // MARK: - The dApp

    private static var server: LocalDappServer?
    static let dappPort: UInt16 = 10193
    private var dappURL: URL { URL(string: "http://127.0.0.1:\(Self.dappPort)/")! }
    private var locale: String { env["VELA_SAFARI_LOCALE"] ?? "en" }

    private func harnessPort(_ key: String, _ fallback: Int) -> Int {
        Int(env["VELA_HARNESS_\(key)"] ?? "") ?? fallback
    }

    /// A dApp that says what it heard where a UI test can read it (a line of
    /// text, not a `<pre>`), and the development package's doorway: the seed
    /// that points the wallet at the harness, the local index and the
    /// loopback signing page, then opens the create flow.
    private func dappHTML() -> String {
        let page = env["VELA_SIGNER_PAGE"] ?? "http://localhost:10194/"
        let seed = "dev-seed.html?genesis=http://127.0.0.1:\(harnessPort("GENESIS", 10187))"
            + "&amp;relay=http://127.0.0.1:\(harnessPort("RELAY", 10190))"
            + "&amp;peerconnect=ws://127.0.0.1:\(harnessPort("PEER", 10192))"
            + "&amp;rpc=100=http://127.0.0.1:\(harnessPort("FORK", 10145))"
            + "&amp;rpc=8453=http://127.0.0.1:\(harnessPort("UNREAD", 10146))"
            + "&amp;index=\(env["VELA_INDEX"] ?? "http://127.0.0.1:10195")"
            + "&amp;page=\(page)&amp;next=create&amp;locale=\(locale)"
        return """
        <!doctype html><html lang="en"><head><meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1"><title>route dApp</title>
        <style>body{font:17px -apple-system,sans-serif;margin:16px}button,a{display:block;margin:10px 0;padding:12px;font-size:17px}p{word-break:break-all}</style>
        </head><body><h1>Route dApp</h1>
        <a id="seed" data-vela-dev="\(seed)">Create a wallet on the signing page</a>
        <button id="connect" type="button">Connect</button>
        <button id="switch" type="button">Switch to Gnosis</button>
        <button id="sign" type="button">Sign message</button>
        <button id="send" type="button">Send 0.001</button>
        <p id="say">heard:none</p>
        <script>
        const say = document.getElementById('say');
        let me = null;
        const ask = async (key, method, params) => {
          say.textContent = key + ':pending';
          try {
            const result = await window.ethereum.request({ method, params: params || [] });
            if (key === 'connect') me = result[0];
            const shown = typeof result === 'string' ? result : JSON.stringify(result);
            say.textContent = key + ':ok ' + shown;
          } catch (error) {
            say.textContent = key + ':error ' + error.code + ' ' + error.message;
          }
          document.title = say.textContent.slice(0, 60);
        };
        const who = () => window.ethereum.selectedAddress || me;
        document.getElementById('connect').onclick = () => ask('connect', 'eth_requestAccounts');
        document.getElementById('switch').onclick = () => ask('switch', 'wallet_switchEthereumChain', [{ chainId: '0x64' }]);
        document.getElementById('sign').onclick = () => ask('sign', 'personal_sign', ['0x48656c6c6f2c2056656c61', who()]);
        document.getElementById('send').onclick = () => ask('send', 'eth_sendTransaction',
          [{ from: who(), to: '0x000000000000000000000000000000000000dEaD', value: '0x38d7ea4c68000' }]);
        </script></body></html>
        """
    }

    private func serveDapp() throws {
        if Self.server == nil {
            let server = try LocalDappServer(html: dappHTML(), port: Self.dappPort)
            server.start()
            Self.server = server
        }
    }

    /// The words this run's wallet and page say, in its locale.
    private struct Words {
        let createWallet, thisDevice, openPage, addPasskey, confirm, finish, enter: String
        let connect, sign, confirmSend: String
    }

    private var words: Words {
        locale == "zh"
            ? Words(createWallet: "创建钱包", thisDevice: "这台设备", openPage: "去签名页确认",
                    addPasskey: "添加通行密钥", confirm: "确认", finish: "继续", enter: "进入钱包",
                    connect: "连接", sign: "签名", confirmSend: "确认发送")
            : Words(createWallet: "Create Wallet", thisDevice: "This device", openPage: "Continue to signing page",
                    addPasskey: "Add a passkey", confirm: "Confirm", finish: "Continue", enter: "Enter Wallet",
                    connect: "Connect", sign: "Sign", confirmSend: "Confirm send")
    }

    // MARK: - Steps

    /// Settings → Apps → Safari → Extensions → Vela Wallet: on, every website allowed.
    func test01_enableExtension() throws {
        // Safari lists an app's extension once the app has run.
        let app = XCUIApplication()
        app.launch()
        sleep(3)
        capture("01-app-launched", app)
        app.terminate()
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.terminate()
        settings.launch()
        for label in ["Apps", "Safari", "Extensions", "Vela Wallet"] {
            try tapRow(in: settings, label: label)
        }
        let allow = settings.switches["Allow Extension"]
        XCTAssertTrue(allow.waitForExistence(timeout: 5), "Safari lists the Vela extension")
        if (allow.value as? String) != "1" {
            allow.switches.firstMatch.exists ? allow.switches.firstMatch.tap() : allow.tap()
        }
        sleep(1)
        for site in ["127.0.0.1", "localhost", "Other Websites"] {
            let cell = settings.cells[site]
            for _ in 0..<4 where !(cell.exists && cell.isHittable) { settings.swipeUp() }
            guard cell.exists else { continue }
            cell.tap()
            let choice = settings.cells["Allow"].exists ? settings.cells["Allow"] : settings.buttons["Allow"]
            if choice.waitForExistence(timeout: 3) { choice.tap() }
            if settings.navigationBars.buttons.firstMatch.exists { settings.navigationBars.buttons.firstMatch.tap() }
            sleep(1)
        }
        capture("01-extension-on", settings)
        XCTAssertEqual(allow.value as? String, "1", "the extension is on")
    }

    /// Create a wallet from Safari: the seed opens the create flow in the
    /// extension; every key is made — and its membership confirmed — on the
    /// signing page, by the simulator's own passkey.
    func test02_createOnThePage() throws {
        try serveDapp()
        openInSafari(dappURL)
        let seed = safari.links["Create a wallet on the signing page"]
        XCTAssertTrue(seed.waitForExistence(timeout: 20))
        seed.tap()
        // The page's own field — not Safari's address bar.
        let field = safari.webViews.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 60), "the create flow opened in the extension")
        let create = safari.webViews.buttons[words.finish]
        tapWhenReady(field)
        sleep(1)
        // One word, so the keyboard's autocorrect leaves it alone.
        field.typeText(locale == "zh" ? "钱包E2E" : "SafariE2E")
        // Put the keyboard away (its ✓): the agreements sit under it.
        for label in ["Done", "完成"] where safari.buttons[label].exists {
            safari.buttons[label].tap()
            break
        }
        sleep(1)
        for ack in locale == "zh" ? ["我的公钥", "我的私钥", "我已阅读"] : ["My public key", "My private key", "I have read"] {
            tapFirst(containing: ack)
        }
        capture("\(locale)-02a-name")
        tapWhenReady(create)
        let method = element(beginningWith: words.thisDevice)
        XCTAssertTrue(method.waitForExistence(timeout: 30), "the three places")
        capture("\(locale)-02b-keys")
        tapWhenReady(method)
        // The key: made on the page (hand-off card → Open → the page's own
        // button → the system's passkey sheet), then confirmed there.
        try onThePage(pageButton: words.addPasskey, shot: "\(locale)-02c-create")
        try onThePage(pageButton: words.confirm, shot: "\(locale)-02d-confirm")
        try finishCreate()
    }

    /// The key list, one confirmed (synced) key: Create Wallet, and in.
    func test02b_finishCreate() throws {
        try finishCreate()
    }

    func finishCreate() throws {
        let finish = safari.webViews.buttons[words.createWallet]
        XCTAssertTrue(finish.waitForExistence(timeout: 30), "the key list offers Create Wallet")
        capture("\(locale)-02e-key-confirmed")
        tapWhenReady(finish)
        let enter = safari.buttons[words.enter]
        XCTAssertTrue(enter.waitForExistence(timeout: 90), "the wallet is created")
        capture("\(locale)-02f-created")
        tapWhenReady(enter)
        sleep(5)
        capture("\(locale)-02g-wallet")
    }

    /// The hand-off card's Open, the page's own button in its tab, the
    /// system's passkey sheet, and back.
    func onThePage(pageButton: String, shot: String) throws {
        let open = anyButton([words.openPage, "Continue to signing page"])
        XCTAssertTrue(open.waitForExistence(timeout: 30), "the hand-off card offers the page")
        capture("\(shot)-1-card")
        tapWhenReady(open)
        let button = anyButton([pageButton] + (english[pageButton].map { [$0] } ?? []))
        XCTAssertTrue(button.waitForExistence(timeout: 30), "the page offers \(pageButton)")
        sleep(1)
        capture("\(shot)-2-page")
        tapWhenReady(button)
        sleep(2)
        approvePasskeySheet(shot: "\(shot)-3")
        sleep(4)
        capture("\(shot)-4-back")
    }

    /// The dApp asks; the extension raises the wallet in a tab; `decide`
    /// taps what the wallet offers (the page included); the dApp hears.
    func ask(_ key: String, button: String, shot: String, decide: () throws -> Void) throws -> String {
        try serveDapp()
        if !safari.buttons[button].exists { openInSafari(dappURL) }
        let ask = safari.buttons[button]
        XCTAssertTrue(ask.waitForExistence(timeout: 20), "the dApp shows \(button)")
        tapWhenReady(ask)
        sleep(3)
        capture("\(shot)a-request")
        try decide()
        let heard = pageText(containing: "\(key):ok").exists
            || pageText(containing: "\(key):ok").waitForExistence(timeout: 120)
        sleep(1)
        capture("\(shot)z-dapp-heard")
        let line = safari.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "\(key):")).firstMatch
        let said = line.exists ? line.label : "(nothing)"
        try? said.write(to: (shots ?? URL(fileURLWithPath: NSTemporaryDirectory()))
            .appendingPathComponent("\(shot)-dapp-heard.txt"), atomically: true, encoding: .utf8)
        XCTAssertTrue(heard, "the dApp heard \(key):ok — it said \(said)")
        return said
    }

    func test03_connect() throws {
        _ = try ask("connect", button: "Connect", shot: "\(locale)-03") {
            XCTAssertNotNil(tapFirst(labels: [words.connect, "Connect"], timeout: 60))
        }
    }

    func test04_switchToGnosis() throws {
        _ = try ask("switch", button: "Switch to Gnosis", shot: "\(locale)-04") {}
    }

    func test05_personalSignOnThePage() throws {
        _ = try ask("sign", button: "Sign message", shot: "\(locale)-05") {
            // The wallet's own review, then its hand-off to the page.
            XCTAssertNotNil(tapFirst(labels: [words.confirm, "Confirm", words.openPage, "Continue to signing page"], timeout: 60))
            sleep(2)
            try onThePage(pageButton: words.sign, shot: "\(locale)-05b")
        }
    }

    func test06_sendOnThePage() throws {
        _ = try ask("send", button: "Send 0.001", shot: "\(locale)-06") {
            // The wallet's own review, then its hand-off to the page.
            if let tapped = tapFirst(labels: [words.confirmSend, "Confirm send", words.openPage, "Continue to signing page"], timeout: 60),
               tapped == words.confirmSend || tapped == "Confirm send" {
                sleep(2)
            }
            try onThePage(pageButton: words.confirmSend, shot: "\(locale)-06b")
        }
    }

    // MARK: - Helpers

    /// The page's words in English, by this run's words: a request tab opens
    /// in Safari's UI language (see the run notes), and the page follows the
    /// tab that opened it.
    private var english: [String: String] {
        [words.sign: "Sign", words.confirmSend: "Confirm send", words.addPasskey: "Add a passkey",
         words.confirm: "Confirm"]
    }

    /// The first of `labels` Safari shows as a button, waiting up to 30 s.
    func anyButton(_ labels: [String]) -> XCUIElement {
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline {
            for label in labels where safari.buttons[label].exists { return safari.buttons[label] }
            usleep(500_000)
        }
        return safari.buttons[labels[0]]
    }

    @discardableResult
    func tapFirst(labels: [String], timeout: TimeInterval = 30) -> String? {
        let start = Date()
        while Date().timeIntervalSince(start) < timeout {
            for label in labels {
                for element in [safari.buttons[label], safari.links[label]] where element.exists {
                    // Tapped where it is drawn, from one read of its frame: a
                    // web view re-renders under the runner, and an element
                    // resolved twice can be gone by the second time.
                    let frame = element.frame
                    guard frame.height > 0, element.isHittable || Date().timeIntervalSince(start) > 6 else { continue }
                    safari.coordinate(withNormalizedOffset: .zero)
                        .withOffset(CGVector(dx: frame.midX, dy: frame.midY)).tap()
                    return label
                }
            }
            usleep(500_000)
        }
        return nil
    }

    func element(beginningWith text: String) -> XCUIElement {
        safari.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@ AND elementType != %d", text,
                                  XCUIElement.ElementType.staticText.rawValue)).firstMatch
    }

    func tapFirst(containing text: String) {
        let found = safari.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(found.waitForExistence(timeout: 10), "\(text) is on screen")
        // Near its start: an agreement's row carries links (the policy, the
        // terms) that its middle can land on.
        let frame = found.frame
        safari.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: frame.minX + 12, dy: frame.minY + 12)).tap()
        sleep(1)
    }

    private func tapRow(in app: XCUIApplication, label: String) throws {
        let candidates = [app.buttons[label], app.cells[label], app.staticTexts[label]]
        for _ in 0..<8 {
            if let row = candidates.first(where: { $0.exists && $0.isHittable }) {
                row.tap()
                sleep(1)
                return
            }
            app.swipeUp()
        }
        capture("missing-\(label)", app)
        XCTFail("no row \(label)")
    }

    /// The system's passkey sheet: Continue / Save, then Face ID (matched by the host).
    /// The system's passkey sheet (AuthenticationServices' own process): its
    /// one primary button — "Add Passkey" to make a key, "Continue" to use
    /// one — then Face ID, which the host matches. Found by label in the
    /// agent's process; where the runner cannot see that process, tapped
    /// where the sheet draws it (the bottom sheet's primary button).
    func approvePasskeySheet(shot: String, timeout: TimeInterval = 25) {
        let agent = XCUIApplication(bundleIdentifier: "com.apple.AuthenticationServicesUI")
        let labels = ["Add Passkey", "Continue", "Sign In", "Save", "添加通行密钥", "继续", "登录", "存储"]
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            // The sheet's primary button, by the identifier AuthenticationServices gives it.
            let primary = agent.descendants(matching: .any).matching(identifier: "ASAuthorizationControllerContinueButton").firstMatch
            if primary.exists {
                capture("\(shot)-sheet", agent)
                primary.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
                sleep(3)
                return
            }
            for app in [agent, springboard] {
                for label in labels {
                    let button = app.buttons[label]
                    if button.exists {
                        capture("\(shot)-sheet", app)
                        if button.isHittable {
                            button.tap()
                        } else {
                            button.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
                        }
                        sleep(3)
                        return
                    }
                }
            }
            // The sheet is up when Safari's page is dimmed under it and the
            // runner sees no button: tap its primary button by position.
            if Date().addingTimeInterval(timeout - 15) > deadline {
                capture("\(shot)-sheet", safari)
                capture("\(shot)-sheet-agent", agent)
                XCUIApplication(bundleIdentifier: "com.apple.springboard")
                    .coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.8625)).tap()
                sleep(3)
                return
            }
            usleep(500_000)
        }
    }

    func tapWhenReady(_ element: XCUIElement, timeout: TimeInterval = 30) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline, !(element.exists && element.isHittable) { usleep(500_000) }
        let frame = element.frame
        XCUIApplication(bundleIdentifier: "com.apple.springboard").coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: frame.midX, dy: frame.midY)).tap()
    }

    func openInSafari(_ url: URL) {
        XCUIDevice.shared.system.open(url)
        XCTAssertTrue(safari.wait(for: .runningForeground, timeout: 15))
        sleep(2)
        for label in ["Continue", "Not Now", "Close"] where safari.buttons[label].exists {
            safari.buttons[label].tap()
        }
    }

    func pageText(containing text: String) -> XCUIElement {
        safari.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
    }
}
