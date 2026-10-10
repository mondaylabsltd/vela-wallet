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
//    verdict kind.
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
            let kinds = ["out", "send", "swap", "three", "unverified", "nothing", "caution", "danger"]
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
