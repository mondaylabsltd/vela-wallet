//
//  DappActivityScreenshotTests.swift
//  VelaWalletUITests
//
//  Spec 093 — a dApp's swap, permit and sign-in in Activity, seen in the real
//  app: three records the way the signing path stores them, handed to the app
//  through the ARGUMENT domain (`-vela.transactionHistory`, which outranks the
//  stored shelf without writing to it), read by the real feed machine and
//  drawn by the live home. Then the permit's detail, with its technical
//  details opened — the stored request read from the shelf by record id.
//
//      xcodebuild test … -only-testing:VelaWalletUITests/DappActivityScreenshotTests \
//        -resultBundlePath /tmp/093.xcresult
//      xcrun xcresulttool export attachments --path /tmp/093.xcresult --output-path /tmp/093
//
//  A key-less read-only account (`VELA_ACCOUNT`, FR-010) at an address nobody
//  uses, so no live receipt lands among the fixtures.
//

import XCTest

final class DappActivityScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private static let me = "0x0930930930930930930930930930930930930930"
    private static let router = "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad"
    private static let permit2 = "0x000000000022d473030f116ddee9f6b43ac78ba3"
    private static let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    private static let site = "https://app.uniswap.org"

    /// The three records, as `SignExecutor.recordRow` writes them.
    private static func history(now: Int) -> [[String: Any]] {
        let typedData = #"{"types":{"PermitSingle":[{"name":"details","type":"PermitDetails"},{"name":"spender","type":"address"},{"name":"sigDeadline","type":"uint256"}]},"primaryType":"PermitSingle","domain":{"name":"Permit2","chainId":1,"verifyingContract":"\#(permit2)"},"message":{"details":{"token":"\#(usdc)","amount":"1461501637330902918203684832716283019655932542975","expiration":"281474976710655","nonce":"0"},"spender":"\#(router)","sigDeadline":"1759999999"}}"#
        let permitRequest = String(
            data: try! JSONSerialization.data(withJSONObject: [me, typedData]), encoding: .utf8
        )!
        let signInRequest = #"["0x6170702e756e69737761702e6f72672077616e747320796f7520746f207369676e20696e","\#(me)"]"#
        return [
            [
                "id": "dapp-swap-tx", "userOpHash": "0x" + String(repeating: "ab", count: 32),
                "txHash": "0x" + String(repeating: "7e", count: 32), "from": me, "to": router,
                "value": "0x0", "symbol": "ETH", "decimals": 18, "chainId": 1, "timestamp": now,
                "status": "confirmed", "type": "dapp_tx", "dappOrigin": site, "dappUrl": site,
                "signedRequest": #"[{"to":"\#(router)","value":"0x0","data":"0x3593564c"}]"#,
                "requestTruncated": false, "intent": "Swap",
                "dappSummary": ["action": "call", "calls": 1, "contract": router],
                "balanceChanges": [
                    ["type": "erc20_trusted", "token": usdc, "delta": "-100000000",
                     "symbol": "USDC", "decimals": 6, "in_trusted_set": true],
                    ["type": "native", "delta": "30000000000000000"],
                ],
            ],
            [
                "id": "dapp-permit-sig", "userOpHash": "", "txHash": "", "from": me, "to": "",
                "value": "0", "symbol": "", "decimals": 0, "chainId": 1, "timestamp": now - 60,
                "status": "confirmed", "type": "sign_typed_data", "dappOrigin": site, "dappUrl": site,
                "signedRequest": permitRequest, "requestTruncated": false,
                "dappSummary": [
                    "action": "permit", "contract": permit2, "spender": router, "token": usdc,
                    "symbol": "USDC", "decimals": 6, "unlimited": true, "primary_type": "PermitSingle",
                ],
            ],
            [
                "id": "dapp-siwe-msg", "userOpHash": "", "txHash": "", "from": me, "to": "",
                "value": "0", "symbol": "", "decimals": 0, "chainId": 1, "timestamp": now - 120,
                "status": "confirmed", "type": "sign_message", "dappOrigin": site, "dappUrl": site,
                "signedRequest": signInRequest, "requestTruncated": false,
                "dappSummary": ["action": "sign_in", "signin_domain": "app.uniswap.org"],
            ],
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
        let swapTitle: String
        let permitTitle: String
        let technical: String
        let all: String
    }

    func testTheThreeRowsAndThePermitsTechnicalDetails() throws {
        let languages = [
            Words(lang: "zh", swapTitle: "在 Uniswap 兑换", permitTitle: "在 Uniswap 授权签名",
                  technical: "技术细节", all: "全部"),
            Words(lang: "en", swapTitle: "Swap on Uniswap", permitTitle: "Spending permit on Uniswap",
                  technical: "Technical details", all: "All"),
        ]
        for words in languages {
            let app = XCUIApplication()
            app.launchArguments += ["-vela.parallelSpace", "0"]
            app.launchArguments += [
                "-vela.transactionHistory",
                Self.argument(Self.history(now: Int(Date().timeIntervalSince1970))),
            ]
            app.launchEnvironment["VELA_ACCOUNT"] = Self.me
            app.launchEnvironment["VELA_LANG"] = words.lang
            app.launchEnvironment["VELA_THEME"] = "light"
            app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
            app.launchArguments += ["-AppleLanguages", "(\(words.lang))"]
            app.launch()

            // The rows are the core's words, from the stored records.
            XCTAssertTrue(app.staticTexts[words.swapTitle].waitForExistence(timeout: 40),
                          "the swap row never appeared")
            XCTAssertTrue(app.staticTexts[words.permitTitle].exists, "the permit row is missing")
            Thread.sleep(forTimeInterval: 2)
            attach(app.screenshot(), named: "\(words.lang)-home-activity")

            // History: the same three rows.
            tap(app.buttons.matching(identifier: words.all).element(boundBy: 0), "the activity section's \(words.all)")
            Thread.sleep(forTimeInterval: 1.5)
            attach(app.screenshot(), named: "\(words.lang)-history")

            // The permit's detail, then its technical details.
            tap(app.staticTexts[words.permitTitle].firstMatch, "the permit row")
            XCTAssertTrue(app.staticTexts[words.technical].waitForExistence(timeout: 10),
                          "the permit's detail never opened")
            Thread.sleep(forTimeInterval: 1)
            attach(app.screenshot(), named: "\(words.lang)-permit-detail")
            tap(app.staticTexts[words.technical], "technical details")
            XCTAssertTrue(app.staticTexts["PermitSingle"].waitForExistence(timeout: 5),
                          "the technical details never opened")
            Thread.sleep(forTimeInterval: 1)
            attach(app.screenshot(), named: "\(words.lang)-permit-technical")
            app.swipeUp()
            Thread.sleep(forTimeInterval: 1)
            attach(app.screenshot(), named: "\(words.lang)-permit-technical-scrolled")
            app.terminate()
        }
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
