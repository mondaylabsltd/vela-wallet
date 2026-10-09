//
//  PR2PolishDeviceTests.swift
//  VelaWalletUITests
//
//  The correctness batch's walk on a phone (PR 2 polish round). Every test is
//  read-only towards the person's real wallet: account-touching steps run in
//  the parallel space's test wallet, nothing is ever confirmed, signed or
//  sent — each dApp request is declined with the sheet's ✕ — and every
//  setting a test moves (text size, language, hide balance) is put back to
//  what it read first.
//
//  - `testState` — no launch pins at all: the phone as the person left it,
//    shot and dumped (home, the account switcher, Settings top to bottom).
//    Run first and last; the two must match.
//  - `testIssue483Walk` — the reporter's trigger (issue #483): a dApp
//    request prices; text size one step up, the request again; the language
//    changed and back, the request again. Each fee must price; every request
//    is declined. Before the fix the second request sat on "点击重试" for
//    good, under "正在计算网络费用…".
//  - `testInternalFault` — `-vela.faultPool 1,100` (DEBUG, the argument
//    domain: gone with the next launch): the home never reads "Can't reach
//    Ethereum", and a dApp request's fee row and the line under its confirm
//    name the same cause while the core retries by itself.
//  - `testHiddenBalance` — hide balance on: every money surface masked; Send,
//    the signing sheet and Receive still show figures; hide balance back.
//  - `testLeaveSpace` — `VELA_PARALLEL_SPACE=0`: back to the wallet that was
//    in front.
//
//  Skipped in the scheme (they need the phone, the network and the space);
//  run them by name from a copy of the .xctestrun with the skips removed.
//

import XCTest

final class PR2PolishDeviceTests: XCTestCase {

    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    // MARK: - The phone as the person left it

    func testState() throws {
        let label = ProcessInfo.processInfo.environment["STATE_LABEL"] ?? "state"
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        settle(6)
        shoot(app, "\(label)-1-home")
        dump(app, "\(label)-1-home")

        // The account switcher: the rows and the total.
        openSwitcher(app)
        settle(2)
        shoot(app, "\(label)-2-switcher")
        dump(app, "\(label)-2-switcher")
        dismissSheet(app)

        // Settings, top to bottom.
        tapTab(["设置", "Settings"], in: app)
        settle(2)
        for page in 0..<5 {
            shoot(app, "\(label)-3-settings-\(page)")
            dump(app, "\(label)-3-settings-\(page)")
            swipePage(app, from: 0.75, to: 0.3)
            settle(1)
        }
        for _ in 0..<5 { swipePage(app, from: 0.3, to: 0.8) }
        tapTab(["钱包", "Wallet"], in: app)
        settle(1)
        shoot(app, "\(label)-4-home-again")
        app.terminate()
    }

    // MARK: - Issue #483: the trigger that broke it

    func testIssue483Walk() throws {
        startServer()
        let app = launchInSpace()
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)

        // 1. The control: before any Settings change the fee prices.
        let first = requestAndReadFee(app, shot: "483-1-before")
        XCTAssertTrue(first.priced, "the first request's fee did not price: \(first)")
        decline(app)

        // 2. Text size one stop up from where the person keeps it.
        openSettings(app)
        let slider = textSizeSlider(app)
        let read = Int(slider.value as? String ?? "") ?? 3
        // The person's own size, recorded before the walk (`testState`):
        // a re-run after a failed one must not take a moved slider for it.
        let kept = Int(ProcessInfo.processInfo.environment["KEPT_TEXT_SIZE"] ?? "") ?? read
        note("483-2-text-size-kept", "slider value \(read), kept \(kept)")
        if read != kept {
            tap(stop: kept - 1, of: slider, in: app)
            settle(1.5)
        }
        tap(stop: kept, of: textSizeSlider(app), in: app) // 1-based value → the next 0-based stop
        settle(1.5)
        note("483-2-text-size-now", "slider value \(textSizeSlider(app).value ?? "nil")")
        shoot(app, "483-2-text-size-up")

        openExplore(app)
        let afterSize = requestAndReadFee(app, shot: "483-3-after-text-size")
        XCTAssertTrue(afterSize.priced, "after a text-size change the dApp fee did not price: \(afterSize)")
        XCTAssertFalse(afterSize.stuck, "the fee row is stuck on Tap to retry: \(afterSize)")
        decline(app)

        // 3. The language: English, then back to 简体中文.
        openSettings(app)
        pickLanguage("English", in: app)
        settle(2)
        shoot(app, "483-4-language-english")
        pickLanguage("简体中文", in: app)
        settle(2)
        shoot(app, "483-5-language-back")

        openExplore(app)
        let afterLanguage = requestAndReadFee(app, shot: "483-6-after-language")
        XCTAssertTrue(afterLanguage.priced, "after a language change the dApp fee did not price: \(afterLanguage)")
        XCTAssertFalse(afterLanguage.stuck, "the fee row is stuck on Tap to retry: \(afterLanguage)")
        decline(app)

        // 4. The text size back where it was.
        openSettings(app)
        tap(stop: kept - 1, of: textSizeSlider(app), in: app)
        settle(1.5)
        let restored = Int(textSizeSlider(app).value as? String ?? "") ?? -1
        note("483-7-text-size-restored", "slider value \(restored)")
        XCTAssertEqual(restored, kept, "the text size was not put back")
        shoot(app, "483-7-restored")
        app.terminate()
    }

    // MARK: - An internal fault

    func testInternalFault() throws {
        startServer()
        let app = launchInSpace(extraArguments: ["-vela.faultPool", "1,100"])
        settle(15)
        shoot(app, "fault-1-home")
        dump(app, "fault-1-home")
        let blamed = app.staticTexts.containing(NSPredicate(
            format: "label CONTAINS %@ OR label CONTAINS %@ OR label CONTAINS %@",
            "暂时连不上 Ethereum", "连不上 Ethereum", "Can't reach Ethereum"
        ))
        XCTAssertEqual(blamed.count, 0, "the home blamed Ethereum for a fault inside Vela")

        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)
        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "no sheet")
        // Through two of the core's own re-asks (3 s, then 6 s).
        for second in [4, 12, 20] {
            settle(second == 4 ? 4 : 8)
            let reading = read(app)
            shoot(app, "fault-2-sheet-\(second)s")
            note("fault-2-sheet-\(second)s-reading", reading.description)
            XCTAssertFalse(reading.priced, "a faulted pool priced a fee: \(reading)")
            if let footer = reading.footer {
                XCTAssertFalse(footer.contains("点费用重试") || footer.contains("Tap it"),
                               "the footer asks for a tap while the core retries: \(reading)")
            }
        }
        decline(app)
        app.terminate()

        // The fault was the argument domain's: a plain launch reads again.
        let healed = launchInSpace()
        settle(10)
        shoot(healed, "fault-3-healed-home")
        healed.terminate()
    }

    // MARK: - Hide balance

    func testHiddenBalance() throws {
        startServer()
        let app = launchInSpace()
        settle(8)
        shoot(app, "hide-0-shown")
        let shownHero = heroLabel(app)
        note("hide-0-hero", shownHero ?? "nil")

        // On: tap the hero.
        toggleHero(app)
        settle(2)
        shoot(app, "hide-1-home")
        dump(app, "hide-1-home")
        XCTAssertTrue(app.descendants(matching: .any)["显示余额"].exists
                      || app.descendants(matching: .any)["Show balance"].exists,
                      "the hero is not hidden")
        swipePage(app, from: 0.8, to: 0.35)
        settle(1)
        shoot(app, "hide-2-home-lower")
        dump(app, "hide-2-home-lower")

        // Holdings → a token's detail.
        let firstAsset = app.cells.firstMatch
        _ = firstAsset
        if tapFirst(app, labels: ["xDAI", "USDC", "ETH"]) {
            settle(2)
            shoot(app, "hide-3-token-detail")
            dump(app, "hide-3-token-detail")
            goBack(app)
        }
        swipePage(app, from: 0.35, to: 0.85)
        settle(1)

        // Activity → a transfer's detail.
        if tapFirst(app, labels: ["已发送", "Sent"]) {
            settle(2)
            shoot(app, "hide-4-transfer-detail")
            dump(app, "hide-4-transfer-detail")
            goBack(app)
        }

        // The account switcher: rows and total.
        openSwitcher(app)
        settle(2)
        shoot(app, "hide-5-switcher")
        dump(app, "hide-5-switcher")
        dismissSheet(app)

        // Visible on purpose: Send, Receive, the signing sheet.
        if tapFirst(app, labels: ["转账", "Send"]) {
            settle(3)
            shoot(app, "hide-6-send")
            dump(app, "hide-6-send")
            goBack(app)
        }
        if tapFirst(app, labels: ["收款", "Receive"]) {
            settle(3)
            shoot(app, "hide-7-receive")
            dump(app, "hide-7-receive")
            goBack(app)
        }
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)
        let reading = requestAndReadFee(app, shot: "hide-8-sheet")
        dump(app, "hide-8-sheet")
        XCTAssertTrue(reading.priced, "the sheet's fee is a figure while hidden: \(reading)")
        decline(app)

        // Off again, as it was.
        tapTab(["钱包", "Wallet"], in: app)
        settle(2)
        for _ in 0..<3 { swipePage(app, from: 0.3, to: 0.85) }
        toggleHero(app)
        settle(2)
        shoot(app, "hide-9-shown-again")
        XCTAssertFalse(app.descendants(matching: .any)["显示余额"].exists, "hide balance was not turned off")
        app.terminate()
    }

    // MARK: - Out of the space

    func testLeaveSpace() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launch()
        settle(6)
        shoot(app, "leave-home")
        app.terminate()
    }

    // MARK: - The fee, as drawn

    private struct FeeReading: CustomStringConvertible {
        let value: String?
        let reason: String?
        let footer: String?
        var priced: Bool { value?.hasPrefix("~") == true }
        var stuck: Bool { value == "点击重试" || value == "Tap to retry" }
        var description: String {
            "value=\(value ?? "nil") reason=\(reason ?? "nil") footer=\(footer ?? "nil")"
        }
    }

    /// Send dust, then wait up to 40 s for the fee row to settle.
    private func requestAndReadFee(_ app: XCUIApplication, shot: String) -> FeeReading {
        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.buttons["signing.close"].waitForExistence(timeout: 30), "the signing sheet never opened")
        let deadline = Date().addingTimeInterval(40)
        var reading = read(app)
        while Date() < deadline, !reading.priced {
            settle(1)
            reading = read(app)
        }
        shoot(app, shot)
        note(shot + "-reading", reading.description)
        return reading
    }

    private func read(_ app: XCUIApplication) -> FeeReading {
        let figure = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "~")).firstMatch
        let words = ["点击重试", "Tap to retry", "估算中...", "Estimating...", "换一种币付费", "Pay with another coin"]
        var value: String? = figure.exists ? figure.label : nil
        if value == nil {
            for word in words where app.staticTexts[word].firstMatch.exists {
                value = word
                break
            }
        }
        let reason = app.staticTexts["signing.fee.reason"].firstMatch
        let footer = app.staticTexts["signing.confirmBlock"].firstMatch
        return FeeReading(
            value: value,
            reason: reason.exists ? reason.label : nil,
            footer: footer.exists ? footer.label : nil
        )
    }

    /// The sheet's ✕: the page hears 4001, nothing is signed.
    private func decline(_ app: XCUIApplication) {
        let close = app.buttons["signing.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 10), "the sheet has no ✕")
        close.tap()
        XCTAssertTrue(
            waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001", timeout: 30),
            "closing the sheet did not refuse the page"
        )
    }

    // MARK: - Plumbing

    private func startServer() {
        guard server == nil else { return }
        do {
            let server = try LocalDappServer(html: try LocalDappServer.page())
            server.start()
            self.server = server
        } catch {
            XCTFail("the local dApp server: \(error)")
        }
    }

    private func launchInSpace(extraArguments: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += extraArguments
        app.launch()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30), "not in the parallel space")
        return app
    }

    private func openExplore(_ app: XCUIApplication) {
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 3) { return }
        tapTab(["探索", "Explore"], in: app)
        let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
        if row.waitForExistence(timeout: 8) { row.tap() }
    }

    private func connect(_ app: XCUIApplication) {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons.matching(
            NSPredicate(format: "label == %@ OR label == %@ OR label == %@ OR label == %@",
                        "连接", "批准", "Connect", "Approve")
        ).firstMatch
        if approve.waitForExistence(timeout: 12) { approve.tap() }
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_requestAccounts ok"),
                      "the page was never connected")
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")
    }

    private func waitForVerdict(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if lines.contains(where: { $0.label.contains(fragment) }) { return true }
            settle(1)
        }
        return false
    }

    private func openSettings(_ app: XCUIApplication) {
        tapTab(["设置", "Settings"], in: app)
        settle(2)
    }

    /// Settings → Language → `name`.
    private func pickLanguage(_ name: String, in app: XCUIApplication) {
        tapTab(["设置", "Settings"], in: app)
        settle(1)
        let row = app.staticTexts.matching(NSPredicate(format: "label == %@ OR label == %@", "语言", "Language")).firstMatch
        reachElement(row, in: app)
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no Language row")
        row.tap()
        settle(1.5)
        // Each language is a row button in the sheet (the Settings row
        // behind it carries the current one's name as a static text).
        let option = app.buttons[name].firstMatch
        guard option.waitForExistence(timeout: 8) else {
            XCTFail("no \(name) in the language sheet")
            shoot(app, "483-language-sheet-missing-\(name)")
            dismissSheet(app)
            return
        }
        shoot(app, "483-language-sheet-\(name)")
        option.tap()
        settle(2.5)
        // The sheet may stay up: close it if it did.
        if app.buttons[name].firstMatch.exists {
            dismissSheet(app)
        }
    }

    private func textSizeSlider(_ app: XCUIApplication) -> XCUIElement {
        let slider = app.descendants(matching: .any)["text-scale-slider"].firstMatch
        reachElement(slider, in: app)
        XCTAssertTrue(slider.waitForExistence(timeout: 8), "no text-size slider on Settings")
        return slider
    }

    private func reachElement(_ element: XCUIElement, in app: XCUIApplication) {
        let screen = app.frame.height
        var swipes = 0
        while swipes < 8 {
            if element.exists {
                let frame = element.frame
                if frame.maxY > screen * 0.75 {
                    swipePage(app, from: 0.55, to: 0.3)
                } else if frame.minY < screen * 0.2 {
                    swipePage(app, from: 0.3, to: 0.55)
                } else {
                    break
                }
            } else {
                swipePage(app, from: 0.6, to: 0.35)
            }
            swipes += 1
            settle(0.8)
        }
    }

    /// The slider's stops are six (0…5); `stop` is 0-based.
    private func tap(stop: Int, of slider: XCUIElement, in app: XCUIApplication) {
        reachElement(slider, in: app)
        let width = slider.frame.width
        let trackStart: CGFloat = 9 + 4
        let track = width - trackStart - 4 - 14
        let x = trackStart + 22 + (track - 44) * CGFloat(stop) / 5
        slider.coordinate(withNormalizedOffset: CGVector(dx: x / width, dy: 0.5)).tap()
    }

    /// The hero's figure reads "¥1.49 CNY"; its caption "总余额 · CNY" is not it.
    private static let heroFigure = NSPredicate(
        format: "label ENDSWITH %@ AND NOT (label CONTAINS %@)", " CNY", "·"
    )

    private func heroLabel(_ app: XCUIApplication) -> String? {
        let hero = app.descendants(matching: .any).matching(Self.heroFigure).firstMatch
        return hero.exists ? hero.label : nil
    }

    /// The hero figure is the hide-balance toggle.
    private func toggleHero(_ app: XCUIApplication) {
        let hidden = app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@ OR label == %@", "显示余额", "Show balance")
        ).firstMatch
        if hidden.exists {
            hidden.tap()
            return
        }
        let shown = app.descendants(matching: .any).matching(Self.heroFigure).firstMatch
        if shown.exists {
            shown.tap()
        } else {
            XCTFail("no hero figure to toggle")
        }
    }

    private func tapFirst(_ app: XCUIApplication, labels: [String]) -> Bool {
        for label in labels {
            let element = app.staticTexts[label].firstMatch
            if element.exists, element.isHittable {
                element.tap()
                return true
            }
            let button = app.buttons[label].firstMatch
            if button.exists, button.isHittable {
                button.tap()
                return true
            }
        }
        return false
    }

    private func goBack(_ app: XCUIApplication) {
        let back = app.navigationBars.buttons.firstMatch
        if back.exists, back.isHittable {
            back.tap()
        } else {
            // The edge swipe back.
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.01, dy: 0.5))
                .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.8, dy: 0.5)))
        }
        settle(1.5)
    }

    private func openSwitcher(_ app: XCUIApplication) {
        // The account name at the top left opens the switcher.
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.08)).tap()
    }

    /// A sheet goes down by its grabber — never a tap on a row (a row's ✕
    /// removes an account from the device).
    private func dismissSheet(_ app: XCUIApplication) {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.07))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98))
        start.press(forDuration: 0.05, thenDragTo: end)
        settle(1.5)
    }

    private func tapTab(_ labels: [String], in app: XCUIApplication) {
        for label in labels {
            let button = app.buttons[label]
            if button.exists {
                button.tap()
                return
            }
            let matches = app.staticTexts.matching(identifier: label)
            if matches.count > 0 {
                matches.element(boundBy: matches.count - 1).tap()
                return
            }
        }
        XCTFail("no tab called any of \(labels)")
    }

    private func swipePage(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)))
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func shoot(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func dump(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(string: app.debugDescription)
        attachment.name = name + "-tree"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func note(_ name: String, _ text: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
