//
//  PerfTests.swift
//  VelaWalletUITests
//
//  Native parity Phase 4: the numbers a paid app is held to, measured on the
//  shipped (Release) build.
//
//      metric                               target   how
//      cold start → first frame             —        XCTApplicationLaunchMetric
//      cold start → cached home             ≤1.2 s   signpost HomeCachedPaint (from the app's init)
//      scroll: 200-row Activity             0 hitches XCTOSSignpostMetric.scrollDecelerationMetric
//      memory (launch, home, Activity)      ≤180 MB  XCTMemoryMetric — peak physical
//      Confirm → passkey prompt             ≤300 ms  signpost ConfirmToPasskey
//
//  The targets are the AUDIT's (native parity §5, Phase 4) and are NOT
//  asserted here: on a simulator they are indicative only — the floor that
//  counts is a real phone — so a miss is reported, never a red build.
//  `app-ios/scripts/perf/run-perf.sh` runs the `VelaWalletPerf` scheme and
//  prints every figure beside its target from the result bundle.
//
//  ## The wallet they run on
//
//  A Release build reads no launch input, so the wallet is written first, by
//  a Debug launch of the same app on the same simulator (`PerfSeed`,
//  `run-perf.sh` step 2): the parallel space's golden Safe, 200 records in
//  its history and a cached home. Installing the Release build over it keeps
//  the data. Without that seed every test here skips.
//
//  ## Confirm → passkey spends nothing
//
//  The confirm is slid on a self-send of 0.01 xDAI from the golden Safe. A
//  Release build has no parallel-space signer: the slide reaches the REAL
//  passkey prompt, which an unsigned simulator build cannot complete (no
//  associated domain, no credential), so the attempt ends there — nothing is
//  signed, nothing is sent. The interval ends as the prompt is raised. It
//  needs the network (the nonce and the relay's estimate are read before the
//  prompt, as they are on a phone) and skips without it.
//

import XCTest

final class PerfTests: XCTestCase {

    static let subsystem = "app.getvela.VelaWallet"
    static let category = "perf"

    /// The audit's targets — read by `run-perf.sh`'s report, kept here so the
    /// test and its thresholds are reviewed together.
    enum Target {
        static let cachedHomeSeconds = 1.2
        static let scrollHitches = 0
        static let memoryMB = 180.0
        static let confirmToPasskeyMs = 300.0
    }

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func makeApp(animation: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        // The lockup plays once a week (the core's rule, `vela.launch.played`),
        // and a tap ends it; the cached home is composed behind it either way
        // (FR-013a).
        if !animation { app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1" }
        return app
    }

    /// The seeded home is up: its Activity header is on screen.
    @discardableResult
    private func waitForHome(_ app: XCUIApplication, timeout: TimeInterval = 30) -> Bool {
        app.buttons["home.section.activity"].firstMatch.waitForExistence(timeout: timeout)
    }

    private func requireSeed(_ app: XCUIApplication) throws {
        app.launch()
        try XCTSkipUnless(waitForHome(app), "no seeded wallet — run app-ios/scripts/perf/run-perf.sh")
    }

    private func options(_ iterations: Int) -> XCTMeasureOptions {
        let options = XCTMeasureOptions()
        options.iterationCount = iterations
        return options
    }

    // MARK: - Launch

    /// Process start to the first frame (and to responsive), with the launch
    /// lockup as it ships: the core's weekly rule decides, so after the first
    /// of these launches the rest are the everyday cold start.
    func testColdLaunch() throws {
        try requireSeed(makeApp(animation: true))
        measure(metrics: [XCTApplicationLaunchMetric(waitUntilResponsive: true)], options: options(5)) {
            makeApp(animation: true).launch()
        }
    }

    /// The app's init to the home drawing its cached figure.
    func testColdStartToCachedHome() throws {
        try requireSeed(makeApp())
        let painted = XCTOSSignpostMetric(subsystem: Self.subsystem, category: Self.category, name: "HomeCachedPaint")
        measure(metrics: [painted, XCTClockMetric()], options: options(5)) {
            let app = makeApp()
            app.launch()
            XCTAssertTrue(waitForHome(app), "the home never came up")
        }
    }

    // MARK: - Memory

    /// Launch, the home, the whole Activity list scrolled, and back.
    func testMemory() throws {
        let app = makeApp()
        try requireSeed(app)
        measure(metrics: [XCTMemoryMetric(application: app)], options: options(3)) {
            app.launch()
            XCTAssertTrue(waitForHome(app))
            openActivity(app)
            let list = app.scrollViews.firstMatch
            for _ in 0..<6 { list.swipeUp(velocity: .fast) }
        }
    }

    // MARK: - Scroll

    /// The full history — 200 rows over forty-odd day headings — flung end to
    /// end. Each iteration flings down through it; the way back up is not
    /// measured.
    func testActivityScroll() throws {
        let app = makeApp()
        try requireSeed(app)
        openActivity(app)
        let list = app.scrollViews.firstMatch
        let options = XCTMeasureOptions()
        options.iterationCount = 5
        options.invocationOptions = [.manuallyStop]
        measure(metrics: [XCTOSSignpostMetric.scrollDecelerationMetric], options: options) {
            for _ in 0..<4 { list.swipeUp(velocity: .fast) }
            stopMeasuring()
            for _ in 0..<6 { list.swipeDown(velocity: .fast) }
        }
    }

    private func openActivity(_ app: XCUIApplication) {
        app.buttons["home.section.activity"].firstMatch.tap()
        XCTAssertTrue(app.buttons["history.row"].firstMatch.waitForExistence(timeout: 20), "the history never listed")
    }

    // MARK: - Confirm → passkey

    func testConfirmToPasskey() throws {
        let probe = makeApp()
        try requireSeed(probe)
        // The send needs a network and a balance that covers it. Without
        // them there is nothing to measure here: skipped, never failed — the
        // shell's own share is measured hermetically by
        // `SendReceiptFollowsTrackerTests.theConfirmReachesThePromptOnAnInstantNetwork`.
        do {
            _ = try reachConfirm(probe)
        } catch {
            throw XCTSkip("Confirm not reachable here: \(error)")
        }
        probe.terminate()
        let raised = XCTOSSignpostMetric(subsystem: Self.subsystem, category: Self.category, name: "ConfirmToPasskey")
        let options = XCTMeasureOptions()
        options.iterationCount = 3
        options.invocationOptions = [.manuallyStart, .manuallyStop]
        measure(metrics: [raised], options: options) {
            let app = makeApp()
            app.launch()
            XCTAssertTrue(waitForHome(app))
            let confirm: XCUIElement
            do {
                confirm = try reachConfirm(app)
            } catch {
                XCTFail("\(error)")
                return
            }
            startMeasuring()
            confirm.tap()
            // The prompt cannot complete here: whatever the attempt ends on —
            // an alert, the confirm's notice — the interval has ended by then.
            let ended = NSPredicate { _, _ in
                app.alerts.firstMatch.exists
                    || app.descendants(matching: .any)["send.confirm.notice"].firstMatch.exists
                    || !confirm.exists
            }
            expectation(for: ended, evaluatedWith: nil)
            waitForExpectations(timeout: 60)
            stopMeasuring()
            app.terminate()
        }
    }

    private struct NotReached: Error, CustomStringConvertible { let description: String }

    /// Send → xDAI → the golden Safe itself, 0.01 → Continue → the confirm,
    /// its Confirm & Send enabled.
    private func reachConfirm(_ app: XCUIApplication) throws -> XCUIElement {
        app.buttons["Send"].firstMatch.tap()
        guard app.staticTexts["xDAI"].waitForExistence(timeout: 60) else {
            throw NotReached(description: "the picker never listed xDAI (no network?)")
        }
        app.staticTexts["xDAI"].firstMatch.tap()
        let recipient = app.textFields["send.recipient"]
        guard recipient.waitForExistence(timeout: 20) else { throw NotReached(description: "no form") }
        recipient.tap()
        recipient.typeText(golden)
        let amount = app.textFields["send.amount"]
        amount.tap()
        amount.typeText("0.01")
        if app.buttons["keyboard.done"].firstMatch.waitForExistence(timeout: 3) {
            app.buttons["keyboard.done"].firstMatch.tap()
        }
        let next = app.buttons["Continue"].firstMatch
        guard enabled(next) else { throw NotReached(description: "Continue never enabled (balance?)") }
        next.tap()
        let confirm = app.buttons["Confirm & Send"].firstMatch
        guard confirm.waitForExistence(timeout: 60) else { throw NotReached(description: "no confirm") }
        guard enabled(confirm) else { throw NotReached(description: "Confirm & Send never enabled (fee?)") }
        return confirm
    }

    private func enabled(_ element: XCUIElement, timeout: TimeInterval = 60) -> Bool {
        let wait = XCTNSPredicateExpectation(predicate: NSPredicate(format: "isEnabled == true"), object: element)
        return XCTWaiter().wait(for: [wait], timeout: timeout) == .completed
    }
}
