//
//  VelaWalletUITests.swift
//  VelaWalletUITests
//
//  One thin end-to-end smoke (D10): welcome renders, carousel pages by
//  swipe and by dot, both CTAs navigate to their placeholders and back.
//  State logic is unit-tested; this only proves the wiring on-device.
//

import XCTest

final class VelaWalletUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// Spec 012: the launch animation must actually ADVANCE.
    ///
    /// This exists because every unit test passed while the animation was frozen
    /// on frame 0 — the mark centred, all ten wordmark glyphs still at opacity 0
    /// — for its whole run. Nothing that inspects constants or bundles can see
    /// that; only comparing two moments of the real screen can. The cause was an
    /// unguarded `configure { view.configuration = … }`, which SwiftUI re-ran on
    /// every update and which resets playback each time.
    func testLaunchAnimationAdvances() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_LANG"] = "en"
        // Deliberately NOT skipping: this is the one test that watches it play.
        app.launch()

        // Frame 0 is the mark alone, centred. By ~1 s the mark has slid left and
        // several glyphs have faded in, so the two frames cannot match unless
        // playback is stuck.
        let early = app.screenshot().pngRepresentation
        Thread.sleep(forTimeInterval: 1.0)
        let later = app.screenshot().pngRepresentation

        XCTAssertNotEqual(
            early, later,
            "the launch animation did not change over 1 s — it is frozen (frame 0 shows the mark with no wordmark)"
        )

        // And it must still hand off: Welcome is reachable well inside the ceiling.
        XCTAssertTrue(app.buttons["Create Wallet"].waitForExistence(timeout: 8))
    }

    // NOT WRITTEN: a test that the hand-off takes time rather than collapsing.
    //
    // Three instruments were tried and all three lied:
    //   * `waitForExistence` on a Welcome button — true at 1.09 s, because
    //     Welcome is composed under the overlay from the first frame (FR-013a),
    //     so it exists in the tree throughout.
    //   * the same with `.accessibilityHidden(launching)` on the page — still
    //     true at 0.06 s; hiding from accessibility does not change `exists`.
    //   * "when do two consecutive screenshots match" — 0.64 s, because the
    //     animation is genuinely static over its last ~0.5 s (all glyphs are in
    //     by frame 72 of 102) and XCUITest screenshots cost 100–300 ms each, so
    //     a still stretch reads identically to a finished transition.
    //
    // Tuning the threshold until one of them went green would produce a test
    // that passes for the wrong reason — worse than no test, because it would be
    // trusted. `testLaunchAnimationAdvances` above covers the failure mode that
    // IS measurable here (frozen playback); the smoothness of the dissolve is
    // currently verified by eye.

    /// Every gate ticks from its SENTENCE, not only from its 16pt box.
    ///
    /// Founder-found 2026-08-25: two of the three did nothing when their text
    /// was tapped. Nothing below the UI layer could see it — the core toggles
    /// fine, the rows render fine, and only a real tap at a real coordinate
    /// tells you whether the gesture is reachable. The third row is the one
    /// that matters most: it carries the two policy links, and it is drawn by
    /// TextKit precisely so that a tap on its plain words reaches the checkbox.
    func testEveryGateTicksFromItsSentence() throws {
        let app = XCUIApplication()
        // This launch is NOT in the parallel space.
        //
        // The door persists (`vela.parallelSpace`), on purpose: a device test
        // that spans a relaunch must not fall out of the space halfway. The
        // cost is that a session left inside it poisons every later run — which
        // is exactly what happened here, three UI tests failing against a
        // fixture wallet nobody had asked for. An argument-domain value
        // outranks the persisted one WITHOUT writing anything, so each test
        // states its own environment instead of inheriting the last one's.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launch()

        let create = app.buttons["Create Wallet"]
        XCTAssertTrue(create.waitForExistence(timeout: 12))
        // Past the launch animation's hand-off before touching anything.
        Thread.sleep(forTimeInterval: 2.5)
        create.tap()

        // The gates are the only controls on the screen carrying a checked
        // value, which is a sturdier handle than their copy.
        let boxes = app.buttons.matching(NSPredicate(format: "value == '0' OR value == '1'"))
        XCTAssertTrue(app.staticTexts["Name your wallet"].waitForExistence(timeout: 5))
        XCTAssertEqual(boxes.count, 3, "the name screen has three gates")

        // Rows without links are SwiftUI text; the row-wide gesture is what a
        // tap on the label reaches.
        for index in 0..<2 {
            let label = boxes.element(boundBy: index).label
            app.staticTexts[label].tap()
        }

        // The legal row is the TextKit one. Tap near the START of it, which is
        // plain sentence in every locale — the links come later in the line.
        let legal = app.textViews.firstMatch
        XCTAssertTrue(legal.waitForExistence(timeout: 2), "the legal row should be a text view")
        legal.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.25)).tap()

        for index in 0..<3 {
            XCTAssertEqual(
                boxes.element(boundBy: index).value as? String, "1",
                "gate \(index) did not tick from its sentence"
            )
        }
    }
}
