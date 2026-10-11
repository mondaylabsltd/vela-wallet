//
//  ParallelSpaceSitesDeviceTests.swift
//  VelaWalletUITests
//
//  A walk through the parallel space, in the app, on a SIMULATOR — with the
//  app's own store read before, during and after.
//
//  On the owner's phone one walk through the space cost them a real
//  connection (Arc Orbs, 2026-10-11): the test wallet coming to the front was
//  an account switch like any other, every connected site was re-pinned to it
//  and written, and leaving removed them with its record. The rule now is the
//  core's (`account_switched.space`) and `ParallelSpaceSitesTests` pins the
//  shell's half below the web view. This is the same walk through everything
//  above it: the real app, launched four times, its real `UserDefaults`, its
//  own door (`VELA_PARALLEL_SPACE`), real pages in the real `WKWebView`, the
//  consent sheet tapped where a person taps it.
//
//  ## What it drives
//
//  1. OUTSIDE, as a wallet of the person's: the page at 127.0.0.1:8137 asks
//     to connect, the sheet is approved, the page puts itself on Gnosis.
//     → the store now holds that site's grant and network. **Dump 0.**
//  2. INSIDE the space, the same page: it is NOT connected (the sheet comes
//     up again), it is connected there to the test wallet. **Dump 1.**
//  3. INSIDE again, a page the person never met (127.0.0.1:8139): connected
//     to the test wallet, put on Gnosis. **Dump 2.**
//  4. OUTSIDE again, the first page: connected, to the person's wallet, with
//     no sheet — and still on Gnosis. **Dump 3.**
//
//  Every `vela.perm.*` / `vela.chain.*` value in dumps 1, 2 and 3 is the text
//  dump 0 holds, and there is no key for the page met inside.
//
//  ## What it does not drive
//
//  The person's wallet here is `VELA_ACCOUNT`'s key-less record — a site is
//  connected to an address, and needs no key. That seed rewrites the account
//  list at every launch, so on the way out the list is already the person's
//  one record when the space's own `leave` runs; `ParallelSpaceExitTests`
//  covers `leave` removing its record from a list it did not write. And
//  because the seed replaces the list, this runs on a simulator only: on a
//  phone it is skipped (`DeviceSafety`).
//
//  ## How the store is read
//
//  From the app's own preferences file. A simulator's containers are folders
//  on the Mac and this runner is not sandboxed away from them. The file is
//  what `cfprefsd` last flushed, so each dump waits until it holds something
//  that launch wrote AFTER the browser heard about the accounts (the space's
//  flag, the "connected to" row of the connect just made) and has stopped
//  changing.
//

import XCTest

final class ParallelSpaceSitesDeviceTests: XCTestCase {

    /// The person's wallet: any address that is not the fixture keyset's Safe.
    private static let person = "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984"
    private static let appId = "app.getvela.VelaWallet"
    private static let secondPort: UInt16 = 8139

    private var first: LocalDappServer?
    private var second: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = false
        try skipOnDeviceForSeededAccount()
        let page = try LocalDappServer.page()
        let first = try LocalDappServer(html: page)
        let second = try LocalDappServer(html: page, port: Self.secondPort)
        first.start()
        second.start()
        self.first = first
        self.second = second
    }

    override func tearDownWithError() throws {
        first?.stop()
        second?.stop()
        // Out of the space and signed out, whatever happened above: the flag
        // persists, and so would the seeded record.
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "0"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launch()
        _ = app.wait(for: .runningForeground, timeout: 20)
        app.terminate()
    }

    func testAWalkThroughTheSpaceKeepsThePersonsConnection() throws {
        let firstOrigin = "http://127.0.0.1:\(LocalDappServer.port)"
        let secondOrigin = "http://127.0.0.1:\(Self.secondPort)"
        let firstPage = try XCTUnwrap(first).pageUrl
        let secondPage = try XCTUnwrap(second).pageUrl

        // 1. Outside: the person connects the page and puts it on Gnosis.
        var app = launch(space: false, page: firstPage)
        XCTAssertTrue(pageIsUp(app), "the page never loaded outside the space")
        XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "launched outside, and the space is open")
        // On a simulator that has run this before, the grant is still there
        // and the page is answered with no sheet: the walk starts from "the
        // person's site is connected", however it came to be.
        let outside = try XCTUnwrap(connect(app), "the page was never connected outside the space")
        XCTAssertTrue(outside.line.localizedCaseInsensitiveContains(Self.person),
                      "connected to another account than the person's: \(outside.line)")
        tapOnPage(app, "Switch to Gnosis")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"))
        attach(app.screenshot(), named: "1-outside-connected")
        app.terminate()

        let before = try dump("0-before") { store in
            store.sites["vela.perm.\(firstOrigin)"] != nil && store.sites["vela.chain.\(firstOrigin)"] == "100"
        }
        XCTAssertEqual(before.sites.keys.sorted(), ["vela.chain.\(firstOrigin)", "vela.perm.\(firstOrigin)"])
        XCTAssertTrue(try XCTUnwrap(before.sites["vela.perm.\(firstOrigin)"])
            .localizedCaseInsensitiveContains(Self.person), "the stored grant is the person's")
        XCTAssertNil(before.spaceFlag)

        // 2. Inside: the same page is nobody's until it is connected THERE.
        app = launch(space: true, page: firstPage)
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 40), "the space never opened")
        XCTAssertTrue(pageIsUp(app), "the page never loaded inside the space")
        let inside = try XCTUnwrap(connect(app), "the page was never connected inside the space")
        XCTAssertTrue(inside.asked, "the person's connection was carried into the space: no sheet came up")
        XCTAssertFalse(inside.line.localizedCaseInsensitiveContains(Self.person),
                       "inside the space the page was given the person's account: \(inside.line)")
        let testWallet = try XCTUnwrap(Self.address(in: inside.line), "no account in \(inside.line)")
        tapOnPage(app, "Chain")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_chainId ok \"0x64\""),
                      "the page did not keep the network it was on")
        attach(app.screenshot(), named: "2-inside-connected-there")
        app.terminate()

        let during = try dump("1-inside-same-site") { store in
            store.spaceFlag == "1" && store.hasConnectRow(from: testWallet, origin: firstOrigin)
        }
        XCTAssertEqual(during.sites, before.sites, "connecting the person's site inside the space changed their store")

        // 3. Inside: a page the person never met.
        app = launch(space: true, page: secondPage)
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 40))
        XCTAssertTrue(pageIsUp(app), "the second page never loaded")
        let met = try XCTUnwrap(connect(app), "the second page was never connected")
        XCTAssertTrue(met.asked)
        XCTAssertTrue(met.line.localizedCaseInsensitiveContains(testWallet))
        tapOnPage(app, "Switch to Gnosis")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"))
        attach(app.screenshot(), named: "3-inside-new-site")
        app.terminate()

        let duringNew = try dump("2-inside-new-site") { store in
            store.spaceFlag == "1" && store.hasConnectRow(from: testWallet, origin: secondOrigin)
        }
        XCTAssertEqual(duringNew.sites, before.sites, "a site connected inside the space reached the person's store")

        // 4. Outside again: the person's page is still theirs, with no sheet.
        app = launch(space: false, page: firstPage)
        XCTAssertTrue(pageIsUp(app), "the page never loaded after leaving")
        XCTAssertFalse(app.staticTexts["PARALLEL SPACE"].exists, "left, and the space is still open")
        let back = try XCTUnwrap(connect(app), "after leaving the space the page could not be connected at all")
        XCTAssertFalse(back.asked, "the person's connection was lost: the page had to ask again")
        XCTAssertTrue(back.line.localizedCaseInsensitiveContains(Self.person),
                      "after leaving, the page is connected to \(back.line)")
        tapOnPage(app, "Chain")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_chainId ok \"0x64\""),
                      "the page lost the network it was on")
        attach(app.screenshot(), named: "4-outside-still-connected")
        app.terminate()

        let after = try dump("3-after") { store in store.spaceFlag == nil }
        XCTAssertEqual(after.sites, before.sites, "the person's sites are not what they were before the space")
        XCTAssertNil(after.sites["vela.perm.\(secondOrigin)"], "the site met in the space was stored")
        XCTAssertNil(after.sites["vela.chain.\(secondOrigin)"])
    }

    // MARK: - The app

    private func launch(space: Bool, page: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_ACCOUNT"] = Self.person
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = space ? "1" : "0"
        app.launchEnvironment["VELA_PARALLEL_SIGNER"] = "0"
        app.launchEnvironment["VELA_URL"] = page
        app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        app.launch()
        return app
    }

    /// A launch URL lands on its page; should the app open elsewhere, 探索
    /// and the home's resume row bring it forward.
    private func pageIsUp(_ app: XCUIApplication) -> Bool {
        if !app.buttons["explore.bar.back"].waitForExistence(timeout: 30) {
            let tab = app.buttons["探索"].firstMatch
            if tab.exists, tab.isHittable { tab.tap() }
            let row = app.buttons.matching(identifier: "explore.resume.row").firstMatch
            if row.waitForExistence(timeout: 10) { row.tap() }
        }
        return app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 40)
    }

    private func tapOnPage(_ app: XCUIApplication, _ button: String) {
        let element = app.webViews.buttons[button].firstMatch
        XCTAssertTrue(element.waitForExistence(timeout: 15), "the page has no \(button) button")
        element.tap()
    }

    /// Connect from the page. `asked`: the consent sheet came up and was
    /// approved; `line`: the page's own verdict, with the account it was given.
    private func connect(_ app: XCUIApplication) -> (asked: Bool, line: String)? {
        tapOnPage(app, "Connect")
        let approve = app.buttons.matching(
            NSPredicate(format: "label == %@ OR label == %@", "连接", "批准")
        ).firstMatch
        let asked = approve.waitForExistence(timeout: 12)
        if asked {
            attach(app.screenshot(), named: "consent")
            approve.tap()
        }
        guard let line = verdictLine(app, containing: "#verdict eth_requestAccounts ok") else { return nil }
        return (asked, line)
    }

    private func waitForVerdict(_ app: XCUIApplication, containing fragment: String) -> Bool {
        verdictLine(app, containing: fragment) != nil
    }

    private func verdictLine(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if let hit = lines.first(where: { $0.label.contains(fragment) }) {
                let note = XCTAttachment(string: hit.label)
                note.name = "verdict"
                note.lifetime = .keepAlways
                add(note)
                return hit.label
            }
            _ = XCTWaiter.wait(for: [expectation(description: "poll")], timeout: 1.0)
        }
        return nil
    }

    /// The first `0x…` address in a verdict line.
    private static func address(in line: String) -> String? {
        guard let range = line.range(of: "0x[0-9a-fA-F]{40}", options: .regularExpression) else { return nil }
        return String(line[range])
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    // MARK: - The app's store, read from its own preferences file

    private struct Store: Equatable {
        /// Every `vela.perm.*` / `vela.chain.*` key with its stored text.
        var sites: [String: String] = [:]
        var spaceFlag: String?
        var accounts = ""
        var history = ""

        /// The feed's "connected to" row for a connect made as `from`.
        func hasConnectRow(from: String, origin: String) -> Bool {
            guard let data = history.data(using: .utf8),
                  let rows = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
            else { return false }
            return rows.contains {
                $0["type"] as? String == "connect" && $0["dappOrigin"] as? String == origin
                    && ($0["from"] as? String)?.caseInsensitiveCompare(from) == .orderedSame
            }
        }

        var text: String {
            var lines = sites.keys.sorted().map { "\($0) = \(sites[$0] ?? "")" }
            lines.append("vela.parallelSpace = \(spaceFlag ?? "(absent)")")
            lines.append("vela.accounts = \(accounts)")
            return lines.joined(separator: "\n")
        }
    }

    /// The app's store once it holds what `fresh` asks for and two reads a
    /// second apart agree. Attached to the result as it was read.
    private func dump(_ name: String, fresh: (Store) -> Bool) throws -> Store {
        var last: Store?
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline {
            let now = try? readStore()
            if let now, now == last, fresh(now) {
                let note = XCTAttachment(string: now.text)
                note.name = "store-\(name)"
                note.lifetime = .keepAlways
                add(note)
                return now
            }
            last = now
            Thread.sleep(forTimeInterval: 1)
        }
        let seen = last?.text ?? "(the app's preferences file was never readable)"
        XCTFail("store dump \(name): the app's store never held what this step wrote. Last read:\n\(seen)")
        throw XCTSkip("no store to compare")
    }

    private func readStore() throws -> Store {
        let file = try preferencesFile()
        let plist = try PropertyListSerialization.propertyList(from: Data(contentsOf: file), format: nil)
        let values = plist as? [String: Any] ?? [:]
        var store = Store()
        for (key, value) in values where key.hasPrefix("vela.perm.") || key.hasPrefix("vela.chain.") {
            store.sites[key] = value as? String ?? "\(value)"
        }
        store.spaceFlag = values["vela.parallelSpace"] as? String
        store.accounts = values["vela.accounts"] as? String ?? ""
        store.history = values["vela.transactionHistory"] as? String ?? ""
        return store
    }

    /// `…/data/Containers/Data/Application/<the app's container>/Library/
    /// Preferences/<bundle id>.plist`, found from this runner's own container
    /// beside it.
    private func preferencesFile() throws -> URL {
        let containers = URL(fileURLWithPath: NSHomeDirectory()).deletingLastPathComponent()
        let candidates = (try FileManager.default.contentsOfDirectory(
            at: containers, includingPropertiesForKeys: [.contentModificationDateKey]
        )).map { $0.appendingPathComponent("Library/Preferences/\(Self.appId).plist") }
            .filter { FileManager.default.fileExists(atPath: $0.path) }
        let newest = candidates.max { a, b in
            let da = (try? a.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
            let db = (try? b.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
            return (da ?? .distantPast) < (db ?? .distantPast)
        }
        return try XCTUnwrap(newest, "no preferences file for \(Self.appId) beside \(containers.path)")
    }
}
