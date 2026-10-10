//
//  FinalRoundScreenshotTests.swift
//  VelaWalletUITests
//
//  PR 3's FINAL round (shell round 3), in zh and en, light and dark — walked
//  on a simulator, photographed and MEASURED. Every board is a `VELA_PAGE`
//  session (`FinalRoundGalleryScreen`): fixture data, no chain, no camera.
//
//  - `testTheHeroSaysCheckingThenLiveOrCantReach` (F19): the three lines of a
//    wallet that held nothing, and the control under them at one place.
//  - `testALongStatusIsOneLine` (F16): Tempo's token list and Vela's own
//    fault — one line, cut; said whole on what the line opens.
//  - `testTheBreakdownsStatusesAndHeadings` (F20, F21).
//  - `testTheWizardsRpcFieldFollowsTheCore` (F4, F14, F22).
//  - `testTheVerdictLandsInItsPlace` (F2): the confirm at one y under every
//    verdict kind — the tall one included.
//  - `testATallVerdictIsShownWholeOverAPinnedConfirm` (device round): four
//    balance rows and the unverified-token warning, whole, in a place as
//    tall as they are; the confirm where it is with one row and with none.
//    On the board and on the sheet presented as the browser presents it.
//  - `testTheTallVerdictLandsUnderAPinnedConfirm` (device round): the sheet
//    opens on "Checking…" and the tall verdict lands a moment later — the
//    confirm does not move, and the verdict is in view. Run on an iPhone SE
//    too: there the body scrolls (for the drawn send, `sheet-rich-lands`,
//    on any phone), and every row can be brought into view.
//  - `testTheSwitcherCountsItsAccounts` (F15).
//  - `testTheSignOutSheetFitsWhatItHolds` (F26).
//  - `testTheBatchImporterNamesNoPlaceholderCurrency` (F8).
//  - `testTheNarrowPhone` (F16, F24, F26): run on an iPhone SE — the long
//    line in es-MX / it, the three places' lines in ru / pt-BR / de / en at
//    full size, the sign-out sheet in de.
//
//  Each measurement is written to the log as `MEASURE <name> …`. Simulator
//  only; skipped in the scheme (a copy of the .xctestrun with the skip
//  removed runs it).
//

import XCTest

final class FinalRoundScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    private typealias Look = (lang: String, theme: String)

    private static let looks: [Look] = [
        ("zh", "light"), ("en", "light"), ("zh", "dark"), ("en", "dark"),
    ]

    // MARK: - F19: Checking… → Live / Can't reach

    func testTheHeroSaysCheckingThenLiveOrCantReach() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let checking = zh ? "正在检查…" : "Checking…"
            let live = zh ? "实时 · 监听收款中" : "Live · listening for payments"
            let cantReach = zh ? "2 个网络暂时连不上" : "Can't reach 2 networks right now"
            var at: [String: CGFloat] = [:]
            for (board, line) in [("hero-checking", checking), ("hero-live", live), ("hero-cant-reach", cantReach)] {
                let app = launch(board, look)
                let said = app.descendants(matching: .any)
                    .matching(NSPredicate(format: "label == %@", line)).firstMatch
                XCTAssertTrue(said.waitForExistence(timeout: 30), "\(board) does not say \"\(line)\" (\(tag))")
                if board != "hero-live" {
                    XCTAssertFalse(text(app, live).exists, "\(board) says the wallet is live (\(tag))")
                }
                let refresh = app.buttons["balance-refresh"].firstMatch
                XCTAssertTrue(refresh.exists, "no refresh control (\(tag))")
                at[board] = refresh.frame.minY
                settle(1)
                attach(app, "19-\(board)-\(tag)")
                app.terminate()
            }
            log("hero-line \(tag) refresh.y checking=\(at["hero-checking"] ?? -1) live=\(at["hero-live"] ?? -1)"
                + " cant-reach=\(at["hero-cant-reach"] ?? -1)")
            XCTAssertEqual(at["hero-live"] ?? -1, at["hero-checking"] ?? -2, accuracy: 0.5, "\"live\" moved the page (\(tag))")
            XCTAssertEqual(at["hero-cant-reach"] ?? -1, at["hero-checking"] ?? -2, accuracy: 0.5,
                           "\"can't reach\" moved the page (\(tag))")
        }
    }

    // MARK: - F16: a long status is one line

    func testALongStatusIsOneLine() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let fault = zh ? "Vela 内部出了问题" : "Something went wrong inside Vela"

            var app = launch("hero-fault", look)
            let line = app.descendants(matching: .any).matching(identifier: "balance-status").firstMatch
            XCTAssertTrue(line.waitForExistence(timeout: 30), "no status line (\(tag))")
            XCTAssertTrue(line.label.hasPrefix(fault), "the line is not Vela's own fault: \(line.label) (\(tag))")
            let sentence = line.label
            let refresh = app.buttons["balance-refresh"].firstMatch
            log("hero-fault \(tag) line=\(line.frame) refresh.y=\(refresh.frame.minY) chars=\(sentence.count)")
            XCTAssertLessThan(line.frame.height, 30, "the line took a second line (\(tag))")
            XCTAssertLessThanOrEqual(line.frame.maxX, app.frame.width - 23, "the line runs off the hero (\(tag))")
            settle(1)
            attach(app, "16-hero-long-reason-one-line-\(tag)")
            app.terminate()

            // What it opens says the sentence whole, at its top.
            app = launch("breakdown-fault", look)
            let whole = text(app, sentence)
            XCTAssertTrue(whole.waitForExistence(timeout: 30), "the breakdown does not say the line's sentence (\(tag))")
            let total = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", zh ? "总计" : "Total")).firstMatch
            if total.exists {
                XCTAssertLessThan(whole.frame.minY, total.frame.minY, "the sentence is not at the top (\(tag))")
            }
            settle(1)
            attach(app, "16-breakdown-says-the-line-whole-\(tag)")
            app.terminate()
        }
    }

    // MARK: - F20, F21: the breakdown

    func testTheBreakdownsStatusesAndHeadings() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let list = zh ? "代币列表无法读取" : "Token list unavailable"
            let rpc = zh ? "RPC 无法连接" : "RPC unavailable"

            var app = launch("breakdown-status", look)
            XCTAssertTrue(text(app, "Tempo").waitForExistence(timeout: 30), "no Tempo row (\(tag))")
            XCTAssertTrue(text(app, list).exists, "Tempo's row does not read \"\(list)\" (\(tag))")
            XCTAssertTrue(text(app, rpc).exists, "BNB Chain's row does not read \"\(rpc)\" (\(tag))")
            XCTAssertTrue(app.descendants(matching: .any)["balanceDetail.pending"].firstMatch.exists,
                          "networks are out and nothing heads them (\(tag))")
            settle(1)
            attach(app, "21-breakdown-short-status-\(tag)")
            app.terminate()

            app = launch("breakdown-healthy", look)
            XCTAssertTrue(app.descendants(matching: .any)["balanceDetail.done"].firstMatch.waitForExistence(timeout: 30),
                          "no settled networks (\(tag))")
            XCTAssertFalse(app.descendants(matching: .any)["balanceDetail.pending"].firstMatch.exists,
                           "\"Networks still updating\" heads an empty list (\(tag))")
            XCTAssertFalse(app.staticTexts.matching(NSPredicate(
                format: "label CONTAINS %@", zh ? "暂时无法连接" : "couldn't be reached")).firstMatch.exists,
                "a healthy wallet is told networks could not be reached (\(tag))")
            settle(1)
            attach(app, "20-breakdown-healthy-no-empty-heading-\(tag)")
            app.terminate()
        }
    }

    // MARK: - F4, F14, F22: the RPC field and its re-check

    func testTheWizardsRpcFieldFollowsTheCore() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let optional = zh ? "自定义 RPC（可选）" : "Custom RPC (optional)"
            let required = "RPC URL"
            let boards: [(board: String, label: String?, name: String)] = [
                ("compatible", optional, "22-wizard-compatible-field-and-recheck"),
                ("checked-unverified", optional, "22-wizard-unable-to-verify-field-and-recheck"),
                ("no-rpc", required, "04-wizard-no-rpc-field-is-rpc-url"),
                ("unverified", optional, "22-wizard-stop-unable-to-verify"),
                ("no-p256", nil, "22-refusal-no-p256-no-field-no-recheck"),
                ("missing", nil, "22-refusal-missing-contracts-no-field-no-recheck"),
            ]
            for item in boards {
                let app = launch("wizard-\(item.board)", look)
                XCTAssertTrue(text(app, "Zircuit").waitForExistence(timeout: 30), "no candidate (\(tag) \(item.board))")
                settle(1)
                let recheck = app.buttons["addNetwork.recheck"].firstMatch
                let labels = [optional, required].filter { text(app, $0).exists }
                if let label = item.label {
                    XCTAssertEqual(labels, [label], "the field's label (\(tag) \(item.board))")
                    XCTAssertTrue(recheck.exists, "a field with no re-check (\(tag) \(item.board))")
                    if item.board == "no-rpc" {
                        // Why first, then the box.
                        let why = app.descendants(matching: .any)["addNetwork.stop"].firstMatch
                        XCTAssertTrue(why.exists, "the stop says nothing (\(tag))")
                        XCTAssertLessThan(why.frame.minY, text(app, label).frame.minY,
                                          "the field is above the sentence that asks for it (\(tag))")
                    }
                } else {
                    XCTAssertTrue(labels.isEmpty, "an RPC field under a refusal: \(labels) (\(tag) \(item.board))")
                    XCTAssertFalse(recheck.exists, "a re-check under a refusal (\(tag) \(item.board))")
                }
                attach(app, "\(item.name)-\(tag)")
                app.terminate()
            }
        }
    }

    // MARK: - F2: the verdict's place

    func testTheVerdictLandsInItsPlace() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let kinds = ["out", "send", "swap", "three", "unverified", "tall", "nothing", "caution", "danger"]
            var at: [String: CGFloat] = [:]
            for kind in kinds {
                let app = launch("signing-\(kind)", look)
                let confirm = app.buttons["signing.confirm"].firstMatch
                XCTAssertTrue(confirm.waitForExistence(timeout: 30), "no confirm (\(tag) \(kind))")
                settle(1.2)
                at[kind] = confirm.frame.minY
                if kind == "out" {
                    XCTAssertTrue(text(app, zh ? "正在检查…" : "Checking…").exists,
                                  "the place is blank while the simulation is out (\(tag))")
                }
                attach(app, "02-signing-verdict-\(kind)-\(tag)")
                app.terminate()
            }
            log("signing-verdict \(tag) confirm.y " + kinds.map { "\($0)=\(at[$0] ?? -1)" }.joined(separator: " "))
            for kind in kinds {
                XCTAssertEqual(at[kind] ?? -1, at["out"] ?? -2, accuracy: 0.5,
                               "the confirm moved under \"\(kind)\" (\(tag))")
            }
        }
    }

    // MARK: - Device round: a tall verdict, whole, over a pinned confirm

    /// The confirm, the verdict in its place (its own frame: the room a
    /// short one leaves under it is not in it), the fee row's refresh — the
    /// first thing under the place — and the scrolling body.
    private struct Verdict {
        let confirm: CGRect
        let place: CGRect
        let fee: CGRect
        let body: CGRect
        /// From the verdict's last point to the fee row: the least it can
        /// be when the place is exactly as tall as the verdict.
        var gap: CGFloat { fee.minY - place.maxY }
    }

    /// The four rows of the tall verdict, top to bottom, and its warning.
    private func tallLines(_ zh: Bool) -> (rows: [String], warning: String) {
        (["ETH", "WETH", "USDC", zh ? "未验证代币" : "Unverified token"],
         zh ? "无法在链上核实代币数量——批准前请再次确认。"
            : "Token amount couldn't be verified on-chain — double-check it before approving.")
    }

    /// `label` as the verdict's place says it (the request has an "ETH" of
    /// its own above).
    private func said(_ app: XCUIApplication, _ label: String, in place: CGRect) -> XCUIElement? {
        app.staticTexts.matching(NSPredicate(format: "label == %@", label)).allElementsBoundByIndex
            .first { $0.frame.midY >= place.minY - 0.5 && $0.frame.midY <= place.maxY + 0.5 }
    }

    private func read(_ app: XCUIApplication, _ what: String) -> Verdict {
        let confirm = app.buttons["signing.confirm"].firstMatch
        XCTAssertTrue(confirm.waitForExistence(timeout: 30), "no confirm (\(what))")
        let place = app.descendants(matching: .any).matching(identifier: "signing.verdict").firstMatch
        XCTAssertTrue(place.waitForExistence(timeout: 10), "no verdict place (\(what))")
        let body = app.scrollViews.firstMatch
        XCTAssertTrue(body.exists, "no body (\(what))")
        let fee = app.buttons["signing.fee.refresh"].firstMatch
        XCTAssertTrue(fee.exists, "no fee row (\(what))")
        return Verdict(confirm: confirm.frame, place: place.frame, fee: fee.frame, body: body.frame)
    }

    func testATallVerdictIsShownWholeOverAPinnedConfirm() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let lines = tallLines(zh)
            for prefix in ["signing", "sheet"] {
                var seen: [String: Verdict] = [:]
                for kind in ["out", "send", "three", "tall"] {
                    let app = launch("\(prefix)-\(kind)", look)
                    let what = "\(prefix)-\(kind) \(tag)"
                    settle(1.5)
                    let at = read(app, what)
                    seen[kind] = at
                    let screen = app.frame
                    log("tall-verdict \(what) screen=\(Int(screen.width))x\(Int(screen.height)) confirm=\(at.confirm)"
                        + " verdict=\(at.place) fee.y=\(at.fee.minY) body=\(at.body)")
                    // The confirm: whole, on the screen, under the body.
                    XCTAssertTrue(app.buttons["signing.confirm"].firstMatch.isHittable, "the confirm cannot be tapped (\(what))")
                    XCTAssertLessThanOrEqual(at.confirm.maxY, screen.height, "the confirm runs off the screen (\(what))")
                    XCTAssertGreaterThanOrEqual(at.confirm.minY, at.body.maxY - 0.5, "the confirm is inside the scroll (\(what))")
                    // One scroll view: the body. Nothing scrolls inside it.
                    XCTAssertEqual(app.scrollViews.count, 1, "a scroll view inside the sheet's own (\(what))")
                    if kind == "tall" {
                        var bottom = at.place.minY
                        for label in lines.rows + [lines.warning] {
                            guard let line = said(app, label, in: at.place) else {
                                XCTFail("\"\(label)\" is not in the verdict's place (\(what))")
                                continue
                            }
                            XCTAssertGreaterThanOrEqual(line.frame.minY, bottom - 0.5, "\"\(label)\" overlaps the line above (\(what))")
                            XCTAssertLessThanOrEqual(line.frame.maxY, at.place.maxY + 0.5, "\"\(label)\" runs out of the place (\(what))")
                            bottom = line.frame.maxY
                            // On a phone the whole sheet fits: every line is in view.
                            if at.place.maxY <= at.body.maxY + 0.5 {
                                XCTAssertTrue(line.isHittable, "\"\(label)\" is not in view (\(what))")
                                XCTAssertLessThanOrEqual(line.frame.maxY, at.body.maxY + 0.5, "\"\(label)\" is under the fold (\(what))")
                            }
                        }
                        // The card ends a padding under its last line: nothing of it is under a fold of its own.
                        log("tall-verdict \(what) verdict.h=\(at.place.height) last-line.bottom=\(bottom) verdict.bottom=\(at.place.maxY)")
                        XCTAssertLessThanOrEqual(at.place.maxY - bottom, 16, "the verdict's card is taller than what it holds (\(what))")
                    }
                    attach(app, "01-\(prefix)-verdict-\(kind)-\(tag)-\(Int(screen.width))pt")
                    app.terminate()
                }
                guard let out = seen["out"], let send = seen["send"], let three = seen["three"], let tall = seen["tall"]
                else { continue }
                log("tall-verdict \(prefix) \(tag) confirm.y out=\(out.confirm.minY) send=\(send.confirm.minY)"
                    + " three=\(three.confirm.minY) tall=\(tall.confirm.minY)"
                    + " | verdict.h out=\(out.place.height) send=\(send.place.height) three=\(three.place.height) tall=\(tall.place.height)"
                    + " | fee.y out=\(out.fee.minY) send=\(send.fee.minY) three=\(three.fee.minY) tall=\(tall.fee.minY)"
                    + " | gap under the verdict three=\(three.gap) tall=\(tall.gap)")
                // The confirm: one frame, with no verdict, one row, three, and the tall one.
                for (kind, at) in [("send", send), ("three", three), ("tall", tall)] {
                    XCTAssertEqual(at.confirm, out.confirm, "the confirm moved under \"\(kind)\" (\(prefix) \(tag))")
                }
                // The place is a minimum: one row lands in it and moves
                // nothing — the fee row is as far under the place's top as
                // it was under "Checking…" (wherever the body is scrolled).
                XCTAssertEqual(send.fee.minY - send.place.minY, out.fee.minY - out.place.minY, accuracy: 0.5,
                               "one row moved the fee row (\(prefix) \(tag))")
                // …and a taller verdict grows it to exactly its own height:
                // the fee row follows the verdict's end at the same distance
                // under three rows and under the tall one, and the body is
                // longer by what the verdict is taller.
                XCTAssertEqual(tall.gap, three.gap, accuracy: 0.5, "the place is not as tall as its verdict (\(prefix) \(tag))")
                XCTAssertEqual((tall.fee.minY - tall.place.minY) - (three.fee.minY - three.place.minY),
                               tall.place.height - three.place.height, accuracy: 0.5,
                               "the place did not grow by what the verdict did (\(prefix) \(tag))")
                XCTAssertGreaterThan(tall.place.height, three.place.height + 40, "the tall verdict is not tall (\(prefix) \(tag))")
            }
        }
    }

    func testTheTallVerdictLandsUnderAPinnedConfirm() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let lines = tallLines(zh)
            // The plain send, as the browser presents it and on the board;
            // and a request with more to say (the drawn send), whose sheet
            // is taller than a short phone.
            for board in ["sheet-lands", "signing-lands", "sheet-rich-lands"] {
                let app = launch(board, look)
                let what = "\(board) \(tag)"
                XCTAssertTrue(text(app, zh ? "正在检查…" : "Checking…").waitForExistence(timeout: 30),
                              "the sheet did not open on \"Checking…\" (\(what))")
                let before = read(app, what)
                let screen = app.frame
                attach(app, "04-\(board)-before-\(tag)-\(Int(screen.width))pt")

                // The simulation answers.
                let warning = text(app, lines.warning)
                XCTAssertTrue(warning.waitForExistence(timeout: 20), "the tall verdict did not land (\(what))")
                settle(1.2)
                let after = read(app, what)
                log("verdict-lands \(what) screen=\(Int(screen.width))x\(Int(screen.height))"
                    + " confirm before=\(before.confirm) after=\(after.confirm)"
                    + " place before=\(before.place) after=\(after.place) body=\(after.body)")
                XCTAssertEqual(after.confirm, before.confirm, "the confirm moved when the verdict landed (\(what))")
                XCTAssertTrue(app.buttons["signing.confirm"].firstMatch.isHittable, "the confirm cannot be tapped (\(what))")
                XCTAssertGreaterThan(after.place.height, before.place.height + 20, "the place did not grow (\(what))")
                // In view as it landed: whole when the body can hold it, else from its top.
                if after.place.height <= after.body.height {
                    XCTAssertGreaterThanOrEqual(after.place.minY, after.body.minY - 0.5, "the verdict starts above the body (\(what))")
                    XCTAssertLessThanOrEqual(after.place.maxY, after.body.maxY + 0.5, "the verdict ends under the fold (\(what))")
                } else {
                    XCTAssertEqual(after.place.minY, after.body.minY, accuracy: 60, "the verdict does not start at the body's top (\(what))")
                }
                attach(app, "04-\(board)-landed-\(tag)-\(Int(screen.width))pt")

                // Every row and the warning can be read whole: in view now,
                // or after the body is scrolled — and the confirm stays put.
                let body = app.scrollViews.firstMatch
                for label in lines.rows + [lines.warning] {
                    var tries = 0
                    while tries < 4 {
                        let place = app.descendants(matching: .any).matching(identifier: "signing.verdict").firstMatch.frame
                        if let line = said(app, label, in: place),
                           line.frame.minY >= body.frame.minY - 0.5, line.frame.maxY <= body.frame.maxY + 0.5 { break }
                        let up = (said(app, label, in: place)?.frame.minY ?? 0) > body.frame.midY
                        let from = body.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: up ? 0.7 : 0.3))
                        let to = body.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: up ? 0.3 : 0.7))
                        from.press(forDuration: 0.05, thenDragTo: to)
                        settle(0.6)
                        tries += 1
                    }
                    let place = app.descendants(matching: .any).matching(identifier: "signing.verdict").firstMatch.frame
                    let line = said(app, label, in: place)
                    XCTAssertNotNil(line, "\"\(label)\" is not in the verdict's place (\(what))")
                    if let line {
                        XCTAssertTrue(line.frame.minY >= body.frame.minY - 0.5 && line.frame.maxY <= body.frame.maxY + 0.5,
                                      "\"\(label)\" cannot be brought into view: \(line.frame) in \(body.frame) (\(what))")
                    }
                    XCTAssertEqual(app.buttons["signing.confirm"].firstMatch.frame, before.confirm,
                                   "the confirm moved while the body scrolled (\(what))")
                }
                attach(app, "04-\(board)-scrolled-\(tag)-\(Int(screen.width))pt")
                app.terminate()
            }
        }
    }

    // MARK: - F15: the switcher's count

    func testTheSwitcherCountsItsAccounts() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            for (count, lead) in [(1, zh ? "1 个账户 · " : "1 account · "), (2, zh ? "2 个账户 · " : "2 accounts · ")] {
                let app = launch("accounts-\(count)", look)
                let summary = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", lead)).firstMatch
                XCTAssertTrue(summary.waitForExistence(timeout: 30), "the sheet does not read \"\(lead)…\" (\(tag))")
                XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "switcherAccountCount"))
                    .firstMatch.exists, "the count's key is drawn (\(tag))")
                if !zh, count == 1 {
                    XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "1 accounts"))
                        .firstMatch.exists, "\"1 accounts\" (\(tag))")
                }
                settle(1)
                attach(app, "15-switcher-\(count)-account\(count == 1 ? "" : "s")-\(tag)")
                app.terminate()
            }
        }
    }

    // MARK: - F26: the sign-out sheet

    func testTheSignOutSheetFitsWhatItHolds() {
        for look in Self.looks {
            for board in ["signout-one", "signout-many"] {
                signOut(board, look)
            }
        }
    }

    private func signOut(_ board: String, _ look: Look) {
        let tag = "\(look.lang)-\(look.theme)"
        let words: [String: (cancel: String, title: String)] = [
            "zh": ("取消", "退出登录"), "en": ("Cancel", "Sign Out"), "de": ("Abbrechen", ""), "ru": ("Отмена", ""),
        ]
        let app = launch(board, look)
        let cancel = app.buttons[words[look.lang]?.cancel ?? "Cancel"].firstMatch
        XCTAssertTrue(cancel.waitForExistence(timeout: 30), "no sheet (\(tag) \(board))")
        settle(2)
        let screen = app.frame
        // The sheet's own texts: everything from its first line down.
        let inSheet = app.staticTexts.allElementsBoundByIndex
            .filter { $0.frame.minY > screen.height * 0.2 && $0.frame.maxY <= cancel.frame.minY }
        let top = inSheet.map(\.frame.minY).min() ?? -1
        // Content: 32 above its first line, 32 under Cancel.
        let content = cancel.frame.maxY + 32 - (top - 32)
        log("signout \(board) \(tag) screen=\(Int(screen.width))x\(Int(screen.height)) first-line.y=\(top)"
            + " cancel=\(cancel.frame) content=\(content) (was fixed at \(board == "signout-many" ? 460 : 360))")
        XCTAssertTrue(cancel.isHittable, "Cancel is off the sheet (\(tag) \(board))")
        XCTAssertLessThanOrEqual(cancel.frame.maxY, screen.height - 20, "Cancel runs off the bottom (\(tag) \(board))")
        attach(app, "26-\(board)-\(tag)")
        app.terminate()
    }

    // MARK: - F8: the batch importer

    func testTheBatchImporterNamesNoPlaceholderCurrency() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            var app = launch("batch-unknown", look)
            let unit = app.descendants(matching: .any)
                .matching(NSPredicate(format: "label == %@", zh ? "按 … 计价" : "In …")).firstMatch
            XCTAssertTrue(unit.waitForExistence(timeout: 30), "the unit names a currency nobody chose (\(tag))")
            XCTAssertFalse(app.descendants(matching: .any)
                .matching(NSPredicate(format: "label == %@", zh ? "按 USD 计价" : "In USD")).firstMatch.exists,
                "\"USD\" is said before the person's currency is known (\(tag))")
            settle(1)
            attach(app, "08-batch-import-currency-unknown-\(tag)")
            app.terminate()

            app = launch("batch-known", look)
            XCTAssertTrue(app.descendants(matching: .any)
                .matching(NSPredicate(format: "label == %@", zh ? "按 CNY 计价" : "In CNY")).firstMatch
                .waitForExistence(timeout: 30), "the unit does not name the person's currency (\(tag))")
            settle(1)
            attach(app, "08-batch-import-currency-known-\(tag)")
            app.terminate()
        }
    }

    // MARK: - The narrow phone (run on an iPhone SE)

    func testTheNarrowPhone() {
        // F16: Tempo's token list in the two longest languages — one line.
        for lang in ["es-MX", "it"] {
            let look: Look = (lang, "light")
            var app = launch("hero-long", look)
            let line = app.descendants(matching: .any).matching(identifier: "balance-status").firstMatch
            XCTAssertTrue(line.waitForExistence(timeout: 30), "no status line (\(lang))")
            let refresh = app.buttons["balance-refresh"].firstMatch
            log("narrow hero-long \(lang) screen=\(Int(app.frame.width)) line=\(line.frame) refresh.y=\(refresh.frame.minY)"
                + " chars=\(line.label.count) says=\(line.label)")
            XCTAssertLessThan(line.frame.height, 30, "the line took a second line (\(lang))")
            XCTAssertLessThanOrEqual(line.frame.maxX, app.frame.width - 23, "the line runs off the hero (\(lang))")
            XCTAssertTrue(line.label.contains("Tempo"), "the whole sentence is not the line's label (\(lang))")
            let sentence = line.label
            settle(1)
            attach(app, "16-hero-long-token-list-\(lang)-\(Int(app.frame.width))pt")
            app.terminate()

            app = launch("list-token-list", look)
            XCTAssertTrue(text(app, sentence).waitForExistence(timeout: 30),
                          "the list is not titled by the whole sentence (\(lang))")
            settle(1)
            attach(app, "16-list-says-it-whole-\(lang)-\(Int(app.frame.width))pt")
            app.terminate()
        }

        // F24: the three places' lines at full size.
        for lang in ["ru", "pt-BR", "de", "en"] {
            let app = XCUIApplication()
            app.launchEnvironment["VELA_GALLERY"] = "1"
            app.launchEnvironment["VELA_GALLERY_FIXTURE"] = "keys · none"
            configure(app, (lang, "light"))
            app.launch()
            XCTAssertTrue(app.descendants(matching: .any)["create.addHeading"].firstMatch
                .waitForExistence(timeout: 30), "no keys screen (\(lang))")
            settle(1.5)
            // Every line under a place's title: its height is one line's.
            let lines = app.staticTexts.allElementsBoundByIndex.filter { $0.label.count > 28 }
            log("narrow method-rows \(lang) screen=\(Int(app.frame.width)) "
                + lines.map { "[\($0.label.count)ch w=\($0.frame.width) h=\($0.frame.height)]" }.joined(separator: " "))
            attach(app, "24-method-rows-full-size-\(lang)-\(Int(app.frame.width))pt")
            app.terminate()
        }

        // F26: the sign-out sheet with everything it can say, in German.
        signOut("signout-many", ("de", "light"))
        signOut("signout-one", ("de", "light"))
    }

    // MARK: - Helpers

    private func configure(_ app: XCUIApplication, _ look: Look) {
        app.launchEnvironment["VELA_LANG"] = look.lang
        app.launchEnvironment["VELA_THEME"] = look.theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        let system: String
        switch look.lang {
        case "zh": system = "(zh-Hans)"
        default: system = "(\(look.lang))"
        }
        app.launchArguments += ["-AppleLanguages", system]
    }

    private func launch(_ board: String, _ look: Look) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PAGE"] = "pr3c"
        app.launchEnvironment["VELA_STATE"] = board
        configure(app, look)
        app.launch()
        return app
    }

    private func text(_ app: XCUIApplication, _ label: String) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label == %@", label)).firstMatch
    }

    private func settle(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    private func log(_ line: String) {
        print("MEASURE \(line)")
        let attachment = XCTAttachment(string: line)
        attachment.name = "measure"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The whole screen, not the app's frame: on an iPad the iPhone layout
    /// runs in a window, and the app's own screenshot is a crop of it.
    private func attach(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
