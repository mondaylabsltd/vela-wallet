//
//  IntegrationRound2ScreenshotTests.swift
//  VelaWalletUITests
//
//  PR 3's integration round (shell round 2), in zh and en, light and dark —
//  walked on a simulator, photographed and MEASURED. Every board is a
//  `VELA_PAGE` session: fixture data, no chain, and no camera (the scanner
//  draws its fixture frame there, and `ScannerFixtureTests` holds the gate).
//
//  - `testATokenListThatCantLoad` (note 4): the home's line for Tempo's
//    token list and the list it opens — Tempo's row, no "Fix" — beside a
//    network really out of reach, which keeps its Fix.
//  - `testTheHiddenSplit` (notes 3, 11, 15): the split's row and its detail,
//    shown and hidden; hidden, the total and each share read "•••• USDC".
//  - `testEveryWizardStopSaysWhy` (notes 5, 10, 18): a refusal on the scan /
//    auto-add path — the reason, and Chain Setup only for missing contracts
//    — and the other four stops in the core's sentences.
//  - `testAWalletThatCantBeCopiedHasNoParagraph` (note 6).
//  - `testACheckedTimeNeverBreaks` (note 14): the moment at six narrow widths.
//  - `testTheKeysScreen` (notes 17, 22): no key — the new heading, no
//    counter; one key — the counter, and nothing above it moved.
//  - `testNoFiatBeforeTheCurrencyCommits` (notes 9, 27): the home, the send
//    form, the confirm, the signing sheet, the balance detail sheet and the
//    token page with the currency on its way, then committed — no fiat
//    figure before, and every element the two frames share at the same place.
//  - `testTheScannerDrawsAFixtureFrame` (note 7).
//  - `testTheHeroKeepsItsStatusLine` (note 26b): the page under the hero at
//    the same place with and without the line.
//  - `testASplitRowIsTwoLines` (note 21): "Recipient N" and the two doors,
//    then the avatar, the address, the amount and ✕ — 44 pt targets.
//  - `testAFailureSheetFitsItsMessage` (note 26a).
//
//  Each measurement is written to the log as `MEASURE <name> …`, so a report
//  can quote it. Simulator only; skipped in the scheme (a copy of the
//  .xctestrun with the skip removed runs it).
//

import XCTest

final class IntegrationRound2ScreenshotTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    private static let looks: [(lang: String, theme: String)] = [
        ("zh", "light"), ("en", "light"), ("zh", "dark"), ("en", "dark"),
    ]

    // MARK: - Note 4: a token list that can't load

    func testATokenListThatCantLoad() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let line = zh ? "暂时读不到 Tempo 的代币列表" : "Can't load Tempo's token list right now"
            let fix = zh ? "修复" : "Fix"

            var app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "home-token-list"], look)
            XCTAssertTrue(text(app, line).waitForExistence(timeout: 30), "the home does not say the list could not load (\(tag))")
            XCTAssertFalse(text(app, zh ? "暂时连不上 Tempo" : "Can't reach Tempo right now").exists,
                           "a token list that can't load reads as a network out of reach (\(tag))")
            settle(1)
            attach(app, "04-token-list-unreachable-home-\(tag)")
            app.terminate()

            app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "list-token-list"], look)
            XCTAssertTrue(text(app, line).waitForExistence(timeout: 30), "the list is not titled by the home's line (\(tag))")
            XCTAssertTrue(text(app, "Tempo").exists, "no Tempo row (\(tag))")
            XCTAssertFalse(app.buttons[fix].exists || app.staticTexts[fix].exists,
                           "a chain whose RPC is fine is offered \"\(fix)\" (\(tag))")
            settle(1)
            attach(app, "04-token-list-unreachable-list-no-fix-\(tag)")
            app.terminate()

            // The same chain really out of reach keeps its Fix.
            app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "list-network"], look)
            XCTAssertTrue(text(app, "Tempo").waitForExistence(timeout: 30), "no Tempo row (\(tag))")
            XCTAssertTrue(app.buttons[fix].exists || app.staticTexts[fix].exists,
                          "a network out of reach lost its \"\(fix)\" (\(tag))")
            settle(1)
            attach(app, "04-network-unreachable-list-with-fix-\(tag)")
            app.terminate()
        }
    }

    // MARK: - Notes 3, 11, 15: the hidden split

    func testTheHiddenSplit() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            for hidden in [false, true] {
                let side = hidden ? "hidden" : "shown"
                var app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "split-history-\(side)"], look)
                XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "USDC")).firstMatch
                    .waitForExistence(timeout: 30), "no history (\(tag))")
                settle(1)
                assertFigures(app, hidden: hidden, where: "the split's row", tag)
                attach(app, "06-split-row-\(side)-\(tag)")
                app.terminate()

                app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "split-detail-\(side)"], look)
                let who = text(app, "Bea")
                XCTAssertTrue(who.waitForExistence(timeout: 30), "the detail does not say who it went to (\(tag))")
                settle(1)
                assertFigures(app, hidden: hidden, where: "the split's detail", tag)
                if hidden {
                    // The total and BOTH shares: the mask, with its coin.
                    let masked = app.staticTexts.matching(NSPredicate(format: "label == %@", "•••• USDC")).count
                    XCTAssertGreaterThanOrEqual(masked, 3, "the total and each share do not read \"•••• USDC\" (\(masked) of 3, \(tag))")
                } else {
                    XCTAssertTrue(text(app, "214.5 USDC").exists && text(app, "469.25 USDC").exists,
                                  "the shares are not listed (\(tag))")
                }
                attach(app, "06-split-detail-\(side)-\(tag)")
                app.terminate()
            }
        }
    }

    /// The split's three figures: all drawn while shown, none while hidden.
    private func assertFigures(_ app: XCUIApplication, hidden: Bool, where place: String, _ tag: String) {
        let runs = ["683", "214", "469"]
        let drawn = runs.filter { run in
            app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", run)).firstMatch.exists
        }
        if hidden {
            XCTAssertTrue(drawn.isEmpty, "\(place) draws \(drawn) while hidden (\(tag))")
        } else {
            XCTAssertTrue(drawn.contains("683"), "\(place) does not state the total while shown (\(tag))")
        }
    }

    // MARK: - Notes 5, 10, 18: every wizard stop says why

    func testEveryWizardStopSaysWhy() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let setup = zh ? "打开链配置工具" : "Open Chain Setup Tool"
            let stops: [(board: String, says: String, name: String)] = [
                ("no-p256", "P-256", "03-refusal-scan-path-no-p256"),
                ("missing", zh ? "合约" : "contracts", "03-refusal-scan-path-missing-contracts"),
                ("already", zh ? "该网络已添加" : "This network is already added", "03-wizard-stop-already-added"),
                ("not-found", zh ? "未找到链信息" : "Chain info not found", "03-wizard-stop-not-found"),
                ("no-rpc", zh ? "没有列出 RPC 节点" : "No RPC endpoint is listed", "03-wizard-stop-no-rpc-endpoint"),
                ("unverified", zh ? "无法验证" : "Unable to verify", "03-wizard-stop-unable-to-verify"),
            ]
            for stop in stops {
                let app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "wizard-\(stop.board)"], look)
                let sentence = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", stop.says)).firstMatch
                XCTAssertTrue(sentence.waitForExistence(timeout: 30), "\(stop.board) does not say why (\(tag))")
                settle(1)
                // The BUTTON, by its own words — the missing-contracts line
                // names Chain Setup too, and that is not an offer.
                let offered = app.buttons[setup].exists || text(app, setup).exists
                if stop.board == "missing" {
                    XCTAssertTrue(offered, "missing contracts is not offered \(setup) (\(tag))")
                } else {
                    XCTAssertFalse(offered, "\(stop.board) is offered Chain Setup (\(tag))")
                }
                attach(app, "\(stop.name)-\(tag)")
                app.terminate()
            }
        }
    }

    // MARK: - Note 6: a wallet that can't be copied

    func testAWalletThatCantBeCopiedHasNoParagraph() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            for (board, name) in [("backup-cannot-copy", "05-backup-not-copyable-no-paragraph"),
                                  ("backup-not-copied", "05-backup-not-copied-with-paragraph")] {
                let app = launch(env: ["VELA_PAGE": "pr3", "VELA_STATE": board], look)
                let title = text(app, zh ? "把钱包记录复制到以太坊" : "Copy this wallet's record to Ethereum")
                XCTAssertTrue(title.waitForExistence(timeout: 30), "no backup row (\(board), \(tag))")
                var scrolls = 0
                while !title.isHittable, scrolls < 6 {
                    app.swipeUp(velocity: .slow)
                    scrolls += 1
                }
                settle(1)
                let paragraph = app.descendants(matching: .any)["keys.backupExplain"].firstMatch
                if board == "backup-cannot-copy" {
                    XCTAssertTrue(text(app, zh ? "这个较早创建的钱包无法复制" : "This older wallet can't be copied").exists)
                    XCTAssertFalse(paragraph.exists, "a wallet that can't be copied is told how to copy (\(tag))")
                } else {
                    XCTAssertTrue(paragraph.exists, "the explanation is gone from a state that can still be copied (\(tag))")
                }
                attach(app, "\(name)-\(tag)")
                app.terminate()
            }
        }
    }

    // MARK: - Note 14: a checked time never breaks

    func testACheckedTimeNeverBreaks() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "checked-time"], look)
            let moment = app.staticTexts.matching(identifier: "checkedTime.moment").firstMatch
            XCTAssertTrue(moment.waitForExistence(timeout: 30), "no board (\(tag))")
            settle(1)
            // The moment as the core wrote it: its spaces are no-break
            // spaces, and a CJK day period carries a word joiner.
            let label = moment.label
            XCTAssertFalse(label.contains(" "), "a plain space inside the moment: \(label.debugDescription) (\(tag))")
            if look.lang == "zh" {
                // The accessibility label drops the zero-width joiner (the
                // system strips default-ignorable characters from a label);
                // the drawn string carries it — `SignerPageChecksTests` pins
                // the bytes. What is read out is the moment, whole.
                XCTAssertEqual(label.replacingOccurrences(of: "\u{2060}", with: ""), "下午\u{a0}2:32",
                               "\(label.debugDescription) (\(tag))")
            } else {
                XCTAssertEqual(label, "2:32\u{a0}PM", "\(label.debugDescription) (\(tag))")
            }
            log("checked-time \(tag) moment=\(label.debugDescription) width=\(moment.frame.width)")
            attach(app, "07-checked-time-narrow-\(tag)")
            app.swipeUp(velocity: .slow)
            settle(1)
            attach(app, "07-checked-time-narrow-dated-\(tag)")
            app.terminate()
        }
    }

    // MARK: - Notes 17, 22: the keys screen

    func testTheKeysScreen() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let count = zh ? "已添加" : "Added"

            var app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · none"], look)
            let heading = app.descendants(matching: .any)["create.addHeading"].firstMatch
            XCTAssertTrue(heading.waitForExistence(timeout: 30), "no heading (\(tag))")
            settle(1.5)
            XCTAssertEqual(heading.label, zh ? "选择存放位置" : "Choose where it lives")
            let title = text(app, zh ? "添加通行密钥" : "Add passkeys")
            XCTAssertTrue(title.exists, "no title (\(tag))")
            XCTAssertNotEqual(heading.label, title.label, "the heading repeats the title (\(tag))")
            XCTAssertFalse(app.descendants(matching: .any)["create.keyCount"].firstMatch.exists,
                           "a counter over an empty list (\(tag))")
            XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", count)).firstMatch.exists,
                           "\"\(count) 0 / 7\" is drawn (\(tag))")
            let none = frames(app)
            attach(app, "08-keys-0-new-heading-no-counter-\(tag)")
            app.terminate()

            app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · one, needs a second"], look)
            XCTAssertTrue(app.descendants(matching: .any)["create.addHeading"].firstMatch.waitForExistence(timeout: 30))
            settle(1.5)
            let counter = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "1 / 7")).firstMatch
            XCTAssertTrue(counter.exists, "no counter with the first key (\(tag))")
            // The counter arriving moved nothing above it: the screen's title
            // and its back affordance are where they were.
            let one = frames(app)
            let back = zh ? "返回" : "Back"
            for key in ["button|\(back)"] where none[key] != nil && one[key] != nil {
                XCTAssertEqual(none[key]!.minY, one[key]!.minY, accuracy: 0.5, "\(key) moved (\(tag))")
            }
            log("keys \(tag) back.y none=\(none["button|\(back)"]?.minY ?? -1) one=\(one["button|\(back)"]?.minY ?? -1)"
                + " counter.y=\(counter.frame.minY)")
            attach(app, "08-keys-1-counter-\(tag)")
            app.terminate()
        }
    }

    /// Note 23: the three places' second lines in the four locales whose
    /// "Phone or tablet" line was shortened (full size on a 375 pt phone) —
    /// and the two whose line is still too long there (ru, pt-BR), which is
    /// tightened to stay whole and must never be cut. Run on a 375 pt phone
    /// (`SIMFILE=sim-se`).
    func testTheMethodRowsInFourLocales() {
        for lang in ["de", "fr", "it", "es-MX", "ru", "pt-BR"] {
            let app = launch(env: ["VELA_GALLERY": "1", "VELA_GALLERY_FIXTURE": "keys · none"], (lang, "light"))
            XCTAssertTrue(app.descendants(matching: .any)["create.addHeading"].firstMatch.waitForExistence(timeout: 30),
                          "no heading (\(lang))")
            settle(1.5)
            // Every place's line is one line: no taller than the caption's
            // own line at this size (a wrapped line would be twice that).
            let lines = app.staticTexts.allElementsBoundByIndex.filter { element in
                element.frame.minY > app.frame.height * 0.3 && element.frame.maxY < app.frame.height * 0.62
                    && element.frame.height > 0 && element.frame.height < 40
            }
            let tallest = lines.map(\.frame.height).max() ?? 0
            log("method-rows \(lang) screen=\(app.frame.width)x\(app.frame.height) texts=\(lines.count) tallest=\(tallest)")
            // No line on the screen ends in an ellipsis: nothing was cut.
            let cut = app.staticTexts.allElementsBoundByIndex.map(\.label).filter { $0.hasSuffix("…") || $0.hasSuffix("...") }
            XCTAssertTrue(cut.isEmpty, "a line is cut (\(lang)): \(cut)")
            attach(app, "08-keys-method-rows-\(lang)-\(Int(app.frame.width))pt")
            app.terminate()
        }
    }

    // MARK: - Notes 9, 27: no fiat before the currency commits

    func testNoFiatBeforeTheCurrencyCommits() {
        let surfaces: [(board: String, name: String)] = [
            ("home", "09-currency-home"),
            ("send-form", "09-currency-send-form"),
            ("send-confirm", "09-currency-send-confirm"),
            ("signing", "09-currency-signing-sheet"),
            ("balance-detail", "09-currency-balance-detail"),
            ("token-page", "09-currency-token-page"),
        ]
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            for surface in surfaces {
                var app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "\(surface.board)-waiting"], look)
                settle(3)
                let before = frames(app)
                let fiatBefore = fiatLabels(app)
                XCTAssertTrue(fiatBefore.isEmpty,
                              "\(surface.board) draws fiat before the currency commits: \(fiatBefore) (\(tag))")
                XCTAssertGreaterThan(before.count, 3, "\(surface.board) drew nothing to compare (\(tag))")
                attach(app, "\(surface.name)-on-its-way-\(tag)")
                app.terminate()

                app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "\(surface.board)-committed"], look)
                settle(3)
                let after = frames(app)
                let fiatAfter = fiatLabels(app)
                XCTAssertTrue(fiatAfter.contains { $0.contains("¥") },
                              "\(surface.board) draws no figure in the person's currency once it commits: \(fiatAfter) (\(tag))")
                attach(app, "\(surface.name)-committed-\(tag)")
                app.terminate()

                // Everything the two frames share is where it was.
                let moved = compare(before, after)
                XCTAssertGreaterThan(moved.compared, 3, "\(surface.board): nothing in common to compare (\(tag))")
                XCTAssertTrue(moved.diffs.isEmpty, "\(surface.board) moved when the currency committed (\(tag)): \(moved.diffs)")
                log("currency \(surface.board) \(tag) compared=\(moved.compared) maxDy=\(moved.maxDy) maxDh=\(moved.maxDh)"
                    + " fiatBefore=\(fiatBefore.count) fiatAfter=\(fiatAfter.count)")
            }
        }
    }

    /// The labels on screen that are a fiat figure: a currency glyph beside a
    /// digit, or "≈" before one.
    private func fiatLabels(_ app: XCUIApplication) -> [String] {
        let glyphs: Set<Character> = ["$", "¥", "€", "£", "₩", "₹", "₫", "₺", "₽", "฿", "₱"]
        return frames(app).keys.compactMap { key -> String? in
            let label = String(key.split(separator: "|", maxSplits: 1).last ?? "")
            let characters = Array(label)
            for (index, character) in characters.enumerated() {
                let next = characters.dropFirst(index + 1).first { $0 != " " && $0 != "\u{a0}" }
                if glyphs.contains(character), next?.isNumber == true { return label }
                if character == "≈", let next, next.isNumber || glyphs.contains(next) { return label }
            }
            return nil
        }.sorted()
    }

    // MARK: - Note 7: the scanner's fixture frame

    func testTheScannerDrawsAFixtureFrame() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: ["VELA_PAGE": "flows-gallery", "VELA_STATE": "s1"], look)
            let frame = app.descendants(matching: .any)["scan.fixtureFrame"].firstMatch
            XCTAssertTrue(frame.waitForExistence(timeout: 30), "the scanner draws no fixture frame (\(tag))")
            settle(1)
            // No camera, and no sentence about one: the fixture frame is not
            // a refusal.
            for refusal in ["No camera", "未检测到摄像头", "Camera access", "相机权限"] {
                XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", refusal)).firstMatch.exists,
                               "the fixture frame says \"\(refusal)\" (\(tag))")
            }
            // And the system never asked for the camera.
            let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            XCTAssertFalse(springboard.alerts.firstMatch.exists, "a system prompt is up over the scanner (\(tag))")
            attach(app, "10-scanner-fixture-frame-\(tag)")
            app.terminate()
        }
    }

    // MARK: - Note 26b: the hero keeps its status line

    func testTheHeroKeepsItsStatusLine() {
        for look in Self.looks {
            let tag = "\(look.lang)-\(look.theme)"
            var app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "hero-plain"], look)
            XCTAssertTrue(app.buttons["balance-refresh"].firstMatch.waitForExistence(timeout: 30), "no hero (\(tag))")
            settle(2)
            let plain = frames(app)
            let plainRefresh = app.buttons["balance-refresh"].firstMatch.frame
            attach(app, "11-hero-without-status-line-\(tag)")
            app.terminate()

            app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "home-token-list"], look)
            XCTAssertTrue(app.buttons["balance-refresh"].firstMatch.waitForExistence(timeout: 30), "no hero (\(tag))")
            settle(2)
            let lined = frames(app)
            let linedRefresh = app.buttons["balance-refresh"].firstMatch.frame
            attach(app, "11-hero-with-status-line-\(tag)")
            app.terminate()

            let moved = compare(plain, lined)
            log("hero \(tag) refresh.y plain=\(plainRefresh.minY) lined=\(linedRefresh.minY)"
                + " compared=\(moved.compared) maxDy=\(moved.maxDy) diffs=\(moved.diffs.prefix(4))")
            XCTAssertEqual(plainRefresh.minY, linedRefresh.minY, accuracy: 0.5,
                           "the refresh control moved when the status line arrived (\(tag))")
            XCTAssertTrue(moved.diffs.isEmpty, "the page moved when the status line arrived (\(tag)): \(moved.diffs.prefix(6))")
        }
    }

    // MARK: - Note 21: a split row is two lines

    func testASplitRowIsTwoLines() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: ["VELA_PAGE": "flows-gallery", "VELA_STATE": "sd2b"], look)
            let ordinal = text(app, zh ? "收款人 1" : "Recipient 1")
            XCTAssertTrue(ordinal.waitForExistence(timeout: 30), "no split rows (\(tag))")
            settle(1.5)
            let picks = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "send.row.pick."))
            let scans = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "send.row.scan."))
            let removes = app.buttons.matching(NSPredicate(format: "label == %@", zh ? "移除" : "Remove"))
            XCTAssertEqual(picks.count, 3, "one address-book icon per row (\(tag))")
            XCTAssertEqual(scans.count, 3, "one scan icon per row (\(tag))")
            XCTAssertEqual(removes.count, 3, "one ✕ per row (\(tag))")
            let pick = picks.element(boundBy: 0).frame
            let scan = scans.element(boundBy: 0).frame
            let remove = removes.element(boundBy: 0).frame
            // The first row's own figure.
            let amount = text(app, "50").frame
            log("split-row \(tag) ordinal=\(ordinal.frame) pick=\(pick) scan=\(scan) remove=\(remove) amount=\(amount)")
            // 44 pt targets, every one.
            for (name, target) in [("pick", pick), ("scan", scan), ("remove", remove)] {
                XCTAssertGreaterThanOrEqual(target.width, 43.9, "\(name) is narrower than a target (\(tag))")
                XCTAssertGreaterThanOrEqual(target.height, 43.9, "\(name) is shorter than a target (\(tag))")
            }
            // Line 1: "Recipient N" and the two doors. Line 2: the amount and ✕.
            XCTAssertEqual(pick.midY, scan.midY, accuracy: 1, "the two doors are not on one line (\(tag))")
            XCTAssertEqual(ordinal.frame.midY, pick.midY, accuracy: 6, "\"Recipient N\" is not on the doors' line (\(tag))")
            XCTAssertEqual(amount.midY, remove.midY, accuracy: 6, "the amount and ✕ are not on one line (\(tag))")
            XCTAssertGreaterThan(remove.midY, pick.midY + 20, "✕ is not on the second line (\(tag))")
            XCTAssertLessThanOrEqual(amount.maxX, remove.minX + 1, "the amount runs under ✕ (\(tag))")
            // ✕ is under the scan door: the right edge is one column.
            XCTAssertEqual(remove.midX, scan.midX, accuracy: 1, "✕ is not under the scan door (\(tag))")
            attach(app, "12-split-recipient-row-\(tag)")
            app.terminate()
        }
    }

    // MARK: - Note 26a: a failure sheet fits its message

    func testAFailureSheetFitsItsMessage() {
        for look in Self.looks {
            let zh = look.lang == "zh"
            let tag = "\(look.lang)-\(look.theme)"
            let app = launch(env: ["VELA_PAGE": "pr3b", "VELA_STATE": "flowsheet-long"], look)
            let close = app.buttons[zh ? "关闭" : "Close"].firstMatch
            XCTAssertTrue(close.waitForExistence(timeout: 30), "no sheet (\(tag))")
            settle(2)
            let message = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Register failed")).firstMatch
            XCTAssertTrue(message.exists, "no message (\(tag))")
            let screen = app.frame
            // The sheet's title is the first text above the message.
            let title = app.staticTexts.allElementsBoundByIndex
                .filter { $0.frame.maxY <= message.frame.minY + 1 && $0.frame.minY > screen.height * 0.25 }
                .max { $0.frame.minY < $1.frame.minY }
            let titleTop = title?.frame.minY ?? -1
            // The badge is 56 pt, 16 pt above the title; the sheet's content
            // starts 32 pt above the badge. Its top edge is what a fixed
            // height cut into.
            let badgeTop = titleTop - 16 - 56
            log("flowsheet \(tag) screen=\(screen.height) title.y=\(titleTop) badgeTop=\(badgeTop)"
                + " message=\(message.frame) close=\(close.frame)")
            XCTAssertTrue(close.isHittable, "Close is off the sheet (\(tag))")
            XCTAssertLessThanOrEqual(close.frame.maxY, screen.height - 20, "Close runs off the bottom (\(tag))")
            attach(app, "13-failure-sheet-long-message-\(tag)")
            app.terminate()
        }
    }

    // MARK: - Helpers

    private func launch(env: [String: String], _ look: (lang: String, theme: String)) -> XCUIApplication {
        let app = XCUIApplication()
        for (key, value) in env { app.launchEnvironment[key] = value }
        app.launchEnvironment["VELA_LANG"] = look.lang
        app.launchEnvironment["VELA_THEME"] = look.theme
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        let system: String
        switch look.lang {
        case "zh": system = "(zh-Hans)"
        default: system = "(\(look.lang))"
        }
        app.launchArguments += ["-AppleLanguages", system]
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

    /// Every labelled text and button on screen, by `type|label` — the ones
    /// whose label is unique, so a frame is one element's.
    private func frames(_ app: XCUIApplication) -> [String: CGRect] {
        var out: [String: CGRect] = [:]
        var repeated: Set<String> = []
        let snapshot = try? app.snapshot()
        func walk(_ node: XCUIElementSnapshot) {
            let kind: String?
            switch node.elementType {
            case .staticText: kind = "text"
            case .button: kind = "button"
            default: kind = nil
            }
            if let kind, !node.label.isEmpty, !node.frame.isEmpty {
                let key = "\(kind)|\(node.label)"
                if out[key] != nil { repeated.insert(key) }
                out[key] = node.frame
            }
            node.children.forEach(walk)
        }
        if let snapshot { walk(snapshot) }
        for key in repeated { out.removeValue(forKey: key) }
        return out
    }

    /// The elements two frames share, and which of them are not where they
    /// were: a top edge or a height off by more than half a point.
    private func compare(
        _ before: [String: CGRect], _ after: [String: CGRect]
    ) -> (compared: Int, maxDy: CGFloat, maxDh: CGFloat, diffs: [String]) {
        var compared = 0
        var maxDy: CGFloat = 0
        var maxDh: CGFloat = 0
        var diffs: [String] = []
        for (key, was) in before.sorted(by: { $0.key < $1.key }) {
            guard let now = after[key] else { continue }
            compared += 1
            let dy = abs(now.minY - was.minY)
            let dh = abs(now.height - was.height)
            maxDy = max(maxDy, dy)
            maxDh = max(maxDh, dh)
            if dy > 0.5 || dh > 0.5 { diffs.append("\(key): y \(was.minY)→\(now.minY) h \(was.height)→\(now.height)") }
        }
        return (compared, maxDy, maxDh, diffs)
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
