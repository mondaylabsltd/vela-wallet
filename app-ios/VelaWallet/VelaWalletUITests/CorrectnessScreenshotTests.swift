//
//  CorrectnessScreenshotTests.swift
//  VelaWalletUITests
//
//  PR 2's screens, in zh and en, light and dark — looked at, not asserted
//  pixel by pixel:
//
//  - `testHiddenBalanceLive`: the LIVE app with the balance hidden (the
//    stored `vela.balanceHidden`), over a real address's holdings and seeded
//    transfers — home, Assets, a token's detail, History, a transfer's
//    detail, and the account switcher. Every figure the person owns is the
//    mask; Send keeps its own. Needs the network (it reads real balances).
//  - `testBoards`: `VELA_PAGE=pr2` boards drawn by the production builders —
//    the held confirm (sheet and Send), the fee coin switched (provisional →
//    settled), the fee row "chain down" vs "internal", and a refusal told by
//    its reason (sheet and Send); and the onboarding gallery's "can't look up
//    your wallet" prompt, both forms.
//
//  Simulator only (a seeded read-only account); skipped in the scheme.
//

import XCTest

final class CorrectnessScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = true
        try skipOnDeviceForSeededAccount()
    }

    private static let langs = ["zh", "en"]
    private static let themes = ["light", "dark"]

    // MARK: - Boards

    func testBoards() {
        let boards = [
            "held-sheet", "held-send", "fee-provisional", "fee-settled",
            "fee-chain-down", "fee-internal", "refused-sheet", "refused-send",
        ]
        for lang in Self.langs {
            for theme in Self.themes {
                for board in boards {
                    let app = XCUIApplication()
                    app.launchEnvironment["VELA_PAGE"] = "pr2"
                    app.launchEnvironment["VELA_STATE"] = board
                    pin(app, lang: lang, theme: theme)
                    app.launch()
                    settle(2.5)
                    attach(app.screenshot(), named: "\(board)-\(lang)-\(theme)")
                    app.terminate()
                }
                for (fixture, name) in [
                    ("registry unreachable", "registry-unreachable"),
                    ("registry unreachable · offline", "registry-unreachable-offline"),
                ] {
                    let app = XCUIApplication()
                    app.launchEnvironment["VELA_GALLERY"] = "1"
                    app.launchEnvironment["VELA_GALLERY_FIXTURE"] = fixture
                    pin(app, lang: lang, theme: theme)
                    app.launch()
                    settle(2.5)
                    attach(app.screenshot(), named: "\(name)-\(lang)-\(theme)")
                    app.terminate()
                }
            }
        }
    }

    // MARK: - The live app, hidden

    /// A real address with holdings on several networks, read-only.
    private static let me = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"

    private static func history(now: Int) -> [[String: Any]] {
        [
            [
                "id": "pr2-sent", "userOpHash": "0x" + String(repeating: "5e", count: 32),
                "txHash": "0x" + String(repeating: "ab", count: 32),
                "from": me, "to": "0xdddddddddddddddddddddddddddddddddddddddd",
                "value": "163.25", "symbol": "USDC", "decimals": 6, "chainId": 1,
                "timestamp": now - 3_600, "status": "confirmed", "type": "send", "usd": "163.25",
            ],
            [
                "id": "pr2-received", "userOpHash": "", "txHash": "0x" + String(repeating: "ad", count: 32),
                "from": "0xcccccccccccccccccccccccccccccccccccccccc", "to": me,
                "value": "289.5", "symbol": "USDT", "decimals": 6, "chainId": 1,
                "timestamp": now - 600, "status": "confirmed", "type": "receive", "usd": "289.5",
            ],
        ]
    }

    func testHiddenBalanceLive() {
        for lang in Self.langs {
            for theme in Self.themes {
                walkHidden(lang: lang, theme: theme)
            }
        }
    }

    /// The same address shown, once: what the masks stand for.
    func testShownControl() {
        let app = launchLive(lang: "en", theme: "light", hidden: false)
        XCTAssertTrue(app.staticTexts["Assets"].firstMatch.waitForExistence(timeout: 40))
        settle(15)
        attach(app.screenshot(), named: "shown-home-en-light")
        app.terminate()
    }

    private func launchLive(lang: String, theme: String, hidden: Bool) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0"]
        if hidden { app.launchArguments += ["-vela.balanceHidden", "1"] }
        app.launchArguments += [
            "-vela.transactionHistory", Self.argument(Self.history(now: Int(Date().timeIntervalSince1970))),
        ]
        app.launchEnvironment["VELA_ACCOUNT"] = Self.me
        pin(app, lang: lang, theme: theme)
        app.launch()
        return app
    }

    private func walkHidden(lang: String, theme: String) {
        let zh = lang == "zh"
        let tag = "\(lang)-\(theme)"
        let app = launchLive(lang: lang, theme: theme, hidden: true)

        let assetsTitle = app.staticTexts[zh ? "资产" : "Assets"].firstMatch
        XCTAssertTrue(assetsTitle.waitForExistence(timeout: 40), "the home never appeared (\(tag))")
        // The holdings, read from the chains.
        settle(15)
        attach(app.screenshot(), named: "hidden-home-\(tag)")
        app.swipeUp()
        settle(1)
        attach(app.screenshot(), named: "hidden-home-scrolled-\(tag)")
        app.swipeDown()
        settle(1)

        // The two sections' "All": Activity's above, Assets' below.
        func sectionAll(lowest: Bool) -> XCUIElement? {
            let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", zh ? "全部" : "All"))
                .allElementsBoundByIndex.filter { $0.isHittable }
            let sorted = all.sorted { $0.frame.minY < $1.frame.minY }
            return lowest ? sorted.last : sorted.first
        }

        // Assets → a token's detail.
        if let all = sectionAll(lowest: true) {
            all.tap()
            settle(2)
            attach(app.screenshot(), named: "hidden-assets-\(tag)")
            let eth = app.staticTexts["ETH"].firstMatch
            if eth.waitForExistence(timeout: 5) {
                eth.tap()
                settle(2)
                attach(app.screenshot(), named: "hidden-token-detail-\(tag)")
                app.swipeDown(velocity: .fast)
                settle(1.5)
            }
            back(app)
        }

        // History → a transfer's detail.
        if let all = sectionAll(lowest: false) {
            all.tap()
            settle(2)
            attach(app.screenshot(), named: "hidden-history-\(tag)")
            let sent = app.staticTexts[zh ? "已发送" : "Sent"].firstMatch
            if sent.waitForExistence(timeout: 5) {
                sent.tap()
                settle(2)
                attach(app.screenshot(), named: "hidden-transfer-detail-\(tag)")
                app.swipeDown(velocity: .fast)
                settle(1.5)
            }
            back(app)
        }

        // The account switcher, from the name line.
        let name = app.staticTexts["Dev (read-only)"].firstMatch
        if name.waitForExistence(timeout: 5) {
            name.tap()
            settle(2)
            attach(app.screenshot(), named: "hidden-switcher-\(tag)")
        }
        app.terminate()
    }

    // MARK: - Plumbing

    private func pin(_ app: XCUIApplication, lang: String, theme: String) {
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(en)"]
    }

    /// The flow's chevron, top left (it carries no label of its own).
    private func back(_ app: XCUIApplication) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.111)).tap()
        settle(1.5)
    }

    private static func argument(_ records: [[String: Any]]) -> String {
        let json = String(data: try! JSONSerialization.data(withJSONObject: records), encoding: .utf8)!
        let escaped = json
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return "\"\(escaped)\""
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
