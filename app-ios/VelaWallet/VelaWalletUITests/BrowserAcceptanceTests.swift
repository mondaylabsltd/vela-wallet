//
//  BrowserAcceptanceTests.swift
//  VelaWalletUITests
//
//  Spec 053, on the device. The browser is the first surface in this program
//  that runs code the wallet did not write, so every claim here is read off a
//  real page rather than off a model.
//
//  ## How a page is read
//
//  XCUITest sees a `WKWebView` through the accessibility tree, which means it
//  sees what was **painted**. JavaScript state nobody rendered is invisible.
//  So the harness page prints one `#verdict <name> <value>` line per outcome,
//  and these tests wait for those lines.
//
//  ## Where the page comes from
//
//  `LocalDappServer`, inside this runner, on `127.0.0.1:8137`. Not `file://`:
//  `dapp_origin_of` gives a file URL no origin, so the permissions machine
//  would refuse everything and a green run would mean nothing.
//

import XCTest

final class BrowserAcceptanceTests: XCTestCase {

    private var server: LocalDappServer?

    override func setUpWithError() throws {
        continueAfterFailure = false
        let server = try LocalDappServer(html: try LocalDappServer.page())
        server.start()
        self.server = server
    }

    override func tearDownWithError() throws {
        server?.stop()
        server = nil
    }

    /// The app, inside the parallel space, with the harness page open.
    ///
    /// The space rather than the founder's own wallet because everything after
    /// phase 1 signs something, and a device suite that needs a finger on every
    /// run is a suite nobody runs.
    private func launchBrowsing() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["VELA_LANG"] = "zh"
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        app.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        app.launchEnvironment["VELA_URL"] = LocalDappServer.url
        app.launchArguments += ["-AppleLanguages", "(zh)"]
        app.launch()
        return app
    }

    /// A launch with `VELA_URL` opens straight into Explore (spec 070), where
    /// the full-screen browser hides the tab bar; tap the tab only when it is
    /// there to tap.
    private func openExplore(_ app: XCUIApplication) {
        let tab = app.buttons["探索"].firstMatch
        if tab.waitForExistence(timeout: 3), tab.isHittable { tab.tap() }
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// Connect, whether or not this origin is already granted.
    ///
    /// A grant persists across launches — that is the point of one — so a
    /// second run of any of these tests is answered from the mirror with **no
    /// sheet at all** (FR-007). A test that insisted on the sheet would fail
    /// on its own success.
    @discardableResult
    private func connect(_ app: XCUIApplication) -> Bool {
        app.webViews.buttons["Connect"].firstMatch.tap()
        let approve = app.buttons["批准"].firstMatch
        let asked = approve.waitForExistence(timeout: 12)
        if asked { approve.tap() }
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_requestAccounts ok"),
                      "the page was never connected")
        // The chain is the SITE's since spec 070 (Ethereum for a site never
        // seen, research R5), and the parallel space's Safe lives on Gnosis:
        // the page puts itself there, as a dApp would.
        app.webViews.buttons["Switch to Gnosis"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict wallet_switchEthereumChain ok"),
                      "the page could not switch itself to Gnosis")
        return asked
    }

    /// Wait for the page's **latest** verdict to contain `fragment`.
    ///
    /// Reads the one-line `#last` element rather than scanning the page.
    /// XCUITest re-snapshots the whole accessibility tree on every poll, and
    /// this page grows as it works — so scanning cost ~30 seconds of wall
    /// clock per check and the harness became slower than the chain it was
    /// waiting for.
    @discardableResult
    private func waitForVerdict(
        _ app: XCUIApplication, containing fragment: String, timeout: TimeInterval = 30
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let lines = app.webViews.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH %@", "#verdict")
            ).allElementsBoundByIndex
            if let hit = lines.first(where: { $0.label.contains(fragment) }) {
                // Attach the line itself. A hash the run proved and nobody
                // wrote down is a hash somebody has to go and find again.
                let note = XCTAttachment(string: hit.label)
                note.name = "verdict"
                note.lifetime = .keepAlways
                add(note)
                return true
            }
            _ = XCTWaiter.wait(for: [expectation(description: "poll")], timeout: 1.0)
        }
        return false
    }

    // MARK: - US1: a real page, with the wallet inside it

    /// 探索 loads a real page over the network, and the page finds this wallet
    /// before its own scripts run.
    ///
    /// Three claims in one pass, because they fail together: the engine is a
    /// real `WKWebView` (a page renders), the provider was injected into the
    /// page's own content world (`window.ethereum` exists at all), and the
    /// bundled bytes are the extension's (the announcement carries Vela's real
    /// name and reverse-domain id).
    ///
    /// The content world is the part worth stating. A `WKUserScript` defaults
    /// to an ISOLATED world, where a `window.ethereum` runs, errors nowhere,
    /// and is invisible to every dApp on earth.
    func testTheExploreTabRunsARealPageThatFindsTheWallet() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30),
                      "the space must be open — everything after this signs something")

        openExplore(app)

        // The page's own <h1>, painted by WebKit rather than by SwiftUI.
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30),
                      "no page rendered — the engine never loaded, or the URL never reached it")

        // The viewport units, as the page measures them (2026-09-23). Android's
        // in-app browser was handing Chromium no viewport height: every `vh`
        // resolved to 0 and modern sheets collapsed to a pixel. A `vh` of 0
        // here would mean WKWebView is sized the same way, and every page that
        // lays itself out against the viewport is broken in this browser too.
        let viewport = app.webViews.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "#viewport")
        ).firstMatch
        XCTAssertTrue(viewport.waitForExistence(timeout: 20), "the probe never painted")
        let measured = viewport.label
        XCTContext.runActivity(named: measured) { _ in }
        XCTAssertFalse(
            measured.contains("\"vh\":0"),
            "`100vh` resolves to zero in this WebView: \(measured)"
        )

        XCTAssertTrue(waitForVerdict(app, containing: "#verdict announce Vela Wallet app.getvela"),
                      "the page did not hear the discovery announcement: the provider is missing, or it was injected into an isolated content world where no dApp can see it")
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict legacy present isVela=true"),
                      "window.ethereum is not the wallet's, or not there at all")

        attach(app.screenshot(), named: "device-browser-announce")
    }

    /// The address bar shows the page's own host, and the lock tells the
    /// truth about the origin.
    ///
    /// The harness is served over http on LOOPBACK — a local dev server, which
    /// the core counts as secure (spec 070: https, or a loopback / private-
    /// network host; the same rule that lets it sign). What must never appear
    /// here is the INSECURE mark, which is for public http.
    func testTheAddressBarShowsThePagesOwnHost() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)

        XCTAssertTrue(app.staticTexts["127.0.0.1:8137"].waitForExistence(timeout: 30),
                      "the address bar shows a host the page is not on")
        XCTAssertFalse(app.images["explore.insecure"].exists,
                       "a loopback dev server is not public http, and is not flagged as it")

        attach(app.screenshot(), named: "device-browser-address-bar")
    }

    // MARK: - US2: the browser remembers

    /// A pinned site and a visited one survive the app being killed.
    ///
    /// Both halves matter and they come from **different machines**:
    /// `explore_sites` keeps the favourite and `browser_history` keeps the
    /// visit, in two documents, with two different rules about what an
    /// unreadable one means. A test that checked only one would pass while the
    /// other silently wrote nothing.
    ///
    /// The visit half is also the proof that the controller's wait works: that
    /// machine has no `ready` flag, and a visit recorded before its store
    /// answers is dropped without a word.
    func testFavouritesAndRecentsSurviveAForceQuit() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        // The star in the browser toolbar, by its corpus label.
        let star = app.buttons["添加到收藏"].firstMatch
        XCTAssertTrue(star.waitForExistence(timeout: 15), "the bookmark control is missing")
        star.tap()
        attach(app.screenshot(), named: "device-browser-favourited")

        app.terminate()

        // Relaunch WITHOUT a URL. Anything that comes back came off the disk.
        let again = XCUIApplication()
        again.launchEnvironment["VELA_LANG"] = "zh"
        again.launchEnvironment["VELA_THEME"] = "dark"
        again.launchEnvironment["VELA_SKIP_LAUNCH_ANIMATION"] = "1"
        again.launchEnvironment["VELA_PARALLEL_SPACE"] = "1"
        again.launchArguments += ["-AppleLanguages", "(zh)"]
        again.launch()
        XCTAssertTrue(again.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        again.buttons["探索"].firstMatch.tap()

        // **The tab came back, with its page.** That is what a browser does,
        // and it is the first half of the memory: the strip is in
        // `explore_sites`' document, so the engine is rebuilt and the URL
        // reloaded with nothing asking it to.
        XCTAssertTrue(again.staticTexts["127.0.0.1:8137"].waitForExistence(timeout: 30),
                      "the open tab did not survive the relaunch")
        attach(again.screenshot(), named: "device-browser-tab-restored")

        // Leave the page for the start page. The tab stays; only the view
        // changes.
        again.buttons["关闭网页"].firstMatch.tap()

        XCTAssertTrue(again.staticTexts["最近的 dApp"].waitForExistence(timeout: 20),
                      "the recents section is missing — the visit was never recorded, or it was recorded before the history store answered and was dropped")
        XCTAssertTrue(again.staticTexts["收藏"].waitForExistence(timeout: 10),
                      "the favourites section is missing — nothing was pinned")
        // The host appears in BOTH sections; one match is enough to prove the
        // two documents came back.
        XCTAssertTrue(again.staticTexts["127.0.0.1:8137"].firstMatch.waitForExistence(timeout: 10),
                      "neither the favourite nor the recent survived the relaunch")
        attach(again.screenshot(), named: "device-browser-remembered")
    }

    // MARK: - US3: a dApp connects

    /// A page taps its way to an account, and the wallet asks first.
    ///
    /// Four things in one pass, because they are one journey: the consent
    /// surface opens itself, it names the **origin** rather than a name the
    /// page supplied, approving returns the parallel space's own Safe, and a
    /// second read is answered from the grant with no sheet at all.
    func testASiteAsksForAnAccountAndIsAnsweredOnce() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        app.webViews.buttons["Connect"].firstMatch.tap()

        // The sheet opens ITSELF. A request that waited for somebody to find a
        // menu is a request the page thinks is hanging.
        //
        // Unless this origin is ALREADY granted — a grant persists, and then
        // the answer comes from the mirror with no sheet at all (FR-007).
        // Both are correct; the test says which happened.
        let approve = app.buttons["批准"].firstMatch
        if approve.waitForExistence(timeout: 15) {
            XCTAssertTrue(app.staticTexts["127.0.0.1:8137"].exists,
                          "the consent surface did not name the origin")
            attach(app.screenshot(), named: "device-browser-consent")
            approve.tap()
        } else {
            attach(app.screenshot(), named: "device-browser-already-granted")
        }

        // The golden multi-key Safe, which is a function of EVERY fixture key
        // — so seeing it is also proof the whole key set was used.
        XCTAssertTrue(
            waitForVerdict(app, containing: "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
                || waitForVerdict(app, containing: "0x88cca0eedbf2c4426110bbfc998f048689266894"),
            "the page was not given the wallet's address"
        )
        attach(app.screenshot(), named: "device-browser-connected")
    }

    /// A chain read from the page is answered by the wallet's own pool, and a
    /// chain switch moves the wallet and tells the page.
    func testThePageReadsAChainAndSwitchesIt() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        // `eth_chainId` needs no permission: it is the wallet's own answer
        // about the SITE's chain (spec 070: per origin, kept across launches —
        // Ethereum for a site never seen, else the site's own), with no
        // network at all. Which chain depends on this device's history; the
        // notation does not.
        app.webViews.buttons["Chain"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_chainId ok \"0x"),
                      "the page was told the chain in the wrong notation, or not at all")

        // A real read, through this wallet's endpoints for this chain.
        app.webViews.buttons["Block number"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_blockNumber ok"),
                      "the block read never came back — the pool did not answer for the browser's chain")
        attach(app.screenshot(), named: "device-browser-reads")
    }

    /// **`eth_sign` is refused**, and the refusal is not a lie about a human
    /// action.
    ///
    /// 4200 "unsupported method" (spec 070 research R4): nobody declined
    /// anything (4001), and the wallet is not disconnected (4900, what the
    /// native shells answered before the core owned the table). The
    /// extension's answer, now every client's.
    func testEthSignIsRefusedWithoutReachingAnybody() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        app.webViews.buttons["eth_sign"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_sign err 4200"),
                      "eth_sign was not refused as policy")
        attach(app.screenshot(), named: "device-browser-ethsign-refused")
    }

    // MARK: - US4: a dApp asks for a transaction (the gate)

    /// **Dust leaves the golden Safe because a page asked for it.**
    ///
    /// This is the cut's gate: everything after signing is easier to fix than
    /// signing, and everything before it is worthless if signing does not
    /// work.
    ///
    /// Spends real money on Gnosis, so it is behind `-DVELA_LIVE_SEND` like
    /// 052's transfer. The parallel space signs it, so no finger is needed —
    /// the founder's own passkey doing the same thing is SC-012 and is owed.
    ///
    /// What it proves, in order: the sheet describes the call in words before
    /// hex, one slide signs it, the row is written pending at submit, and the
    /// page is answered with a **transaction** hash rather than a
    /// user-operation hash.
    #if VELA_LIVE_SEND
    func testDustLeavesTheSafeBecauseAPageAskedForIt() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))

        connect(app)

        app.webViews.buttons["Send dust"].firstMatch.tap()

        // The sheet says what it DOES before it says what it is. A plain
        // transfer with no calldata must read as a send, never as the blind
        // "cannot decode" card — which is what Android shipped for one
        // screenshot.
        XCTAssertTrue(app.staticTexts["发送"].waitForExistence(timeout: 30),
                      "the sheet did not describe a no-calldata transfer as a send")
        attach(app.screenshot(), named: "device-browser-signing-sheet")

        // The slide carries an accessibility action, because VoiceOver and
        // Switch Control confirm by activating rather than dragging — so the
        // harness activates it too, and a drag whose geometry drifts cannot
        // make this test lie.
        // `matching`, not `containing`: `containing` finds elements whose
        // DESCENDANTS match, and the slide is a single accessibility element
        // with no children — so `containing` silently matched an ancestor,
        // reported it enabled, and tapped a container that does nothing.
        let slide = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "滑动以确认")
        ).firstMatch
        XCTAssertTrue(slide.waitForExistence(timeout: 20), "the confirm slide is missing")
        XCTAssertTrue(slide.isEnabled,
                      "the slide is shut — one of the three gating machines never said yes")

        // **A real drag, not a tap.**
        //
        // The slide carries an `accessibilityAction` so VoiceOver and Switch
        // Control can confirm by activating — but XCUITest's `tap()`
        // synthesises a touch at the element's centre rather than invoking
        // that action. The drag gesture then sees a press at ~50% of the
        // track, which is under the 88% commit threshold, and resets. The
        // sheet is left looking exactly as if nothing had been tapped, which
        // is what it was.
        let knob = slide.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.5))
        let end = slide.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5))
        knob.press(forDuration: 0.05, thenDragTo: end)

        // **The wallet's own surface first, and the page's second.**
        //
        // Reading the page costs a full accessibility snapshot of rendered
        // web content on every poll; reading the sheet costs almost nothing.
        // So the submit is observed where it is cheap — and the sheet saying
        // 已提交 is the stronger claim anyway: it means the RELAY accepted the
        // operation, not merely that some string reached the page.
        let submitted = app.staticTexts.matching(
            NSPredicate(format: "label BEGINSWITH %@", "已提交")
        ).firstMatch
        let accepted = submitted.waitForExistence(timeout: 120)
        attach(app.screenshot(), named: "device-browser-after-slide")
        XCTAssertTrue(accepted,
                      "the relay never accepted the operation — the sheet never said 已提交")

        // Then the page, once. The receipt wait can take a couple of minutes,
        // and a transaction hash is what a dApp must be handed.
        let answered = waitForVerdict(
            app, containing: "#verdict eth_sendTransaction ok \"0x", timeout: 150
        )
        attach(app.screenshot(), named: "device-browser-sent")
        XCTAssertTrue(answered, "the page never got a hash back")
    }
    #endif

    // MARK: - Spec 069: the dApp sheet chooses a speed

    /// A page asks to send dust, and the sheet offers the send form's own
    /// speed control under its fee: folded on the tier in force, opened onto
    /// three speeds, a pick folding it onto the new one.
    ///
    /// **Nothing here spends.** The slide is never touched: the sheet is
    /// swiped away, which is the refusal, and the page is answered 4001.
    func testTheDappSheetOffersASpeedAndNeverSignsOneItLeft() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)

        app.webViews.buttons["Send dust"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["发送"].waitForExistence(timeout: 30),
                      "the signing sheet never opened")
        let speed = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "速度")).firstMatch
        XCTAssertTrue(speed.waitForExistence(timeout: 30),
                      "the sheet has no speed control under its fee")
        _ = XCTWaiter.wait(for: [expectation(description: "quote")], timeout: 8)
        attach(app.screenshot(), named: "dapp-speed-folded")

        speed.tap()
        XCTAssertTrue(app.staticTexts["较慢"].waitForExistence(timeout: 10),
                      "opening the control offered no speeds")
        XCTAssertTrue(app.staticTexts["仅这一笔，下次仍用默认"].exists, "the one-shot promise is not said")
        // Each option prices itself: give the previews their round trips.
        _ = XCTWaiter.wait(for: [expectation(description: "previews")], timeout: 12)
        attach(app.screenshot(), named: "dapp-speed-open")

        app.staticTexts["较慢"].firstMatch.tap()
        _ = XCTWaiter.wait(for: [expectation(description: "repriced")], timeout: 8)
        XCTAssertFalse(app.staticTexts["仅这一笔，下次仍用默认"].exists, "a pick did not fold the control")
        XCTAssertTrue(speed.label.contains("较慢"), "the control does not name the speed picked: \(speed.label)")
        attach(app.screenshot(), named: "dapp-speed-picked-slow")

        // Away, unsigned: the refusal.
        app.swipeDown(velocity: .fast)
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict eth_sendTransaction err 4001", timeout: 30),
                      "dismissing the sheet did not refuse the page")
    }

    // MARK: - US5: an unlimited approval never leaves

    /// A site asks to spend everything, forever. The wallet does not let that
    /// happen, and does not make the person read hex to find out.
    ///
    /// The device half of FR-010: the editor is there, the "as requested" chip
    /// is **disabled** rather than merely unselected, and the slide is **shut**
    /// until a finite cap is named. That the signed calldata then carries the
    /// cap is proved hermetically (`DisplayedIsSignedTests`) — it is a fact
    /// about bytes, and a screenshot cannot show it.
    func testAnUnlimitedApprovalIsKeptAsAskedAndSaid() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)

        app.webViews.buttons["Approve unlimited"].firstMatch.tap()

        XCTAssertTrue(app.staticTexts["授权上限"].waitForExistence(timeout: 30),
                      "the spending-cap editor never appeared for an unlimited approval")
        attach(app.screenshot(), named: "device-browser-unlimited")

        XCTAssertTrue(app.staticTexts["无限额"].exists,
                      "the requested amount must read as the unlimited grant it is")

        // 2026-09-26: the site's ask is kept (Permit2 bundles revert when the
        // wallet re-encodes the approve) — on its own chip, and SAID.
        let requested = app.buttons["请求额度"].firstMatch
        XCTAssertTrue(requested.exists, "the 'as requested' chip must be present")
        XCTAssertTrue(requested.isEnabled, "the site's own ask is a choice the person can keep")
        let warning = app.staticTexts.matching(
            NSPredicate(format: "label BEGINSWITH %@", "无限额 —")
        ).firstMatch
        XCTAssertTrue(warning.exists, "an unlimited approval must never go out unsaid")

        // Kept as asked, the guard agrees; the fee machine is the third gate,
        // so the slide arms when the quote lands. It is NOT slid: this test
        // proves the sheet lets the ask through, not that the chain takes it.
        let slide = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "滑动以确认")
        ).firstMatch
        XCTAssertTrue(slide.waitForExistence(timeout: 20))
        let armed = XCTWaiter().wait(
            for: [expectation(
                for: NSPredicate(format: "isEnabled == true"), evaluatedWith: slide
            )],
            timeout: 90
        )
        XCTAssertEqual(armed, .completed,
                       "the kept ask did not arm the slide — the guard, or the fee, never agreed")

        // A cap is one chip away. 撤销 is a finite choice — zero — and needs
        // no typing.
        //
        // **Tap the HITTABLE one.** The chips live inside a `ViewThatFits`,
        // which measures every candidate layout, so each chip appears in the
        // accessibility tree more than once. `firstMatch` picks whichever came
        // first — often a measured copy that is not on screen — and tapping it
        // does nothing at all.
        let revoke = app.buttons.matching(identifier: "撤销").allElementsBoundByIndex
        guard let hittable = revoke.first(where: { $0.isHittable }) else {
            XCTFail("no on-screen 撤销 chip among \(revoke.count) matches")
            return
        }
        hittable.tap()
        attach(app.screenshot(), named: "device-browser-unlimited-capped")
        XCTAssertFalse(warning.waitForExistence(timeout: 3),
                       "a revoked approval is not unlimited — the warning must go")
        XCTAssertTrue(slide.isEnabled, "a finite choice keeps the slide armed")
    }

    // MARK: - US6: a signature the page can verify

    /// `personal_sign` is answered, and the page verifies the signature
    /// **on-chain** for the Safe's own address.
    ///
    /// The whole loop in one test: the wallet signs with the Safe's message
    /// hash, packs the EIP-1271 envelope, and the page then calls
    /// `isValidSignature` through the wallet's own read proxy. `0x1626ba7e` is
    /// the contract saying yes.
    func testAMessageSignatureVerifiesOnChainThroughTheWalletsOwnProxy() throws {
        let app = launchBrowsing()
        XCTAssertTrue(app.staticTexts["PARALLEL SPACE"].waitForExistence(timeout: 30))
        openExplore(app)
        XCTAssertTrue(app.webViews.staticTexts["Vela test dApp"].waitForExistence(timeout: 30))
        connect(app)

        app.webViews.buttons["Sign"].firstMatch.tap()

        // A message is readable, so the sheet shows the words rather than hex.
        XCTAssertTrue(app.staticTexts["Hello, Vela"].waitForExistence(timeout: 30),
                      "the sheet did not show the message it was asked to sign")
        attach(app.screenshot(), named: "device-browser-message")

        let slide = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "滑动以确认")
        ).firstMatch
        XCTAssertTrue(slide.waitForExistence(timeout: 20))
        // No network fee to wait for: an off-chain signature costs nothing.
        XCTAssertTrue(slide.isEnabled, "a message signature must not wait for a fee quote")
        // The parallel space signs with its built-in key: no passkey sheet
        // follows the slide, and the sheet says so instead of offering
        // choices it would not use (owner, 2026-09-22).
        XCTAssertTrue(app.staticTexts["平行空间内置钥匙"].exists,
                      "the sheet does not say the parallel space's key will sign")
        slide.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.5))
            .press(forDuration: 0.05,
                   thenDragTo: slide.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)))

        XCTAssertTrue(waitForVerdict(app, containing: "#verdict personal_sign ok", timeout: 90),
                      "the page never got a signature")

        app.webViews.buttons["Verify sign"].firstMatch.tap()
        XCTAssertTrue(waitForVerdict(app, containing: "#verdict verify valid 0x1626ba7e", timeout: 90),
                      "the Safe did not accept its own signature — the EIP-1271 envelope, the message hash, or the read proxy is wrong")
        attach(app.screenshot(), named: "device-browser-verified")
    }
}
