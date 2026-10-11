import XCTest

/// The router's stack, held to a budget (see `StackGauge`).
///
/// On the owner's iPhone 11 a Debug build died opening Send: the main
/// thread's megabyte of stack was used up by `RootView`'s nested bodies
/// before the screen's model was built. A simulator's main thread has eight
/// megabytes, so the same build ran there without a sign of it — every gate
/// was green. The frames are the same size on both, though, so a simulator
/// can MEASURE what it cannot feel: the app writes the depth it has reached
/// where the router asks for a pushed screen's model, and this holds it
/// under a budget that leaves a phone room for the model itself.
///
/// Parallel space, like every suite that opens the live wallet here: the
/// fixture wallet, nothing signed, nothing sent.
final class StackDepthUITests: XCTestCase {

    /// Where the router may stand when it asks for a pushed screen's model.
    ///
    /// A phone's main thread has about 1,008 KB. Under `flowModel` the
    /// deepest builder measured (`build` → `sendForm` → the core's string
    /// lookup) takes about 70 KB more, so the cliff is near 930 KB. Today the
    /// router stands at 695 KB (it stood at 946 KB when the phone crashed);
    /// the budget is between the two, and far enough above today's figure
    /// that another toolchain's frame sizes do not trip it.
    private static let budget = 820 * 1024

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testTheRouterLeavesAPhoneRoomToBuildAScreen() throws {
        let gauge = NSTemporaryDirectory() + "vela-stack-gauge-\(UUID().uuidString).txt"
        defer { try? FileManager.default.removeItem(atPath: gauge) }

        let app = XCUIApplication()
        app.launchArguments += ["-vela.parallelSpace", "1", "-AppleLanguages", "(zh-Hans)"]
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_STACK_GAUGE"] = gauge
        app.launch()

        XCTAssertTrue(app.staticTexts["资产"].waitForExistence(timeout: 40), "the home never appeared")
        // Two pushed screens, each built through the whole router: Receive,
        // then Send's coin list (which needs no balance to be drawn).
        tap(app.buttons["收款"].firstMatch, "收款")
        XCTAssertTrue(app.buttons["显示二维码"].firstMatch.waitForExistence(timeout: 20), "Receive never appeared")
        tap(app.buttons["返回"].firstMatch, "back from Receive")
        tap(app.buttons["转账"].firstMatch, "转账")
        XCTAssertTrue(app.buttons["返回"].firstMatch.waitForExistence(timeout: 20), "Send never appeared")

        let text = (try? String(contentsOfFile: gauge, encoding: .utf8)) ?? ""
        let depths: [Int] = text.split(separator: "\n").compactMap { line in
            let parts = line.split(separator: " ")
            guard parts.count == 2, parts[0] == "flowModel" else { return nil }
            return Int(parts[1])
        }
        let report = XCTAttachment(string: text)
        report.name = "stack-gauge"
        report.lifetime = .keepAlways
        add(report)

        // Not vacuous: the router did ask for a pushed screen's model.
        let deepest = try XCTUnwrap(depths.max(), "the gauge recorded nothing — is the build Debug, and did a flow screen open?")
        XCTAssertGreaterThan(deepest, 64 * 1024, "a depth this small is not the router's")
        XCTAssertLessThan(
            deepest, Self.budget,
            "the router stands \(deepest / 1024) KB deep when it asks for a screen's model; a phone's main thread has ~1,008 KB and the model needs ~70 KB more"
        )
    }

    private func tap(_ element: XCUIElement, _ what: String, timeout: TimeInterval = 15) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "\(what) never appeared")
        element.tap()
    }
}
