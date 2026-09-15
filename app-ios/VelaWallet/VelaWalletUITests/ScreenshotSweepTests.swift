//
//  ScreenshotSweepTests.swift
//  VelaWalletUITests
//
//  Look at every iOS screen, on a real iPhone, without a screen-recording
//  permission or a GUI (spec 019 T142).
//
//  ## Why this file exists
//
//  Spec 019 shipped the whole onboarding surface to iOS and could not SEE any of
//  it. Two paths were tried and both are dead on this host:
//
//  - `simctl io screenshot` returns the simulator's springboard rather than the
//    app, across a SpringBoard restart and a full device erase.
//  - `idevicescreenshot` needs the Developer Disk Image mounted through
//    lockdown, and iOS 17+ replaced that with a personalized DDI carried over
//    CoreDevice's tunnel, which libimobiledevice does not speak.
//
//  XCUITest sidesteps both: the screenshot is taken BY the test process running
//  on the device, so no host-side capture and no TCC prompt is involved. The
//  images come back inside the `.xcresult` bundle, which `xcrun xcresulttool
//  export attachments` unpacks.
//
//      xcodebuild test \
//        -project app-ios/VelaWallet/VelaWallet.xcodeproj \
//        -scheme VelaWallet \
//        -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/ScreenshotSweepTests \
//        -resultBundlePath /tmp/sweep.xcresult
//      xcrun xcresulttool export attachments \
//        --path /tmp/sweep.xcresult --output-path /tmp/sweep-images
//
//  ## What it can and cannot see
//
//  It drives the app under test, not the person's live session, so it sees the
//  DESIGN of every state and never a real wallet. That is the right split: a
//  real wallet needs a real finger on a real authenticator, and the states that
//  need looking at are the ones no test can reach by fingerprint anyway — a
//  failed publish, a key at the cap, a seven-key list.
//

import XCTest

final class ScreenshotSweepTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// Every gallery fixture, one launch each.
    ///
    /// One launch per fixture rather than tapping through the list: the gallery
    /// already takes `VELA_GALLERY_FIXTURE` for exactly this, and a launch is
    /// the only way to be certain which state is on screen. Tapping a row by
    /// index would silently shift the moment the fixture list changes — and this
    /// sweep exists to be trusted about WHICH screen it photographed.
    func testSweepGalleryFixtures() throws {
        for (code, label) in Self.fixtures {
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
            app.launchEnvironment["VELA_GALLERY"] = "1"
            app.launchEnvironment["VELA_GALLERY_FIXTURE"] = code
            app.launchEnvironment["VELA_LANG"] = Self.language
            app.launchEnvironment["VELA_THEME"] = Self.theme
            app.launchArguments += ["-AppleLanguages", "(\(Self.language))"]
            app.launch()

            // The fixture presents as a full-screen cover or a sheet; either way
            // it is up within a frame or two of launch. A fixed wait beats
            // polling for an element, because half these states are deliberately
            // static and have nothing unique to poll for.
            Thread.sleep(forTimeInterval: 1.2)
            attach(app.screenshot(), named: "\(Self.theme)-\(Self.language)-\(label)")
            app.terminate()
        }
    }

    /// The live app's entry point, which the gallery cannot show.
    func testWelcome() throws {
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
        app.launchEnvironment["VELA_LANG"] = Self.language
        app.launchEnvironment["VELA_THEME"] = Self.theme
        app.launchArguments += ["-AppleLanguages", "(\(Self.language))"]
        app.launch()

        XCTAssertTrue(
            app.buttons.firstMatch.waitForExistence(timeout: 12),
            "Welcome never appeared — the launch animation did not hand off"
        )
        // The animation runs ~1.7 s and this test is about the screen it hands
        // off TO, so wait past it rather than photographing a dissolve.
        Thread.sleep(forTimeInterval: 2.5)
        attach(app.screenshot(), named: "\(Self.theme)-\(Self.language)-welcome")
    }

    // MARK: - Plumbing

    /// `SIMCTL_CHILD_`-style overrides so one run can sweep a second locale or
    /// the light palette without an edit. Defaults match the design's own
    /// starting point.
    private static let language = ProcessInfo.processInfo.environment["SWEEP_LANG"] ?? "zh"
    private static let theme = ProcessInfo.processInfo.environment["SWEEP_THEME"] ?? "dark"

    /// The fixture codes, paired with filesystem-safe labels.
    ///
    /// The codes are `FlowFixtures.all`'s, and the middot in them is exactly why
    /// the label is separate: an attachment name becomes a filename, and a
    /// filename with `·` in it is a filename somebody's tooling will mangle.
    private static let fixtures: [(code: String, label: String)] = [
        ("name · empty", "01-name-empty"),
        ("name · filled", "02-name-filled"),
        ("name · too long", "03-name-too-long"),
        ("name · draft waiting", "04-name-draft-waiting"),
        ("keys · one, needs a second", "05-keys-needs-second"),
        ("keys · two, ready", "06-keys-ready"),
        ("keys · unconfirmed row", "07-keys-unconfirmed"),
        ("keys · at the cap", "08-keys-at-cap"),
        ("progress · verify", "09-progress-verify"),
        ("progress · derive", "10-progress-derive"),
        ("progress · publish", "11-progress-publish"),
        ("retry · publish failed", "12-retry"),
        ("done", "13-done"),
        ("unsupported", "14-sheet-unsupported"),
        ("not discoverable", "15-sheet-not-discoverable"),
        ("recover offer", "16-sheet-recover-offer"),
        ("create failed · server", "17-sheet-create-failed"),
        ("sign-in failed", "18-sheet-sign-in-failed"),
    ]

    /// `.keepAlways`, because the whole point is the image and a passing test
    /// discards attachments by default.
    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
