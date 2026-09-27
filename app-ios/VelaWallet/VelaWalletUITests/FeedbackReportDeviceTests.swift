//
//  FeedbackReportDeviceTests.swift
//  VelaWalletUITests
//
//  The in-app report and the Community links, on a REAL iPhone (078,
//  2026-09-27), reached the way a person reaches them — the Settings tab, the
//  row — never a deep link or a gallery state:
//
//  - Settings → Community: each row opens exactly its URL, and Vela is intact
//    when it comes back.
//  - Settings → Send feedback: the system PhotosPicker, the processing tiles,
//    remove, the public line, the REAL keyboard (Pinyin included, and an edit
//    in the middle of a line), light/dark and the largest text.
//  - Every outcome is visible: offline → the fallback block; closed mid-send →
//    a toast on Settings (success and fallback, against a stub the runner
//    serves on 127.0.0.1, which also inspects the payload the app really
//    sent); and — only with `TEST_RUNNER_VELA_REAL_SEND=1` — ONE real report
//    through the live endpoint, whose success state stays in the sheet.
//
//  The images are GENERATED here and saved to the library by this runner
//  (never the app). `testZZDeleteSeededPhotos` deletes exactly those assets by
//  their local identifiers (`TEST_RUNNER_VELA_DELETE_IDS`) or, as a fallback,
//  by their original file names (`vela-feedback-test-*`). Xcode's XCTRunner
//  template already carries NSPhotoLibraryUsageDescription.
//
//  Chinese is typed with the phone's own Pinyin keyboard (already installed on
//  the test iPhone; the globe key sits below the keyboard on Face ID phones).
//
//      xcodebuild test-without-building -xctestrun <built.xctestrun> \
//        -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/FeedbackReportDeviceTests/<case>
//

import Network
import Photos
import UIKit
import UniformTypeIdentifiers
import XCTest

final class FeedbackReportDeviceTests: XCTestCase {

    /// A host that can never answer: the report falls back, nothing is sent.
    static let unreachable = "https://vela-offline.invalid/api/bug-report"
    static let filePrefix = "vela-feedback-test-"

    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    private var env: [String: String] { ProcessInfo.processInfo.environment }

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    // MARK: - Seed and delete (runner side)

    func testAASeedPhotos() throws {
        try authorize()
        let base = Date()
        let images: [(String, Data)] = [
            ("\(Self.filePrefix)1.png", Self.screenshotPNG(number: 1)),
            ("\(Self.filePrefix)2.jpg", Self.gpsJPEG(number: 2, size: CGSize(width: 1600, height: 1200))),
            ("\(Self.filePrefix)3.jpg", Self.gpsJPEG(number: 3, size: CGSize(width: 3024, height: 4032))),
        ]
        for (name, data) in images {
            let attachment = XCTAttachment(
                data: data,
                uniformTypeIdentifier: name.hasSuffix(".png") ? UTType.png.identifier : UTType.jpeg.identifier
            )
            attachment.name = "input-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
        var ids: [String] = []
        try PHPhotoLibrary.shared().performChangesAndWait {
            for (index, (name, data)) in images.enumerated() {
                let request = PHAssetCreationRequest.forAsset()
                let options = PHAssetResourceCreationOptions()
                options.originalFilename = name
                request.addResource(with: .photo, data: data, options: options)
                request.creationDate = base.addingTimeInterval(Double(index))
                if let placeholder = request.placeholderForCreatedAsset {
                    ids.append(placeholder.localIdentifier)
                }
            }
        }
        XCTAssertEqual(ids.count, images.count)
        let fetched = PHAsset.fetchAssets(withLocalIdentifiers: ids, options: nil)
        var lines: [String] = []
        fetched.enumerateObjects { asset, _, _ in
            let name = PHAssetResource.assetResources(for: asset).first?.originalFilename ?? "?"
            let location = asset.location.map { "\($0.coordinate.latitude),\($0.coordinate.longitude)" } ?? "none"
            lines.append("\(asset.localIdentifier) \(name) \(asset.pixelWidth)x\(asset.pixelHeight) location=\(location)")
        }
        let report = "VELA_SEEDED_IDS=\(ids.joined(separator: ","))\n" + lines.joined(separator: "\n")
        print(report)
        note(report, "seeded-ids")
        XCTAssertEqual(fetched.count, images.count, "not every seeded image is in the library")
    }

    func testZZDeleteSeededPhotos() throws {
        try authorize()
        var ids = (env["VELA_DELETE_IDS"] ?? "").split(separator: ",").map(String.init).filter { !$0.isEmpty }
        if ids.isEmpty {
            // Fallback: every asset whose original file name is ours.
            let recent = PHFetchOptions()
            recent.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
            recent.fetchLimit = 200
            PHAsset.fetchAssets(with: .image, options: recent).enumerateObjects { asset, _, _ in
                let name = PHAssetResource.assetResources(for: asset).first?.originalFilename ?? ""
                if name.hasPrefix(Self.filePrefix) { ids.append(asset.localIdentifier) }
            }
        }
        let assets = PHAsset.fetchAssets(withLocalIdentifiers: ids, options: nil)
        var names: [String] = []
        assets.enumerateObjects { asset, _, _ in
            names.append(PHAssetResource.assetResources(for: asset).first?.originalFilename ?? "?")
        }
        print("VELA_DELETING \(assets.count): \(names)")
        // Only ours, whatever the ids said.
        XCTAssertTrue(names.allSatisfy { $0.hasPrefix(Self.filePrefix) }, "refusing to delete \(names)")
        guard assets.count > 0 else { return }

        let done = expectation(description: "deleted")
        var outcome: (Bool, Error?) = (false, nil)
        PHPhotoLibrary.shared().performChanges({
            PHAssetChangeRequest.deleteAssets(assets)
        }) { ok, error in
            outcome = (ok, error)
            done.fulfill()
        }
        // The system asks "Allow … to delete N photos?".
        let deadline = Date().addingTimeInterval(25)
        var tapped = false
        while Date() < deadline, !tapped {
            tapped = tapSystemButton(["Delete", "删除"])
            if !tapped { Thread.sleep(forTimeInterval: 0.4) }
        }
        wait(for: [done], timeout: 30)
        XCTAssertTrue(outcome.0, "delete failed: \(String(describing: outcome.1))")
        let left = PHAsset.fetchAssets(withLocalIdentifiers: ids, options: nil)
        print("VELA_DELETED ok=\(outcome.0) remaining=\(left.count)")
        XCTAssertEqual(left.count, 0, "some test images are still in the library")
    }

    // MARK: - Settings → Community

    /// Each row opens exactly its URL outside the app, and Vela is intact when
    /// the person comes back.
    func testBACommunityRows() {
        let app = launchApp(theme: "light")
        openSettings(app)
        scrollSettings(to: "X (Twitter)", in: app)
        shot(app, "b1-settings-community")
        let rows: [(String, String)] = [
            ("X (Twitter)", "x.com/realvelawallet"),
            ("Telegram", "t.me/velawallet"),
            ("Discord", "discord.gg/23gWrtaYSa"),
        ]
        for (title, expected) in rows {
            let label = app.staticTexts[title]
            XCTAssertTrue(label.waitForExistence(timeout: 5), "no \(title) row")
            label.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
            // Out of Vela: whichever app took the link.
            let takers = ["com.apple.mobilesafari", "com.atebits.Tweetie2", "ph.telegra.Telegraph",
                          "com.hammerandchisel.discord"].map { XCUIApplication(bundleIdentifier: $0) }
            let left = waitUntil(timeout: 12) { takers.contains { $0.state == .runningForeground } }
            XCTAssertTrue(left, "\(title) did not leave the app")
            settle(4)
            let screen = XCUIScreen.main.screenshot()
            attach(screen, "b2-opened-\(title)")
            let landed = foregroundDescription(expecting: expected)
            print("VELA_COMMUNITY \(title) → \(landed)")
            note(landed, "b2-opened-\(title)-where")
            // Back to Vela, on the page it was left on.
            app.activate()
            XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10), "Vela did not come back")
            settle(1.5)
            XCTAssertTrue(app.staticTexts[title].waitForExistence(timeout: 5),
                          "Vela came back somewhere other than Settings → Community")
            shot(app, "b3-back-from-\(title)")
        }
        app.terminate()
    }

    /// The group and the report row in zh (largest text), de (dark) and es-MX.
    func testBBSettingsLanguages() {
        for (lang, theme, scale) in [("zh", "light", "xlarge"), ("de", "dark", nil), ("es-MX", "light", nil)] as [(String, String, String?)] {
            let app = launchApp(theme: theme, textScale: scale, lang: lang)
            openSettings(app)
            scrollSettings(to: "Discord", in: app)
            // Past the last group, so the report row shows too.
            slowDrag(app, from: 0.7, to: 0.4)
            shot(app, "b4-settings-\(lang)-\(theme)-\(scale ?? "std")")
            app.terminate()
        }
    }

    /// Close the three Safari tabs the Community rows opened — and only
    /// those: it stops at the first tab that is not x.com / t.me / discord.
    func testBZCloseCommunityTabs() {
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
        safari.activate()
        settle(2)
        let ours = ["x.com", "t.me", "discord.com", "discord.gg"]
        var closed: [String] = []
        for attempt in 0..<4 {
            let title = safari.textFields["TabBarItemTitle"]
            let url = ((title.value as? String) ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "\u{200E} "))
            guard ours.contains(where: { url == $0 || url.hasSuffix("." + $0) }) else {
                print("VELA_SAFARI stop at \(url)")
                break
            }
            // The current tab's own close: the tab overview, where each tab
            // wears a close button; the current one is the selected cell.
            safari.buttons["MoreMenuButton"].tap()
            settle(1)
            safari.buttons["TabOverviewButton"].tap()
            settle(1.5)
            if attempt == 0 { dump(safari, "safari-overview") ; shot(safari, "safari-overview") }
            let current = safari.cells.matching(NSPredicate(format: "selected == true")).firstMatch
            let close = current.buttons.matching(NSPredicate(format: "label IN %@ OR identifier CONTAINS[c] 'close'",
                                                             ["关闭", "Close", "关闭标签页"])).firstMatch
            guard current.exists, close.exists else {
                shot(safari, "safari-no-close")
                XCTFail("no close button on the current tab")
                break
            }
            print("VELA_SAFARI closing \(url) cell=\(current.label)")
            close.tap()
            settle(1.2)
            // Back into whatever tab is now current.
            let other = safari.cells.matching(NSPredicate(format: "selected == true")).firstMatch
            if other.exists { other.tap() } else if safari.buttons["完成"].exists { safari.buttons["完成"].tap() }
            settle(1.5)
            closed.append(url)
        }
        print("VELA_SAFARI closed=\(closed)")
        note("closed \(closed)", "safari-closed")
        shot(safari, "safari-after-close")
        XCUIApplication().activate()
        settle(1)
    }

    // MARK: - Settings → Send feedback

    /// Light, standard text, offline: empty → pick 3 → processing → tiles →
    /// remove one → the real keyboard (English, Pinyin, a mid-line edit) →
    /// Send → the fallback block, in the sheet → Try again.
    func testCAFromRowOffline() {
        let app = launchApp(theme: "light", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        shot(app, "c01-light-empty-top")
        slowDrag(app, from: 0.75, to: 0.3)
        shot(app, "c02-light-empty-bottom")
        slowDrag(app, from: 0.3, to: 0.8)

        openPicker(app)
        shot(app, "c03-light-picker")
        pick([1, 2, 3], in: app)
        shot(app, "c04-light-picker-selected")
        confirmPicker(app)
        captureProcessing(app, name: "c05-light-processing")
        waitForTiles(app, count: 3)
        settle(0.5)
        shot(app, "c06-light-three-tiles")

        let remove = app.buttons["feedback.removeScreenshot.3"]
        XCTAssertTrue(remove.waitForExistence(timeout: 5), "no remove badge on tile 3")
        remove.tap()
        settle(0.8)
        XCTAssertFalse(app.buttons["feedback.removeScreenshot.3"].exists, "tile 3 was not removed")
        XCTAssertTrue(app.staticTexts["2 / 5"].exists, "the counter does not read 2 / 5")
        XCTAssertTrue(element(app, "feedback.screenshotsPublic").exists, "the public line is gone")
        shot(app, "c07-light-removed-one")

        typeWithRealKeyboard(app)

        tapSend(app)
        settle(0.3)
        shot(app, "c11-light-sending")
        let fallback = element(app, "feedback.fallback")
        XCTAssertTrue(fallback.waitForExistence(timeout: 30), "no fallback block after an unreachable send")
        settle(1)
        XCTAssertTrue(element(app, "feedback.addScreenshots").exists || element(app, "feedback.what").exists,
                      "the sheet closed — the person was dropped back on Settings")
        assertFallbackInView(app)
        shot(app, "c12-light-fallback")
        let retry = app.buttons["feedback.send"]
        XCTAssertEqual(retry.label, "Try again", "the retry button does not read Try again")
        XCTAssertFalse(app.buttons["feedback.github"].exists, "the Prefer GitHub link is still shown in fallback")
        XCTAssertTrue(app.staticTexts["Screenshots can't be carried over. Add them in the GitHub form."].exists,
                      "no fallbackScreenshots line with images attached")
        dump(app, "c12-light-fallback-tree")
        slowDrag(app, from: 0.6, to: 0.25)
        shot(app, "c13-light-fallback-bottom")
        tapSend(app)
        settle(0.2)
        shot(app, "c14-light-retrying")
        XCTAssertTrue(fallback.waitForExistence(timeout: 30), "no fallback after Try again")
        settle(1)
        assertFallbackInView(app)
        shot(app, "c15-light-fallback-again")
        app.terminate()
    }

    /// Dark, the app's largest text (a launch-argument override; nothing is
    /// stored): empty, two tiles, the public line, and the fallback.
    func testCBDarkLargestOffline() {
        let app = launchApp(theme: "dark", textScale: "xlarge", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        shot(app, "c20-dark-xl-empty-top")
        slowDrag(app, from: 0.75, to: 0.3)
        shot(app, "c21-dark-xl-empty-bottom")
        slowDrag(app, from: 0.3, to: 0.8)
        openPicker(app)
        pick([1, 2], in: app)
        confirmPicker(app)
        captureProcessing(app, name: "c22-dark-xl-processing")
        waitForTiles(app, count: 2)
        settle(0.5)
        shot(app, "c23-dark-xl-two-tiles")
        type("Dark, largest text — offline check.", into: "feedback.what", in: app)
        tapSend(app)
        XCTAssertTrue(element(app, "feedback.fallback").waitForExistence(timeout: 30), "no fallback in dark")
        settle(1)
        assertFallbackInView(app)
        shot(app, "c24-dark-xl-fallback")
        slowDrag(app, from: 0.75, to: 0.3)
        shot(app, "c25-dark-xl-fallback-bottom")
        app.terminate()
    }

    /// Closed while sending → the report still lands, and Settings says so,
    /// with "View on GitHub". Against the runner's stub (filed after 8 s),
    /// which also checks the payload the app really sent.
    func testCCCloseMidSendSuccessToast() throws {
        let stub = try StubReportServer(answer: .filed(delay: 8))
        stub.start()
        defer { stub.stop() }
        let app = launchApp(theme: "light", endpoint: stub.url)
        openFeedbackFromRow(app)
        openPicker(app)
        pick([1, 2], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 2)
        type("Close-mid-send check (device verification, stub endpoint).", into: "feedback.what", in: app)
        tapSend(app)
        settle(0.5)
        shot(app, "c30-sending-before-close")
        closeSheet(app)
        XCTAssertTrue(app.staticTexts["Send feedback"].waitForExistence(timeout: 5), "not back on Settings")
        shot(app, "c31-closed-while-sending")
        let toast = element(app, "feedback.toast")
        XCTAssertTrue(toast.waitForExistence(timeout: 30), "no toast after the report landed")
        settle(0.6)
        shot(app, "c32-success-toast")
        dump(app, "c32-success-toast-tree")
        XCTAssertTrue(app.staticTexts["Thanks\u{00A0}— your report is in"].exists, "the toast does not say it was filed")
        XCTAssertTrue(app.buttons["feedback.toastAction"].label == "View on GitHub", "no View on GitHub action")
        inspect(stub, expectScreenshots: 2)
        app.buttons["feedback.toastClose"].tap()
        settle(0.6)
        XCTAssertFalse(toast.exists, "the toast did not close")
        // The next sheet opens fresh, not on the old success.
        openFeedbackFromRow(app)
        XCTAssertTrue(app.staticTexts["Optional · up to 5"].exists, "the reopened sheet is not a fresh form")
        shot(app, "c33-reopened-fresh")
        app.terminate()
    }

    /// Closed while sending → the endpoint refused it → Settings says so,
    /// with "Open GitHub form", and the toast waits for the person.
    func testCDCloseMidSendFallbackToast() throws {
        let stub = try StubReportServer(answer: .status(503, #"{"error":"not_configured"}"#, delay: 6))
        stub.start()
        defer { stub.stop() }
        let app = launchApp(theme: "light", endpoint: stub.url)
        openFeedbackFromRow(app)
        type("Close-mid-send refusal check (device verification, stub endpoint).", into: "feedback.what", in: app)
        tapSend(app)
        settle(0.5)
        closeSheet(app)
        shot(app, "c40-closed-while-sending")
        let toast = element(app, "feedback.toast")
        XCTAssertTrue(toast.waitForExistence(timeout: 30), "no toast after the refusal")
        settle(0.6)
        shot(app, "c41-fallback-toast")
        XCTAssertTrue(app.staticTexts["Couldn't send from the app"].exists, "the toast does not say it failed")
        XCTAssertEqual(app.buttons["feedback.toastAction"].label, "Open GitHub form")
        // A fallback waits for the person (a success goes after 8 s).
        settle(10)
        XCTAssertTrue(toast.exists, "the fallback toast went away on its own")
        shot(app, "c42-fallback-toast-after-10s")
        app.buttons["feedback.toastClose"].tap()
        settle(0.6)
        XCTAssertFalse(toast.exists)
        app.terminate()
    }

    // MARK: - The ONE real report

    func testCERealReportFromRow() throws {
        try XCTSkipUnless(env["VELA_REAL_SEND"] == "1", "the real send runs only with TEST_RUNNER_VELA_REAL_SEND=1")
        let app = launchApp(theme: "light", endpoint: nil)
        openFeedbackFromRow(app)
        openPicker(app)
        pick([1, 2], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 2)
        settle(0.5)
        type("[Test] In-app report with screenshots — end-to-end check from iOS (iPhone 11). Please ignore; will be closed.",
             into: "feedback.what", in: app)
        let addSteps = app.buttons["+ Add steps to reproduce"]
        XCTAssertTrue(addSteps.waitForExistence(timeout: 5), "no Add steps button")
        addSteps.tap()
        settle(0.5)
        type("Automated device verification of the 078 feedback feature.", into: "feedback.steps", in: app)
        let whatValue = (app.textViews["feedback.what"].value as? String) ?? ""
        let stepsValue = (app.textViews["feedback.steps"].value as? String) ?? ""
        print("VELA_WHAT=\(whatValue)\nVELA_STEPS=\(stepsValue)")
        XCTAssertEqual(whatValue, "[Test] In-app report with screenshots — end-to-end check from iOS (iPhone 11). Please ignore; will be closed.")
        XCTAssertEqual(stepsValue, "Automated device verification of the 078 feedback feature.")
        shot(app, "e40-real-before-send")

        tapSend(app)
        settle(0.2)
        shot(app, "e41-real-sending")
        let title = element(app, "feedback.successTitle")
        let filed = title.waitForExistence(timeout: 60)
        settle(1)
        shot(app, "e42-real-outcome")
        dump(app, "e42-real-outcome-tree")
        XCTAssertTrue(filed, "the report did not file (fallback or timeout) — do NOT retry blindly")
        let body = app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "#")).firstMatch
        print("VELA_SUCCESS_BODY=\(body.label)")
        note(body.label, "e42-success-body")
        let done = app.buttons["feedback.done"]
        XCTAssertTrue(done.exists, "no Done on the success state")
        done.tap()
        settle(1)
        XCTAssertTrue(app.staticTexts["Send feedback"].waitForExistence(timeout: 5), "Done did not return to Settings")
        shot(app, "e43-real-after-done")
        app.terminate()
    }

    // MARK: - A scroll never removes a screenshot (regression, offline)

    /// A scroll that starts on a remove badge's 44-pt area keeps the tile (a
    /// Button there fired after the scroll and cost the real report a
    /// screenshot); a tap still removes. Offline; nothing is sent.
    func testCFScrollFromBadgeKeepsTile() {
        let app = launchApp(theme: "light", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        var log: [String] = []
        func tiles() -> Int {
            (1...5).filter { app.buttons["feedback.removeScreenshot.\($0)"].exists }.count
        }
        func fill() {
            while tiles() < 2 {
                openPicker(app)
                pick(tiles() == 0 ? [1, 2] : [1], in: app)
                confirmPicker(app)
                waitForTiles(app, count: 2)
                settle(0.6)
            }
        }
        let origin = app.coordinate(withNormalizedOffset: .zero)
        let cases: [(String, (XCUIElement) -> CGPoint)] = [
            ("neutral right of the row", { _ in CGPoint(x: 330, y: 0) }),
            ("badge 1 reported centre", { badge in CGPoint(x: badge.frame.midX, y: badge.frame.midY) }),
            ("real-run spot (0.3 w, 10 pt above badge top)", { badge in CGPoint(x: app.frame.width * 0.3, y: badge.frame.minY + 10) }),
        ]
        for (name, point) in cases {
            fill()
            let badge = app.buttons["feedback.removeScreenshot.1"]
            var start = point(badge)
            if start.y == 0 { start.y = badge.frame.midY }
            let tileFrames = app.images.allElementsBoundByIndex.filter { $0.frame.width > 40 && $0.frame.width < 80 }.map { "\($0.frame)" }
            let before = tiles()
            let rowTop = badge.frame.minY
            origin.withOffset(CGVector(dx: start.x, dy: start.y))
                .press(forDuration: 0.15, thenDragTo: origin.withOffset(CGVector(dx: start.x, dy: start.y - 200)),
                       withVelocity: .slow, thenHoldForDuration: 0.15)
            settle(1)
            let after = tiles()
            let line = "\(name): start=\(start) badge1=\(badge.frame) tiles \(before)→\(after) tileFrames=\(tileFrames)"
            print("VELA_ORIGIN " + line)
            log.append(line)
            XCTAssertEqual(after, before, "a scroll from \(name) removed a screenshot")
            shot(app, "hl-\(name)")
            // Back down by exactly what the content moved — never past the
            // top, where a downward drag dismisses the sheet.
            let moved = rowTop - app.buttons["feedback.removeScreenshot.1"].frame.minY
            if moved > 20 {
                let y = app.frame.height * 0.3
                origin.withOffset(CGVector(dx: 330, dy: y))
                    .press(forDuration: 0.15, thenDragTo: origin.withOffset(CGVector(dx: 330, dy: y + moved - 10)),
                           withVelocity: .slow, thenHoldForDuration: 0.15)
                settle(0.8)
            }
        }
        // And a plain tap on the badge still removes.
        fill()
        let before = tiles()
        app.buttons["feedback.removeScreenshot.1"].tap()
        settle(0.8)
        let line = "tap on badge 1: tiles \(before)→\(tiles())"
        print("VELA_ORIGIN " + line)
        log.append(line)
        XCTAssertEqual(tiles(), before - 1, "a tap on the badge no longer removes")
        shot(app, "hl-tap-removes")
        note(log.joined(separator: "\n"), "hl-results")
        app.terminate()
    }

    // MARK: - Tap a screenshot to preview it (spec C)

    /// Settings → Send feedback → three screenshots → tap tile 2, and every
    /// action's aftermath: the viewer at 2 / 3, paging both ways (and the
    /// bounce at the end), double-tap zoom, a pan while zoomed that neither
    /// pages nor closes, pinch zoom, a swipe down that springs back (captured
    /// mid-swipe) and one that closes, the ✕, then Remove — middle, last,
    /// only. Offline; nothing is sent.
    ///
    /// The pictures are told apart by their shape: seed 1 is a phone
    /// screenshot (887×1920 once processed), 2 is 4:3, 3 is 3:4.
    func testCGViewerFromRow() {
        let app = launchApp(theme: "light", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        openPicker(app)
        pick([1, 2, 3], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 3)
        settle(0.6)
        shot(app, "g00-three-tiles")

        // Open on tile 2 — frames taken while it opens show the picture
        // flying out of the tile.
        capturing("g01a-opening", count: 5, every: 0.07) {
            app.buttons["feedback.viewScreenshot.2"].tap()
        }
        XCTAssertTrue(app.buttons["feedback.viewer.close"].waitForExistence(timeout: 5), "tile 2 did not open the viewer")
        settle(0.5)
        shot(app, "g01-viewer-3-open-at-2")
        let image = viewerImage(app)
        dump(app, "g01-viewer-tree")
        XCTAssertEqual(image.label, "View screenshot 2")
        XCTAssertEqual(image.value as? String, "2 / 3")
        assertShape(image, Self.shape2, "tile 2 opened something else")
        assertBetweenBars(image, in: app)
        XCTAssertTrue(app.buttons["feedback.viewer.remove"].isHittable, "no Remove")
        XCTAssertEqual(app.buttons["feedback.viewer.close"].label, "Close")
        let closeFrame = app.buttons["feedback.viewer.close"].frame
        XCTAssertLessThan(closeFrame.midX, app.frame.width / 2, "the ✕ is not at the leading edge")

        // Paging.
        image.swipeLeft()
        settle(0.9)
        XCTAssertEqual(image.value as? String, "3 / 3", "a swipe left did not page")
        assertShape(image, Self.shape3, "page 3 is not seed 3")
        shot(app, "g02-paged-to-3")
        image.swipeLeft()
        settle(0.9)
        XCTAssertEqual(image.value as? String, "3 / 3", "paged past the last")
        image.swipeRight()
        settle(0.9)
        image.swipeRight()
        settle(0.9)
        XCTAssertEqual(image.value as? String, "1 / 3", "two swipes right did not reach the first")
        assertShape(image, Self.shape1, "page 1 is not seed 1")
        shot(app, "g03-paged-to-1")
        let fit = image.frame

        // Double tap: 2×, and back.
        doubleTapCentre(app)
        settle(0.9)
        print("VELA_ZOOM fit=\(fit) zoomed=\(image.frame)")
        XCTAssertGreaterThan(image.frame.width, fit.width * 1.8, "a double tap did not zoom")
        shot(app, "g04-zoomed-double-tap")
        // Zoomed, a drag pans: no page, no close.
        let before = image.frame
        drag(app, from: CGVector(dx: 0.5, dy: 0.5), to: CGVector(dx: 0.2, dy: 0.3))
        settle(0.8)
        print("VELA_PAN before=\(before) after=\(image.frame)")
        XCTAssertNotEqual(image.frame.origin, before.origin, "a drag while zoomed did not pan")
        XCTAssertEqual(image.value as? String, "1 / 3", "a drag while zoomed paged")
        XCTAssertTrue(viewerOpen(app), "a drag while zoomed closed the viewer")
        shot(app, "g05-zoomed-panned")
        drag(app, from: CGVector(dx: 0.5, dy: 0.35), to: CGVector(dx: 0.5, dy: 0.75))
        settle(0.8)
        XCTAssertTrue(viewerOpen(app), "a drag DOWN while zoomed closed the viewer")
        XCTAssertGreaterThan(image.frame.width, fit.width * 1.5, "a drag down while zoomed un-zoomed it")
        doubleTapCentre(app)
        settle(0.9)
        XCTAssertEqual(image.frame.width, fit.width, accuracy: 2, "a second double tap did not reset")

        // Pinch.
        image.pinch(withScale: 3, velocity: 3)
        settle(1)
        print("VELA_PINCH fit=\(fit) pinched=\(image.frame)")
        XCTAssertGreaterThan(image.frame.width, fit.width * 2, "a pinch did not zoom")
        shot(app, "g06-pinched")
        doubleTapCentre(app)
        settle(0.9)
        XCTAssertEqual(image.frame.width, fit.width, accuracy: 2)

        // A swipe down that stops short springs back — the frame is taken
        // while the finger is still down.
        dragHoldingWithCapture(from: CGVector(dx: 0.5, dy: 0.42), to: CGVector(dx: 0.55, dy: 0.6),
                               hold: 3, captureAfter: 2, name: "g07-mid-swipe-down", in: app)
        settle(1)
        XCTAssertTrue(viewerOpen(app), "a short swipe down closed the viewer")
        XCTAssertEqual(image.frame.minY, fit.minY, accuracy: 2, "it did not spring back")
        shot(app, "g08-sprung-back")

        // A real swipe down closes — back on the sheet, tile 1 in its place.
        drag(app, from: CGVector(dx: 0.5, dy: 0.4), to: CGVector(dx: 0.5, dy: 0.85), velocity: .fast)
        settle(1.2)
        XCTAssertFalse(viewerOpen(app), "a swipe down did not close the viewer")
        XCTAssertTrue(app.buttons["feedback.viewScreenshot.1"].isHittable, "back on the sheet, tile 1 is not there")
        XCTAssertTrue(element(app, "feedback.what").exists, "not back on the report sheet")
        shot(app, "g09-closed-by-swipe")

        // The ✕ — frames taken while it closes show the picture flying back.
        openTile(1, in: app)
        capturing("g10a-closing", count: 5, every: 0.07) {
            app.buttons["feedback.viewer.close"].tap()
        }
        settle(1)
        XCTAssertFalse(viewerOpen(app), "the ✕ did not close the viewer")
        XCTAssertTrue(app.buttons["feedback.viewScreenshot.1"].isHittable)
        shot(app, "g10-closed-by-x")

        // Remove the middle one: the next takes its place.
        openTile(2, in: app)
        app.buttons["feedback.viewer.remove"].tap()
        settle(1)
        XCTAssertTrue(viewerOpen(app), "removing one of three closed the viewer")
        XCTAssertEqual(image.value as? String, "2 / 2")
        assertShape(image, Self.shape3, "after removing the middle, the next did not show")
        shot(app, "g11-removed-middle")
        // Now on the last: removing it shows the previous.
        app.buttons["feedback.viewer.remove"].tap()
        settle(1)
        XCTAssertTrue(viewerOpen(app), "removing one of two closed the viewer")
        XCTAssertEqual(image.label, "View screenshot 1")
        XCTAssertTrue(((image.value as? String) ?? "").isEmpty, "one picture still shows a counter")
        assertShape(image, Self.shape1, "after removing the last, the previous did not show")
        shot(app, "g12-one-image")
        // The only one: the viewer closes, the sheet is empty again.
        app.buttons["feedback.viewer.remove"].tap()
        settle(1.2)
        XCTAssertFalse(viewerOpen(app), "removing the only picture left the viewer open")
        XCTAssertFalse(app.buttons["feedback.viewScreenshot.1"].exists, "a tile is left")
        XCTAssertTrue(app.staticTexts["Optional · up to 5"].exists, "the section is not back to empty")
        XCTAssertTrue(element(app, "feedback.addScreenshots").isHittable, "no add target")
        shot(app, "g13-removed-only")
        app.terminate()
    }

    /// Dark app theme, the largest text: the viewer stays black, its words
    /// grow, and the picture still fits between the bars.
    func testCHViewerDarkLargest() {
        let app = launchApp(theme: "dark", textScale: "xlarge", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        openPicker(app)
        pick([1, 2, 3], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 3)
        settle(0.6)
        shot(app, "h00-dark-xl-tiles")
        openTile(1, in: app)
        let image = viewerImage(app)
        XCTAssertEqual(image.value as? String, "1 / 3")
        assertBetweenBars(image, in: app)
        shot(app, "h01-dark-xl-viewer")
        doubleTapCentre(app)
        settle(0.9)
        shot(app, "h02-dark-xl-zoomed")
        doubleTapCentre(app)
        settle(0.9)
        dragHoldingWithCapture(from: CGVector(dx: 0.5, dy: 0.42), to: CGVector(dx: 0.45, dy: 0.6),
                               hold: 3, captureAfter: 2, name: "h03-dark-xl-mid-swipe-down", in: app)
        settle(1)
        XCTAssertTrue(viewerOpen(app))
        image.swipeLeft()
        settle(0.9)
        shot(app, "h04-dark-xl-page-2")
        app.buttons["feedback.viewer.close"].tap()
        settle(1)
        XCTAssertFalse(viewerOpen(app))
        shot(app, "h05-dark-xl-closed")
        app.terminate()
    }

    /// A scroll that starts on a tile scrolls — it never opens the viewer; a
    /// plain tap still does. (A scroll from the remove badge once deleted a
    /// screenshot; the tile is a tap that fails on movement for the same
    /// reason.)
    func testCIScrollFromTileNeverOpens() {
        let app = launchApp(theme: "light", endpoint: Self.unreachable)
        openFeedbackFromRow(app)
        openPicker(app)
        pick([1, 2], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 2)
        settle(0.6)
        let origin = app.coordinate(withNormalizedOffset: .zero)
        var log: [String] = []
        for (name, dx, dy) in [("up from tile 1", 0.0, -220.0), ("up-left from tile 2", -60.0, -180.0),
                               ("sideways on tile 1", 120.0, 0.0)] as [(String, CGFloat, CGFloat)] {
            let tileNumber = name.contains("tile 2") ? 2 : 1
            let tile = app.buttons["feedback.viewScreenshot.\(tileNumber)"]
            XCTAssertTrue(tile.waitForExistence(timeout: 5))
            let start = CGPoint(x: tile.frame.midX, y: tile.frame.midY)
            let rowTop = tile.frame.minY
            origin.withOffset(CGVector(dx: start.x, dy: start.y))
                .press(forDuration: 0.12, thenDragTo: origin.withOffset(CGVector(dx: start.x + dx, dy: start.y + dy)),
                       withVelocity: .slow, thenHoldForDuration: 0.1)
            settle(1)
            let opened = viewerOpen(app)
            let moved = rowTop - app.buttons["feedback.viewScreenshot.\(tileNumber)"].frame.minY
            let line = "\(name): start=\(start) contentMoved=\(moved) opened=\(opened) tiles=\(tileCount(app))"
            print("VELA_TILE_SCROLL " + line)
            log.append(line)
            XCTAssertFalse(opened, "a drag \(name) opened the viewer")
            XCTAssertEqual(tileCount(app), 2, "a drag \(name) removed a screenshot")
            shot(app, "i-\(name)")
            if opened { app.buttons["feedback.viewer.close"].tap(); settle(1) }
            // Back down by what the content moved — never past the top.
            if moved > 20 {
                let y = app.frame.height * 0.3
                origin.withOffset(CGVector(dx: 330, dy: y))
                    .press(forDuration: 0.12, thenDragTo: origin.withOffset(CGVector(dx: 330, dy: y + moved - 10)),
                           withVelocity: .slow, thenHoldForDuration: 0.1)
                settle(0.8)
            }
        }
        // A tap still opens.
        openTile(1, in: app)
        log.append("tap on tile 1: opened=\(viewerOpen(app))")
        app.buttons["feedback.viewer.close"].tap()
        settle(1)
        // A held press shows the 0.97 press (frame taken while the finger is
        // down, before the half-second hold opens it) — never a dead press.
        let tile = app.buttons["feedback.viewScreenshot.2"]
        let box = CaptureBox()
        let done = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .userInitiated).async {
            Thread.sleep(forTimeInterval: 0.3)
            box.shot = XCUIScreen.main.screenshot()
            done.signal()
        }
        tile.press(forDuration: 1.4)
        _ = done.wait(timeout: .now() + 10)
        if let pressed = box.shot { attach(pressed, "i-pressed-tile-2") }
        XCTAssertTrue(app.buttons["feedback.viewer.close"].waitForExistence(timeout: 5), "a held press opened nothing")
        log.append("held press on tile 2: opened=\(viewerOpen(app))")
        app.buttons["feedback.viewer.close"].tap()
        settle(1)
        XCTAssertEqual(tileCount(app), 2)
        note(log.joined(separator: "\n"), "i-results")
        app.terminate()
    }

    /// No tile opens while the report is on its way (against the runner's
    /// stub, refused after 8 s), and they open again once it is answered.
    func testCJProcessingAndSendingNeverOpen() throws {
        let stub = try StubReportServer(answer: .status(503, #"{"error":"not_configured"}"#, delay: 8))
        stub.start()
        defer { stub.stop() }
        let app = launchApp(theme: "light", endpoint: stub.url)
        openFeedbackFromRow(app)
        // (A tile still PROCESSING cannot be tapped from here: XCUITest
        // holds every synthesized tap until the app is idle, and a spinning
        // tile keeps it busy — three tries, recorded, each tap landed after
        // the tile had finished and rightly opened it. That rule is held by
        // `ScreenshotViewerTests.aProcessingTileDoesNotOpenAndIsNotAPage`.)
        openPicker(app)
        pick([3], in: app)
        confirmPicker(app)
        waitForTiles(app, count: 1)
        settle(0.6)

        type("Viewer inert-while-sending check (device verification, stub endpoint).", into: "feedback.what", in: app)
        tapSend(app)
        settle(0.4)
        let tile = app.buttons["feedback.viewScreenshot.1"]
        print("VELA_SENDING_TILE exists=\(tile.exists) hittable=\(tile.isHittable) frame=\(tile.frame)")
        revealTile(1, in: app)
        XCTAssertTrue(tile.isHittable, "the tile is not reachable while sending")
        let sendingBefore = element(app, "feedback.fallback").exists
        let tapStart = Date()
        tile.tap()
        let tapTook = Date().timeIntervalSince(tapStart)
        let answeredBeforeTapLanded = element(app, "feedback.fallback").exists
        print("VELA_SENDING_TAP took=\(String(format: "%.2f", tapTook))s fallbackBefore=\(sendingBefore) fallbackAfterTap=\(answeredBeforeTapLanded) opened=\(viewerOpen(app))")
        XCTAssertFalse(answeredBeforeTapLanded, "the tap landed after the send was answered — it proves nothing")
        settle(0.8)
        shot(app, "j02-tapped-while-sending")
        XCTAssertFalse(viewerOpen(app), "a tile opened the viewer while the report was sending")
        XCTAssertTrue(element(app, "feedback.fallback").waitForExistence(timeout: 30), "no fallback after the refusal")
        settle(1)
        // Answered: the tiles open again.
        revealTile(1, in: app)
        openTile(1, in: app)
        shot(app, "j03-opens-after-send")
        app.buttons["feedback.viewer.close"].tap()
        settle(1)
        app.terminate()
    }

    // MARK: Viewer plumbing

    /// Processed shapes of the three seeds (width ÷ height).
    static let shape1: CGFloat = 887.0 / 1920.0
    static let shape2: CGFloat = 1600.0 / 1200.0
    static let shape3: CGFloat = 1440.0 / 1920.0

    private func openTile(_ number: Int, in app: XCUIApplication) {
        let tile = app.buttons["feedback.viewScreenshot.\(number)"]
        XCTAssertTrue(tile.waitForExistence(timeout: 5), "no tile \(number)")
        XCTAssertEqual(tile.label, "View screenshot \(number)")
        tile.tap()
        let close = app.buttons["feedback.viewer.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 5), "tile \(number) did not open the viewer")
        settle(0.5)
    }

    /// A double tap in the middle of the screen — a zoomed picture's own
    /// centre may be off screen.
    private func doubleTapCentre(_ app: XCUIApplication) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).doubleTap()
    }

    /// Bring a tile the sheet scrolled away back into reach with short
    /// downward drags — one long one at the top would pull the sheet down.
    private func revealTile(_ number: Int, in app: XCUIApplication) {
        let tile = app.buttons["feedback.viewScreenshot.\(number)"]
        var drags = 0
        while !(tile.exists && tile.isHittable && tile.frame.minY > 60), drags < 6 {
            slowDrag(app, from: 0.35, to: 0.47)
            drags += 1
        }
        print("VELA_REVEAL tile \(number) drags=\(drags) frame=\(tile.frame)")
    }

    private func viewerImage(_ app: XCUIApplication) -> XCUIElement {
        let image = element(app, "feedback.viewer.image")
        XCTAssertTrue(image.waitForExistence(timeout: 5), "no picture in the viewer")
        return image
    }

    private func viewerOpen(_ app: XCUIApplication) -> Bool {
        app.buttons["feedback.viewer.close"].exists
    }

    private func tileCount(_ app: XCUIApplication) -> Int {
        (1...5).filter { app.buttons["feedback.viewScreenshot.\($0)"].exists }.count
    }

    private func assertShape(_ image: XCUIElement, _ shape: CGFloat, _ message: String) {
        let frame = image.frame
        let seen = frame.width / max(1, frame.height)
        XCTAssertEqual(seen, shape, accuracy: 0.03, "\(message) — \(frame)")
    }

    /// The picture sits between the ✕ row and the Remove row, never under
    /// either (C2).
    private func assertBetweenBars(_ image: XCUIElement, in app: XCUIApplication) {
        let close = app.buttons["feedback.viewer.close"].frame
        let remove = app.buttons["feedback.viewer.remove"].frame
        print("VELA_BARS close=\(close) remove=\(remove) image=\(image.frame) screen=\(app.frame)")
        XCTAssertGreaterThanOrEqual(image.frame.minY, close.maxY - 1, "the picture runs under the top bar")
        XCTAssertLessThanOrEqual(image.frame.maxY, remove.minY + 1, "the picture runs under the bottom bar")
    }

    private func drag(_ app: XCUIApplication, from: CGVector, to: CGVector, velocity: XCUIGestureVelocity = .slow) {
        app.coordinate(withNormalizedOffset: from)
            .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: to),
                   withVelocity: velocity, thenHoldForDuration: 0.05)
    }

    /// Frames taken from another thread while `action` runs on this one —
    /// an opening or closing animation, which a screenshot after the tap
    /// would miss.
    private func capturing(_ name: String, count: Int, every interval: TimeInterval, _ action: () -> Void) {
        let box = FramesBox()
        let done = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .userInitiated).async {
            for _ in 0..<count {
                box.append(XCUIScreen.main.screenshot())
                Thread.sleep(forTimeInterval: interval)
            }
            done.signal()
        }
        action()
        _ = done.wait(timeout: .now() + 15)
        for (index, frame) in box.frames.enumerated() { attach(frame, "\(name)-\(index)") }
    }

    /// A frame taken WHILE the finger is still down. XCUITest's gestures
    /// block the main thread, so the screen is read from another thread
    /// during the hold.
    private func dragHoldingWithCapture(from: CGVector, to: CGVector, hold: TimeInterval, captureAfter: TimeInterval,
                                        name: String, in app: XCUIApplication) {
        let box = CaptureBox()
        let done = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .userInitiated).async {
            Thread.sleep(forTimeInterval: captureAfter)
            box.shot = XCUIScreen.main.screenshot()
            done.signal()
        }
        app.coordinate(withNormalizedOffset: from)
            .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: to),
                   withVelocity: .slow, thenHoldForDuration: hold)
        _ = done.wait(timeout: .now() + 15)
        if let captured = box.shot { attach(captured, name) } else { note("no capture", name) }
    }

    // MARK: - The real keyboard in the report

    /// English, then Pinyin (n-i-h-a-o → 你好, via the candidate bar) typed at
    /// a caret put in the MIDDLE of the line, then a second mid-line edit —
    /// each value read back from the field.
    private func typeWithRealKeyboard(_ app: XCUIApplication) {
        let field = app.textViews["feedback.what"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        settle(0.8)
        let original = "Offline check end."
        field.typeText(original)
        settle(0.4)
        shot(app, "c08-keyboard-english")

        let chinese = "你好"
        let pinyin = switchTo(pinyin: true, in: app)
        print("VELA_PINYIN_KEYBOARD=\(pinyin)")
        // The caret between "check " and "end.".
        placeCaret(in: field, after: "Offline check ")
        if pinyin {
            shot(app, "c09a-keyboard-pinyin")
            for letter in ["n", "i", "h", "a", "o"] { tapKey(letter, in: app) }
            settle(0.6)
            shot(app, "c09b-pinyin-composing")
            dump(app, "c09b-pinyin-composing-tree")
            print("VELA_COMPOSING=\((field.value as? String) ?? "")")
            let candidate = pinyinCandidate(chinese, in: app)
            XCTAssertTrue(candidate.exists, "no 你好 candidate in the bar")
            candidate.tap()
            settle(0.6)
            shot(app, "c09c-pinyin-committed")
            XCTAssertTrue(switchTo(pinyin: false, in: app), "could not switch back to English")
        } else {
            note("no Pinyin keyboard on the phone — Chinese typed through the automation path", "c09-no-pinyin")
            field.typeText(chinese)
        }
        var value = (field.value as? String) ?? ""
        print("VELA_AFTER_PINYIN=\(value)")
        assertInsertedInside(value, inserted: chinese, original: original)
        shot(app, "c09d-after-chinese")

        // A second edit in the middle: after "Offline".
        let before = value
        placeCaret(in: field, after: "Offline")
        field.typeText(" (edited)")
        settle(0.5)
        value = (field.value as? String) ?? ""
        print("VELA_AFTER_EDIT=\(value)")
        assertInsertedInside(value, inserted: " (edited)", original: before)
        shot(app, "c10-after-mid-line-edit")
    }

    /// The fallback leaves nobody stranded (v3 B6): no keyboard over it, the
    /// block's top and its "Open GitHub form" button on screen.
    private func assertFallbackInView(_ app: XCUIApplication) {
        let block = element(app, "feedback.fallback")
        let open = app.buttons["feedback.openGithub"]
        print("VELA_FALLBACK_VIEW keyboard=\(app.keyboards.count) block=\(block.frame) open=\(open.frame)")
        XCTAssertEqual(app.keyboards.count, 0, "the keyboard came back over the fallback")
        XCTAssertTrue(open.exists && open.isHittable, "Open GitHub form is not reachable")
        XCTAssertLessThan(open.frame.maxY, app.frame.height, "Open GitHub form is below the screen")
        XCTAssertGreaterThan(block.frame.minY, 0, "the fallback block's title is above the screen")
    }

    /// `inserted` sits strictly INSIDE the line (not at either end), exactly
    /// once, and taking it out gives back the text as it was — no doubled
    /// pinyin letters, nothing lost.
    private func assertInsertedInside(_ value: String, inserted: String, original: String) {
        XCTAssertEqual(value.components(separatedBy: inserted).count - 1, 1, "\(inserted) is not in \(value) exactly once")
        XCTAssertFalse(value.hasSuffix(inserted) || value.hasPrefix(inserted), "\(inserted) landed at an end: \(value)")
        XCTAssertEqual(value.replacingOccurrences(of: inserted, with: ""), original, "the rest of the line changed: \(value)")
        XCTAssertFalse(value.contains("nihao") || value.contains("ni hao"), "raw pinyin was left in the field: \(value)")
    }

    /// Put the caret right after `prefix` with one tap on the first line.
    /// iOS snaps a tap to the nearest word boundary, so the estimate only has
    /// to land in the right half of the last word or the left half of the
    /// next; `assertInsertedInside` checks where it really went.
    private func placeCaret(in field: XCUIElement, after prefix: String) {
        // Measured on this phone: text starts ~5.5 pt in, the first line's
        // middle is ~20 pt down, Latin ≈ 8.4 pt and a CJK glyph ≈ 15 pt at the
        // standard size (Plus Jakarta Sans 15).
        let width = prefix.reduce(CGFloat(0)) { sum, char in
            sum + ((char.unicodeScalars.first?.value ?? 0) > 0x2E80 ? 15 : 8.4)
        }
        let x = 5.5 + width - 2
        field.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: 20)).tap()
        settle(0.7)
    }

    // MARK: - Plumbing

    private func launchApp(theme: String?, textScale: String? = nil, endpoint: String? = FeedbackReportDeviceTests.unreachable,
                           lang: String = "en") -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchArguments += ["-AppleLanguages", lang == "zh" ? "(zh-Hans)" : "(\(lang))"]
        if let theme { app.launchEnvironment["VELA_THEME"] = theme }
        // NSArgumentDomain: read as the stored size for this launch only.
        if let textScale { app.launchArguments += ["-vela.textScale", textScale] }
        if let endpoint { app.launchEnvironment["VELA_BUG_REPORT_ENDPOINT"] = endpoint }
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        settle(5)
        return app
    }

    /// The Settings tab — the bar is icons only; its buttons keep their names.
    private func openSettings(_ app: XCUIApplication) {
        let tabs = app.buttons.matching(NSPredicate(format: "label IN %@", ["Settings", "设置", "Einstellungen", "Configuración", "Ajustes"]))
        let tab = tabs.firstMatch
        XCTAssertTrue(tab.waitForExistence(timeout: 30), "no Settings tab")
        tab.tap()
        settle(2)
    }

    /// Scroll the settings page until `label` is on screen above the tab bar.
    private func scrollSettings(to label: String, in app: XCUIApplication) {
        let target = app.staticTexts[label]
        var drags = 0
        while drags < 12 {
            if target.exists, target.isHittable, target.frame.maxY < app.frame.height * 0.8 { break }
            slowDrag(app, from: 0.7, to: 0.35)
            drags += 1
        }
        XCTAssertTrue(target.exists && target.isHittable, "could not scroll to \(label)")
    }

    /// Settings → scroll → tap the Send feedback row, as a person would.
    private func openFeedbackFromRow(_ app: XCUIApplication) {
        if !app.staticTexts["Send feedback"].exists { openSettings(app) }
        scrollSettings(to: "Send feedback", in: app)
        app.staticTexts["Send feedback"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(element(app, "feedback.addScreenshots").waitForExistence(timeout: 10),
                      "the Send feedback row did not open the report sheet")
        settle(1)
    }

    private func closeSheet(_ app: XCUIApplication) {
        let close = app.buttons["xmark"]
        XCTAssertTrue(close.waitForExistence(timeout: 5), "no ✕ on the sheet")
        close.tap()
        settle(1)
    }

    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id].firstMatch
    }

    private func openPicker(_ app: XCUIApplication) {
        let add = element(app, "feedback.addScreenshots")
        XCTAssertTrue(add.waitForExistence(timeout: 10), "no add target")
        if !add.isHittable { slowDrag(app, from: 0.3, to: 0.8) }
        add.tap()
        settle(2.5)
    }

    /// The picker's grid cells (iOS 26: `PXGGridLayout-Info`), in reading
    /// order. The grid is NEWEST FIRST, so the three seeded images are the
    /// first three cells: test 3, test 2, test 1.
    private func photoCells(_ app: XCUIApplication) -> [XCUIElement] {
        app.images.matching(identifier: "PXGGridLayout-Info").allElementsBoundByIndex
            .sorted { a, b in
                abs(a.frame.minY - b.frame.minY) > 4 ? a.frame.minY < b.frame.minY : a.frame.minX < b.frame.minX
            }
    }

    /// Tap seeded images, in the order given (1 = the PNG, 2 = the 1600×1200
    /// GPS JPEG, 3 = the 12 MP GPS JPEG). Refuses unless the first three
    /// cells all carry the seed's timestamp (`TEST_RUNNER_VELA_SEED_LABEL`),
    /// so a real photo is never picked.
    private func pick(_ numbers: [Int], in app: XCUIApplication) {
        let cells = photoCells(app)
        if cells.count < 3 { dump(app, "pick-no-cells-tree") }
        XCTAssertGreaterThanOrEqual(cells.count, 3, "the picker shows fewer than three photos")
        let newest = Array(cells.prefix(3))
        let seedLabel = env["VELA_SEED_LABEL"] ?? "unset"
        for cell in newest {
            XCTAssertTrue(cell.label.hasPrefix("Photo") && cell.label.hasSuffix(seedLabel),
                          "refusing to pick \(cell.label) — not a seeded image (\(seedLabel))")
        }
        for number in numbers {
            let cell = newest[3 - number]
            print("VELA_PICK \(number): \(cell.label) \(cell.frame)")
            // The picker is a remote view: its cells are never "hittable" to
            // XCUITest, so the tap goes to the cell's centre on screen.
            app.coordinate(withNormalizedOffset: .zero)
                .withOffset(CGVector(dx: cell.frame.midX, dy: cell.frame.midY)).tap()
            settle(0.6)
        }
    }

    private func confirmPicker(_ app: XCUIApplication) {
        for label in ["Add", "添加", "Done", "完成"] {
            let button = app.buttons[label]
            if button.exists, button.isEnabled {
                app.coordinate(withNormalizedOffset: .zero)
                    .withOffset(CGVector(dx: button.frame.midX, dy: button.frame.midY)).tap()
                return
            }
        }
        dump(app, "confirm-picker-tree")
        XCTFail("no Add/Done button on the picker")
    }

    /// A screenshot while a tile is still a spinner — the processing state.
    private func captureProcessing(_ app: XCUIApplication, name: String) {
        let deadline = Date().addingTimeInterval(6)
        var captured = false
        var polls = 0
        let started = Date()
        while Date() < deadline {
            polls += 1
            if app.activityIndicators.count > 0 {
                shot(app, name)
                captured = true
                break
            }
            Thread.sleep(forTimeInterval: 0.05)
        }
        print("VELA_PROCESSING captured=\(captured) polls=\(polls)")
        if !captured { shot(app, name + "-missed") }
        // How long the tiles took, from the picker closing to no spinner.
        let end = Date().addingTimeInterval(30)
        while app.activityIndicators.count > 0, Date() < end { Thread.sleep(forTimeInterval: 0.05) }
        print("VELA_PROCESSING_SECONDS=\(String(format: "%.2f", Date().timeIntervalSince(started)))")
    }

    private func waitForTiles(_ app: XCUIApplication, count: Int) {
        let last = app.buttons["feedback.removeScreenshot.\(count)"]
        XCTAssertTrue(last.waitForExistence(timeout: 30), "tile \(count) never appeared")
        let deadline = Date().addingTimeInterval(30)
        while app.activityIndicators.count > 0, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.2)
        }
        XCTAssertEqual(app.activityIndicators.count, 0, "a tile is still processing after 30 s")
        XCTAssertTrue(app.staticTexts["\(count) / 5"].exists, "the counter does not read \(count) / 5")
    }

    private func type(_ text: String, into id: String, in app: XCUIApplication) {
        let field = app.textViews[id]
        XCTAssertTrue(field.waitForExistence(timeout: 5), "no \(id) field")
        if !field.isHittable { slowDrag(app, from: 0.3, to: 0.8) }
        field.tap()
        settle(0.8)
        field.typeText(text)
        settle(0.5)
    }

    /// Scroll the sheet with a finger-speed drag (a flick does not scroll it
    /// with the keyboard up on this phone — XCUITest's `swipeUp` either).
    private func slowDrag(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: from))
            .press(forDuration: 0.15, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: to)),
                   withVelocity: .slow, thenHoldForDuration: 0.15)
        settle(0.7)
    }

    /// Send, scrolled into reach — with the keyboard still up if it is up: a
    /// person does not have to put it away first.
    private func tapSend(_ app: XCUIApplication) {
        let send = app.buttons["feedback.send"]
        XCTAssertTrue(send.waitForExistence(timeout: 5), "no send button")
        var drags = 0
        while !send.isHittable, drags < 6 {
            slowDrag(app, from: 0.55, to: 0.2)
            drags += 1
        }
        XCTAssertTrue(send.isHittable, "send is not reachable")
        XCTAssertTrue(send.isEnabled, "send is disabled")
        print("VELA_SEND_REACHED drags=\(drags) keyboard=\(app.keyboards.count)")
        send.tap()
    }

    // MARK: Keyboards

    /// Switch to (or away from) the Pinyin keyboard with the globe key. On a
    /// Face ID iPhone the globe sits in the strip BELOW the keyboard and is not
    /// in the accessibility tree, so it is tapped where it is drawn: 16 pt in
    /// from the left, halfway down the strip under the keyboard.
    private func switchTo(pinyin: Bool, in app: XCUIApplication) -> Bool {
        for attempt in 0..<6 {
            if pinyinActive(app) == pinyin { return true }
            let keyboard = app.keyboards.firstMatch
            guard keyboard.exists else { return false }
            let stripTop = keyboard.frame.maxY
            let stripMid = (stripTop + app.frame.height) / 2
            app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 40, dy: stripMid)).tap()
            settle(0.9)
            note(app.keyboards.firstMatch.debugDescription, "kb-after-globe-\(pinyin ? "to" : "from")-pinyin-\(attempt)")
        }
        return pinyinActive(app) == pinyin
    }

    /// The Pinyin keyboard's space and return keys read in Chinese.
    private func pinyinActive(_ app: XCUIApplication) -> Bool {
        let keyboard = app.keyboards.firstMatch
        guard keyboard.exists else { return false }
        let chinese = NSPredicate(format: "label IN %@ OR label BEGINSWITH '拼音' OR label BEGINSWITH '简体'",
                                  ["空格", "换行", "确认", "选拼音", "中文"])
        return keyboard.keys.matching(chinese).count > 0 || keyboard.buttons.matching(chinese).count > 0
    }

    private func tapKey(_ letter: String, in app: XCUIApplication) {
        let keyboard = app.keyboards.firstMatch
        let key = keyboard.keys[letter].exists ? keyboard.keys[letter] : keyboard.keys[letter.uppercased()]
        XCTAssertTrue(key.exists, "no \(letter) key")
        key.tap()
        settle(0.15)
    }

    private func pinyinCandidate(_ text: String, in app: XCUIApplication) -> XCUIElement {
        let exact = app.keyboards.descendants(matching: .any)
            .matching(NSPredicate(format: "label == %@", text)).firstMatch
        if exact.exists { return exact }
        return app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", text)).firstMatch
    }

    // MARK: Leaving the app

    /// Which app took the link, and the URL it shows when it is Safari.
    private func foregroundDescription(expecting host: String) -> String {
        let candidates: [(String, String)] = [
            ("com.apple.mobilesafari", "Safari"),
            ("com.atebits.Tweetie2", "X"),
            ("ph.telegra.Telegraph", "Telegram"),
            ("com.hammerandchisel.discord", "Discord"),
        ]
        for (bundle, name) in candidates {
            let other = XCUIApplication(bundleIdentifier: bundle)
            if other.state == .runningForeground {
                if bundle == "com.apple.mobilesafari" {
                    dump(other, "safari-tree-\(host.prefix(12))")
                    let url = safariURL(other)
                    return "\(name): \(url) (expected \(host)) match=\(url.contains(host.components(separatedBy: "/").first ?? host))"
                }
                return "\(name) app (universal link for \(host))"
            }
        }
        return "unknown foreground app (expected \(host))"
    }

    private func safariURL(_ safari: XCUIApplication) -> String {
        for id in ["URL", "TabBarItemTitle", "Address"] {
            let field = safari.descendants(matching: .any)[id].firstMatch
            if field.exists {
                let value = (field.value as? String) ?? ""
                return value.isEmpty ? field.label : value
            }
        }
        dump(safari, "safari-tree")
        return "?"
    }

    // MARK: Photos authorization (runner)

    private func authorize() throws {
        let current = PHPhotoLibrary.authorizationStatus(for: .readWrite)
        if current == .authorized { return }
        let answered = expectation(description: "photos authorization")
        var status = current
        PHPhotoLibrary.requestAuthorization(for: .readWrite) { result in
            status = result
            answered.fulfill()
        }
        let deadline = Date().addingTimeInterval(20)
        while Date() < deadline {
            if tapSystemButton(["Allow Full Access", "允许完全访问", "Allow Access to All Photos", "允许访问所有照片"]) {
                Thread.sleep(forTimeInterval: 1)
            }
            if status != .notDetermined { break }
            Thread.sleep(forTimeInterval: 0.4)
        }
        wait(for: [answered], timeout: 20)
        print("VELA_PHOTOS_AUTH=\(status.rawValue)")
        XCTAssertTrue(status == .authorized || status == .limited, "photos access was not granted (\(status.rawValue))")
    }

    @discardableResult
    private func tapSystemButton(_ labels: [String]) -> Bool {
        for label in labels {
            let button = springboard.buttons[label].firstMatch
            if button.exists {
                button.tap()
                return true
            }
        }
        return false
    }

    // MARK: Stub payload

    /// What the app REALLY sent: the new fields, and screenshots that are
    /// JPEGs with no APP1 (EXIF/GPS) and an edge ≤ 1920 — saved as attachments.
    private func inspect(_ stub: StubReportServer, expectScreenshots count: Int) {
        let bodies = stub.bodies
        XCTAssertEqual(bodies.count, 1, "the stub saw \(bodies.count) requests")
        guard let body = bodies.first,
              let json = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
        else { return XCTFail("the stub got no JSON") }
        let shots = (json["screenshots"] as? [String]) ?? []
        var summary = "keys=\(json.keys.sorted())\nclient=\(json["client"] ?? "-") os=\(json["os"] ?? "-") appVersion=\(json["appVersion"] ?? "-")\n"
        summary += "what=\(json["what"] ?? "-")\nenvironment=\(json["environment"] ?? "-")\nscreenshots=\(shots.count) bodyBytes=\(body.count)\n"
        XCTAssertEqual(json["client"] as? String, "ios")
        XCTAssertTrue((json["os"] as? String)?.hasPrefix("iOS ") == true)
        XCTAssertEqual(shots.count, count)
        for (index, base64) in shots.enumerated() {
            guard let data = Data(base64Encoded: base64) else { XCTFail("screenshot \(index) is not base64"); continue }
            let isJPEG = data.starts(with: [0xFF, 0xD8, 0xFF])
            let app1 = Self.hasAPP1(data)
            var width = 0, height = 0, gps = false
            if let source = CGImageSourceCreateWithData(data as CFData, nil),
               let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] {
                width = props[kCGImagePropertyPixelWidth] as? Int ?? 0
                height = props[kCGImagePropertyPixelHeight] as? Int ?? 0
                gps = props[kCGImagePropertyGPSDictionary] != nil
            }
            summary += "screenshot \(index + 1): \(data.count) bytes jpeg=\(isJPEG) app1=\(app1) gps=\(gps) \(width)x\(height)\n"
            XCTAssertTrue(isJPEG, "screenshot \(index + 1) is not a JPEG")
            XCTAssertFalse(app1, "screenshot \(index + 1) carries an APP1 (EXIF) segment")
            XCTAssertFalse(gps, "screenshot \(index + 1) carries GPS")
            XCTAssertLessThanOrEqual(max(width, height), 1920)
            let attachment = XCTAttachment(data: data, uniformTypeIdentifier: UTType.jpeg.identifier)
            attachment.name = "sent-screenshot-\(index + 1)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
        print("VELA_STUB_PAYLOAD\n\(summary)")
        note(summary, "stub-payload")
    }

    private static func hasAPP1(_ jpeg: Data) -> Bool {
        let bytes = [UInt8](jpeg)
        var index = 2
        while index + 4 <= bytes.count, bytes[index] == 0xFF {
            let marker = bytes[index + 1]
            if marker == 0xDA { return false }
            if marker == 0xE1 { return true }
            index += 2 + (Int(bytes[index + 2]) << 8 | Int(bytes[index + 3]))
        }
        return false
    }

    // MARK: Small things

    private func waitUntil(timeout: TimeInterval, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            Thread.sleep(forTimeInterval: 0.3)
        }
        return condition()
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        attach(app.screenshot(), name)
    }

    private func attach(_ screenshot: XCUIScreenshot, _ name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func dump(_ app: XCUIApplication, _ name: String) {
        note(app.debugDescription, name)
    }

    private func note(_ text: String, _ name: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    // MARK: - Generated images

    /// A phone-sized "screenshot" (1170 × 2532) that says what it is.
    static func screenshotPNG(number: Int) -> Data {
        let size = CGSize(width: 1170, height: 2532)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        return UIGraphicsImageRenderer(size: size, format: format).pngData { context in
            UIColor(red: 0.97, green: 0.96, blue: 0.94, alpha: 1).setFill()
            context.fill(CGRect(origin: .zero, size: size))
            draw("9:41", at: CGPoint(x: 90, y: 50), size: 48, weight: .semibold, color: .black)
            draw("Vela feedback test \(number)", at: CGPoint(x: 90, y: 260), size: 96, weight: .bold, color: .black)
            draw("Generated for device verification — not a real screenshot.",
                 at: CGPoint(x: 90, y: 400), size: 40, weight: .regular, color: .darkGray)
            let colors: [UIColor] = [.systemBlue, .systemGreen, .systemOrange, .systemPurple, .systemTeal]
            for row in 0..<5 {
                let y = 560 + CGFloat(row) * 260
                let card = UIBezierPath(roundedRect: CGRect(x: 60, y: y, width: 1050, height: 220), cornerRadius: 40)
                UIColor.white.setFill()
                card.fill()
                colors[row].setFill()
                UIBezierPath(ovalIn: CGRect(x: 110, y: y + 50, width: 120, height: 120)).fill()
                draw("Test row \(row + 1)", at: CGPoint(x: 280, y: y + 60), size: 52, weight: .semibold, color: .black)
                draw("placeholder content", at: CGPoint(x: 280, y: y + 130), size: 36, weight: .regular, color: .gray)
            }
            draw("#\(number)", at: CGPoint(x: 90, y: 2000), size: 300, weight: .heavy,
                 color: UIColor.systemIndigo.withAlphaComponent(0.25))
        }
    }

    /// A JPEG tagged with a GPS position (the Eiffel Tower) — the metadata the
    /// report must never carry.
    static func gpsJPEG(number: Int, size: CGSize) -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        let image = UIGraphicsImageRenderer(size: size, format: format).image { context in
            let colors = [UIColor.systemTeal.cgColor, UIColor.systemIndigo.cgColor] as CFArray
            let gradient = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 1])!
            context.cgContext.drawLinearGradient(gradient, start: .zero,
                                                 end: CGPoint(x: size.width, y: size.height), options: [])
            let scale = size.width / 1600
            draw("Vela feedback test \(number)", at: CGPoint(x: 80 * scale, y: 120 * scale),
                 size: 110 * scale, weight: .bold, color: .white)
            draw("GPS-tagged JPEG · \(Int(size.width))×\(Int(size.height))",
                 at: CGPoint(x: 80 * scale, y: 280 * scale), size: 60 * scale, weight: .medium, color: .white)
            draw("Generated for device verification.", at: CGPoint(x: 80 * scale, y: 380 * scale),
                 size: 50 * scale, weight: .regular, color: .white)
        }
        let out = NSMutableData()
        let destination = CGImageDestinationCreateWithData(out, UTType.jpeg.identifier as CFString, 1, nil)!
        let gps: [CFString: Any] = [
            kCGImagePropertyGPSLatitude: 48.85837, kCGImagePropertyGPSLatitudeRef: "N",
            kCGImagePropertyGPSLongitude: 2.294481, kCGImagePropertyGPSLongitudeRef: "E",
            kCGImagePropertyGPSAltitude: 35.0, kCGImagePropertyGPSAltitudeRef: 0,
            kCGImagePropertyGPSDateStamp: "2026:09:27", kCGImagePropertyGPSTimeStamp: "12:00:00",
        ]
        let exif: [CFString: Any] = [
            kCGImagePropertyExifDateTimeOriginal: "2026:09:27 12:00:00",
            kCGImagePropertyExifUserComment: "Vela feedback test \(number) - GPS tagged",
        ]
        let tiff: [CFString: Any] = [kCGImagePropertyTIFFMake: "VelaTest", kCGImagePropertyTIFFModel: "Generated"]
        let properties: [CFString: Any] = [
            kCGImagePropertyGPSDictionary: gps,
            kCGImagePropertyExifDictionary: exif,
            kCGImagePropertyTIFFDictionary: tiff,
            kCGImageDestinationLossyCompressionQuality: 0.92,
        ]
        CGImageDestinationAddImage(destination, image.cgImage!, properties as CFDictionary)
        CGImageDestinationFinalize(destination)
        return out as Data
    }

    private static func draw(_ text: String, at point: CGPoint, size: CGFloat,
                             weight: UIFont.Weight, color: UIColor) {
        (text as NSString).draw(at: point, withAttributes: [
            .font: UIFont.systemFont(ofSize: size, weight: weight),
            .foregroundColor: color,
        ])
    }
}

/// The report endpoint, stubbed inside the test runner on 127.0.0.1 — the app
/// and the runner share loopback on the phone. Reads each request to its
/// Content-Length, keeps the body, and answers after a delay.
final class StubReportServer {
    enum Answer {
        case filed(delay: TimeInterval)
        case status(Int, String, delay: TimeInterval)
    }

    static let port: UInt16 = 8139
    var url: String { "http://127.0.0.1:\(Self.port)/api/bug-report" }

    private let listener: NWListener
    private let answer: Answer
    private let queue = DispatchQueue(label: "vela.stub-report")
    private let lock = NSLock()
    private var received: [Data] = []

    var bodies: [Data] {
        lock.lock()
        defer { lock.unlock() }
        return received
    }

    init(answer: Answer) throws {
        self.answer = answer
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: Self.port)!)
    }

    func start() {
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { return }
            connection.start(queue: self.queue)
            self.read(connection, Data())
        }
        listener.start(queue: queue)
    }

    func stop() {
        listener.cancel()
    }

    private func read(_ connection: NWConnection, _ buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, done, error in
            guard let self else { return }
            var buffer = buffer
            if let data { buffer.append(data) }
            if let (headerEnd, length) = Self.frame(buffer), buffer.count >= headerEnd + length {
                let body = buffer.subdata(in: headerEnd..<(headerEnd + length))
                self.lock.lock()
                self.received.append(body)
                self.lock.unlock()
                self.respond(connection)
            } else if done || error != nil {
                connection.cancel()
            } else {
                self.read(connection, buffer)
            }
        }
    }

    /// The end of the headers and the Content-Length, once the headers are in.
    private static func frame(_ buffer: Data) -> (Int, Int)? {
        guard let range = buffer.range(of: Data("\r\n\r\n".utf8)) else { return nil }
        let head = String(decoding: buffer[..<range.lowerBound], as: UTF8.self)
        var length = 0
        for line in head.split(separator: "\r\n") where line.lowercased().hasPrefix("content-length:") {
            length = Int(line.split(separator: ":")[1].trimmingCharacters(in: .whitespaces)) ?? 0
        }
        return (range.upperBound, length)
    }

    private func respond(_ connection: NWConnection) {
        let (status, body, delay): (Int, String, TimeInterval) = switch answer {
        case .filed(let delay):
            (200, #"{"number":999999,"url":"https://example.invalid/stub-issue/999999","deduped":false,"screenshots":2,"screenshotsDropped":0}"#, delay)
        case .status(let code, let body, let delay):
            (code, body, delay)
        }
        queue.asyncAfter(deadline: .now() + delay) {
            let payload = Data(body.utf8)
            let header = "HTTP/1.1 \(status) \(status == 200 ? "OK" : "Error")\r\n"
                + "Content-Type: application/json\r\n"
                + "Content-Length: \(payload.count)\r\n"
                + "Connection: close\r\n\r\n"
            var response = Data(header.utf8)
            response.append(payload)
            connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
        }
    }
}

/// The frame a background thread took during a held gesture.
private final class CaptureBox: @unchecked Sendable {
    var shot: XCUIScreenshot?
}

private final class FramesBox: @unchecked Sendable {
    private let lock = NSLock()
    private var shots: [XCUIScreenshot] = []
    var frames: [XCUIScreenshot] { lock.lock(); defer { lock.unlock() }; return shots }
    func append(_ shot: XCUIScreenshot) { lock.lock(); shots.append(shot); lock.unlock() }
}
