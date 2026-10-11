//
//  PR4DeviceTests.swift
//  VelaWalletUITests
//
//  The second device walk's fixes, on a phone (PR 4 device round). Every test
//  is read-only towards the person's real wallet: nothing is ever confirmed,
//  signed or sent — every dApp request and every sheet is closed with its ✕
//  or its Cancel — no wallet is created, no key registered, no report
//  submitted, no record deleted, no contact written.
//
//  The ONE approval this file ever gives is the connect consent of the local
//  test dApp (`http://127.0.0.1:8137`), inside the parallel space, and only
//  when ONE snapshot of the tree holds the space's badge, the asking site and
//  the test wallet's address on the sheet (`connectInSpace`). Anything else
//  that asks — on any account — is refused (`refuseAnythingAsked`), and
//  `tearDownWithError` refuses whatever is still up when a test ends: the
//  test dApp's "Send dust" pays the person's real address.
//
//  The one setting moved is hide balance (`testHiddenBalanceKeepsUnits`): it
//  is read first, must equal `KEPT_HIDDEN`, and is put back at once.
//
//  Scrolls on the person's own pages are drags down the page's leading
//  margin (`safeDrag`), where no row, slider or button stands.
//
//  On the person's own wallet — LOOK ONLY:
//  - `testAsFound` — launches nothing, taps nothing: what is in front.
//  - `testHeroColdStart` — the hero's status and its refresh control on one
//    line, the figure and the first row under the hero still from
//    "Checking…" to "Updated", every figure in the core's sign.
//  - `testSettingsKeysSteady` — Settings from a cold start, one snapshot per
//    tick while the keys block loads: nothing under it moves.
//  - `testStatusBarGround` — Settings and the home scrolled: nothing drawn
//    behind the clock.
//  - `testCurrencyRowAndPicker` — the row, and the picker opened and closed
//    with nothing picked.
//  - `testReportSheet` — the facts name networks; the ✕ is in a header the
//    body scrolls under. Nothing typed, attached or sent.
//  - `testHistoryAndTransferDetail` — History left idle (the kernel's
//    EXC_GUARD line was seen there), then a transfer's detail: "Delete
//    record" is a quiet text action. It is NEVER tapped.
//  - `testWhichPageTheKernelLineFollows` — the Assets page and History, each
//    opened twice and left alone, with the clock of every tap noted.
//  - `testIdenticonSheet` — both actions whole, above the home indicator.
//  - `testHiddenBalanceKeepsUnits` — see above.
//  - `testOwnerRecords` — the contacts, Settings' Advanced rows, and each
//    Explore tab brought forward and read: its account chip, its site
//    menu's line, its mark. `RECORDS_LABEL` names the pass (before / after
//    the space); the two must read the same.
//
//  In the parallel space (`VELA_PARALLEL_SPACE=1`):
//  - `testSpaceConnectAndMarks` — the test dApp: the local mark (never a
//    padlock) on the bar, the menu, the consent and the panel; "this site is
//    not connected" under the bar after a refused personal_sign, gone once
//    the site connects; the page answers the test wallet's account.
//  - `testSpaceSigningSheetSteady` — "Send dust", one snapshot per tick from
//    the sheet's first frame: the ✕, the header, the fee row, the signing
//    account and the confirm keep one place each. DECLINED by the ✕.
//  - `testSpaceSplitRemainder` — Send to several recipients: the remainder
//    reads like the balance beside it. Left by back, nothing sent.
//  - `testLeaveSpace` — `VELA_PARALLEL_SPACE=0`: the wallet that was in front.
//  - `testRoundTrip1…6` — the records before, the three looks inside, the
//    way out and the records after, in that order from ONE install.
//
//  `PR2PolishDeviceTests/testState` before and after must match.
//
//  Skipped in the scheme (they need the phone, the network and the space);
//  run them by name from a copy of the .xctestrun with the skips removed.
//

import Foundation
import UIKit
import XCTest

final class PR4DeviceTests: XCTestCase {

    /// The parallel space's test wallet, as a sheet writes it.
    private static let testWalletShort = "0x88cC…6894"
    private static let testWalletFull = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    /// The site this walk may connect, and nothing else.
    private static let testSite = "127.0.0.1:8137"
    private static let appId = "app.getvela.VelaWallet"

    private var server: LocalDappServer?
    /// The app as a test launched it — so that a test which stops half-way
    /// never leaves a request's sheet open behind it.
    private var launched: XCUIApplication?
    /// Approvals given by this test (each is the test dApp's connect, in the
    /// space) and anything refused that the walk did not ask for.
    private var approvals: [String] = []
    private var incidents: [String] = []
    /// The Explore tabs' rows as the first records pass of this run found
    /// them: a later pass brings the tabs forward so they end in this order.
    private static var tabsAsFound: [String] = []

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    override func tearDownWithError() throws {
        // Whatever still asks is REFUSED before anything else can happen to
        // the app: a sheet must never be left open unattended.
        if let app = launched, app.state == .runningForeground {
            refuseAnythingAsked(app, context: "tearDown")
        }
        if !approvals.isEmpty { note("APPROVALS", approvals.joined(separator: "\n")) }
        if !incidents.isEmpty {
            note("INCIDENTS", incidents.joined(separator: "\n"))
            XCTFail("incidents: \(incidents)")
        }
        launched = nil
        server?.stop()
        server = nil
    }

    // MARK: - 0. What is in front, before anything is launched

    /// Launches nothing and taps nothing. Whether the app is running and in
    /// front; if it is, its screen and its tree. No picture is taken of any
    /// other app.
    ///
    /// By bundle identifier, so that it can run from an .xctestrun with no
    /// target application: installing the build under test ends the app that
    /// is running, and this is about the one the person left.
    func testAsFound() throws {
        let app = XCUIApplication(bundleIdentifier: Self.appId)
        let names = ["unknown", "notRunning", "runningBackgroundSuspended", "runningBackground", "runningForeground"]
        let state = Int(app.state.rawValue)
        note("asfound-state", "Vela Wallet: \(state < names.count ? names[state] : "\(state)") at \(Self.clock())")
        if app.state == .runningForeground {
            shoot(app, "asfound-screen")
            dump(app, "asfound")
        }
    }

    // MARK: - 1. Cold start: the hero's one line, the core's sign (items 4, 8, 1)

    /// The person's own wallet, exactly as they launch it, and once more with
    /// the launch animation skipped. ONE snapshot per tick: the caption, the
    /// figure, what the line says, the refresh control and the first row
    /// under the hero — where each is while it checks and once it has.
    func testHeroColdStart() throws {
        // How long the home is watched after it first says when it was updated
        // (`HERO_AFTER`), and at most (`HERO_CAP`): a slow network keeps a
        // status on the line for a while before the stamp's own words show.
        let env = ProcessInfo.processInfo.environment
        let after = TimeInterval(env["HERO_AFTER"] ?? "") ?? 8
        let cap = TimeInterval(env["HERO_CAP"] ?? "") ?? 50
        var report: [String] = []
        for skipAnimation in [false, true] {
            let tag = skipAnimation ? "skip" : "real"
            let app = XCUIApplication()
            app.terminate()
            if skipAnimation { app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1" }
            let started = Date()
            app.launch()
            launched = app

            var lines: [String] = []
            var figureY = Set<CGFloat>(), rowY = Set<CGFloat>(), controlY = Set<CGFloat>(), controlBox = Set<String>()
            var apart: [String] = []
            var foreign: [String] = []
            var checkingAt: (figure: CGFloat, row: CGFloat)?
            var updatedAt: (figure: CGFloat, row: CGFloat)?
            var updatedSince: TimeInterval?
            var lastWords = "", index = 0, shots = 0
            var said: [String] = []
            while Date().timeIntervalSince(started) < cap {
                let at = Date().timeIntervalSince(started)
                guard let seen = see(app) else { settle(0.2); continue }
                let figure = seen.native.first { node in
                    node.elementType == .button
                        && ((node.label.hasSuffix(" CNY") && !node.label.contains("·")) || node.label == "显示余额")
                }
                let receive = seen.native.first { $0.elementType == .button && $0.label == "收款" }
                guard let receive else {
                    lines.append(String(format: "t=%.2f (no home yet)", at))
                    index += 1
                    continue
                }
                let caption = seen.texts.first { $0.label.hasPrefix("总余额") }
                let checking = seen.id("balance-checking")
                let status = seen.id("balance-status")
                let control = seen.id("balance-refresh")
                let line = checking ?? status
                // With nothing more pressing on the line, the control's own
                // words are drawn in it (they are its name either way).
                let words = line?.label ?? control.map { "(the stamp) \($0.label)" } ?? "(nothing)"
                if said.last != words { said.append(words) }
                if let figure { figureY.insert(Self.tenth(figure.frame.minY)) }
                rowY.insert(Self.tenth(receive.frame.minY))
                if let control {
                    controlY.insert(Self.tenth(control.frame.minY))
                    controlBox.insert("\(Self.tenth(control.frame.minX)),\(Self.tenth(control.frame.minY)) \(control.frame.width)x\(control.frame.height)")
                    if let line, abs(line.frame.midY - control.frame.midY) > 2 {
                        apart.append(String(format: "t=%.2f words mid %.1f, control mid %.1f", at, line.frame.midY, control.frame.midY))
                    }
                }
                // A figure in a sign that is not this reader's: "CN¥", or a dollar.
                for text in seen.texts where text.label.contains("CN¥") || text.label.hasPrefix("$") || text.label.contains("≈$") {
                    foreign.append(String(format: "t=%.2f %@", at, text.label))
                }
                if checking != nil, checkingAt == nil, let figure { checkingAt = (figure.frame.minY, receive.frame.minY) }
                if checking == nil, let control, control.label.hasPrefix("上次更新"), let figure {
                    updatedAt = (figure.frame.minY, receive.frame.minY)
                    if updatedSince == nil { updatedSince = at }
                }
                lines.append(String(
                    format: "t=%.2f caption=%@ figure=%@@%.1f..%.1f line=[%@]@%@ control=[%@]@%@ receive.y=%.1f",
                    at, caption?.label ?? "-", figure?.label ?? "(no figure)", figure?.frame.minY ?? -1, figure?.frame.maxY ?? -1, words,
                    line.map { String(format: "%.1f..%.1f", $0.frame.minY, $0.frame.maxY) } ?? "-",
                    control?.label ?? "-",
                    control.map { String(format: "%.1f,%.1f %.0fx%.0f", $0.frame.minX, $0.frame.minY, $0.frame.width, $0.frame.height) } ?? "-",
                    receive.frame.minY
                ))
                if shots < 4 || words != lastWords {
                    shoot(app, String(format: "hero-%@-%02d", tag, index))
                    shots += 1
                }
                lastWords = words
                index += 1
                if let updatedSince, at - updatedSince > after { break }
                if at > 20 { settle(0.6) }
            }
            note("hero-\(tag)-timeline", lines.joined(separator: "\n"))
            shoot(app, "hero-\(tag)-end")
            dump(app, "hero-\(tag)-end")
            if let seen = see(app) { note("hero-\(tag)-titles", Self.titles(seen)) }
            report.append("""
            \(tag): figure y \(figureY.sorted()) · first row under the hero y \(rowY.sorted()) · control \(controlBox.sorted()) \
            · while checking \(checkingAt.map { "figure \($0.figure) row \($0.row)" } ?? "(not caught)") \
            · once updated \(updatedAt.map { "figure \($0.figure) row \($0.row)" } ?? "(not reached)") \
            · the line said, in order: \(said.joined(separator: " → ")) \
            · words and control apart: \(apart.isEmpty ? "never" : apart.joined(separator: "; ")) \
            · foreign signs: \(foreign.isEmpty ? "none" : foreign.joined(separator: "; "))
            """)
            XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "this walk is the person's own wallet, not the space")
            XCTAssertEqual(figureY.count, 1, "the figure moved (\(tag)): \(figureY.sorted())")
            XCTAssertEqual(rowY.count, 1, "the first row under the hero moved (\(tag)): \(rowY.sorted())")
            XCTAssertEqual(controlY.count, 1, "the refresh control moved or never showed (\(tag)): \(controlY.sorted())")
            XCTAssertTrue(apart.isEmpty, "the line's words and the refresh control are not one line (\(tag)): \(apart)")
            XCTAssertTrue(foreign.isEmpty, "a figure in another reader's sign (\(tag)): \(foreign)")
            XCTAssertNotNil(updatedAt, "the hero never said when it was updated (\(tag))")
            app.terminate()
        }
        note("hero-report", report.joined(separator: "\n"))
    }

    // MARK: - 2. Settings: nothing under the keys block moves (item 13.1)

    /// Settings opened as soon as the tab bar is there, ONE snapshot per
    /// tick while the keys block asks the registry: where the block's rows,
    /// the record row, its paragraph and the first rows under the block are.
    func testSettingsKeysSteady() throws {
        var report: [String] = []
        for skipAnimation in [true, false] {
            let tag = skipAnimation ? "skip" : "real"
            let app = XCUIApplication()
            app.terminate()
            if skipAnimation { app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1" }
            app.launch()
            launched = app
            let tab = app.buttons["设置"]
            XCTAssertTrue(tab.waitForExistence(timeout: 40), "no tab bar")
            tab.tap()
            let started = Date()

            var lines: [String] = []
            var venueY = Set<CGFloat>(), appearanceY = Set<CGFloat>(), recordY = Set<CGFloat>()
            var last = "", index = 0, shots = 0
            while Date().timeIntervalSince(started) < 24 {
                let at = Date().timeIntervalSince(started)
                guard let seen = see(app) else { settle(0.2); continue }
                let header = seen.texts.first { $0.label.hasPrefix("钥匙") }
                let record = seen.text("把钱包记录复制到以太坊")
                let explain = seen.id("keys.backupExplain")
                let venue = seen.ids("settings.venue").min { $0.frame.minY < $1.frame.minY }
                let appearance = seen.text("外观")
                let language = seen.text("语言")
                let top = header?.frame.maxY ?? CGFloat.infinity
                let bottom = record?.frame.minY ?? venue?.frame.minY ?? -CGFloat.infinity
                let rows = seen.native.filter { node in
                    node.elementType == .button && node.frame.width > 300
                        && node.frame.minY > top && node.frame.maxY <= bottom
                }
                let state = record.flatMap { record in
                    seen.texts.first { text in
                        abs(text.frame.minX - record.frame.minX) < 1
                            && text.frame.minY > record.frame.minY + 2 && text.frame.minY < record.frame.minY + 44
                    }
                }
                if let venue { venueY.insert(Self.tenth(venue.frame.minY)) }
                if let appearance { appearanceY.insert(Self.tenth(appearance.frame.minY)) }
                if let record { recordY.insert(Self.tenth(record.frame.minY)) }
                let body = String(
                    format: "keys=%@ rows=[%@] record.y=%@ state=[%@] paragraph=%@ FIRST-ROW-UNDER(venue).y=%@ 外观.y=%@ 语言.y=%@",
                    header?.label ?? "-",
                    rows.map { String(format: "%@@%.1f h%.1f", String($0.label.prefix(18)), $0.frame.minY, $0.frame.height) }
                        .joined(separator: " | "),
                    record.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                    state?.label ?? "-",
                    explain.map { String(format: "%.1f h%.1f", $0.frame.minY, $0.frame.height) } ?? "-",
                    venue.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                    appearance.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                    language.map { String(format: "%.1f", $0.frame.minY) } ?? "-"
                )
                lines.append(String(format: "t=%.2f ", at) + body)
                if shots < 4 || body != last {
                    shoot(app, String(format: "keys-%@-%02d", tag, index))
                    shots += 1
                }
                last = body
                index += 1
            }
            note("keys-\(tag)-timeline", lines.joined(separator: "\n"))
            dump(app, "keys-\(tag)-end")
            report.append("\(tag): first row under the keys block y \(venueY.sorted()) · 外观 y \(appearanceY.sorted()) · record row y \(recordY.sorted()) · \(index) samples")
            XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "this walk is the person's own wallet, not the space")
            XCTAssertEqual(venueY.count, 1, "the first row under the keys block moved (\(tag)): \(venueY.sorted())")
            XCTAssertEqual(appearanceY.count, 1, "the section under the keys block moved (\(tag)): \(appearanceY.sorted())")
            app.terminate()
        }
        note("keys-report", report.joined(separator: "\n"))
    }

    // MARK: - 3. Nothing is drawn behind the clock (item 13.4)

    /// Settings and the home, scrolled: a picture at each place. The strip
    /// above the safe area is read from the pictures afterwards.
    func testStatusBarGround() throws {
        let app = launchOwner()
        tapTab(["设置", "Settings"], in: app)
        settle(5)
        shoot(app, "bar-settings-0")
        for step in 1...5 {
            safeDrag(app, from: 0.66, to: 0.40)
            shoot(app, "bar-settings-\(step)")
        }
        dump(app, "bar-settings-scrolled")
        for _ in 0..<7 { safeDrag(app, from: 0.36, to: 0.70) }
        tapTab(["钱包", "Wallet"], in: app)
        settle(6)
        shoot(app, "bar-home-0")
        for step in 1...3 {
            safeDrag(app, from: 0.66, to: 0.40)
            shoot(app, "bar-home-\(step)")
        }
        dump(app, "bar-home-scrolled")
        for _ in 0..<5 { safeDrag(app, from: 0.36, to: 0.70) }
        tapTab(["探索", "Explore"], in: app)
        settle(2)
        shoot(app, "bar-explore-0")
        for step in 1...2 {
            safeDrag(app, from: 0.66, to: 0.40)
            shoot(app, "bar-explore-\(step)")
        }
        for _ in 0..<4 { safeDrag(app, from: 0.36, to: 0.70) }
        app.terminate()
    }

    // MARK: - 4. The currency row and its picker (item 8)

    /// The row, then the picker: opened, read, closed by its ✕. NOTHING is
    /// picked — every row of it is a button that would move the setting.
    func testCurrencyRowAndPicker() throws {
        let app = launchOwner()
        tapTab(["设置", "Settings"], in: app)
        settle(3)
        let row = app.staticTexts["货币"].firstMatch
        reach(row, in: app)
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 货币 row")
        func rowValue() -> String {
            guard let seen = see(app), let title = seen.text("货币") else { return "(unread)" }
            return seen.texts.first { text in
                abs(text.frame.midY - title.frame.midY) < 12 && text.frame.minX > title.frame.maxX
            }?.label ?? "(no value)"
        }
        let before = rowValue()
        shoot(app, "currency-1-row")
        note("currency-row", before)
        XCTAssertTrue(before.contains("¥"), "the row does not write the yuan with ¥: \(before)")
        XCTAssertFalse(before.contains("CN¥"), "the row writes CN¥ for a Chinese reader: \(before)")

        row.tap()
        let close = app.buttons["sheet.close"].firstMatch
        guard close.waitForExistence(timeout: 8) else {
            XCTFail("the currency picker did not open")
            shoot(app, "currency-2-NO-PICKER")
            app.terminate()
            return
        }
        settle(1.2)
        shoot(app, "currency-2-picker")
        dump(app, "currency-2-picker")
        var rows: [[String]] = []
        if let seen = see(app), let title = seen.id("sheet.title") {
            for node in seen.native where node.elementType == .button && node.identifier != "sheet.close"
                && node.frame.minY >= title.frame.maxY - 1 && node.frame.width > 250 {
                let words = Self.words(node)
                // A row of the picker names a currency's code; a row of the page behind does not.
                let coded = words.contains { $0.count == 3 && $0.allSatisfy { $0.isASCII && $0.isUppercase } }
                if coded { rows.append(words) }
            }
        }
        note("currency-picker-rows", rows.map { $0.joined(separator: " | ") }.joined(separator: "\n"))
        func rowOf(_ code: String) -> String { rows.first { $0.contains(code) }?.joined(separator: " | ") ?? "" }
        XCTAssertTrue(rowOf("CNY").contains("¥") && !rowOf("CNY").contains("CN¥") && !rowOf("CNY").contains("JP¥"),
                      "CNY is not written ¥: [\(rowOf("CNY"))]")
        XCTAssertTrue(rowOf("JPY").contains("JP¥"), "JPY is not written JP¥: [\(rowOf("JPY"))]")
        XCTAssertTrue(rowOf("HKD").contains("HK$"), "HKD is not written HK$: [\(rowOf("HKD"))]")
        // No sign on two rows.
        var owners: [String: [String]] = [:]
        for words in rows {
            let code = words.first ?? "?"
            for word in words where word.rangeOfCharacter(from: CharacterSet(charactersIn: "¥$€£₩₫₹₺₱₪฿₴₦₽")) != nil {
                owners[word, default: []].append(code)
            }
        }
        let shared = owners.filter { $0.value.count > 1 }
        note("currency-picker-signs", owners.keys.sorted().map { "\($0) ← \(owners[$0] ?? [])" }.joined(separator: "\n"))
        XCTAssertTrue(shared.isEmpty, "two rows of the picker share a sign: \(shared)")

        // Out by the ✕. Nothing was picked.
        close.tap()
        XCTAssertTrue(close.waitForNonExistence(timeout: 6), "the picker is still up")
        settle(1)
        let after = rowValue()
        note("currency-row-after", after)
        XCTAssertEqual(after, before, "the currency row changed")
        shoot(app, "currency-3-closed")
        app.terminate()
    }

    // MARK: - 5. Report a problem: the facts, and the ✕'s header (items 9, 13.3)

    /// Nothing is typed, nothing attached, nothing sent. The sheet is read,
    /// scrolled by its margin, closed by its ✕, and opened once more.
    func testReportSheet() throws {
        let app = launchOwner()
        tapTab(["设置", "Settings"], in: app)
        settle(3)
        let row = app.staticTexts["反馈"].firstMatch
        reach(row, in: app)
        XCTAssertTrue(row.waitForExistence(timeout: 8), "no 反馈 row")
        row.tap()
        let what = app.textViews["feedback.what"]
        let close = app.buttons["sheet.close"].firstMatch
        guard what.waitForExistence(timeout: 10), close.exists else {
            XCTFail("no report sheet")
            shoot(app, "report-NO-SHEET")
            app.terminate()
            return
        }
        settle(1.2)
        shoot(app, "report-1-sheet")
        dump(app, "report-1-sheet")
        note("report-draft-on-open", (what.value as? String).map { $0.isEmpty ? "(empty)" : $0 } ?? "(nil)")

        // The facts that would be sent, as the sheet shows them: a label and,
        // on its line, the value.
        if let seen = see(app) {
            var facts: [String] = []
            var networks: String?
            for label in ["应用版本", "平台", "语言", "连不上的网络", "最近失败"] {
                guard let name = seen.texts.first(where: { $0.label == label && $0.frame.minY > 100 }) else { continue }
                let value = seen.texts.first { text in
                    abs(text.frame.midY - name.frame.midY) < 4 && text.frame.minX >= name.frame.maxX
                }?.label ?? "(no value)"
                facts.append("\(label) | \(value)")
                if label == "连不上的网络" { networks = value }
            }
            note("report-facts", facts.joined(separator: "\n"))
            XCTAssertNotNil(networks, "the facts do not say which networks cannot be reached")
            for entry in (networks ?? "").components(separatedBy: CharacterSet(charactersIn: ",，、")) {
                let name = entry.trimmingCharacters(in: .whitespaces)
                XCTAssertFalse(name.isEmpty, "an empty entry in [\(networks ?? "")]")
                XCTAssertTrue(name.rangeOfCharacter(from: CharacterSet.decimalDigits.inverted) != nil,
                              "a bare chain id in the facts: [\(networks ?? "")]")
            }
            let old = seen.native.filter { $0.label.contains("无法连接的 RPC") || $0.label.contains("chain-") }.map { $0.label }
            XCTAssertTrue(old.isEmpty, "the old line is still in the facts: \(old)")
        }

        // The whole form fits a phone this tall: it scrolls only with the
        // keyboard up. The field is given the keyboard — NOTHING is typed —
        // and the page is dragged by its margin, above the keys: it goes
        // UNDER the ✕'s header.
        let rested = close.frame
        what.tap()
        let keyboard = app.keyboards.firstMatch
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "no keyboard")
        settle(1)
        shoot(app, "report-2-keyboard")
        let keysTop = keyboard.exists ? keyboard.frame.minY : app.frame.height * 0.55
        // From a little above the keys (and their Done bar) to under the header.
        let from = max(0.25, (keysTop - 70) / app.frame.height)
        let to = max(0.16, from - 0.22)
        var places: [String] = []
        var under = false
        for step in 0..<8 {
            if step > 0 { safeDrag(app, from: from, to: to) }
            guard let seen = see(app) else { continue }
            let button = seen.id("sheet.close")
            let title = seen.id("sheet.title")
            let hint = seen.texts.first { $0.label.contains("最多 5 张") }
            places.append(String(
                format: "step %d ✕ %@ title %@ hint[可选 · 最多 5 张] %@",
                step,
                button.map { String(format: "%.1f..%.1f x%.1f", $0.frame.minY, $0.frame.maxY, $0.frame.minX) } ?? "-",
                title.map { String(format: "%.1f..%.1f", $0.frame.minY, $0.frame.maxY) } ?? "-",
                hint.map { String(format: "%.1f..%.1f x%.1f..%.1f", $0.frame.minY, $0.frame.maxY, $0.frame.minX, $0.frame.maxX) } ?? "-"
            ))
            shoot(app, "report-2-scroll-\(step)")
            if let button {
                XCTAssertEqual(button.frame.minY, rested.minY, accuracy: 0.5, "the ✕ moved when the page scrolled (step \(step))")
                if let hint, hint.frame.minY < button.frame.maxY, hint.frame.maxY > button.frame.minY { under = true }
            }
            if let hint, let button, hint.frame.maxY < button.frame.minY - 20 { break }
        }
        note("report-scroll-places", places.joined(separator: "\n") + "\nthe hint crossed the ✕'s line: \(under)")
        dump(app, "report-2-scrolled")
        XCTAssertTrue(close.isHittable, "the ✕ is not on the sheet")
        let typed = what.value as? String ?? ""
        XCTAssertTrue(typed.isEmpty, "the walk typed nothing, and the field holds: \(typed)")
        // The keyboard away, by its own Done.
        let done = app.buttons["keyboard.done"].firstMatch
        if done.exists { done.tap() }
        XCTAssertTrue(keyboard.waitForNonExistence(timeout: 6), "the keyboard is still up")
        settle(0.8)
        shoot(app, "report-2-keyboard-away")

        // Out by the ✕, with nothing sent.
        close.tap()
        XCTAssertTrue(app.buttons["feedback.send"].waitForNonExistence(timeout: 6), "the report sheet is still up")
        settle(1)
        shoot(app, "report-3-closed")

        // Opened again: empty.
        let again = app.staticTexts["反馈"].firstMatch
        reach(again, in: app)
        if again.exists { again.tap() }
        let field = app.textViews["feedback.what"]
        if field.waitForExistence(timeout: 8) {
            settle(1)
            let kept = field.value as? String ?? ""
            note("report-draft-on-reopen", kept.isEmpty ? "(empty)" : kept)
            XCTAssertTrue(kept.isEmpty, "the reopened report sheet is not empty: \(kept)")
            shoot(app, "report-4-reopened")
            let closeAgain = app.buttons["sheet.close"].firstMatch
            XCTAssertTrue(closeAgain.exists, "no ✕ on the reopened sheet")
            if closeAgain.exists { closeAgain.tap() }
            settle(1.2)
        } else {
            XCTFail("the report sheet did not open a second time")
        }
        XCTAssertFalse(app.buttons["feedback.send"].exists, "the report sheet is still up")
        app.terminate()
    }

    // MARK: - 6. History, idle; a transfer's detail (items 14, 10, 1)

    /// History is opened and left alone for `HISTORY_IDLE` seconds (default
    /// 22) — nothing is asked of the app meanwhile; the phone's log is read on
    /// the Mac. Then one transfer's detail: where "Delete record" stands and
    /// how big it is. It is read from the tree and the picture, never tapped.
    func testHistoryAndTransferDetail() throws {
        let idle = TimeInterval(ProcessInfo.processInfo.environment["HISTORY_IDLE"] ?? "") ?? 22
        let app = launchOwner()
        tapTab(["钱包", "Wallet"], in: app)
        settle(6)
        if let seen = see(app) { note("history-0-home-titles", Self.titles(seen)) }
        let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
            .allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        guard let all else {
            XCTFail("no 全部 over Activity")
            app.terminate()
            return
        }
        all.tap()
        var clock = ["History tapped open at \(Self.clock())"]
        settle(idle)
        clock.append("idle until \(Self.clock())")
        note("history-clock", clock.joined(separator: "\n"))
        shoot(app, "history-1")
        dump(app, "history-1")
        if let seen = see(app) { note("history-1-titles", Self.titles(seen)) }

        // A transfer (sent or received); a dApp row's detail is the same view.
        let transfer = app.buttons.matching(NSPredicate(
            format: "label BEGINSWITH %@ OR label BEGINSWITH %@", "已收到", "已发送"
        )).firstMatch
        var drags = 0
        while drags < 8, !(transfer.exists && transfer.isHittable) {
            safeDrag(app, from: 0.66, to: 0.40)
            drags += 1
        }
        let row: XCUIElement
        if transfer.exists, transfer.isHittable {
            row = transfer
        } else {
            for _ in 0..<10 { safeDrag(app, from: 0.36, to: 0.70) }
            row = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "、"))
                .allElementsBoundByIndex.first { $0.isHittable && $0.frame.width > 250 } ?? transfer
            note("history-no-transfer", "no sent / received row in History: the first row's detail instead")
        }
        guard row.exists else {
            XCTFail("no row in History")
            app.terminate()
            return
        }
        shoot(app, "history-2-row")
        note("detail-row", row.label)
        row.tap()
        settle(3)
        shoot(app, "detail-1")
        dump(app, "detail-1")
        let delete = app.buttons["txDetail.delete"].firstMatch
        XCTAssertTrue(delete.waitForExistence(timeout: 8), "no 删除记录 on the detail")
        // Brought into view by the page's margin — never by a touch on it.
        var scrolls = 0
        while scrolls < 6, delete.exists, delete.frame.maxY > app.frame.height - 40 {
            safeDrag(app, from: 0.66, to: 0.42)
            scrolls += 1
        }
        shoot(app, "detail-2-delete")
        dump(app, "detail-2-delete")
        if let seen = see(app), let node = seen.id("txDetail.delete") {
            let explorer = seen.native.first { $0.elementType == .button && $0.label.contains("浏览器") }
            let above = seen.native.filter { node in
                node.elementType == .button && node.frame.width > 200 && node.identifier != "txDetail.delete"
            }.map { String(format: "%@ %.1f..%.1f w%.0f", String($0.label.prefix(16)), $0.frame.minY, $0.frame.maxY, $0.frame.width) }
            note("detail-frames", String(
                format: "删除记录: label [%@] y %.1f..%.1f x %.1f..%.1f (w %.1f h %.1f) of screen %.0fx%.0f\nexplorer: %@\nwide controls: %@",
                node.label, node.frame.minY, node.frame.maxY, node.frame.minX, node.frame.maxX,
                node.frame.width, node.frame.height, app.frame.width, app.frame.height,
                explorer.map { String(format: "[%@] y %.1f..%.1f w %.1f", $0.label, $0.frame.minY, $0.frame.maxY, $0.frame.width) } ?? "(none on this record)",
                above.joined(separator: " | ")
            ))
            XCTAssertEqual(node.label, "删除记录")
            XCTAssertGreaterThanOrEqual(node.frame.height, 43.5, "the delete is not a finger tall")
            XCTAssertLessThan(node.frame.width, app.frame.width * 0.5, "the delete is a wide control again")
            if let explorer {
                XCTAssertGreaterThanOrEqual(node.frame.minY, explorer.frame.maxY - 1, "the delete is not below the useful action")
                XCTAssertLessThan(node.frame.width, explorer.frame.width * 0.6, "the delete is as heavy as the explorer control")
            }
        }
        // Out — by the sheet's ✕ or the page's back. The delete was not touched.
        closeTop(app)
        XCTAssertTrue(delete.waitForNonExistence(timeout: 6), "the detail is still up")
        shoot(app, "detail-9-left")
        closeTop(app)
        settle(1)
        app.terminate()
    }

    /// Which page's opening the kernel's EXC_GUARD line follows: the Assets
    /// page and History, each opened twice from the home and left alone for
    /// `GUARD_IDLE` seconds (default 8), with the clock of every tap noted.
    /// LOOK ONLY: nothing is tapped but the home's two 「全部」 and each
    /// page's back. The phone's log is read on the Mac against these times.
    func testWhichPageTheKernelLineFollows() throws {
        let idle = TimeInterval(ProcessInfo.processInfo.environment["GUARD_IDLE"] ?? "") ?? 8
        let app = launchOwner()
        tapTab(["钱包", "Wallet"], in: app)
        settle(8)
        var clock = ["home settled at \(Self.clock())"]
        func open(_ name: String, lower: Bool) {
            let alls = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "全部"))
                .allElementsBoundByIndex.filter { $0.isHittable }.sorted { $0.frame.minY < $1.frame.minY }
            guard alls.count == 2, let all = lower ? alls.last : alls.first else {
                clock.append("\(name): the home's two 全部 are not both on screen (\(alls.count))")
                return
            }
            clock.append("\(name): tap begins \(Self.clock())")
            all.tap()
            clock.append("\(name): tap returned \(Self.clock())")
            settle(idle)
            let back = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "返回", "Back"))
                .allElementsBoundByIndex.first { $0.isHittable }
            clock.append("\(name): idle until \(Self.clock()); back \(back != nil)")
            back?.tap()
            settle(3)
        }
        open("Assets #1", lower: true)
        open("History #1", lower: false)
        open("Assets #2", lower: true)
        open("History #2", lower: false)
        clock.append("home again at \(Self.clock())")
        note("guard-clock", clock.joined(separator: "\n"))
        shoot(app, "guard-home-after")
        XCTAssertTrue(app.buttons["设置"].exists, "not back on the home")
        app.terminate()
    }

    // MARK: - 7. The identicon sheet (item 6)

    func testIdenticonSheet() throws {
        let app = launchOwner()
        tapTab(["钱包", "Wallet"], in: app)
        settle(4)
        let avatar = app.buttons["查看身份图标"].firstMatch
        XCTAssertTrue(avatar.waitForExistence(timeout: 8), "no avatar on the home")
        avatar.tap()
        let close = app.buttons.matching(NSPredicate(format: "label == %@", "关闭")).firstMatch
        let copy = app.buttons.matching(NSPredicate(format: "label == %@", "复制地址")).firstMatch
        XCTAssertTrue(close.waitForExistence(timeout: 8), "the identicon sheet has no 关闭")
        settle(1.5)
        shoot(app, "identicon-1")
        dump(app, "identicon-1")
        let screen = app.frame
        // The home indicator's room on a phone without a home button.
        let indicator: CGFloat = 34
        note("identicon-frames", String(
            format: "screen %.0fx%.0f · 复制地址 %@ hittable %@ · 关闭 %@ hittable %@ · home indicator's room from y %.0f",
            screen.width, screen.height,
            copy.exists ? "\(copy.frame)" : "absent", "\(copy.exists && copy.isHittable)",
            close.exists ? "\(close.frame)" : "absent", "\(close.exists && close.isHittable)",
            screen.height - indicator
        ))
        XCTAssertTrue(copy.exists && copy.isHittable, "复制地址 is not on the sheet")
        XCTAssertTrue(close.exists && close.isHittable, "关闭 is not on the sheet")
        if copy.exists, close.exists {
            XCTAssertLessThanOrEqual(close.frame.maxY, screen.height - indicator + 0.5, "关闭 runs under the home indicator")
            XCTAssertLessThanOrEqual(copy.frame.maxY, screen.height - indicator + 0.5, "复制地址 runs under the home indicator")
        }
        // 复制地址 is not tapped (it would write the pasteboard). Out by 关闭.
        if close.exists { close.tap() }
        XCTAssertTrue(copy.waitForNonExistence(timeout: 6), "the identicon sheet is still up")
        shoot(app, "identicon-9-closed")
        app.terminate()
    }

    // MARK: - 8. Hide balance: every coin keeps its unit (item 3)

    /// The person's own wallet. Hide balance is READ first and must be what
    /// `KEPT_HIDDEN` says the person keeps; it is turned on, the home's
    /// holdings and one token's page are read, and it is put back at once.
    func testHiddenBalanceKeepsUnits() throws {
        guard let kept = ProcessInfo.processInfo.environment["KEPT_HIDDEN"].map({ $0 == "1" }) else {
            XCTFail("KEPT_HIDDEN is not set: the person's own setting must be known before it is moved")
            return
        }
        let app = launchOwner()
        tapTab(["钱包", "Wallet"], in: app)
        settle(8)
        guard isHidden(app) == kept else {
            XCTFail("hide balance reads \(isHidden(app)) but KEPT_HIDDEN says \(kept): nothing was moved")
            shoot(app, "hide-MISMATCH")
            app.terminate()
            return
        }
        shoot(app, "hide-0-as-kept")
        if kept { toggleHero(app); settle(2) }       // start from shown, whatever is kept
        toggleHero(app)
        settle(2.5)
        XCTAssertTrue(isHidden(app), "the hero is not hidden")
        shoot(app, "hide-1-home")
        dump(app, "hide-1-home")
        safeDrag(app, from: 0.70, to: 0.36)
        safeDrag(app, from: 0.70, to: 0.36)
        shoot(app, "hide-2-holdings")
        dump(app, "hide-2-holdings")

        // The holdings rows: "•••• <unit>" for the coin, the bare mask for its worth.
        var rows: [String] = []
        if let seen = see(app) {
            for node in seen.native where node.elementType == .button && node.frame.width > 250 && node.label.contains("••••") {
                let words = Self.words(node)
                let masked = words.filter { $0.contains("••••") }
                rows.append("\(node.label)  ⇒ masked words \(masked)")
                let amount = masked.first { $0 != "••••" }
                let worth = masked.contains("••••")
                // A holding: two masked words. (A feed row has one: its amount.)
                if masked.count >= 2 {
                    // A holding the wallet has no symbol for has no unit to
                    // keep — shown or hidden. Said as what it is.
                    if words.count < 4 {
                        XCTFail("a holding with NO symbol: its row has no unit, hidden or shown: \(node.label)")
                        continue
                    }
                    XCTAssertNotNil(amount, "a hidden holding lost its unit: \(node.label)")
                    XCTAssertTrue(amount?.hasPrefix("•••• ") == true && (amount?.count ?? 0) > 5,
                                  "a hidden holding does not read •••• <unit>: \(node.label)")
                    XCTAssertTrue(worth, "a hidden holding's worth is not the bare mask: \(node.label)")
                }
            }
        }
        note("hide-holdings-rows", rows.joined(separator: "\n"))
        XCTAssertFalse(rows.isEmpty, "no masked row on the home")

        // One token's own page.
        let holding = app.buttons.matching(NSPredicate(format: "label ENDSWITH %@", "、••••")).allElementsBoundByIndex
            .first { $0.isHittable && $0.frame.maxY < app.frame.height - 100 }
        if let holding {
            note("hide-token-row", holding.label)
            holding.tap()
            settle(3.5)
            shoot(app, "hide-3-token-page")
            dump(app, "hide-3-token-page")
            if let seen = see(app) {
                let masked = seen.texts.filter { $0.label.contains("••••") }.map { $0.label }
                note("hide-token-page-masked", masked.joined(separator: "\n"))
                XCTAssertTrue(masked.contains { $0.hasPrefix("•••• ") && $0.count > 5 },
                              "the token's page does not read •••• <unit>: \(masked)")
            }
            closeTop(app)
        } else {
            XCTFail("no masked holding row to open")
        }

        // Back to what the person keeps — at once.
        restoreHidden(app, kept)
        app.terminate()
    }

    /// Hide balance put back to the person's own setting, and nothing else.
    func testRestoreHidden() throws {
        guard let kept = ProcessInfo.processInfo.environment["KEPT_HIDDEN"].map({ $0 == "1" }) else {
            XCTFail("KEPT_HIDDEN is not set")
            return
        }
        let app = launchOwner()
        settle(4)
        restoreHidden(app, kept)
        app.terminate()
    }

    private func restoreHidden(_ app: XCUIApplication, _ kept: Bool) {
        var backs = 0
        while backs < 4, !app.buttons["钱包"].exists {
            closeTop(app)
            backs += 1
        }
        tapTab(["钱包", "Wallet"], in: app)
        settle(1.5)
        for _ in 0..<4 { safeDrag(app, from: 0.36, to: 0.72) }
        if isHidden(app) != kept { toggleHero(app) }
        settle(2)
        shoot(app, "hide-9-restored")
        XCTAssertEqual(isHidden(app), kept, "hide balance was NOT put back")
    }

    private func isHidden(_ app: XCUIApplication) -> Bool {
        app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@ OR label == %@", "显示余额", "Show balance")
        ).firstMatch.exists
    }

    /// The hero's figure reads "¥1.49 CNY"; its caption "总余额 · CNY" is not it.
    private static let heroFigure = NSPredicate(
        format: "label ENDSWITH %@ AND NOT (label CONTAINS %@)", " CNY", "·"
    )

    /// The hero figure is the hide-balance toggle.
    private func toggleHero(_ app: XCUIApplication) {
        let hidden = app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@ OR label == %@", "显示余额", "Show balance")
        ).firstMatch
        if hidden.exists {
            hidden.tap()
            return
        }
        let shown = app.buttons.matching(Self.heroFigure).firstMatch
        if shown.exists {
            shown.tap()
        } else {
            XCTFail("no hero figure to toggle")
        }
    }

    // MARK: - 9. What the person's wallet shows about sites, contacts, defaults (item 5 a / c)

    /// LOOK ONLY. The contacts, Settings' Advanced rows (unfolded for the
    /// look: the fold is not kept), and each Explore tab brought forward —
    /// last row first, so the rows end in the order they were found — with
    /// its account chip, its site menu's line and its mark read. No page is
    /// touched; whatever a page asks by itself is refused and reported.
    func testOwnerRecords() throws {
        ownerRecords(label: ProcessInfo.processInfo.environment["RECORDS_LABEL"] ?? "records")
    }

    private func ownerRecords(label: String) {
        startServer()
        let app = launchOwner()
        guard !app.staticTexts["PARALLEL SPACE"].exists else {
            XCTFail("in the parallel space: the person's records are not read from there")
            app.terminate()
            return
        }
        tapTab(["钱包", "Wallet"], in: app)
        settle(3)
        let account = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "0x")).firstMatch
        var lines = ["account in front: \(account.exists ? account.label : "(unread)")"]

        // Contacts.
        tapTab(["通讯录", "Contacts"], in: app)
        settle(2.5)
        shoot(app, "\(label)-1-contacts")
        dump(app, "\(label)-1-contacts")
        if let seen = see(app) {
            let rows = seen.native.filter { $0.elementType == .button && $0.frame.width > 250 && $0.frame.minY > 100 }
                .map { $0.label }
            lines.append("contacts: \(rows.count) rows: \(rows.joined(separator: " | "))")
        }

        // Settings → Advanced, unfolded.
        tapTab(["设置", "Settings"], in: app)
        settle(2.5)
        let advanced = app.staticTexts["高级"].firstMatch
        reach(advanced, in: app)
        if advanced.exists {
            advanced.tap()
            settle(1.2)
            safeDrag(app, from: 0.66, to: 0.40)
            shoot(app, "\(label)-2-advanced")
            dump(app, "\(label)-2-advanced")
            if let seen = see(app), let top = seen.text("高级"), let bottom = seen.text("社区") {
                let rows = seen.texts.filter { $0.frame.minY > top.frame.maxY && $0.frame.maxY < bottom.frame.minY }
                    .map { $0.label }
                lines.append("advanced: \(rows.joined(separator: " | "))")
            }
        } else {
            lines.append("advanced: (no 高级 row found)")
        }
        for _ in 0..<8 { safeDrag(app, from: 0.36, to: 0.72) }

        // Explore: the tabs, then each one in front.
        tapTab(["探索", "Explore"], in: app)
        settle(2.5)
        shoot(app, "\(label)-3-explore")
        dump(app, "\(label)-3-explore")
        let header = app.staticTexts.matching(identifier: "explore.resume.header").firstMatch
        let labels = app.buttons.matching(identifier: "explore.resume.row").allElementsBoundByIndex.map { $0.label }
        // A row is known by its site — the last part of its label: a page's
        // title can change once the page has loaded.
        let rows = labels.map(Self.site)
        lines.append("explore: \(header.exists ? header.label : "(no open tabs line)") · rows in order: \(labels.joined(separator: " | "))")
        if Self.tabsAsFound.isEmpty { Self.tabsAsFound = rows }
        // Last row first, so the rows end in the order they were found
        // (the most recently shown tab is the first row).
        let wanted = Self.tabsAsFound.filter { rows.contains($0) } + rows.filter { !Self.tabsAsFound.contains($0) }
        var sites: [String] = []
        for (index, name) in wanted.enumerated().reversed() {
            let row = app.buttons.matching(identifier: "explore.resume.row")
                .matching(NSPredicate(format: "label ENDSWITH %@", name)).firstMatch
            guard row.waitForExistence(timeout: 6) else {
                sites.append("\(name): the row is gone")
                continue
            }
            row.tap()
            let chip = app.buttons["explore.bar.account"].firstMatch
            guard chip.waitForExistence(timeout: 15) else {
                sites.append("\(name): no address bar came up")
                shoot(app, "\(label)-4-\(index)-NO-BAR")
                tapTab(["探索", "Explore"], in: app)
                continue
            }
            settle(7)
            refuseAnythingAsked(app, context: "\(label) \(name)")
            var line = name
            if let seen = see(app) {
                let bar = seen.id("explore.bar.account")
                let host = seen.texts.first { text in
                    guard let bar else { return false }
                    return abs(text.frame.midY - bar.frame.midY) < 14 && text.frame.maxX <= bar.frame.minX + 1
                }
                let banner = seen.texts.first { $0.identifier.hasPrefix("explore.status") || $0.label.contains("尚未连接") }
                line += " · bar host [\(host?.label ?? "-")] chip [\(bar?.label ?? "-")] mark \(Self.mark(seen, "explore"))"
                    + " tabs \(seen.id("explore.bar.tabs")?.value as? String ?? "-")"
                    + (banner.map { " banner [\($0.label)]" } ?? "")
            }
            shoot(app, "\(label)-4-\(index)-bar")
            // The site menu: who the site is, and whether it is connected.
            let menu = app.buttons["explore.bar.menu"].firstMatch
            if menu.exists, menu.isHittable {
                menu.tap()
                let closePage = app.buttons.matching(NSPredicate(format: "label == %@", "关闭网页")).firstMatch
                if closePage.waitForExistence(timeout: 6) {
                    settle(0.8)
                    if let seen = see(app) {
                        let status = seen.texts.first { $0.label == "已连接" || $0.label == "暂无活跃连接" }
                        let items = seen.native.filter { node in
                            node.elementType == .button && node.frame.width > 250 && node.frame.minY > (status?.frame.maxY ?? 0)
                        }.map { $0.label }
                        line += " · menu [\(status?.label ?? "-")] mark \(Self.mark(seen, "explore.menu")) items \(items)"
                    }
                    shoot(app, "\(label)-4-\(index)-menu")
                    closeExploreSheet(app)
                    // A menu that will not close ENDS the look: its rows
                    // disconnect the site and close the tab, and nothing more
                    // is tapped over it.
                    guard closePage.waitForNonExistence(timeout: 6) else {
                        XCTFail("the site menu is still up (\(name)): the app is ended, nothing more is tapped")
                        shoot(app, "\(label)-4-\(index)-MENU-STUCK")
                        sites.append(line + " · MENU STUCK")
                        note("\(label)-records", (lines + sites.reversed()).joined(separator: "\n"))
                        app.terminate()
                        return
                    }
                } else {
                    line += " · menu did not open"
                }
            }
            refuseAnythingAsked(app, context: "\(label) \(name) after the menu")
            sites.append(line)
            // Back to the Explore home: the tab stays open, its page untouched.
            guard tapTab(["探索", "Explore"], in: app) else {
                shoot(app, "\(label)-4-\(index)-TAB-COVERED")
                note("\(label)-records", (lines + sites.reversed()).joined(separator: "\n"))
                app.terminate()
                return
            }
            settle(1.5)
        }
        lines.append(contentsOf: sites.reversed())
        shoot(app, "\(label)-5-explore-after")
        let labelsAfter = app.buttons.matching(identifier: "explore.resume.row").allElementsBoundByIndex.map { $0.label }
        let after = labelsAfter.map(Self.site)
        lines.append("explore rows after the look: \(labelsAfter.joined(separator: " | "))")
        note("\(label)-records", lines.joined(separator: "\n"))
        XCTAssertEqual(after, Self.tabsAsFound.isEmpty ? rows : Self.tabsAsFound,
                       "the tabs do not end in the order they were found")
        tapTab(["钱包", "Wallet"], in: app)
        settle(1)
        app.terminate()
    }

    // MARK: - 10. In the space: the local mark, the banner, the connection (items 5 b, 7, 11)

    func testSpaceConnectAndMarks() throws {
        spaceConnectAndMarks()
    }

    private func spaceConnectAndMarks() {
        startServer()
        let app = launchInSpace()
        openExplore(app)
        guard testDappInFront(app) else {
            XCTFail("the page in front is not the test dApp: nothing on it is tapped")
            shoot(app, "space-1-NOT-THE-TEST-DAPP")
            dump(app, "space-1-NOT-THE-TEST-DAPP")
            app.terminate()
            return
        }
        settle(1.5)
        shoot(app, "space-1-page")
        dump(app, "space-1-page")
        var report: [String] = []
        var connectedAtStart = false
        if let seen = see(app) {
            let chip = seen.id("explore.bar.account")?.label ?? "-"
            connectedAtStart = chip.contains("已连接")
            report.append("on arrival: chip [\(chip)] bar mark \(Self.mark(seen, "explore"))")
            // Item 11: plain http on this device is a LOCAL page — no padlock.
            XCTAssertNotNil(seen.id("explore.local"), "the address bar does not mark 127.0.0.1 a local page")
            XCTAssertNil(seen.id("explore.insecure"), "the address bar warns about a local page")
        }
        XCTAssertFalse(connectedAtStart, "the space remembered a connection across a launch")

        // The site menu, not connected.
        report.append("menu before: " + readSiteMenu(app, shot: "space-2-menu-before"))

        // Item 7: a refused personal_sign says "not connected" under the bar.
        if !connectedAtStart {
            app.webViews.buttons["Sign"].firstMatch.tap()
            settle(3)
            XCTAssertFalse(app.buttons["signing.close"].exists, "a page that is not connected reached the signing sheet")
            refuseAnythingAsked(app, context: "personal_sign before connecting")
            shoot(app, "space-3-banner")
            dump(app, "space-3-banner")
            let banner = bannerText(app)
            report.append("after a refused personal_sign: banner [\(banner ?? "none")] · page: \(lastVerdict(app, "personal_sign") ?? "-")")
            XCTAssertTrue(banner?.contains("此网站尚未连接") == true && banner?.contains("personal_sign") == true,
                          "no 「此网站尚未连接 · personal_sign」 under the address bar: \(banner ?? "none")")
        }

        // The ONE approval: the test dApp, to the test wallet, in the space.
        guard let account = connectInSpace(app, shot: "space-4-consent") else {
            note("space-report", report.joined(separator: "\n"))
            app.terminate()
            return
        }
        report.append("the page's account: \(account)")

        // The banner is GONE once the site is connected.
        var banners: [String] = []
        let began = Date()
        while Date().timeIntervalSince(began) < 6 {
            banners.append(String(format: "t=%.1f %@", Date().timeIntervalSince(began), bannerText(app) ?? "(none)"))
            settle(0.8)
        }
        shoot(app, "space-5-connected")
        dump(app, "space-5-connected")
        report.append("banner after connecting: \(banners.joined(separator: " · "))")
        XCTAssertNil(bannerText(app), "「此网站尚未连接」 is still under the address bar after the site connected")
        if let seen = see(app) {
            let chip = seen.id("explore.bar.account")?.label ?? "-"
            report.append("after connecting: chip [\(chip)] bar mark \(Self.mark(seen, "explore"))")
            XCTAssertTrue(chip.contains("已连接"), "the chip does not say the site is connected: \(chip)")
        }

        // The connection panel, connected: the local mark, the test wallet.
        let chipButton = app.buttons["explore.bar.account"].firstMatch
        if chipButton.exists, chipButton.isHittable { chipButton.tap() }
        let disconnect = app.buttons.matching(NSPredicate(format: "label == %@", "断开连接")).firstMatch
        if disconnect.waitForExistence(timeout: 6) {
            settle(0.8)
            if let seen = see(app) {
                let words = seen.texts.filter { $0.frame.minY > 200 }.map { $0.label }.filter { !$0.isEmpty }
                report.append("panel: mark \(Self.mark(seen, "explore.connection")) · \(words.prefix(10).joined(separator: " | "))")
                XCTAssertNotNil(seen.id("explore.connection.local"), "the connection panel does not mark 127.0.0.1 a local page")
                XCTAssertTrue(seen.texts.contains { $0.label == Self.testWalletShort }, "the panel does not name the test wallet")
            }
            shoot(app, "space-6-panel")
            dump(app, "space-6-panel")
            // 断开连接 is not tapped. Out by the ✕.
            closeExploreSheet(app)
            guard disconnect.waitForNonExistence(timeout: 6) else {
                XCTFail("the connection panel is still up: the app is ended, nothing more is tapped")
                note("space-report", report.joined(separator: "\n"))
                app.terminate()
                return
            }
        } else {
            XCTFail("the connection panel did not open")
            closeExploreSheet(app)
        }
        report.append("menu connected: " + readSiteMenu(app, shot: "space-7-menu-connected"))
        note("space-report", report.joined(separator: "\n"))
        shoot(app, "space-9-end")
        app.terminate()
    }

    /// The page in front is the test dApp — its own heading on the page, its
    /// own address in the bar. Nothing on a page is tapped before this holds:
    /// the walk's buttons ("Connect", "Sign", "Send dust") are that page's.
    private func testDappInFront(_ app: XCUIApplication) -> Bool {
        guard app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 45) else { return false }
        guard let seen = see(app), let bar = seen.id("explore.bar.account") else { return false }
        return seen.texts.contains { text in
            text.label == Self.testSite && abs(text.frame.midY - bar.frame.midY) < 14 && text.frame.maxX <= bar.frame.minX + 1
        }
    }

    /// The site menu opened, read and closed by its ✕. No row is tapped.
    private func readSiteMenu(_ app: XCUIApplication, shot: String) -> String {
        let menu = app.buttons["explore.bar.menu"].firstMatch
        guard menu.exists, menu.isHittable else { return "(no ⋯ to tap)" }
        menu.tap()
        let closePage = app.buttons.matching(NSPredicate(format: "label == %@", "关闭网页")).firstMatch
        guard closePage.waitForExistence(timeout: 6) else {
            XCTFail("the site menu did not open")
            return "(did not open)"
        }
        settle(0.8)
        var line = "(unread)"
        if let seen = see(app) {
            let status = seen.texts.first { $0.label == "已连接" || $0.label == "暂无活跃连接" }
            line = "[\(status?.label ?? "-")] mark \(Self.mark(seen, "explore.menu"))"
            XCTAssertNotNil(seen.id("explore.menu.local"), "the site menu does not mark 127.0.0.1 a local page")
        }
        shoot(app, shot)
        closeExploreSheet(app)
        if !closePage.waitForNonExistence(timeout: 6) {
            XCTFail("the site menu is still up: the app is ended, nothing more is tapped")
            app.terminate()
        }
        return line
    }

    /// The line under the address bar, if it is there.
    private func bannerText(_ app: XCUIApplication) -> String? {
        guard let seen = see(app) else { return nil }
        if let said = seen.texts.first(where: { $0.label.contains("尚未连接") }) { return said.label }
        guard let line = seen.id("explore.status") else { return nil }
        return Self.words(line).joined(separator: " ")
    }

    // MARK: - 11. In the space: the signing sheet holds still (item 2)

    /// "Send dust" pays the person's REAL address: the sheet is sampled and
    /// DECLINED by its ✕ the moment the samples are in, and the page's 4001
    /// is read back. The confirm is never tapped.
    func testSpaceSigningSheetSteady() throws {
        spaceSigningSheetSteady()
    }

    private func spaceSigningSheetSteady() {
        let seconds = TimeInterval(ProcessInfo.processInfo.environment["SHEET_SECONDS"] ?? "") ?? 24
        startServer()
        let app = launchInSpace()
        openExplore(app)
        guard testDappInFront(app) else {
            XCTFail("the page in front is not the test dApp: nothing on it is tapped")
            shoot(app, "dust-0-NOT-THE-TEST-DAPP")
            dump(app, "dust-0-NOT-THE-TEST-DAPP")
            app.terminate()
            return
        }
        guard connectInSpace(app, shot: "dust-0-consent") != nil else {
            app.terminate()
            return
        }
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")

        app.webViews.buttons["Send dust"].firstMatch.tap()
        let places = sampleSheet(app, "dust", seconds: seconds)
        // DECLINED, first thing after the samples.
        let close = app.buttons["signing.close"].firstMatch
        if close.exists { close.tap() }
        let refused = waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001")
        XCTAssertTrue(refused, "closing the sheet did not refuse the send")
        note("dust-declined", "✕ tapped at \(Self.clock()); the page heard 4001: \(refused) · \(lastVerdict(app, "eth_sendTransaction") ?? "-")")
        XCTAssertFalse(app.buttons["signing.close"].exists, "a sheet is still up")
        shoot(app, "dust-9-declined")

        note("dust-places", places.summary)
        XCTAssertGreaterThan(places.samples, 5, "the sheet was hardly sampled")
        XCTAssertEqual(places.close.count, 1, "the ✕ moved: \(places.close.sorted())")
        XCTAssertEqual(places.header.count, 1, "the header moved: \(places.header.sorted())")
        XCTAssertEqual(places.fee.count, 1, "the fee row moved: \(places.fee.sorted())")
        XCTAssertEqual(places.signer.count, 1, "the signing account moved: \(places.signer.sorted())")
        XCTAssertEqual(places.confirm.count, 1, "the confirm moved: \(places.confirm.sorted())")
        app.terminate()
    }

    private struct SheetPlaces {
        var samples = 0
        var close = Set<CGFloat>(), header = Set<CGFloat>(), fee = Set<CGFloat>()
        var signer = Set<CGFloat>(), confirm = Set<CGFloat>(), speed = Set<CGFloat>()
        var fees: [String] = []
        var summary: String {
            """
            \(samples) samples with the sheet up
            ✕ y \(close.sorted()) · header y \(header.sorted()) · fee row y \(fee.sorted()) · speed row y \(speed.sorted()) \
            · signing account y \(signer.sorted()) · confirm y \(confirm.sorted())
            the fee row said, in order: \(fees.joined(separator: " → "))
            """
        }
    }

    /// The sheet's five places, read from ONE snapshot of the tree per tick
    /// from the tap on — asking the live tree for a row a moment after asking
    /// whether it exists fails the test when the fee has redrawn it.
    private func sampleSheet(_ app: XCUIApplication, _ name: String, seconds: TimeInterval) -> SheetPlaces {
        var places = SheetPlaces()
        var lines: [String] = []
        let started = Date()
        var index = 0
        var firstUp: TimeInterval?
        while Date().timeIntervalSince(started) < (firstUp.map { $0 + seconds } ?? 40) {
            let at = Date().timeIntervalSince(started)
            guard let seen = see(app) else {
                lines.append(String(format: "t=%.2f (no snapshot)", at))
                settle(0.2)
                continue
            }
            guard let close = seen.id("signing.close") else {
                lines.append(String(format: "t=%.2f (no sheet yet)", at))
                settle(0.1)
                continue
            }
            if firstUp == nil { firstUp = at }
            // The header: the asking site, on the ✕'s line.
            let header = seen.texts.first { text in
                text.label == Self.testSite && abs(text.frame.minY - close.frame.minY) < 12 && text.frame.minX > 60
            }
            // The fee row: its name, and on its line what it says.
            let fee = seen.texts.first { $0.label == "网络费" }
            let feeSays = fee.flatMap { fee in
                seen.texts.first { abs($0.frame.midY - fee.frame.midY) < 8 && $0.frame.minX > fee.frame.maxX }
            }?.label ?? "-"
            let speed = seen.native.first { $0.elementType == .button && $0.label.hasPrefix("速度") }
            let signer = seen.texts.first { $0.label == "签名账户" }
            let signerName = signer.flatMap { signer in
                seen.texts.first { abs($0.frame.midY - signer.frame.midY) < 6 && $0.frame.minX > signer.frame.maxX }
            }
            let confirm = seen.id("signing.confirm") ?? seen.id("signing.openSigner")
            let footer = seen.id("signing.confirmBlock")
            let verdict = seen.id("signing.verdict")
            let reasons = (seen.ids("signing.fee.reason") + seen.ids("signing.fee.option.reason"))
                .map { String(format: "%@@%.0f..%.0f", String($0.label.prefix(16)), $0.frame.minY, $0.frame.maxY) }
            let said = seen.texts.filter { text in
                ["不够付", "不足以支付", "不额外收费", "已改用", "正在重试", "点费用重试", "没有", "付不了", "更快"]
                    .contains { text.label.contains($0) } && text.identifier != "signing.confirmBlock"
                    && !text.identifier.hasPrefix("signing.fee")
            }.map { String(format: "%@@%.0f", String($0.label.prefix(14)), $0.frame.minY) }
            places.samples += 1
            places.close.insert(Self.tenth(close.frame.minY))
            if let header { places.header.insert(Self.tenth(header.frame.minY)) }
            if let fee {
                places.fee.insert(Self.tenth(fee.frame.minY))
                if places.fees.last != feeSays { places.fees.append(feeSays) }
            }
            if let speed { places.speed.insert(Self.tenth(speed.frame.minY)) }
            if let signer { places.signer.insert(Self.tenth(signer.frame.minY)) }
            if let confirm { places.confirm.insert(Self.tenth(confirm.frame.minY)) }
            lines.append(String(
                format: "t=%.2f ✕=%.1f header=%@ verdict=%@ fee=[%@]@%@ speed=%@ lines=[%@] signer=%@(%@) confirm=%@ enabled=%@ footer=[%@]@%@",
                at, close.frame.minY,
                header.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                verdict.map { String(format: "%.0f h%.0f", $0.frame.minY, $0.frame.height) } ?? "-",
                feeSays,
                fee.map { String(format: "%.1f h%.1f", $0.frame.minY, $0.frame.height) } ?? "-",
                speed.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                (reasons + said).joined(separator: " | "),
                signer.map { String(format: "%.1f", $0.frame.minY) } ?? "-",
                signerName?.label ?? "-",
                confirm.map { String(format: "%.1f h%.1f", $0.frame.minY, $0.frame.height) } ?? "-",
                confirm.map { "\($0.isEnabled)" } ?? "-",
                footer?.label ?? "-",
                footer.map { String(format: "%.0f", $0.frame.minY) } ?? "-"
            ))
            if index < 5 || index % 6 == 0 { shoot(app, String(format: "%@-f%02d", name, index)) }
            index += 1
        }
        note("\(name)-frames", lines.joined(separator: "\n"))
        shoot(app, "\(name)-last")
        dump(app, "\(name)-last")
        return places
    }

    // MARK: - 12. In the space: a split's remainder (item 13.2)

    /// Send → a coin the test wallet holds → a second recipient → an amount
    /// in one row. The form is never left forward: 继续 is not tapped, and
    /// the way out is back.
    func testSpaceSplitRemainder() throws {
        spaceSplitRemainder()
    }

    private func spaceSplitRemainder() {
        let app = launchInSpace(page: false)
        tapTab(["钱包", "Wallet"], in: app)
        settle(7)
        shoot(app, "split-0-home")
        if let seen = see(app) { note("split-0-home-titles", Self.titles(seen)) }

        app.buttons["转账"].firstMatch.tap()
        settle(3)
        shoot(app, "split-1-coins")
        let coinName = ProcessInfo.processInfo.environment["SPLIT_COIN"] ?? "BNB"
        let coin = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", coinName)).firstMatch
        guard coin.waitForExistence(timeout: 10) else {
            XCTFail("the test wallet holds no \(coinName) to send")
            dump(app, "split-1-coins")
            leaveSend(app)
            app.terminate()
            return
        }
        coin.tap()
        settle(3)
        let add = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "添加收款人")).firstMatch
        XCTAssertTrue(add.waitForExistence(timeout: 8), "no 添加收款人")
        if add.exists { add.tap() }
        settle(2)
        func remainder() -> String? { see(app)?.texts.first { $0.label.hasPrefix("还剩") }?.label }
        func balance() -> String? { see(app)?.texts.first { $0.label.contains("余额") }?.label }
        var report = ["balance line: \(balance() ?? "-")", "two recipients, nothing typed: \(remainder() ?? "(no remainder line)")"]
        shoot(app, "split-2-two-recipients")
        dump(app, "split-2-two-recipients")

        // An amount in the first row.
        let amount = app.textFields.matching(NSPredicate(format: "placeholderValue == %@", "0")).firstMatch
        XCTAssertTrue(amount.exists, "no amount field")
        if amount.exists {
            amount.tap()
            settle(0.8)
            amount.typeText(ProcessInfo.processInfo.environment["SPLIT_AMOUNT"] ?? "0.0001")
            settle(1.5)
            report.append("an amount typed in row 1: \(remainder() ?? "(no remainder line)")")
            shoot(app, "split-3-amount")
            dump(app, "split-3-amount")
            let done = app.buttons["keyboard.done"].firstMatch
            if done.exists { done.tap() }
            settle(1)
            shoot(app, "split-4-keyboard-away")
        }
        note("split-remainder", report.joined(separator: "\n"))
        for line in report where line.contains("还剩") {
            let said = String(line[line.range(of: "还剩")!.lowerBound...])
            let figure = said.dropFirst("还剩".count).trimmingCharacters(in: .whitespaces)
                .components(separatedBy: " ").first ?? ""
            let decimals = figure.split(whereSeparator: { $0 == "." || $0 == "," }).dropFirst().last.map { $0.count } ?? 0
            XCTAssertLessThanOrEqual(decimals, 6, "the remainder is not a short figure: \(said)")
            XCTAssertTrue(said.hasSuffix(" \(coinName)"), "the remainder lost its unit: \(said)")
        }
        XCTAssertTrue(report.contains { $0.contains("还剩") }, "the form never said what is left")
        // Out by back. Nothing was confirmed; 继续 was never tapped.
        leaveSend(app)
        shoot(app, "split-9-left")
        XCTAssertTrue(app.buttons["设置"].exists, "Send was not left")
        app.terminate()
    }

    private func leaveSend(_ app: XCUIApplication) {
        var backs = 0
        while backs < 6, !app.buttons["设置"].exists {
            let back = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "返回", "Back"))
                .allElementsBoundByIndex.first { $0.isHittable }
            if let back { back.tap() } else { break }
            settle(1.5)
            backs += 1
        }
    }

    // MARK: - Out of the space

    func testLeaveSpace() throws {
        leaveSpace()
    }

    private func leaveSpace() {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        Self.pinChinese(app)
        app.launch()
        launched = app
        XCTAssertTrue(app.buttons["钱包"].waitForExistence(timeout: 40), "no tab bar")
        settle(6)
        shoot(app, "leave-home")
        dump(app, "leave-home")
        XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "still in the parallel space")
        let account = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "0x")).firstMatch
        note("leave-account", account.exists ? account.label : "(unread)")
        if let expected = ProcessInfo.processInfo.environment["OWNER_ACCOUNT"], account.exists {
            XCTAssertTrue(account.label.contains(expected), "the account in front is \(account.label), not \(expected)")
        }
        XCTAssertFalse(account.exists && account.label.contains(Self.testWalletShort), "the test wallet is still in front")
        app.terminate()
    }

    // MARK: - The whole round trip through the space, from ONE install

    //  Every run of a test installs the build again, and on a phone each
    //  install is authorised over the phone's own network before the app may
    //  launch (the walk lost seven minutes to that on 2026-10-11, between two
    //  runs). XCTest runs a class's tests in the order of their names, so the
    //  six steps below, asked for together, are one install and one sequence:
    //  the person's records, the three looks inside the space, the way out,
    //  the person's records again. Each is also a test of its own above.

    func testRoundTrip1OwnerRecordsBefore() throws { ownerRecords(label: "before") }
    func testRoundTrip2SpaceConnectAndMarks() throws { spaceConnectAndMarks() }
    func testRoundTrip3SpaceSigningSheetSteady() throws { spaceSigningSheetSteady() }
    func testRoundTrip4SpaceSplitRemainder() throws { spaceSplitRemainder() }
    func testRoundTrip5LeaveSpace() throws { leaveSpace() }
    func testRoundTrip6OwnerRecordsAfter() throws { ownerRecords(label: "after") }

    // MARK: - Plumbing: the one approval, and everything else refused

    /// Connect the test dApp to the test wallet. The consent is approved ONLY
    /// when one snapshot of the tree holds all of: the space's badge, the
    /// sheet asking for 127.0.0.1:8137, the test wallet's address on it, no
    /// other account's address on the app's own chrome, and both answers.
    /// Otherwise it is refused and the test fails. Returns the page's own
    /// line naming the account it was given.
    private func connectInSpace(_ app: XCUIApplication, shot: String) -> String? {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let title = app.descendants(matching: .any)["explore.connection.title"].firstMatch
        if title.waitForExistence(timeout: 12) {
            settle(1)
            // ONE tree, whole: a snapshot of the app leaves out what a sheet
            // covers — the space's badge is behind this one — so the check
            // reads the app's own description of its whole hierarchy, and
            // that very text is kept as the evidence.
            let whole = wholeTree(app)
            let own = whole.lines.filter { !$0.web }
            let badge = own.contains { $0.type == "StaticText" && $0.label == "PARALLEL SPACE" }
            let asked = own.first { $0.identifier == "explore.connection.title" }?.label ?? ""
            let addresses = own.filter { $0.type == "StaticText" }.map { $0.label }.filter { label in
                label.range(of: "^0x[0-9a-fA-F]{4}…[0-9a-fA-F]{4}$", options: .regularExpression) != nil
            }
            let approve = own.first { $0.type == "Button" && $0.label == "连接" }
            let reject = own.first { $0.type == "Button" && $0.label == "拒绝" }
            let local = own.first { $0.identifier == "explore.connection.local" }
            let mark = local.map { "local[\($0.label)]" }
                ?? (own.contains { $0.identifier == "explore.connection.insecure" } ? "insecure" : "(no named mark)")
            let facts = """
            at \(Self.clock()), in one tree of \(whole.lines.count) elements: badge PARALLEL SPACE \(badge) · sheet asks [\(asked)] \
            · addresses on the app's chrome \(addresses) · mark \(mark) · 连接 \(approve != nil) · 拒绝 \(reject != nil)
            """
            note("\(shot)-check", facts)
            note("\(shot)-tree", whole.text)
            shoot(app, shot)
            let safe = badge && asked.contains(Self.testSite) && addresses.contains(Self.testWalletShort)
                && addresses.allSatisfy { $0 == Self.testWalletShort } && approve != nil && reject != nil
            guard safe else {
                refuseAnythingAsked(app, context: "the consent was not the test wallet's in the space")
                XCTFail("the consent was NOT approved: \(facts)")
                return nil
            }
            XCTAssertTrue(mark.contains("local"), "the consent does not mark 127.0.0.1 a local page: \(mark)")
            // The sheet's own 连接, where the snapshot that was just checked has
            // it (the page's own button reads "Connect", and is not the app's).
            guard let approve, let again = see(app)?.native.first(where: { $0.elementType == .button && $0.label == "连接" }),
                  abs(again.frame.midY - approve.frame.midY) < 1.5, abs(again.frame.midX - approve.frame.midX) < 1.5 else {
                refuseAnythingAsked(app, context: "the consent's 连接 was not where the snapshot had it")
                XCTFail("the consent moved between the snapshot and the tap: refused")
                return nil
            }
            tap(approve.frame, in: app)
            approvals.append("APPROVED connect \(Self.testSite) → \(Self.testWalletShort), PARALLEL SPACE on screen: \(facts)")
        } else {
            note("\(shot)-no-consent", "no consent sheet came up after the page's Connect")
        }
        guard let line = verdictLine(app, containing: "#verdict eth_requestAccounts ok") else {
            XCTFail("the page was never connected")
            return nil
        }
        XCTAssertTrue(line.lowercased().contains(Self.testWalletFull.lowercased()),
                      "the page was not given the test wallet: \(line)")
        return line
    }

    /// Refuse whatever is asking: a signing sheet by its ✕, a connect consent
    /// by its 拒绝, an add-network sheet by its dismiss. Returns what it
    /// refused. Used after every step that brings a page forward, and when a
    /// test ends.
    @discardableResult
    private func refuseAnythingAsked(_ app: XCUIApplication, context: String) -> [String] {
        var refused: [String] = []
        let signing = app.buttons["signing.close"].firstMatch
        if signing.exists {
            shoot(app, "REFUSED-signing")
            signing.tap()
            refused.append("a signing sheet was up (\(context)) — closed by its ✕ (refused) at \(Self.clock())")
        }
        let consent = app.descendants(matching: .any)["explore.connection.title"].firstMatch
        if consent.exists {
            let title = consent.label
            shoot(app, "REFUSED-consent")
            // The sheet's own answer — the app's node, never a page's button.
            let reject = see(app)?.native.first { $0.elementType == .button && ($0.label == "拒绝" || $0.label == "Reject") }
            if let reject {
                tap(reject.frame, in: app)
                refused.append("a connect consent was up (\(context)): \(title) — answered 拒绝 at \(Self.clock())")
            } else {
                refused.append("a connect consent was up (\(context)): \(title) — NO 拒绝 found; left for the next launch")
            }
        }
        let network = app.buttons["explore.addNetwork.dismiss"].firstMatch
        if network.exists {
            shoot(app, "REFUSED-addnetwork")
            network.tap()
            refused.append("an add-network sheet was up (\(context)) — dismissed at \(Self.clock())")
        }
        if !refused.isEmpty {
            incidents.append(contentsOf: refused)
            settle(1)
        }
        return refused
    }

    // MARK: - Plumbing: launches

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

    /// A phone is given NO pin here: it speaks Chinese already and holds the
    /// person's own wallets. A simulator has neither, so there — and only
    /// there (`DevAccountSeed` ignores the seed on a device) — the walk pins
    /// the language it reads and a key-less account to stand for the person.
    private static func pinChinese(_ app: XCUIApplication) {
        guard DeviceSafety.onSimulator else { return }
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_ACCOUNT"] = "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984"
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
    }

    /// The person's own wallet, no pins but the launch animation.
    private func launchOwner() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        Self.pinChinese(app)
        app.launch()
        launched = app
        XCTAssertTrue(app.buttons["钱包"].waitForExistence(timeout: 40), "no tab bar")
        XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "this walk is the person's own wallet, not the space")
        return app
    }

    private func launchInSpace(page: Bool = true) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        if page { app.launchEnvironment["VELA_URL"] = LocalDappServer.url }
        Self.pinChinese(app)
        app.launch()
        launched = app
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30), "not in the parallel space")
        return app
    }

    private func openExplore(_ app: XCUIApplication) {
        if app.buttons["explore.bar.back"].waitForExistence(timeout: 4) { return }
        tapTab(["探索", "Explore"], in: app)
        let row = app.buttons.matching(identifier: "explore.resume.row")
            .matching(NSPredicate(format: "label CONTAINS %@", Self.testSite)).firstMatch
        if row.waitForExistence(timeout: 8) { row.tap() }
    }

    private func waitForVerdict(_ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30) -> Bool {
        verdictLine(app, containing: fragment, timeout: timeout) != nil
    }

    private func verdictLine(_ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30) -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex.map { $0.label }
            if let hit = lines.first(where: { $0.contains(fragment) }) { return hit }
            settle(1)
        }
        return nil
    }

    /// The page's latest line about `method`, read once.
    private func lastVerdict(_ app: XCUIApplication, _ method: String) -> String? {
        app.webViews.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "#verdict \(method)"))
            .allElementsBoundByIndex.map { $0.label }.first
    }

    // MARK: - Plumbing: one snapshot of the tree

    /// ONE snapshot, flattened. `native` leaves out what a web page drew.
    private struct Seen {
        var all: [XCUIElementSnapshot] = []
        var native: [XCUIElementSnapshot] = []
        /// The app's own texts — never a page's.
        var texts: [XCUIElementSnapshot] { native.filter { $0.elementType == .staticText } }
        func id(_ identifier: String) -> XCUIElementSnapshot? { native.first { $0.identifier == identifier } }
        func ids(_ identifier: String) -> [XCUIElementSnapshot] { native.filter { $0.identifier == identifier } }
        func text(_ label: String) -> XCUIElementSnapshot? {
            native.first { $0.elementType == .staticText && $0.label == label }
        }
    }

    private func see(_ app: XCUIApplication) -> Seen? {
        guard let root = try? app.snapshot() else { return nil }
        var seen = Seen()
        func walk(_ node: XCUIElementSnapshot, web: Bool) {
            let inside = web || node.elementType == .webView
            seen.all.append(node)
            if !inside { seen.native.append(node) }
            for child in node.children { walk(child, web: inside) }
        }
        walk(root, web: false)
        return seen
    }

    /// One element of the app's whole hierarchy, as its own description
    /// prints it. `web`: under a web view — a page's, not the app's.
    private struct TreeLine {
        let type: String
        let frame: CGRect
        let identifier: String
        let label: String
        let web: Bool
    }

    /// The app's WHOLE hierarchy from one description of it — what is behind
    /// a sheet included, which `snapshot()` leaves out — and the text it was
    /// read from. Anything the reader cannot parse is simply not in `lines`:
    /// a check that needs a line then fails closed.
    private func wholeTree(_ app: XCUIApplication) -> (lines: [TreeLine], text: String) {
        let text = app.debugDescription
        guard let element = try? NSRegularExpression(
                pattern: #"^(\s*)→?([A-Za-z]+)(?: \([^)]*\))?, 0x[0-9a-fA-F]+, \{\{(-?[0-9.]+), (-?[0-9.]+)\}, \{(-?[0-9.]+), (-?[0-9.]+)\}\}(.*)$"#
              ),
              let identifier = try? NSRegularExpression(pattern: #"identifier: '([^']*)'"#),
              let label = try? NSRegularExpression(pattern: #"label: '(.*?)'(?=$|, [A-Za-z])"#)
        else { return ([], text) }
        var lines: [TreeLine] = []
        var webDepth: Int?
        for raw in text.components(separatedBy: "\n") {
            if raw.hasPrefix("Path to element") { break }
            let line = raw as NSString
            guard let match = element.firstMatch(in: raw, range: NSRange(location: 0, length: line.length)) else { continue }
            let depth = match.range(at: 1).length
            let type = line.substring(with: match.range(at: 2))
            func number(_ index: Int) -> CGFloat { CGFloat(Double(line.substring(with: match.range(at: index))) ?? 0) }
            let rest = line.substring(with: match.range(at: 7))
            func first(_ expression: NSRegularExpression) -> String {
                let all = NSRange(location: 0, length: (rest as NSString).length)
                return expression.firstMatch(in: rest, range: all).map { (rest as NSString).substring(with: $0.range(at: 1)) } ?? ""
            }
            if let open = webDepth, depth <= open { webDepth = nil }
            let web = webDepth != nil
            if webDepth == nil, type == "WebView" { webDepth = depth }
            lines.append(TreeLine(
                type: type,
                frame: CGRect(x: number(3), y: number(4), width: number(5), height: number(6)),
                identifier: first(identifier), label: first(label), web: web
            ))
        }
        return (lines, text)
    }

    /// The texts under a node, in order.
    private static func words(_ node: XCUIElementSnapshot) -> [String] {
        var out: [String] = []
        func walk(_ node: XCUIElementSnapshot) {
            if node.elementType == .staticText, !node.label.isEmpty { out.append(node.label) }
            node.children.forEach(walk)
        }
        walk(node)
        return out
    }

    /// Which mark a surface draws beside a site ("explore", "explore.menu",
    /// "explore.connection"). The padlock is decorative to a screen reader:
    /// it may not be in the tree at all, and then the picture is the evidence.
    private static func mark(_ seen: Seen, _ prefix: String) -> String {
        var marks: [String] = []
        for kind in ["lock", "local", "insecure"] {
            if let node = seen.id("\(prefix).\(kind)") {
                marks.append("\(kind)[\(node.label)]@\(Int(node.frame.minX)),\(Int(node.frame.minY))")
            }
        }
        return marks.isEmpty ? "(no named mark: padlock or none — see the picture)" : marks.joined(separator: "+")
    }

    /// Every dApp row's title in its parts: where each part stands.
    private static func titles(_ seen: Seen) -> String {
        let parts = seen.native.filter { $0.identifier.hasPrefix("title.") }
        guard !parts.isEmpty else { return "(no title in parts on this screen)" }
        return parts.sorted { ($0.frame.minY, $0.frame.minX) < ($1.frame.minY, $1.frame.minX) }.map { part in
            String(format: "%@ [%@] x %.1f..%.1f y %.1f..%.1f (h %.1f)", part.identifier, part.label,
                   part.frame.minX, part.frame.maxX, part.frame.minY, part.frame.maxY, part.frame.height)
        }.joined(separator: "\n")
    }

    /// The site of an Explore row: the last part of "<title>、<host>".
    private static func site(_ label: String) -> String {
        label.components(separatedBy: "、").last ?? label
    }

    private static func tenth(_ value: CGFloat) -> CGFloat { (value * 10).rounded() / 10 }

    private static func clock() -> String {
        let format = DateFormatter()
        format.locale = Locale(identifier: "en_US_POSIX")
        format.dateFormat = "HH:mm:ss.SSS"
        return format.string(from: Date())
    }

    // MARK: - Plumbing: touches

    /// A tab is tapped only when nothing covers it: a tap sent through a
    /// sheet lands on whatever row of the sheet is there. Says whether it
    /// tapped.
    @discardableResult
    private func tapTab(_ labels: [String], in app: XCUIApplication) -> Bool {
        for label in labels {
            let button = app.buttons[label]
            if button.exists {
                guard button.isHittable else {
                    XCTFail("the \(label) tab is covered: NOT tapped")
                    return false
                }
                button.tap()
                return true
            }
        }
        XCTFail("no tab called any of \(labels)")
        return false
    }

    /// A slow drag down the page's LEADING MARGIN (x = 20 pt), where no row,
    /// slider or button stands: it scrolls and can press nothing.
    private func safeDrag(_ app: XCUIApplication, from: CGFloat, to: CGFloat) {
        let dx = 20 / max(app.frame.width, 1)
        app.coordinate(withNormalizedOffset: CGVector(dx: dx, dy: from))
            .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: dx, dy: to)),
                   withVelocity: XCUIGestureVelocity(260), thenHoldForDuration: 0.25)
        settle(0.6)
    }

    /// Bring an element to the middle of the screen by the margin.
    private func reach(_ element: XCUIElement, in app: XCUIApplication) {
        let screen = app.frame.height
        var drags = 0
        while drags < 14 {
            if element.exists {
                let frame = element.frame
                if frame.maxY > screen * 0.72 {
                    safeDrag(app, from: 0.62, to: 0.38)
                } else if frame.minY < screen * 0.2 {
                    safeDrag(app, from: 0.38, to: 0.62)
                } else {
                    break
                }
            } else {
                safeDrag(app, from: 0.62, to: 0.38)
            }
            drags += 1
        }
    }

    /// Close whatever is on top: a Settings sheet's ✕, else a 「关闭」 that is
    /// the sheet's own, else a page's 「返回」 — never a row, and never the
    /// status line's dismiss. Not for the tab switcher, whose ✕s close tabs.
    private func closeTop(_ app: XCUIApplication) {
        let sheet = app.buttons["sheet.close"].firstMatch
        if sheet.exists, sheet.isHittable {
            sheet.tap()
            settle(1.4)
            return
        }
        let close = app.buttons.matching(NSPredicate(
            format: "label == %@ AND identifier != %@", "关闭", "explore.status.dismiss"
        )).allElementsBoundByIndex.filter { $0.isHittable }.min { $0.frame.minY < $1.frame.minY }
        if let close {
            close.tap()
            settle(1.4)
            return
        }
        let back = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "返回", "Back"))
            .allElementsBoundByIndex.first { $0.isHittable }
        if let back {
            back.tap()
            settle(1.4)
        }
    }

    /// An Explore sheet (the site menu, the connection panel) by its own ✕:
    /// the 「关闭」 that is not the status line's dismiss and not a row.
    private func closeExploreSheet(_ app: XCUIApplication) {
        // The app's own nodes only: a page may draw a button of any name.
        let closes = see(app)?.native.filter { node in
            node.elementType == .button && node.label == "关闭" && node.identifier != "explore.status.dismiss"
        } ?? []
        guard closes.count == 1, let close = closes.first else {
            XCTFail("the sheet's ✕ is not the one 「关闭」 on screen (\(closes.count) found): nothing was tapped")
            return
        }
        tap(close.frame, in: app)
        settle(1.2)
    }

    /// A tap at the middle of a frame that was just read from the tree.
    private func tap(_ frame: CGRect, in app: XCUIApplication) {
        app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: frame.midX - app.frame.minX, dy: frame.midY - app.frame.minY)).tap()
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
