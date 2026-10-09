//
//  Spec099Tests.swift
//  VelaWalletTests
//
//  Spec 099 on iOS: every rule is the core's (「跨端同样要使用 crux core 来保持
//  规则逻辑一致性」), so these tests drive the REAL core through the shell's
//  wiring and check the shell asks it the right question and does what it
//  answers:
//
//  - which tabs keep a live engine (`browserEnginePlan`), what a suspended
//    tab tells the browser machine, and the "reloaded" line when it wakes;
//  - every read answers by its deadline, and the failure kind the core is
//    told (timed out, rate limited, no endpoint) is the one it names;
//  - the shell's clock reaches the record (`consent_rejected` with `now_ms`);
//  - the tab's status line and record, in the core's words;
//  - the signing confirm's gate (`signConfirmState`) and its line;
//  - the landing's countdown (`landingPace`), from the relay's send;
//  - a passkey failure carries its kind, and the sheet names the signer.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

private let dapp = "https://dapp.example"
private let a1 = "0x1111111111111111111111111111111111111111"
private let a2 = "0x2222222222222222222222222222222222222222"

// MARK: - Which tabs keep a live engine (FR-004)

@MainActor
struct EnginePlanTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func explore(_ ids: [String], selected: String?, recent: [String]) -> ExploreViewWire {
        var view = ExploreViewWire(
            favorites: [],
            tabs: ids.map { ExploreTabWire(id: $0, url: "https://\($0).example", title: $0, host: "\($0).example") },
            selectedTab: selected, favoritesHidden: false, recentHidden: false,
            favoritesFull: false, tabsFull: false, ready: true
        )
        view.recentTabs = recent
        return view
    }

    private func dbr(busy: [String]) -> DbrViewWire {
        DbrViewWire(
            ready: true, consent: nil,
            tabs: busy.map {
                var tab = DbrTabViewWire(tab: $0, origin: dapp, connectedAddress: nil, chainId: 1,
                                         secure: true, crashed: false)
                tab.busy = true
                tab.openRequests = 1
                return tab
            },
            sites: [], signing: nil, queuedSigning: 0
        )
    }

    /// The shell hands the core the strip, the tab in front, the recency, the
    /// busy tabs and its engines — and lets go of exactly what it names: the
    /// least recently used idle tabs past six, never the one in front, never
    /// a busy one.
    @Test func theCoreNamesTheTabsToLetGoOf() throws {
        let ids = (1...8).map { "t\($0)" }
        let view = explore(ids, selected: "t1", recent: ids)
        let input = BrowserController.engineInput(
            explore: view, dbr: dbr(busy: ["t8"]), live: ids, pressure: false
        )
        #expect(input["recent"] as? [String] == ids)
        #expect(input["busy"] as? [String] == ["t8"])
        #expect(input["selected"] as? String == "t1")
        #expect(BrowserController.enginePlan(input) == ["t6", "t7"],
                "six kept: the selected, the busy one, then the most recent")

        // The system asked for memory: only the tab in front and the busy one.
        let pressed = BrowserController.engineInput(
            explore: view, dbr: dbr(busy: ["t8"]), live: ids, pressure: true
        )
        #expect(BrowserController.enginePlan(pressed) == ["t2", "t3", "t4", "t5", "t6", "t7"])

        // A tab with no engine has nothing to let go of.
        let few = BrowserController.engineInput(
            explore: view, dbr: dbr(busy: []), live: ["t1", "t2"], pressure: false
        )
        #expect(BrowserController.enginePlan(few).isEmpty)
    }

    /// What the browser machine hears when a page goes and its tab stays —
    /// never `tab_closed`, whose id is never a page again.
    @Test func aSuspendedPageIsAPageThatWentNotATabThatClosed() {
        let events = BrowserController.pageGoneEvents(tab: "t3", nowMs: 42)
        #expect(events.map { $0["type"] as? String } == ["navigation_started", "load_finished"])
        #expect(events.allSatisfy { $0["url"] as? String == "about:blank" && $0["tab"] as? String == "t3" })
        #expect(events.allSatisfy { $0["now_ms"] as? Double == 42 })
    }

    /// End to end on real engines: the memory warning lets the background
    /// tab's page go — unless it has something open — and selecting it again
    /// loads it and says so until the person leaves it.
    ///
    /// Each tab is a real WKWebView (see `DebugModeTests` on why the limit).
    @Test(.timeLimit(.minutes(5)))
    func aSuspendedTabWakesReloadedAndABusyOneIsKept() async throws {
        let h = BrowserHarness()
        await h.boot()
        h.browser.start()
        // Port 9 refuses at once: nothing here is about the page loading.
        h.browser.open("http://127.0.0.1:9/one")
        await Wait.until { h.browser.explore.selected.map { h.browser.engineForTesting($0.id) != nil } ?? false }
        let first = try #require(h.browser.explore.selectedTab)
        h.browser.newTab()
        await Wait.until { h.browser.explore.tabs.count == 2 }
        h.browser.open("http://127.0.0.1:9/two")
        await Wait.until {
            h.browser.explore.selected.map { $0.id != first && h.browser.engineForTesting($0.id) != nil } ?? false
        }
        let second = try #require(h.browser.explore.selectedTab)

        // The background tab has a read open: it is busy, and kept.
        h.holdReads = true
        await h.hello(first, doc: "d1", origin: dapp)
        h.browser.pageMessageForTesting(tab: first, frameOrigin: dapp, body: CoreJSON.string([
            "t": "req", "doc": "d1", "id": "1", "method": "eth_blockNumber", "params": [],
        ]))
        // The pool has the read (and the harness holds it): the tab is busy.
        await Wait.until { h.reads.count == 1 }
        #expect(h.browser.dbr.tab(first)?.busy == true)
        h.browser.memoryWarning()
        #expect(h.browser.engineForTesting(first) != nil, "a tab with a request open is never let go")
        #expect(h.browser.suspendedForTesting.isEmpty)

        // Answered: now it may go, and the warning lets it.
        h.releaseReads()
        await Wait.until { h.answer(first, id: "1") != nil }
        await h.settle()
        #expect(h.browser.dbr.tab(first)?.busy == false)
        h.browser.memoryWarning()
        #expect(h.browser.engineForTesting(first) == nil, "the idle background page was let go")
        #expect(h.browser.suspendedForTesting == [first])
        #expect(h.browser.engineForTesting(second) != nil, "the tab in front is never let go")
        #expect(h.browser.explore.tabs.map(\.id).contains(first), "the TAB stays, with its URL")
        #expect(h.browser.reloadedTab == nil)

        // Shown again: a new engine, loading again, and the line says why.
        h.browser.selectTab(first)
        await Wait.until { h.browser.engineForTesting(first) != nil }
        #expect(h.browser.reloadedTab == first)
        #expect(h.browser.suspendedForTesting.isEmpty)
        let line = BrowserStatusLive.line(
            tabId: first, tab: h.browser.dbr.tab(first), reloadedTab: h.browser.reloadedTab, loc: loc
        )
        #expect(line?.text == loc.t("componentsUi.browserStatus.reloaded"))
        #expect(line?.warning == false)

        // Leaving that tab ends the line.
        h.browser.selectTab(second)
        await Wait.until { h.browser.explore.selectedTab == second }
        #expect(h.browser.reloadedTab == nil)
    }
}

// MARK: - Every read answers by its deadline (FR-008, FR-009)

@MainActor
struct ReadDeadlineTests {

    /// The executor alone: a read that never answers is answered at the
    /// deadline as `timed_out`, and its late body is dropped.
    @Test func aReadThatNeverAnswersIsAnsweredAtTheDeadline() async throws {
        let suite = "vela.tests.dbr099.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        var late: CheckedContinuation<Void, Never>?
        var finished = false
        let executor = DbrExecutor(store: store, now: { 7 })
        executor.ports.poolCall = { _, _, _, _ in
            await withCheckedContinuation { late = $0 }
            finished = true
            return .answered(["result": "0x1"])
        }
        let answer = try CoreJSON.object(await executor.perform([
            "type": "read", "tab": "t1", "id": "1", "chain_id": 1, "method": "eth_blockNumber",
            "params_json": "[]", "bundler": false, "deadline_ms": 30.0,
        ]))
        #expect(answer["type"] as? String == "read_answered")
        #expect(answer["body_json"] is NSNull)
        #expect(answer["failure"] as? String == "timed_out")
        #expect(answer["now_ms"] as? Double == 7)

        // The pool answers after all: nobody is listening any more.
        late?.resume()
        await Wait.until { finished }
    }

    @Test func theFailureKindIsThePoolsWord() throws {
        func failure(_ read: DbrExecutor.Read?) throws -> Any? {
            try CoreJSON.object(DbrExecutor.readAnswered(read, nowMs: 1))["failure"]
        }
        #expect(try failure(.unanswered(rateLimited: true)) as? String == "rate_limited")
        #expect(try failure(.unanswered(rateLimited: false)) as? String == "no_endpoint")
        #expect(try failure(nil) as? String == "timed_out")
        #expect(try failure(.answered(["result": "0x1"])) is NSNull)
        let body = try CoreJSON.object(DbrExecutor.readAnswered(.answered(["result": "0x1"]), nowMs: 1))
        #expect((body["body_json"] as? String)?.contains("0x1") == true)
    }

    /// Through the real core: the page is answered, and the tab's record
    /// names the network layer and the reason — in the core's own key, which
    /// the shell's mirror of the vocabulary agrees with.
    @Test(.timeLimit(.minutes(2)))
    func theCoreNamesEachReadFailure() async throws {
        for reason in ["timed_out", "rate_limited", "no_endpoint"] {
            let h = BrowserHarness()
            await h.boot()
            switch reason {
            case "timed_out":
                h.holdReads = true
                h.browser.readDeadlineForTesting(30)
            case "rate_limited":
                h.readAnswer = nil
                h.readRateLimited = true
            default:
                h.readAnswer = nil
            }
            await h.hello("t1", doc: "d1", origin: dapp)
            await h.ask("t1", doc: "d1", origin: dapp, id: "1", method: "eth_blockNumber")
            // Not "until quiet": a held read is quiet, and only its deadline
            // answers it.
            await Wait.until { h.answer("t1", id: "1") != nil }
            await h.settle()
            #expect(h.answer("t1", id: "1")?.errorCode == -32603, "\(reason): the page is answered")
            let note = try #require(h.browser.dbr.tab("t1")?.lastFailure, "\(reason)")
            #expect(note.layer == "network")
            #expect(note.reason == reason)
            #expect(note.method == "eth_blockNumber")
            #expect(note.key == DbrRecordWords.reason(reason), "the mirror of `DbrReason::key` drifted")

            if reason == "timed_out" {
                // The read the pool was still holding answers late: dropped.
                let answers = h.answers("t1").count
                h.releaseReads()
                await h.settle()
                #expect(h.answers("t1").count == answers, "a late body never reaches the page")
            }
        }
    }
}

// MARK: - The tab's record and status (FR-007, FR-014)

@MainActor
struct BrowserStatusTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// `consent_rejected` carries the shell's clock: the row it ends is timed
    /// with it, not with the last clock any earlier event carried.
    @Test func aRejectedConsentIsTimedWithTheShellsClock() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "c", method: "eth_requestAccounts")
        await h.until { h.browser.dbr.consent != nil }
        let started = h.clock.ms
        h.clock.ms += 5_000
        h.browser.consentRejected()
        await h.until { h.answer("t1", id: "c") != nil }
        #expect(h.answer("t1", id: "c")?.errorCode == 4001)

        h.browser.inspectorOpened(tab: "t1")
        await h.until { h.browser.dbr.inspector != nil }
        let row = try #require(h.browser.dbr.inspector?.rows.first { $0.method == "eth_requestAccounts" })
        #expect(row.startedMs == started)
        #expect(row.endedMs == started + 5_000, "consent_rejected's now_ms never reached the core")
        #expect(row.durationMs == 5_000)
        #expect(row.reason == "rejected_by_person")
        #expect(row.layer == "sheet")
        // Declining is the person's choice, not trouble: no status line for it.
        #expect(h.browser.dbr.tab("t1")?.lastFailure == nil)
    }

    /// The panel's record is carried only while it is open, and Copy copies
    /// the core's own report.
    @Test func theRecordIsCarriedWhileThePanelIsOpen() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "1", method: "eth_chainId")
        await h.ask("t1", doc: "d1", origin: dapp, id: "2", method: "eth_frobnicate")
        await h.until { h.answer("t1", id: "2") != nil }
        #expect(h.browser.dbr.inspector == nil, "no record in the view while nobody looks")

        h.browser.inspectorOpened(tab: "t1")
        await h.until { h.browser.dbr.inspector != nil }
        let record = try #require(h.browser.dbr.inspector)
        #expect(record.tab == "t1")
        #expect(record.origin == dapp)
        #expect(record.rows.map(\.method) == ["eth_chainId", "eth_frobnicate"])
        #expect(BrowserStatusLive.outcome(record.rows[0], loc: loc).hasPrefix("✓"))
        #expect(BrowserStatusLive.outcome(record.rows[1], loc: loc)
                == loc.t("componentsUi.browserStatus.reason.unsupportedMethod"))
        #expect(record.report.contains("eth_frobnicate"))
        #expect(!record.report.contains(a1), "the report carries no address")

        // The status line: the latest trouble, in the core's words, with its method.
        let tab = try #require(h.browser.dbr.tab("t1"))
        let line = try #require(BrowserStatusLive.line(tabId: "t1", tab: tab, reloadedTab: nil, loc: loc))
        #expect(line.text == "\(loc.t("componentsUi.browserStatus.reason.unsupportedMethod")) · eth_frobnicate")
        #expect(line.warning)

        h.browser.inspectorClosed()
        await h.until { h.browser.dbr.inspector == nil }
        #expect(h.browser.dbr.inspector == nil)
    }

    /// The line's order: reloaded, then the wallet not offered to a loaded
    /// page, then the latest trouble — and nothing when there is nothing.
    @Test func theStatusLineSaysTheFirstThingThatApplies() {
        var tab = DbrTabViewWire(tab: "t1", origin: "http://plain.example", connectedAddress: nil,
                                 chainId: 1, secure: false, crashed: false)
        #expect(BrowserStatusLive.line(tabId: "t1", tab: tab, reloadedTab: nil, loc: loc) == nil)

        tab.page = "ready"
        tab.provider = "insecure_origin"
        tab.lastFailure = DbrFailureNoteWire(
            layer: "network", reason: "no_endpoint", method: "eth_call",
            key: "componentsUi.browserStatus.reason.noEndpoint"
        )
        tab.failedRecent = 1
        #expect(BrowserStatusLive.line(tabId: "t1", tab: tab, reloadedTab: nil, loc: loc)?.text
                == loc.t("componentsUi.browserStatus.provider.insecureOrigin"))
        #expect(BrowserStatusLive.line(tabId: "t1", tab: tab, reloadedTab: "t1", loc: loc)?.text
                == loc.t("componentsUi.browserStatus.reloaded"))

        tab.provider = "offered"
        let trouble = BrowserStatusLive.line(tabId: "t1", tab: tab, reloadedTab: "t2", loc: loc)
        #expect(trouble?.text == "\(loc.t("componentsUi.browserStatus.reason.noEndpoint")) · eth_call")
        #expect(trouble?.seen == "1:eth_call:no_endpoint")
    }

    /// Every line the shell's mirror of the vocabulary names is in the corpus.
    @Test func everyRecordWordIsInTheCorpus() {
        let keys = Array(DbrRecordWords.reasons.values) + Array(DbrRecordWords.pages.values)
            + Array(DbrRecordWords.providers.values)
            + ["componentsUi.browserStatus.title", "componentsUi.browserStatus.requests",
               "componentsUi.browserStatus.noRequests", "componentsUi.browserStatus.copy",
               "componentsUi.browserStatus.copied", "componentsUi.browserStatus.reloaded"]
        for key in keys {
            let text = loc.t(key)
            #expect(!text.isEmpty && text != key, "\(key) is not in the corpus")
        }
    }

    /// A state this build has never heard of reads, never fails the view.
    @Test func anUnknownStateNeverFailsTheView() throws {
        let tab = try CoreJSON.decode(DbrTabViewWire.self, from: [
            "tab": "t1", "origin": dapp, "connected_address": NSNull(), "chain_id": 1,
            "secure": true, "crashed": false, "busy": false, "page": "teleporting",
            "provider": "elsewhere", "open_requests": 0, "failed_recent": 0,
            "last_failure": ["layer": "quantum", "reason": "gremlins", "method": "eth_call",
                             "key": "componentsUi.browserStatus.reason.relayFailed"],
        ])
        #expect(tab.page == "teleporting")
        #expect(DbrRecordWords.page(tab.page) == nil)
        #expect(DbrRecordWords.reason("gremlins") == nil)
    }
}

// MARK: - The signing confirm's gate (FR-010)

@MainActor
struct ConfirmGateTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// A machine's own idle view, patched — the real wire, so the core reads it.
    private func view(_ bridge: CoreBridge, _ patch: [String: Any]) throws -> String {
        var object = try CoreJSON.object(bridge.view())
        for (key, value) in patch { object[key] = value }
        return CoreJSON.string(object)
    }

    private func openSign(_ patch: [String: Any] = [:]) throws -> String {
        try view(SignRequestCore(), (["confirm_gate_open": true, "confirm_block": NSNull()] as [String: Any])
            .merging(patch) { $1 })
    }

    private func guardView(_ patch: [String: Any] = [:]) throws -> String {
        try view(ApprovalGuardCore(), (["confirm_allowed": true, "surface": "none"] as [String: Any])
            .merging(patch) { $1 })
    }

    private func clear(_ surface: String, resolving: Bool = false) throws -> String {
        try view(ClearSigningCore(), ["surface": surface, "resolving": resolving])
    }

    private func estimate(_ tier: String) -> [String: Any] {
        [
            "chain_id": 100, "total_wei": "2100000000000000", "max_fee_per_gas": "2000000000",
            "network_fee_per_gas": "1000000000", "relayer_fee_per_gas": "1000000000",
            "bundler_gas_price": "1000000000", "in_band_gas_basis": "0",
            "effective_gas_price": NSNull(), "max_gas_price": NSNull(), "total_gas": "21000",
            "deployed": true, "tier": tier, "quoted": true, "fee_asset": ["type": "native"],
            "fee_recipient": "0xfee",
        ]
    }

    private func fee(ready: Bool, busy: Bool = false, tier: String? = nil) throws -> String {
        try view(FeePolicyCore(), [
            "busy": busy, "confirm_fee_ready": ready,
            "fee": tier.map { estimate($0) as Any } ?? NSNull(),
        ])
    }

    private func gate(
        sign: String, guard guardJson: String, clear: String, fee: String?, speed: String? = nil
    ) -> SignConfirmStateWire {
        SignConfirmStateWire.of(sign: sign, guard: guardJson, clear: clear, fee: fee, speedTier: speed)
    }

    @Test func everyPartSaysYesAndTheConfirmArms() throws {
        let state = gate(sign: try openSign(), guard: try guardView(), clear: try clear("clear_sign"),
                         fee: try fee(ready: true, tier: "fast"), speed: "fast")
        #expect(state.enabled)
        #expect(state.block == nil && state.key == nil)
    }

    /// A message has no network fee: with no fee session at all, it arms —
    /// iOS's second copy of the gate (SigningController's) required a ready
    /// fee and would have kept a `personal_sign` shut forever.
    @Test func aMessageWithNoFeeArms() throws {
        let message = gate(sign: try openSign(), guard: try guardView(), clear: try clear("message_sign"), fee: nil)
        #expect(message.enabled)
        // The same request read as a transaction waits for its fee, and says so.
        let transaction = gate(sign: try openSign(), guard: try guardView(), clear: try clear("clear_sign"), fee: nil)
        #expect(!transaction.enabled)
        #expect(transaction.block == "fee_measuring")
        #expect(transaction.key == "componentsUi.signing.confirmBlock.feeMeasuring")
    }

    /// Spec 096 F7: a request still being read is not signable — the second
    /// iOS copy of the gate lacked this rule too.
    @Test func aRequestStillBeingReadIsShutAndSaysSo() throws {
        let state = gate(sign: try openSign(), guard: try guardView(),
                         clear: try clear("loading", resolving: true), fee: try fee(ready: true))
        #expect(!state.enabled)
        #expect(state.block == "reading")
        #expect(state.key == "componentsUi.signing.confirmBlock.reading")
    }

    @Test func anApprovalWithNoChoiceAndAShortFeeAndAnotherSpeedSayWhich() throws {
        let choice = gate(sign: try openSign(), guard: try guardView(["confirm_allowed": false, "surface": "approval_editor"]),
                          clear: try clear("clear_sign"), fee: try fee(ready: true))
        #expect(choice.block == "approval_choice")
        #expect(choice.key == "componentsUi.signing.confirmBlock.approvalChoice")

        let short = gate(sign: try openSign(), guard: try guardView(), clear: try clear("clear_sign"),
                         fee: try fee(ready: false, tier: "fast"), speed: "fast")
        #expect(short.block == "fee_short")
        // Issue #438: the fee section says a short coin; the confirm repeated it.
        #expect(short.key == nil)

        // Issue 681: the figure is another speed's — not this one's to sign.
        let another = gate(sign: try openSign(), guard: try guardView(), clear: try clear("clear_sign"),
                           fee: try fee(ready: true, tier: "fast"), speed: "slow")
        #expect(!another.enabled)
        #expect(another.block == "fee_measuring")
    }

    /// Spec 096 F8: a failure held on the sheet shuts the confirm, and its line
    /// says what opens it — Try again when the core offers it, else that the
    /// request has ended. Drawn under the confirm by the sheet's model.
    @Test func aHeldFailureNamesTheWayOn() throws {
        let held = { (retryable: Bool) throws -> SignConfirmStateWire in
            self.gate(
                sign: try self.view(SignRequestCore(), [
                    "confirm_gate_open": false, "confirm_block": "answered", "failure_retryable": retryable,
                ]),
                guard: try self.guardView(), clear: try self.clear("clear_sign"), fee: try self.fee(ready: true)
            )
        }
        let retry = try held(true)
        #expect(!retry.enabled)
        #expect(retry.block == "answered")
        #expect(retry.key == "componentsUi.signing.confirmBlock.answeredRetry")
        #expect(try held(false).key == "componentsUi.signing.confirmBlock.answered")

        // The sheet draws the line under the shut confirm, in the person's words.
        let model = SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc),
            request: SigningController.Incoming(
                id: "r", method: "personal_sign", paramsJson: #"["0x68656c6c6f"]"#,
                origin: dapp, transportId: "t1", chainId: 100
            ),
            sign: .empty, clear: .empty, guard: .empty, fee: nil,
            context: SigningLive.Context(
                loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
                walletName: "Me", walletAddress: a1
            ),
            gate: retry
        )
        #expect(model.confirm?.enabled == false)
        #expect(model.confirmBlockLine == loc.t("componentsUi.signing.confirmBlock.answeredRetry"))

        // A view that does not read keeps the confirm shut, silently.
        #expect(gate(sign: "{}", guard: try guardView(), clear: try clear("clear_sign"), fee: nil) == .shut)
    }
}

// MARK: - The landing's countdown (FR-011)

@MainActor
struct LandingPaceTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let op = "0x" + String(repeating: "cd", count: 32)

    @Test func theCoresPaceCountsFromTheRelaysSend() {
        let unsent = LandingPaceWire.of(sentAtMs: nil, typicalS: 5, nowMs: 60_000)
        #expect(unsent.waiting)
        #expect(unsent.progress == nil, "the ring roams")
        let noUsual = LandingPaceWire.of(sentAtMs: 1_000, typicalS: nil, nowMs: 60_000)
        #expect(noUsual.line == "none")
        let remaining = LandingPaceWire.of(sentAtMs: 1_000, typicalS: 10, nowMs: 5_000)
        #expect(remaining.line == "remaining")
        #expect(remaining.seconds == 6)
        #expect(LandingPaceWire.of(sentAtMs: 1_000, typicalS: 10, nowMs: 14_000).line == "elapsed")
        #expect(LandingPaceWire.of(sentAtMs: 1_000, typicalS: 10, nowMs: 30_000).line == "slow")
    }

    private func sign(op: String) -> SignViewWire {
        SignViewWire(
            surface: .sheet,
            request: SignRequestViewWire(
                id: "r1", method: "eth_sendTransaction", kind: .transaction, paramsJson: "[]",
                origin: dapp, dapp: nil, chainId: 100, signerAddress: nil
            ),
            isSigning: false, isSubmitting: false, pendingOpHash: op, error: nil, funding: nil,
            confirmGateOpen: false, reconcilePending: false, swipeAction: .dismiss, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
    }

    private func context(sentAt: Double?, now: Double) -> SigningLive.Context {
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: a1
        )
        context.track = TrackEntryWire(
            userOpHash: op, chainId: 100, recordIds: ["rec"], status: "pending", txHash: nil,
            polling: true, submittedAtMs: 1_000, outcome: "landing", relaySentAtMs: sentAt
        )
        context.typicalS = 5
        context.nowMs = now
        return context
    }

    /// Accepted, and the relay has not sent it: "the relay is sending it", no
    /// countdown — a minute in, never "taking longer than usual".
    @Test func beforeTheRelaySendsItTheLandingSaysSo() {
        let receipt = SigningLive.receipt(sign: sign(op: op), blocks: [], context: context(sentAt: nil, now: 61_000))
        #expect(receipt?.captions.contains(loc.t("send.txRelaySending")) == true)
        #expect(receipt?.captions.contains(loc.t("send.txWaitingConfirm")) == false)
        #expect(receipt?.eta == nil)
    }

    /// Once the relay has sent it, the chain's clock runs from that moment.
    @Test func onceSentTheChainsClockRunsFromTheSend() throws {
        let receipt = SigningLive.receipt(sign: sign(op: op), blocks: [], context: context(sentAt: 60_000, now: 61_000))
        #expect(receipt?.captions.contains(loc.t("send.txWaitingConfirm")) == true)
        let eta = try #require(receipt?.eta)
        #expect(eta.sentAtMs == 60_000)
        #expect(eta.lines(nowMs: 62_000).last == loc.t("send.txRemaining", vars: ["remaining": "3"]))
        #expect(eta.progress(nowMs: 62_000) != nil)
    }
}

// MARK: - The signer says how it failed (FR-013)

@MainActor
struct SignerKindTests {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The spine keeps the passkey classifier's kind; a cancel stays a cancel.
    @Test func theSpineKeepsTheClassifiersKind() async {
        for kind in [FailureKind.notSupported, .notDiscoverable, .other] {
            let signer = CountingSigner()
            signer.failure = PasskeyFailure(kind: kind, message: "platform said no")
            let spine = UserOpSpine(
                relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
                accounts: ScriptedAccounts(), signer: { signer }
            )
            var caught: UserOpSpine.Failure?
            do {
                _ = try await spine.signMessage(chainId: 100, account: golden, originalHash: Data(count: 32))
            } catch let refused as UserOpSpine.Refused {
                caught = refused.failure
            } catch {}
            #expect(caught == .signer(kind: kind, message: "platform said no"))
        }
    }

    /// The outcome the core is told: `failed` with the kind's wire name, or
    /// `passkey_cancelled` for a cancel; every other failure names no signer.
    @Test func theOutcomeCarriesTheKindsWireName() {
        #expect(SignExecutor.signerFailed(.notSupported, "m")["signer"] as? String == "not_supported")
        #expect(SignExecutor.signerFailed(.notDiscoverable, "m")["signer"] as? String == "not_discoverable")
        #expect(SignExecutor.signerFailed(.other, "m")["signer"] as? String == "other")
        #expect(SignExecutor.signerFailed(.cancelled, "m")["type"] as? String == "passkey_cancelled")
        #expect(SignExecutor.failed("relay down")["signer"] is NSNull)
    }

    /// End to end through the real controller and core: the passkey cannot be
    /// used, the core names the signer, the confirm stays shut with its line,
    /// and the sheet says it in the signer's words — tried again only when a
    /// retry can help.
    @Test(.timeLimit(.minutes(2)))
    func theSheetNamesTheSigner() async throws {
        for (kind, expected, retryable) in [
            (FailureKind.notSupported, SignErrorKind.signerUnavailable, false),
            (.notDiscoverable, .signerNotDiscoverable, false),
            (.other, .signerFailed, true),
        ] {
            let signer = CountingSigner()
            signer.failure = PasskeyFailure(kind: kind, message: "platform said no")
            let port = ScriptedRelayPort()
            port.rpc["eth_getCode"] = .ok("0x")
            let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
            let accounts = ScriptedAccounts()
            let store = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.signer.\(UUID().uuidString)")!)
            let controller = SigningController(
                wallet: (address: golden, credentialId: "cred-1"),
                relay: relay, accounts: accounts,
                spine: UserOpSpine(relay: relay, accounts: accounts, signer: { signer }),
                store: store, pool: RpcPool(store: store, accounts: AccountStore()),
                ports: SigningController.Ports(knownChains: { [100] })
            )
            controller.open(SigningController.Incoming(
                id: "m1", method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(golden)"]"#,
                origin: dapp, transportId: "tab-1", chainId: 100, grantedAddress: golden
            ))
            await Wait.until { controller.confirmState.enabled }
            #expect(controller.fee == nil, "a message has no fee session — and arms without one")
            controller.approve()
            await Wait.until { controller.sign.error != nil }

            #expect(controller.sign.error?.kind == expected, "\(kind)")
            #expect(signer.calls == 1)
            let gate = controller.confirmState
            #expect(!gate.enabled)
            #expect(gate.block == "answered")
            #expect(gate.key == (retryable
                ? "componentsUi.signing.confirmBlock.answeredRetry"
                : "componentsUi.signing.confirmBlock.answered"))

            let words = loc.t(try #require(expected.signerReasonKey))
            let receipt = SigningLive.receipt(
                sign: controller.shownSign, blocks: [],
                context: SigningLive.Context(
                    loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
                    walletName: "Me", walletAddress: golden
                )
            )
            #expect(receipt?.stage == .failed)
            #expect(receipt?.captions.contains(words) == true, "\(kind): \(receipt?.captions ?? [])")
            #expect((receipt?.retry != nil) == retryable)
            #expect(SigningLive.statusBlocks(sign: controller.shownSign, loc: loc).contains {
                if case .warning(_, let text) = $0 { return text == words } else { return false }
            })
        }
    }
}

// MARK: - Close other tabs, tabs to the right, all tabs

/// Chrome's batch closes. Which tabs a scope takes is the core's
/// (`exploreTabsClosedBy`), and one `tabs_closed` closes them; these check the
/// shell asks with the strip it has, offers only what the core says takes a
/// tab, and lets each closed tab's page go as a single close does.
@MainActor
struct TabBatchCloseTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func strip(_ ids: [String], startPage: Set<String> = []) -> [ExploreTabWire] {
        ids.map {
            ExploreTabWire(id: $0, url: startPage.contains($0) ? nil : "https://\($0).example",
                           title: $0, host: startPage.contains($0) ? "" : "\($0).example")
        }
    }

    private func closed(_ scope: ExploreTabCloseScope, _ tabs: [ExploreTabWire]) -> [String] {
        BrowserController.tabsClosed(by: scope, strip: BrowserController.stripJSON(tabs))
    }

    /// Each scope takes the tabs the core names — in strip order, and a start
    /// page tab (no URL) is a tab like any other.
    @Test func eachScopeTakesTheTabsTheCoreNames() {
        let tabs = strip(["t1", "t2", "t3", "t4"], startPage: ["t3"])
        #expect(closed(.others(keep: "t2"), tabs) == ["t1", "t3", "t4"])
        #expect(closed(.right(of: "t2"), tabs) == ["t3", "t4"])
        #expect(closed(.right(of: "t4"), tabs).isEmpty, "nothing is right of the last tab")
        #expect(closed(.all, tabs) == ["t1", "t2", "t3", "t4"])
        #expect(closed(.others(keep: "t1"), strip(["t1"])).isEmpty, "one tab has no others")
        #expect(closed(.others(keep: "gone"), tabs).isEmpty, "an id the strip does not carry closes nothing")
        #expect(closed(.right(of: "gone"), tabs).isEmpty)
        #expect(closed(.all, []).isEmpty)
    }

    /// The strip the shell hands the core is the explore view's `tabs`, as the
    /// core reads it — a start page's missing URL included.
    @Test func theStripReadsInTheCore() throws {
        let json = BrowserController.stripJSON(strip(["a", "b"], startPage: ["b"]))
        let rows = try #require(
            try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [[String: Any]]
        )
        #expect(rows.map { $0["id"] as? String } == ["a", "b"])
        #expect(rows[1]["url"] is NSNull)
        #expect(exploreTabsClosedBy(tabsJson: json, scopeJson: #"{"type":"all"}"#) == #"["a","b"]"#)
        #expect(exploreTabsClosedBy(tabsJson: "nope", scopeJson: #"{"type":"all"}"#) == nil)
        #expect(BrowserController.tabsClosed(by: .all, strip: "nope").isEmpty,
                "input the core cannot read closes nothing")
    }

    /// A card's long-press menu offers a batch close only where the core says
    /// it takes a tab: no "close other tabs" with one tab, no "close tabs to
    /// the right" on the last card.
    @Test func theMenuOffersOnlyWhatTakesATab() {
        func view(_ ids: [String]) -> ExploreViewWire {
            ExploreViewWire(
                favorites: [], tabs: strip(ids), selectedTab: ids.first,
                favoritesHidden: false, recentHidden: false,
                favoritesFull: false, tabsFull: false, ready: true
            )
        }
        let three = ExploreLive.tabs(explore: view(["a", "b", "c"]), loc: loc)
        #expect(three.map(\.closesOthers) == [true, true, true])
        #expect(three.map(\.closesRight) == [true, true, false])

        let one = ExploreLive.tabs(explore: view(["a"]), loc: loc)
        #expect(one.map(\.closesOthers) == [false])
        #expect(one.map(\.closesRight) == [false])

        // The gallery's switcher asks the same core (DESIGN N's board strip:
        // three sites and the start page).
        let gallery = ExploreFixtures.buildMobileState(.e5, loc: loc).tabs
        #expect(gallery.map(\.closesOthers) == [true, true, true, true])
        #expect(gallery.map(\.closesRight) == [true, true, true, false])
    }

    /// The menu's words are the corpus's, never a key echoed back.
    @Test func theMenuSaysItInTheCorpusWords() {
        for tag in ["en", "zh"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let copy = ExploreFixtures.buildMobileState(.e5, loc: loc).tabsScreen
            #expect(copy.close == loc.t("explore.closeTab"))
            #expect(copy.closeOthers == loc.t("explore.closeOtherTabs"))
            #expect(copy.closeRight == loc.t("explore.closeTabsToRight"))
            #expect(copy.closeAll == loc.t("explore.closeAllTabs"))
            for (word, key) in [
                (copy.closeOthers, "explore.closeOtherTabs"),
                (copy.closeRight, "explore.closeTabsToRight"),
            ] {
                #expect(word != key && !word.isEmpty, "`\(key)` echoed the key (\(tag))")
            }
        }
    }

    // MARK: Through the controller and the real core

    /// Four start-page tabs (no engines), `selected` in front.
    private func fourTabs(_ h: BrowserHarness, selected at: Int) async throws -> [String] {
        h.browser.start()
        for n in 1...4 {
            h.browser.newTab()
            await Wait.until { h.browser.explore.tabs.count == n }
        }
        let ids = h.browser.explore.tabs.map(\.id)
        try #require(ids.count == 4)
        h.browser.selectTab(ids[at])
        await Wait.until { h.browser.explore.selectedTab == ids[at] }
        return ids
    }

    private func close(_ h: BrowserHarness, _ scope: ExploreTabCloseScope, leaving n: Int) async {
        h.browser.closeTabs(scope)
        await Wait.until { h.browser.explore.tabs.count == n }
    }

    // The event leaves the core's selection: a selection that survives stays;
    // a closed one moves to the nearest surviving tab on its right, else its
    // left; none left is the start page. One browser per test, each with its
    // own budget: Swift Testing runs suites side by side, and four browsers
    // under one minute timed out on a loaded CI runner.

    /// A selection that survives stays.
    @Test(.timeLimit(.minutes(2)))
    func aSurvivingSelectionStays() async throws {
        let h = BrowserHarness()
        let ids = try await fourTabs(h, selected: 1)
        #expect(h.browser.closeTabs(.right(of: ids[1])) == [ids[2], ids[3]])
        await Wait.until { h.browser.explore.tabs.count == 2 }
        #expect(h.browser.explore.tabs.map(\.id) == [ids[0], ids[1]])
        #expect(h.browser.explore.selectedTab == ids[1])
    }

    /// Closed, nothing on its right survives: the nearest on its left.
    @Test(.timeLimit(.minutes(2)))
    func aClosedSelectionMovesLeftWhenNothingSurvivesOnItsRight() async throws {
        let h = BrowserHarness()
        let ids = try await fourTabs(h, selected: 3)
        await close(h, .right(of: ids[0]), leaving: 1)
        #expect(h.browser.explore.tabs.map(\.id) == [ids[0]])
        #expect(h.browser.explore.selectedTab == ids[0])
    }

    /// Closed, a survivor on its right: that one — "close other tabs" lands on
    /// the tab kept.
    @Test(.timeLimit(.minutes(2)))
    func closingOthersSelectsTheTabKept() async throws {
        let h = BrowserHarness()
        let ids = try await fourTabs(h, selected: 0)
        await close(h, .others(keep: ids[2]), leaving: 1)
        #expect(h.browser.explore.tabs.map(\.id) == [ids[2]])
        #expect(h.browser.explore.selectedTab == ids[2])
    }

    /// All: nothing selected — the start page — and the write landed: a
    /// browser over the same store reads the empty strip back.
    @Test(.timeLimit(.minutes(2)))
    func closingAllLeavesTheStartPageAndPersists() async throws {
        let h = BrowserHarness()
        _ = try await fourTabs(h, selected: 2)
        h.browser.closeAllTabs()
        await Wait.until { h.browser.explore.tabs.isEmpty }
        #expect(h.browser.explore.selectedTab == nil)
        #expect(h.browser.closeTabs(.all).isEmpty, "nothing left: nothing sent")

        let again = BrowserController(store: h.store)
        again.start()
        await Wait.until { again.explore.ready }
        #expect(again.explore.tabs.isEmpty)
    }

    /// End to end on real engines: the tabs a batch close takes lose their
    /// pages — a live one torn down, a suspended one forgotten — and the
    /// browser machine hears `tab_closed` for each, as for a single close;
    /// the tab kept keeps its record. The selection that falls onto it is
    /// not anybody asking to see it (DESIGN N): it wakes — said — only when
    /// it is resumed.
    ///
    /// Each tab is a real WKWebView (see `DebugModeTests` on why the limit).
    @Test(.timeLimit(.minutes(5)))
    func theTabsItTakesLoseTheirPages() async throws {
        let h = BrowserHarness()
        await h.boot()
        h.browser.start()
        var ids: [String] = []
        for n in 1...3 {
            if n > 1 {
                h.browser.newTab()
                await Wait.until { h.browser.explore.tabs.count == n }
            }
            // Port 9 refuses at once: nothing here is about the page loading.
            h.browser.open("http://127.0.0.1:9/\(n)")
            await Wait.until {
                h.browser.explore.selected.map {
                    !ids.contains($0.id) && h.browser.engineForTesting($0.id) != nil
                } ?? false
            }
            let id = try #require(h.browser.explore.selectedTab)
            ids.append(id)
            await h.hello(id, doc: "d\(n)", origin: dapp)
        }
        let (a, b, c) = (ids[0], ids[1], ids[2])
        #expect(h.browser.explore.tabs.map(\.id) == ids)

        // The two background pages let go: `b` keeps its tab with no engine.
        h.browser.memoryWarning()
        #expect(h.browser.suspendedForTesting == [a, b])
        #expect(h.browser.engineForTesting(c) != nil)
        for id in ids { #expect(h.browser.dbr.tab(id) != nil, "\(id) has a record before") }

        // Right of `a`: `b` (suspended) and `c` (live, in front).
        #expect(h.browser.closeTabs(.right(of: a)) == [b, c])
        await h.until { h.browser.explore.tabs.count == 1 && h.browser.dbr.tab(c) == nil }
        await h.settle()
        #expect(h.browser.explore.tabs.map(\.id) == [a])
        #expect(h.browser.engineForTesting(c) == nil, "the live page closed with its tab")
        #expect(!h.browser.suspendedForTesting.contains(b), "the suspended one is forgotten")
        #expect(h.browser.dbr.tab(b) == nil, "the browser machine heard tab_closed for b")
        #expect(h.browser.dbr.tab(c) == nil, "the browser machine heard tab_closed for c")

        // The selection fell left, onto `a`, behind the switcher the batch
        // close came from: nobody asked to see it, so it stays dormant.
        #expect(h.browser.explore.selectedTab == a)
        #expect(h.browser.engineForTesting(a) == nil, "the batch close woke the tab it selected")
        #expect(h.browser.current == nil)
        #expect(h.browser.dbr.tab(a) != nil, "the tab kept keeps its record")

        // Resumed, its page loads again, said.
        h.browser.selectTab(a)
        #expect(h.browser.engineForTesting(a) != nil)
        #expect(h.browser.reloadedTab == a)
        #expect(h.browser.current === h.browser.engineForTesting(a))
        #expect(h.browser.dbr.tab(a) != nil)
    }
}
