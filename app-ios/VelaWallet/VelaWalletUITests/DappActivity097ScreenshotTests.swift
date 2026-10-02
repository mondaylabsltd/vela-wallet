//
//  DappActivity097ScreenshotTests.swift
//  VelaWalletUITests
//
//  Spec 097 part B — the second real-money pass's rows in Activity, seen in
//  the real app: a USDC→BNB swap that sent no coin, an Aave borrow whose USDC
//  the scan also recorded, and a withdraw the network refused — stored as the
//  tracker closed them (`settlement`), handed to the app through the ARGUMENT
//  domain (`-vela.transactionHistory`), read by the real feed machine and
//  drawn by the live home. Then each one's detail.
//
//      xcodebuild test … -only-testing:VelaWalletUITests/DappActivity097ScreenshotTests \
//        -resultBundlePath /tmp/097.xcresult
//      xcrun xcresulttool export attachments --path /tmp/097.xcresult --output-path /tmp/097
//
//  A key-less read-only account (`VELA_ACCOUNT`, FR-010) at an address nobody
//  uses, so no live receipt lands among the fixtures.
//

import XCTest

final class DappActivity097ScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private static let me = "0x0970970970970970970970970970970970970970"
    private static let usdc = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"
    private static let pool = "0x6807dc923806fe8fd134338eabca509979a7e0cb"
    private static let router = "0xd9c500dff816a1da21a48a732d3498bf09dc9aeb"
    private static let permit2 = "0x31c2f6fcff4f8759b3bd5bf0e1084a055615c768"

    /// The pass's rows, as the shells store them once the tracker closed them.
    private static func history(now: Int) -> [[String: Any]] {
        let borrowTx = "0x" + String(repeating: "cb", count: 32)
        func dapp(_ id: String, _ at: Int, _ extra: [String: Any]) -> [String: Any] {
            var row: [String: Any] = [
                "id": id, "userOpHash": "0x" + String(repeating: String(format: "%02x", id.count), count: 32),
                "txHash": "", "from": me, "to": pool, "value": "0x0", "symbol": "BNB", "decimals": 18,
                "chainId": 56, "timestamp": at, "status": "confirmed", "type": "dapp_tx",
                "dappOrigin": "https://app.aave.com", "dappUrl": "https://app.aave.com",
                "requestTruncated": false,
            ]
            row.merge(extra) { $1 }
            return row
        }
        return [
            dapp("dapp-swap-tx", now - 300, [
                "to": usdc, "txHash": "0x" + String(repeating: "3c", count: 32),
                "dappOrigin": "https://pancakeswap.finance", "dappUrl": "https://pancakeswap.finance",
                "intent": "Swap",
                "signedRequest": #"[{"calls":[{"to":"\#(usdc)","data":"0x095ea7b3"},{"to":"\#(router)","data":"0x3593564c"}]}]"#,
                "dappSummary": [
                    "action": "batch", "calls": 3, "contract": router, "spender": permit2, "token": usdc,
                    "amount": "1160000000000000000",
                    "tokens": [["address": usdc, "symbol": "USDC", "decimals": 18]],
                ],
                "balanceChanges": [
                    ["type": "erc20_trusted", "token": usdc, "delta": "-1160000000000000000",
                     "symbol": "USDC", "decimals": 18, "in_trusted_set": true],
                ],
                "settlement": ["moved": [
                    ["token": usdc, "delta": "-1160000000000000000"],
                    ["token": NSNull(), "delta": "1499036349071560"],
                ]],
            ]),
            dapp("dapp-borrow-tx", now - 200, [
                "txHash": borrowTx, "intent": "Borrow",
                "signedRequest": #"[{"to":"\#(pool)","data":"0xa415bcad"}]"#,
                "dappSummary": ["action": "call", "calls": 1, "contract": pool],
                "settlement": ["moved": [
                    ["token": "0xcdbbed5606d9c5c98eeedd67933991dc17f0c68d", "delta": "300000000000000001"],
                    ["token": usdc, "delta": "300000000000000000"],
                ]],
            ]),
            [
                "id": "rx-borrow", "userOpHash": "", "txHash": borrowTx, "from": pool, "to": me,
                "value": "0.3", "symbol": "USDC", "decimals": 18, "chainId": 56,
                "timestamp": now - 190, "status": "confirmed", "type": "receive",
            ],
            dapp("dapp-withdraw-tx", now - 100, [
                "status": "failed", "intent": "Withdraw",
                "signedRequest": #"[{"to":"\#(pool)","data":"0x69328dec"}]"#,
                "dappSummary": ["action": "call", "calls": 1, "contract": pool],
                "settlement": ["failure": "refused"],
            ]),
        ]
    }

    /// The shelf's JSON text as an argument-domain value: an old-style plist
    /// quoted string, so it reaches `UserDefaults.string(forKey:)` as text.
    private static func argument(_ records: [[String: Any]]) -> String {
        let json = String(data: try! JSONSerialization.data(withJSONObject: records), encoding: .utf8)!
        let escaped = json
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return "\"\(escaped)\""
    }

    private struct Words {
        let lang: String
        let swap: String
        let borrow: String
        let withdraw: String
        let technical: String
    }

    func testThePassRowsAndTheirDetails() throws {
        let languages = [
            Words(lang: "en", swap: "Swap on PancakeSwap", borrow: "Borrow on Aave",
                  withdraw: "Withdraw on Aave", technical: "Technical details"),
            Words(lang: "zh", swap: "在 PancakeSwap 兑换", borrow: "在 Aave 借入",
                  withdraw: "在 Aave 取出", technical: "技术细节"),
        ]
        for words in languages {
            let app = launch(words.lang)
            XCTAssertTrue(app.staticTexts[words.borrow].waitForExistence(timeout: 40),
                          "the borrow row never appeared")
            XCTAssertTrue(app.staticTexts[words.swap].exists, "the swap row is missing")
            XCTAssertTrue(app.staticTexts[words.withdraw].exists, "the failed row is missing")
            Thread.sleep(forTimeInterval: 2)
            attach(app.screenshot(), named: "\(words.lang)-097-activity")
            app.terminate()

            for (title, name) in [(words.swap, "swap"), (words.borrow, "borrow"), (words.withdraw, "failed")] {
                let again = launch(words.lang)
                tap(again.staticTexts[title].firstMatch, "the \(name) row", timeout: 40)
                XCTAssertTrue(again.staticTexts[words.technical].waitForExistence(timeout: 10),
                              "the \(name) detail never opened")
                Thread.sleep(forTimeInterval: 1)
                attach(again.screenshot(), named: "\(words.lang)-097-\(name)-detail")
                again.terminate()
            }
        }
    }

    /// The live app over the pass's records, in `lang`.
    private func launch(_ lang: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchArguments += [
            "-vela.transactionHistory",
            Self.argument(Self.history(now: Int(Date().timeIntervalSince1970))),
        ]
        app.launchEnvironment["VELA_ACCOUNT"] = Self.me
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = "light"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", "(\(lang))"]
        app.launch()
        return app
    }

    // MARK: - Plumbing

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
