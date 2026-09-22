//
//  ZZSpec081DeviceTests.swift
//  VelaWalletUITests
//
//  THROWAWAY. Spec 081 follow-up device run on the iPhone 11 `ABC`.
//  DELETE after the run — this file is a camera, not a suite.
//
//  Nothing here signs, sends or erases. Every launch pins
//  VELA_PARALLEL_SPACE=1 so the phone is left in exactly the space the
//  founder left it in (`=0` would LEAVE the space and drop the fixture
//  account record — see ParallelSpaceBinding).
//

import XCTest
import Network

final class ZZSpec081DeviceTests: XCTestCase {

    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    // MARK: - helpers

    private func attach(_ shot: XCUIScreenshot, named name: String) {
        let a = XCTAttachment(screenshot: shot)
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    private func note(_ text: String, named name: String) {
        let a = XCTAttachment(string: text)
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    private func settle(_ s: TimeInterval = 2.0) { Thread.sleep(forTimeInterval: s) }

    /// ONE snapshot of the whole tree.
    ///
    /// `allElementsBoundByIndex` re-queries the app for EVERY element, so a
    /// screen that is still animating can lose an index between two of them —
    /// which is exactly how the refusal sheet's first run died ("No matches
    /// found for Element at index 19"). `debugDescription` is a single
    /// snapshot, and it carries frames, which is what shows clipping.
    private func tree(_ app: XCUIApplication) -> String { app.debugDescription }

    private func lines(_ app: XCUIApplication, ofType type: String) -> String {
        tree(app)
            .components(separatedBy: "\n")
            .filter { $0.contains("\(type),") }
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .joined(separator: "\n")
    }

    /// Everything readable on screen, so the report can quote rather than guess.
    private func labels(_ app: XCUIApplication) -> String {
        lines(app, ofType: "StaticText")
    }

    private func controls(_ app: XCUIApplication) -> String {
        let snapshot = tree(app)
        func rows(_ type: String) -> String {
            snapshot.components(separatedBy: "\n")
                .filter { $0.contains("\(type),") }
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .joined(separator: "\n")
        }
        return """
        --- Button ---
        \(rows("Button"))
        --- Slider ---
        \(rows("Slider"))
        --- TextField / SecureTextField ---
        \(rows("TextField"))
        --- Switch ---
        \(rows("Switch"))
        --- Other (named) ---
        \(rows("Other").split(separator: "\n").filter { $0.contains("label:") }.joined(separator: "\n"))
        """
    }

    private func fieldValues(_ app: XCUIApplication) -> String {
        lines(app, ofType: "TextField")
    }

    private func launchSettings(state: String, lang: String = "en") -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_PAGE"] = "settings-live"
        app.launchEnvironment["VELA_STATE"] = state
        app.launchEnvironment["VELA_LANG"] = lang
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(\(lang))"]
        app.launch()
        return app
    }

    // MARK: - FR-001, the Service Endpoints page

    func test01ServiceEndpoints() throws {
        var app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60),
                      "the endpoints page never opened")
        settle(2)
        attach(app.screenshot(), named: "ep-01-opened")
        note(fieldValues(app), named: "ep-01-values")
        note(labels(app), named: "ep-01-labels")

        // The probes are four network round trips.
        settle(18)
        attach(app.screenshot(), named: "ep-02-probed")
        note(labels(app), named: "ep-02-labels")
        note(controls(app), named: "ep-02-controls")

        // --- type into the passkey index (field 2 of 4) -----------------
        let passkey = app.textFields.element(boundBy: 1)
        XCTAssertTrue(passkey.waitForExistence(timeout: 10), "no second endpoint field")
        // Caret at the END: a tap in the middle of a URL deletes the wrong half.
        passkey.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
        settle(2)
        let before = (passkey.value as? String) ?? ""
        note(before, named: "ep-03-value-before-typing")
        passkey.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: before.count + 10))
        settle(1)
        attach(app.screenshot(), named: "ep-03-cleared")
        passkey.typeText("https://index.invalid")
        settle(1)
        attach(app.screenshot(), named: "ep-04-typed")
        note(fieldValues(app), named: "ep-04-values")

        // --- leave the field WITHOUT pressing Done: does a blur save? ----
        // Tap the page's own description, which owns no gesture.
        let away = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS[c] %@", "power your wallet")
        ).firstMatch
        if away.exists && away.isHittable {
            away.tap()
            note("blurred by tapping the page description", named: "ep-05-how")
        } else {
            let label = app.staticTexts["CHAIN DATA INDEX"]
            if label.exists { label.tap(); note("blurred by tapping CHAIN DATA INDEX", named: "ep-05-how") }
            else { app.tap(); note("blurred by tapping the app", named: "ep-05-how") }
        }
        settle(3)
        note("keyboard up after tapping away: \(app.keyboards.count > 0)", named: "ep-05-keyboard")
        attach(app.screenshot(), named: "ep-05-after-tap-away")

        // The blur is where the core probes. index.invalid resolves nowhere,
        // so give the probe its full timeout.
        settle(20)
        attach(app.screenshot(), named: "ep-06-after-blur-probe")
        note(fieldValues(app), named: "ep-06-values")
        note(labels(app), named: "ep-06-labels")

        // --- force quit and reopen: did the BLUR alone save it? ----------
        app.terminate()
        settle(3)
        app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60),
                      "the endpoints page never reopened")
        settle(20)
        attach(app.screenshot(), named: "ep-07-relaunched")
        note(fieldValues(app), named: "ep-07-values")
        note(labels(app), named: "ep-07-labels")

        // --- Reset (spec 081 renamed it "Reset all four to defaults") ----
        let resetLabels = ["Reset all four to defaults", "Reset to Defaults"]
        var reset: XCUIElement?
        for candidate in resetLabels {
            let button = app.buttons[candidate]
            if button.exists { reset = button; break }
            let text = app.staticTexts[candidate]
            if text.exists { reset = text; break }
        }
        if reset == nil {
            app.swipeUp()
            settle(2)
            for candidate in resetLabels {
                let button = app.buttons[candidate]
                if button.exists { reset = button; break }
            }
        }
        note("reset affordance found: \(reset?.label ?? "NONE")", named: "ep-08-reset-label")
        attach(app.screenshot(), named: "ep-08-before-reset")
        XCTAssertNotNil(reset, "no reset affordance on the endpoints page")
        reset?.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(20)
        attach(app.screenshot(), named: "ep-09-reset")
        note(fieldValues(app), named: "ep-09-values")
        note(labels(app), named: "ep-09-labels")

        // --- and the reset survives a relaunch too ----------------------
        app.terminate()
        settle(3)
        app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(18)
        attach(app.screenshot(), named: "ep-10-after-reset-relaunch")
        note(fieldValues(app), named: "ep-10-values")
        note(labels(app), named: "ep-10-labels")
        app.terminate()
    }

    // MARK: - FR-009, the network readiness verdict

    func test02AddNetworkZora() throws {
        let app = launchSettings(state: "st10")
        XCTAssertTrue(app.staticTexts["Add Network"].waitForExistence(timeout: 60),
                      "the add-network page never opened")
        settle(3)
        attach(app.screenshot(), named: "net-01-wizard")
        note(controls(app), named: "net-01-controls")

        let search = app.textFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 30), "no search field on the wizard")
        search.tap()
        settle(1)
        search.typeText("7777777")
        settle(10)
        attach(app.screenshot(), named: "net-02-searched")
        note(labels(app), named: "net-02-labels")

        let zora = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS[c] %@", "Zora")
        ).firstMatch
        if zora.waitForExistence(timeout: 30) {
            zora.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        } else {
            note("no row mentioning Zora appeared", named: "net-03-missing")
        }
        // The compatibility probe is a fistful of eth_getCode calls. Poll for a
        // verdict rather than sleeping blind.
        let verdict = app.staticTexts.matching(
            NSPredicate(format: "label == %@ OR label == %@", "Compatible", "Incompatible")
        ).firstMatch
        _ = verdict.waitForExistence(timeout: 90)
        settle(3)
        attach(app.screenshot(), named: "net-03-verdict")
        note(labels(app), named: "net-03-labels")
        note(controls(app), named: "net-03-controls")
        app.swipeUp()
        settle(2)
        attach(app.screenshot(), named: "net-04-scrolled")
        note(labels(app), named: "net-04-labels")
        app.swipeUp()
        settle(2)
        attach(app.screenshot(), named: "net-05-bottom")
        note(labels(app), named: "net-05-labels")
        // NOTHING is added: the primary CTA is never tapped.
        app.terminate()
    }

    // MARK: - FR-005/006, the refusal sheet

    func test03BlockedRequests() throws {
        // The page, plus a pure OBSERVER that prints `error.kind` — the page's
        // own `verdict()` prints only code and message. Nothing in the
        // worktree's testdapp.html is edited.
        let observer = """
        <pre id="kindout">#kind waiting</pre>
        <script>
        (function () {
          var el = document.getElementById('kindout');
          var lines = [];
          function say(method, e) {
            var own = [];
            try { own = Object.getOwnPropertyNames(e || {}); } catch (x) {}
            lines.push('#kind ' + method
              + ' code=' + (e && e.code)
              + ' kind=' + (e && e.kind !== undefined ? e.kind : 'ABSENT')
              + ' own=' + JSON.stringify(own)
              + ' msg=' + (e && e.message));
            el.textContent = lines.slice(-4).join('\\n');
          }
          function hook(p) {
            if (!p || typeof p.request !== 'function' || p.__velaKindHooked) return;
            var orig = p.request.bind(p);
            var wrapped = function (args) {
              return orig(args).catch(function (e) {
                say(args && args.method, e);
                throw e;
              });
            };
            try {
              p.request = wrapped;
              if (p.request !== wrapped) { throw new Error('assignment ignored'); }
            } catch (err) {
              try { Object.defineProperty(p, 'request', { value: wrapped, configurable: true, writable: true }); }
              catch (err2) { el.textContent = '#kind HOOK FAILED ' + err2; return; }
            }
            p.__velaKindHooked = true;
          }
          hook(window.ethereum);
          window.addEventListener('eip6963:announceProvider', function (ev) { hook(ev.detail && ev.detail.provider); });
        })();
        </script>
        """
        let page = try LocalDappServer.page()
            .replacingOccurrences(of: "</body>", with: observer + "\n</body>")
        let server = try LocalDappServer(html: page)
        server.start()
        self.server = server

        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(en)"]
        app.launch()
        settle(6)
        attach(app.screenshot(), named: "blk-00-launched")
        note(controls(app), named: "blk-00-controls")

        // Into the browser tab.
        for name in ["Explore", "探索"] {
            let tab = app.buttons[name].firstMatch
            if tab.exists { tab.tap(); break }
        }
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 90),
                      "the test page never loaded")
        settle(3)
        attach(app.screenshot(), named: "blk-01-page")

        // Connect, whether or not the origin is already granted.
        app.webViews.buttons["Connect"].firstMatch.tap()
        settle(3)
        attach(app.screenshot(), named: "blk-02-consent")
        for name in ["Approve", "批准", "Connect"] {
            let button = app.buttons[name].firstMatch
            if button.waitForExistence(timeout: 8) { button.tap(); break }
        }
        settle(6)
        attach(app.screenshot(), named: "blk-03-connected")
        note(webText(app), named: "blk-03-page-text")

        for (button, tag) in [("Add owner (blocked)", "addowner"),
                              ("Enable module (blocked)", "enablemodule"),
                              ("SafeTx typed data (blocked)", "safetx")] {
            let target = app.webViews.buttons[button].firstMatch
            if !target.exists { app.swipeUp(); settle(2) }
            XCTAssertTrue(target.waitForExistence(timeout: 30), "no \(button) button")
            target.tap()
            settle(10)
            attach(app.screenshot(), named: "blk-\(tag)-01-sheet")
            note(labels(app), named: "blk-\(tag)-01-labels")
            note(controls(app), named: "blk-\(tag)-01-controls")
            // The WHOLE tree, once: the only honest way to say "there is no
            // slide-to-confirm on this sheet" is to show everything that is.
            note(tree(app), named: "blk-\(tag)-01-tree")
            // Down it goes, unsigned.
            app.swipeDown(velocity: .fast)
            settle(4)
            attach(app.screenshot(), named: "blk-\(tag)-02-dismissed")
            note(webText(app), named: "blk-\(tag)-03-page-text")
            settle(2)
        }

        // THE CRITICAL ONE: a NORMAL request right after a dismissed refusal,
        // in the same app session. -32002 here is the P1 regression.
        let sign = app.webViews.buttons["Sign"].firstMatch
        if !sign.exists { app.swipeUp(); settle(2) }
        XCTAssertTrue(sign.waitForExistence(timeout: 30), "no Sign button")
        sign.tap()
        settle(10)
        attach(app.screenshot(), named: "blk-after-01-normal-request-sheet")
        note(labels(app), named: "blk-after-01-labels")
        note(controls(app), named: "blk-after-01-controls")
        app.swipeDown(velocity: .fast)
        settle(4)
        attach(app.screenshot(), named: "blk-after-02-dismissed")
        note(webText(app), named: "blk-after-03-page-text")

        // And one more blocked one AFTER that, to prove the guard still clears.
        let again = app.webViews.buttons["Add owner (blocked)"].firstMatch
        if again.exists {
            again.tap()
            settle(10)
            attach(app.screenshot(), named: "blk-again-01-sheet")
            note(labels(app), named: "blk-again-01-labels")
            app.swipeDown(velocity: .fast)
            settle(4)
            note(webText(app), named: "blk-again-02-page-text")
        }
        attach(app.screenshot(), named: "blk-zz-final-page")
        app.terminate()
    }

    /// Every `<pre>` the page paints, which is where its verdicts live.
    ///
    /// From the one tree snapshot, for the same reason `labels` is.
    private func webText(_ app: XCUIApplication) -> String {
        tree(app)
            .components(separatedBy: "\n")
            .filter { $0.contains("#verdict") || $0.contains("#kind") || $0.contains("announced") }
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .joined(separator: "\n")
    }

    // MARK: - FR-010, forward-verified names

    func test04RecipientNames() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(en)"]
        app.launch()
        settle(6)
        attach(app.screenshot(), named: "name-00-home")
        note(controls(app), named: "name-00-controls")

        var opened = false
        for name in ["Send", "发送"] {
            let button = app.buttons[name].firstMatch
            if button.waitForExistence(timeout: 30) { button.tap(); opened = true; break }
        }
        XCTAssertTrue(opened, "no send action on the home screen")
        settle(5)
        attach(app.screenshot(), named: "name-01-picker")
        note(labels(app), named: "name-01-labels")

        // Pick the first asset row so the form opens.
        for symbol in ["xDAI", "USDC", "ETH", "WXDAI"] {
            let row = app.staticTexts[symbol].firstMatch
            if row.waitForExistence(timeout: 8) {
                row.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
                break
            }
        }
        settle(5)
        attach(app.screenshot(), named: "name-02-form")
        note(controls(app), named: "name-02-controls")
        note(labels(app), named: "name-02-labels")

        // The recipient box, by placeholder where possible.
        var recipient = app.textFields.matching(
            NSPredicate(format: "placeholderValue CONTAINS[c] %@ OR placeholderValue CONTAINS[c] %@",
                        "0x", "address")
        ).firstMatch
        if !recipient.exists { recipient = app.textFields.element(boundBy: 0) }
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "no recipient field")
        recipient.tap()
        settle(1)
        recipient.typeText("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
        settle(25)
        attach(app.screenshot(), named: "name-03-vitalik")
        note(labels(app), named: "name-03-labels")

        // Now an address nobody has named.
        recipient.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
        settle(1)
        let current = (recipient.value as? String) ?? ""
        recipient.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count + 10))
        settle(1)
        recipient.typeText("0x000000000000000000000000000000000000dEaD")
        settle(25)
        attach(app.screenshot(), named: "name-04-nameless")
        note(labels(app), named: "name-04-labels")
        // STOP at the recipient field. Nothing is confirmed and nothing is signed.
        app.terminate()
    }

    // MARK: - the relay-treasury copy (SR4, the relayer sheet)

    func test05RelayerCopy() throws {
        let app = launchSettings(state: "sr4")
        settle(8)
        attach(app.screenshot(), named: "relay-01-sheet")
        note(labels(app), named: "relay-01-labels")
        note(controls(app), named: "relay-01-controls")
        app.swipeUp()
        settle(2)
        attach(app.screenshot(), named: "relay-02-scrolled")
        note(labels(app), named: "relay-02-labels")
        app.terminate()
    }

    // MARK: - FR-009 again, reaching the verdict

    /// `test02` found the Zora row and tapping its label did nothing — the
    /// keyboard was still up. This one dismisses the keyboard first and taps
    /// the ROW's full-width strip at the label's height, which is the shape
    /// `.contentShape(Rectangle())` actually gives the gesture.
    func test08AddNetworkZoraVerdict() throws {
        let app = launchSettings(state: "st10")
        XCTAssertTrue(app.staticTexts["Add Network"].waitForExistence(timeout: 60))
        settle(3)

        let search = app.textFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 30))
        search.tap()
        settle(1)
        search.typeText("7777777")
        settle(10)
        attach(app.screenshot(), named: "zora-01-searched")

        // Put the keyboard away: it covers the lower half and eats taps.
        for key in ["done", "Done", "Go", "Return", "return"] {
            let button = app.keyboards.buttons[key]
            if button.exists { button.tap(); break }
        }
        settle(3)
        note("keyboard up: \(app.keyboards.count > 0)", named: "zora-02-keyboard")
        attach(app.screenshot(), named: "zora-02-keyboard-down")

        let zora = app.staticTexts.matching(
            NSPredicate(format: "label == %@", "Zora")
        ).firstMatch
        XCTAssertTrue(zora.waitForExistence(timeout: 20), "no Zora row")
        // The row spans the full width; the label only its own text.
        let frame = zora.frame
        let screen = app.frame
        let point = app.coordinate(withNormalizedOffset: CGVector(
            dx: 0.5,
            dy: (frame.midY - screen.minY) / screen.height
        ))
        note("zora label frame \(frame), app frame \(screen)", named: "zora-03-geometry")
        point.tap()
        settle(4)
        attach(app.screenshot(), named: "zora-03-tapped")
        note(labels(app), named: "zora-03-labels")

        // Eleven eth_getCode calls. Poll for a verdict.
        let verdict = app.staticTexts.matching(
            NSPredicate(format: "label == %@ OR label == %@ OR label == %@",
                        "Compatible", "Incompatible", "Compatibility Check")
        ).firstMatch
        let arrived = verdict.waitForExistence(timeout: 120)
        note("verdict element arrived: \(arrived)", named: "zora-04-arrived")
        settle(6)
        attach(app.screenshot(), named: "zora-04-verdict")
        note(labels(app), named: "zora-04-labels")
        note(controls(app), named: "zora-04-controls")

        app.swipeUp()
        settle(2)
        attach(app.screenshot(), named: "zora-05-scrolled")
        note(labels(app), named: "zora-05-labels")
        app.swipeUp()
        settle(2)
        attach(app.screenshot(), named: "zora-06-bottom")
        note(labels(app), named: "zora-06-labels")
        note(controls(app), named: "zora-06-controls")
        // NOTHING is added: "Add Network" is never pressed.
        app.terminate()
    }

    // MARK: - FR-001 again, with a blur that really is one

    /// `test01` proved the page is live and that tapping the page's own text
    /// does NOT take focus off the box. This one moves focus for real — first
    /// by tapping a SIBLING field, then, if that did not save, by the
    /// keyboard's Done key — and only then judges the commit.
    ///
    /// It always ends on a Reset, so the phone is left on the defaults.
    func test07EndpointCommitPaths() throws {
        var app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(16)
        attach(app.screenshot(), named: "cm-01-opened")
        note(labels(app), named: "cm-01-labels")

        func typeInvalid(into index: Int) {
            let box = app.textFields.element(boundBy: index)
            box.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
            settle(2)
            let before = (box.value as? String) ?? ""
            box.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: before.count + 10))
            settle(1)
            box.typeText("https://index.invalid")
            settle(1)
        }

        // --- PATH A: blur by tapping a sibling field --------------------
        typeInvalid(into: 1)
        attach(app.screenshot(), named: "cm-02-typed")
        note(fieldValues(app), named: "cm-02-values")

        let sibling = app.textFields.element(boundBy: 0)
        sibling.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        settle(3)
        note("keyboard up after tapping the sibling field: \(app.keyboards.count > 0)",
             named: "cm-03-keyboard")
        attach(app.screenshot(), named: "cm-03-after-sibling-tap")
        note(fieldValues(app), named: "cm-03-values")
        // index.invalid resolves nowhere; give the probe its full timeout.
        settle(25)
        attach(app.screenshot(), named: "cm-04-after-probe")
        note(labels(app), named: "cm-04-labels")

        app.terminate()
        settle(3)
        app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(20)
        attach(app.screenshot(), named: "cm-05-relaunch-after-sibling-blur")
        note(fieldValues(app), named: "cm-05-values")
        note(labels(app), named: "cm-05-labels")
        let savedByBlur = fieldValues(app).contains("index.invalid")
        note("PATH A (blur by sibling tap) saved: \(savedByBlur)", named: "cm-05-verdict")

        // --- PATH B: the keyboard's Done key ----------------------------
        if !savedByBlur {
            typeInvalid(into: 1)
            attach(app.screenshot(), named: "cm-06-typed-again")
            note(
                "keyboard keys: "
                    + app.keyboards.buttons.allElementsBoundByIndex.map(\.label).description,
                named: "cm-06-keyboard-keys"
            )
            var submitted = false
            for key in ["Done", "done", "Go", "go", "Return", "return", "\u{2713}"] {
                let button = app.keyboards.buttons[key]
                if button.exists { button.tap(); submitted = true
                                   note("submitted with key: \(key)", named: "cm-06-how"); break }
            }
            if !submitted {
                app.textFields.element(boundBy: 1).typeText("\n")
                note("submitted with a newline", named: "cm-06-how")
            }
            settle(25)
            attach(app.screenshot(), named: "cm-07-after-submit")
            note(fieldValues(app), named: "cm-07-values")
            note(labels(app), named: "cm-07-labels")

            app.terminate()
            settle(3)
            app = launchSettings(state: "st12")
            XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
            settle(20)
            attach(app.screenshot(), named: "cm-08-relaunch-after-submit")
            note(fieldValues(app), named: "cm-08-values")
            note(labels(app), named: "cm-08-labels")
            note("PATH B (keyboard Done) saved: \(fieldValues(app).contains("index.invalid"))",
                 named: "cm-08-verdict")
        }

        // --- Reset, whatever is stored ----------------------------------
        let reset = app.buttons["Reset all four to defaults"]
        if reset.waitForExistence(timeout: 10) {
            reset.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        } else {
            note("no reset button to press", named: "cm-09-missing-reset")
        }
        settle(20)
        attach(app.screenshot(), named: "cm-09-reset")
        note(fieldValues(app), named: "cm-09-values")

        app.terminate()
        settle(3)
        app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(20)
        attach(app.screenshot(), named: "cm-10-final")
        note(fieldValues(app), named: "cm-10-values")
        note(labels(app), named: "cm-10-labels")
        // The phone must be left on the defaults.
        XCTAssertFalse(fieldValues(app).contains("index.invalid"),
                       "the phone was left with index.invalid stored")
        app.terminate()
    }

    // MARK: - FR-005/006 again, from an origin with NO stale grant

    /// `test03` connected to `http://127.0.0.1:8137`, an origin this phone
    /// already had a permission record for — bound to the OTHER account. The
    /// page was handed `0xa5B4…9514` while the wallet signs as
    /// `0x88cC…6894`, so `to == from` was never the wallet's own address and
    /// the self-call guard had nothing to catch.
    ///
    /// Same page, a port nothing has been granted, so consent is asked fresh.
    func test09BlockedRequestsFreshOrigin() throws {
        let observer = """
        <pre id="kindout">#kind waiting</pre>
        <script>
        (function () {
          var el = document.getElementById('kindout');
          var lines = [];
          function say(method, e) {
            var own = [];
            try { own = Object.getOwnPropertyNames(e || {}); } catch (x) {}
            lines.push('#kind ' + method
              + ' code=' + (e && e.code)
              + ' kind=' + (e && e.kind !== undefined ? e.kind : 'ABSENT')
              + ' own=' + JSON.stringify(own)
              + ' msg=' + (e && e.message));
            el.textContent = lines.slice(-4).join('\\n');
          }
          function hook(p) {
            if (!p || typeof p.request !== 'function' || p.__velaKindHooked) return;
            var orig = p.request.bind(p);
            var wrapped = function (args) {
              return orig(args).catch(function (e) { say(args && args.method, e); throw e; });
            };
            try {
              p.request = wrapped;
              if (p.request !== wrapped) { throw new Error('assignment ignored'); }
            } catch (err) {
              try { Object.defineProperty(p, 'request', { value: wrapped, configurable: true, writable: true }); }
              catch (err2) { el.textContent = '#kind HOOK FAILED ' + err2; return; }
            }
            p.__velaKindHooked = true;
          }
          hook(window.ethereum);
          window.addEventListener('eip6963:announceProvider', function (ev) { hook(ev.detail && ev.detail.provider); });
        })();
        </script>
        """
        let page = try LocalDappServer.page()
            .replacingOccurrences(of: "</body>", with: observer + "\n</body>")
        let server = try FreshOriginServer(html: page)
        server.start()
        defer { server.stop() }

        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_URL"] = FreshOriginServer.url
        app.launchArguments += ["-AppleLanguages", "(en)"]
        app.launch()
        settle(6)

        for name in ["Explore", "探索"] {
            let tab = app.buttons[name].firstMatch
            if tab.exists { tab.tap(); break }
        }
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 90),
                      "the test page never loaded")
        settle(3)
        attach(app.screenshot(), named: "fx-01-page")

        app.webViews.buttons["Connect"].firstMatch.tap()
        settle(4)
        attach(app.screenshot(), named: "fx-02-consent")
        note(labels(app), named: "fx-02-consent-labels")
        for name in ["Approve", "批准", "Connect"] {
            let button = app.buttons[name].firstMatch
            if button.waitForExistence(timeout: 10) { button.tap(); break }
        }
        settle(8)
        attach(app.screenshot(), named: "fx-03-connected")
        note(webText(app), named: "fx-03-page-text")

        for (button, tag) in [("Add owner (blocked)", "addowner"),
                              ("Enable module (blocked)", "enablemodule"),
                              ("SafeTx typed data (blocked)", "safetx")] {
            let target = app.webViews.buttons[button].firstMatch
            if !target.exists { app.swipeUp(); settle(2) }
            XCTAssertTrue(target.waitForExistence(timeout: 30), "no \(button) button")
            target.tap()
            settle(12)
            attach(app.screenshot(), named: "fx-\(tag)-01-sheet")
            note(labels(app), named: "fx-\(tag)-01-labels")
            note(controls(app), named: "fx-\(tag)-01-controls")
            note(tree(app), named: "fx-\(tag)-01-tree")
            app.swipeDown(velocity: .fast)
            settle(5)
            attach(app.screenshot(), named: "fx-\(tag)-02-dismissed")
            note(webText(app), named: "fx-\(tag)-03-page-text")
            settle(2)
        }

        // A NORMAL request right after a dismissed refusal. -32002 is the P1.
        let sign = app.webViews.buttons["Sign"].firstMatch
        if !sign.exists { app.swipeUp(); settle(2) }
        XCTAssertTrue(sign.waitForExistence(timeout: 30), "no Sign button")
        sign.tap()
        settle(12)
        attach(app.screenshot(), named: "fx-after-01-normal-sheet")
        note(labels(app), named: "fx-after-01-labels")
        note(controls(app), named: "fx-after-01-controls")
        app.swipeDown(velocity: .fast)
        settle(5)
        attach(app.screenshot(), named: "fx-after-02-dismissed")
        note(webText(app), named: "fx-after-03-page-text")

        // And a blocked one again, to prove the guard still clears.
        let again = app.webViews.buttons["Add owner (blocked)"].firstMatch
        if again.exists {
            again.tap()
            settle(12)
            attach(app.screenshot(), named: "fx-again-01-sheet")
            note(labels(app), named: "fx-again-01-labels")
            app.swipeDown(velocity: .fast)
            settle(5)
            note(webText(app), named: "fx-again-02-page-text")
        }
        attach(app.screenshot(), named: "fx-zz-final")
        app.terminate()
    }

    // MARK: - Reset, tapped where it can actually be hit; and the restore

    /// `test07` "pressed" Reset at a point below the fold and nothing moved.
    /// This scrolls until the button is hittable before tapping it — and, if
    /// Reset still does not restore the default, types the default back by
    /// hand, so the phone is not left pointing at `index.invalid`.
    func test10ResetAndRestore() throws {
        var app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(18)
        attach(app.screenshot(), named: "rs-01-opened")
        note(fieldValues(app), named: "rs-01-values")
        note(labels(app), named: "rs-01-labels")

        let reset = app.buttons["Reset all four to defaults"]
        XCTAssertTrue(reset.waitForExistence(timeout: 15), "no reset button in the tree")
        var scrolls = 0
        while !reset.isHittable && scrolls < 6 {
            app.swipeUp()
            settle(2)
            scrolls += 1
        }
        note("reset hittable after \(scrolls) swipes: \(reset.isHittable), frame \(reset.frame)",
             named: "rs-02-hittable")
        attach(app.screenshot(), named: "rs-02-scrolled-to-reset")
        reset.tap()
        settle(22)
        attach(app.screenshot(), named: "rs-03-after-reset")
        note(fieldValues(app), named: "rs-03-values")
        note(labels(app), named: "rs-03-labels")

        app.terminate()
        settle(3)
        app = launchSettings(state: "st12")
        XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
        settle(20)
        attach(app.screenshot(), named: "rs-04-relaunch")
        note(fieldValues(app), named: "rs-04-values")
        note(labels(app), named: "rs-04-labels")
        let resetWorked = !fieldValues(app).contains("index.invalid")
        note("RESET restored the default: \(resetWorked)", named: "rs-04-verdict")

        // --- restore by hand if it did not ------------------------------
        if !resetWorked {
            let passkey = app.textFields.element(boundBy: 1)
            passkey.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
            settle(2)
            let before = (passkey.value as? String) ?? ""
            passkey.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue,
                                    count: before.count + 10))
            settle(1)
            passkey.typeText("https://p256-index-v2.getvela.app")
            settle(1)
            // The blur that commits: focus moves to a sibling.
            app.textFields.element(boundBy: 0)
                .coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
            settle(20)
            attach(app.screenshot(), named: "rs-05-typed-back")
            note(fieldValues(app), named: "rs-05-values")

            app.terminate()
            settle(3)
            app = launchSettings(state: "st12")
            XCTAssertTrue(app.staticTexts["Service Endpoints"].waitForExistence(timeout: 60))
            settle(20)
            attach(app.screenshot(), named: "rs-06-restored")
            note(fieldValues(app), named: "rs-06-values")
            note(labels(app), named: "rs-06-labels")
        }
        XCTAssertFalse(fieldValues(app).contains("index.invalid"),
                       "the phone is still pointing at index.invalid")
        app.terminate()
    }

    // MARK: - FR-010 again, in the RECIPIENT box

    /// `test04` typed the address into the AMOUNT box: the send form's text
    /// fields are `["0", ""]` — index 0 is the amount, index 1 the recipient —
    /// and the placeholder predicate matched neither, so it fell back to 0.
    /// The address rendered as ≈¥381,821.32 and no name was ever asked for.
    func test11RecipientNamesFixed() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "en"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchArguments += ["-AppleLanguages", "(en)"]
        app.launch()
        settle(8)

        for name in ["Send", "\u{53d1}\u{9001}"] {
            let button = app.buttons[name].firstMatch
            if button.waitForExistence(timeout: 30) { button.tap(); break }
        }
        settle(5)
        for symbol in ["xDAI", "USDC", "ETH"] {
            let row = app.staticTexts[symbol].firstMatch
            if row.waitForExistence(timeout: 8) {
                row.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
                break
            }
        }
        XCTAssertTrue(app.staticTexts["Recipient"].waitForExistence(timeout: 30),
                      "the send form never opened")
        settle(3)
        attach(app.screenshot(), named: "nm-01-form")
        note(controls(app), named: "nm-01-controls")

        // Index 1. Index 0 is the amount, and it is the one that looks like a
        // text field first.
        let recipient = app.textFields.element(boundBy: 1)
        XCTAssertTrue(recipient.waitForExistence(timeout: 20), "no recipient field")
        note("recipient frame \(recipient.frame), amount frame \(app.textFields.element(boundBy: 0).frame)",
             named: "nm-01-frames")
        recipient.tap()
        settle(2)
        recipient.typeText("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
        settle(30)
        attach(app.screenshot(), named: "nm-02-vitalik")
        note(labels(app), named: "nm-02-labels")
        note(fieldValues(app), named: "nm-02-values")

        // Now an address nobody has named.
        recipient.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5)).tap()
        settle(2)
        let current = (recipient.value as? String) ?? ""
        recipient.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue,
                                  count: current.count + 12))
        settle(2)
        attach(app.screenshot(), named: "nm-03-cleared")
        recipient.typeText("0x000000000000000000000000000000000000dEaD")
        settle(30)
        attach(app.screenshot(), named: "nm-04-nameless")
        note(labels(app), named: "nm-04-labels")
        note(fieldValues(app), named: "nm-04-values")
        // STOP at the recipient field. Continue is never pressed.
        app.terminate()
    }

    // MARK: - what is on this phone (read only)

    func test06WhatIsOnThisPhone() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launch()
        settle(10)
        attach(app.screenshot(), named: "phone-01-home")
        note(labels(app), named: "phone-01-labels")
        settle(15)
        attach(app.screenshot(), named: "phone-02-home-settled")
        note(labels(app), named: "phone-02-labels")
        app.terminate()
    }
}

/// The harness's own server on a port this phone has never granted.
///
/// A copy of `LocalDappServer` in all but the number: 8137 already carries a
/// permission record bound to the other account, and reusing it is what made
/// the refusal test prove nothing. THROWAWAY, with the test that needs it.
final class FreshOriginServer {

    static let port: UInt16 = 8147
    static var url: String { "http://127.0.0.1:\(port)/" }

    private let listener: NWListener
    private let body: Data
    private let queue = DispatchQueue(label: "vela.testdapp.fresh")

    init(html: String) throws {
        body = Data(html.utf8)
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: Self.port)!)
    }

    func start() {
        listener.newConnectionHandler = { [body] connection in
            connection.start(queue: .global())
            connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { _, _, _, _ in
                let header = "HTTP/1.1 200 OK\r\n"
                    + "Content-Type: text/html; charset=utf-8\r\n"
                    + "Content-Length: \(body.count)\r\n"
                    + "Cache-Control: no-store\r\n"
                    + "Connection: close\r\n\r\n"
                var response = Data(header.utf8)
                response.append(body)
                connection.send(content: response, completion: .contentProcessed { _ in
                    connection.cancel()
                })
            }
        }
        listener.start(queue: queue)
    }

    func stop() { listener.cancel() }
}
