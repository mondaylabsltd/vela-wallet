//
//  BrowserLoadTests.swift
//  VelaWalletTests
//
//  Spec 079 US3: a page on a bad network never looks frozen or broken.
//  The engine is driven by the delegate's events as WebKit would send them —
//  no page is loaded (the loader is a sink) and no socket is opened; the
//  retry timer is fired by hand instead of waited for.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BrowserLoadTests {

    private let offline = NSURLErrorNotConnectedToInternet  // -1009
    private let url = "https://app.uniswap.org/swap"

    private func engine() -> (BrowserEngine, () -> [URL]) {
        let engine = BrowserEngine(id: "tab-\(UUID().uuidString)")
        var asked: [URL] = []
        engine.loader = { request in if let url = request.url { asked.append(url) } }
        engine.stopper = {}
        return (engine, { asked })
    }

    /// Progress shows the moment a load is asked for, not at the commit.
    @Test func progressShowsFromTheRequest() {
        let (engine, asked) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        #expect(engine.loading)
        #expect(engine.progress >= BrowserEngine.requestedProgress)
        #expect(asked().map(\.absoluteString) == [url])
        #expect(engine.failure == nil)
    }

    /// A network failure: the reason, retries at 2 s, 5 s, 10 s while on
    /// screen, the panel kept through every attempt, and the first page that
    /// gets through ends it.
    @Test func aNetworkFailureRetriesOnTheCoresScheduleWithThePanelKept() {
        let (engine, asked) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        engine.provisionalStarted(attempt: url)
        engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)

        #expect(engine.failure?.class == "offline")
        #expect(engine.failureReasonKey == "explore.loadOffline")
        #expect(!engine.loading)
        #expect(engine.url == url, "the address bar keeps where it was going")
        #expect(engine.retryPendingMs == nil, "off screen: no retry runs")

        engine.setOnScreen(true)
        #expect(engine.retryPendingMs == 2_000)

        var schedule: [UInt32] = []
        for expected in [UInt32(2_000), 5_000, 10_000] {
            #expect(engine.retryPendingMs == expected)
            schedule.append(engine.retryPendingMs ?? 0)
            engine.fireRetry()
            #expect(engine.retrying, "the panel says it is retrying")
            #expect(engine.loading)
            #expect(engine.failure != nil, "the panel stays through the attempt")
            engine.provisionalStarted(attempt: url)
            #expect(engine.failure != nil, "a provisional start does not drop the panel")
            engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)
            #expect(!engine.retrying)
        }
        #expect(schedule == [2_000, 5_000, 10_000])
        #expect(engine.retryPendingMs == nil, "three attempts, then the person's own Retry")
        #expect(asked().count == 4, "the first load and three retries, all of the same page")
        #expect(Set(asked().map(\.absoluteString)) == [url])

        // The person's Retry starts the count again.
        engine.reload()
        #expect(engine.retrying)
        engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.retryPendingMs == 2_000)

        // A page that commits ends the failure and its schedule.
        engine.fireRetry()
        engine.provisionalStarted(attempt: url)
        engine.committed()
        #expect(engine.failure == nil)
        #expect(!engine.retrying)
        #expect(engine.retryPendingMs == nil)
    }

    /// WebKit commits and finishes its own `about:blank` in a fresh view whose
    /// first load was refused. That is not the site arriving: the panel, its
    /// reason and the address stay (iPhone pass: a white page, an empty bar).
    @Test func webKitsOwnBlankPageIsNotTheSiteArriving() {
        let (engine, asked) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        engine.provisionalStarted(attempt: url)
        engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)
        engine.reportedURL = { URL(string: "about:blank") }
        engine.provisionalStarted(attempt: "about:blank")
        engine.committed()
        engine.finished()
        #expect(engine.failure?.class == "offline", "the panel stays")
        #expect(engine.url == url, "the address bar keeps the site")
        #expect(engine.host == "app.uniswap.org")
        #expect(!engine.loading)

        // Retry asks for the site again, not the blank document.
        engine.reload()
        #expect(asked().last?.absoluteString == url)

        // The site itself committing ends it.
        engine.reportedURL = { URL(string: self.url) }
        engine.committed()
        #expect(engine.failure == nil)
    }

    /// Behind a proxy WebKit reports nothing for a site that never answered:
    /// the provisional start, then its own blank. That is a failure too.
    @Test func aLoadThatEndsOnWebKitsBlankWithNoWordIsAFailure() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        engine.provisionalStarted(attempt: url)
        engine.reportedURL = { URL(string: "about:blank") }
        engine.committed()
        engine.finished()
        #expect(engine.failure?.class == "offline")
        #expect(engine.failureReasonKey == "explore.loadOffline")
        #expect(engine.url == url, "the address bar keeps the site")
        engine.setOnScreen(true)
        #expect(engine.retryPendingMs == 2_000, "and it retries on the core's schedule")

        // A retry ends the same silent way: the attempt closes, the next is set.
        engine.fireRetry()
        #expect(engine.retrying)
        engine.provisionalStarted(attempt: url)
        engine.committed()
        engine.finished()
        #expect(!engine.retrying, "the button says 重试 again, not 正在重试… forever")
        #expect(engine.retryPendingMs == 5_000)
    }

    /// A failure WebKit did report keeps its own reason when the blank follows.
    @Test func aReportedFailureIsNotOverwrittenByTheBlankThatFollows() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        engine.provisionalStarted(attempt: url)
        engine.provisionalFailed(code: NSURLErrorServerCertificateUntrusted, domain: NSURLErrorDomain, attempt: url)
        engine.reportedURL = { URL(string: "about:blank") }
        engine.committed()
        engine.finished()
        #expect(engine.failure?.class == "certificate")
        engine.setOnScreen(true)
        #expect(engine.retryPendingMs == nil, "a certificate is never retried by itself")
    }

    /// A wrong name and a bad certificate never retry by themselves.
    @Test func aWrongNameOrACertificateIsNeverRetriedAutomatically() {
        for (code, key) in [
            (NSURLErrorCannotFindHost, "explore.loadNotFound"),
            (NSURLErrorServerCertificateUntrusted, "explore.loadCertificate"),
        ] {
            let (engine, _) = engine()
            engine.setOnScreen(true)
            engine.load(url)
            engine.provisionalFailed(code: code, domain: NSURLErrorDomain, attempt: url)
            #expect(engine.failureReasonKey == key)
            #expect(engine.retryPendingMs == nil, "\(key) is never retried by itself")
            engine.tearDown()
        }
    }

    /// A cancelled navigation is not a failure; a retry cut short by one is
    /// simply over, and the panel it was retrying stays.
    @Test func aCancelledNavigationIsNotAFailure() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        engine.provisionalFailed(code: NSURLErrorCancelled, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.failure == nil)
        #expect(!engine.loading)

        engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)
        engine.reload()
        engine.provisionalFailed(code: 102, domain: "WebKitErrorDomain", attempt: url)
        #expect(engine.failure != nil)
        #expect(!engine.retrying)
    }

    /// Retries stop when the page leaves the screen or the app the front,
    /// and resume when it comes back.
    @Test func retriesRunOnlyForThePageInFront() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.setOnScreen(true)
        engine.load(url)
        engine.provisionalFailed(code: offline, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.retryPendingMs == 2_000)

        engine.setOnScreen(false)
        #expect(engine.retryPendingMs == nil)
        engine.fireRetry()
        #expect(!engine.retrying, "a timer that fires off screen does nothing")

        engine.setOnScreen(true)
        #expect(engine.retryPendingMs == 2_000)
        engine.setAppActive(false)
        #expect(engine.retryPendingMs == nil)
        engine.setAppActive(true)
        #expect(engine.retryPendingMs == 2_000)

        // A new address wins over the old retry.
        engine.load("https://example.com/")
        #expect(engine.failure == nil)
        #expect(engine.retryPendingMs == nil)
    }

    // MARK: - The visit (FR-013)

    /// A visit is the page's own address, title and icon from one read, put
    /// through the core's rule — never an error status or a non-web page.
    @Test func aVisitIsReadFromOneDocumentAndPutThroughTheCoresRule() {
        let facts = #"{"href":"https://bscscan.com/","title":"BscScan","icon":"https://bscscan.com/favicon.ico"}"#
        let visit = BrowserEngine.visit(facts: facts, httpStatus: 200)
        #expect(visit?.url == "https://bscscan.com/")
        #expect(visit?.title == "BscScan")
        #expect(visit?.favicon == "https://bscscan.com/favicon.ico")

        #expect(BrowserEngine.visit(facts: facts, httpStatus: 404) == nil, "a site's own 404 page is not a visit")
        #expect(BrowserEngine.visit(facts: facts, httpStatus: 500) == nil)
        #expect(BrowserEngine.visit(facts: #"{"href":"about:blank","title":"","icon":""}"#, httpStatus: nil) == nil)
        #expect(BrowserEngine.visit(facts: #"{"href":"https://x.io/","title":"X","icon":""}"#, httpStatus: nil)?.favicon == nil,
                "no icon named is no icon, not a guess")
        #expect(BrowserEngine.visit(facts: nil, httpStatus: nil) == nil)
        #expect(BrowserEngine.pageFactsScript.contains("location.href")
                && BrowserEngine.pageFactsScript.contains("document.title"),
                "address and title come from the same read as the icon")
    }

    /// A jump within the page is not a load; anything else is.
    @Test func onlyANewDocumentShowsProgress() {
        let here = URL(string: "https://app.uniswap.org/swap")!
        #expect(!BrowserEngine.leavesDocument(from: here, to: URL(string: "https://app.uniswap.org/swap#top")!))
        #expect(BrowserEngine.leavesDocument(from: here, to: URL(string: "https://app.uniswap.org/pool")!))
        #expect(BrowserEngine.leavesDocument(from: here, to: here))
        #expect(BrowserEngine.leavesDocument(from: nil, to: here))
    }

    // MARK: - Spec 082: the watchdog, Stop, and the classes (T112)

    /// Twenty seconds with no commit and WebKit never alive: the load is
    /// stopped, the core's stalled panel shows with the host it was going to,
    /// and it retries on the usual schedule — not the ~75 s WebKit gave up at
    /// (G32).
    @Test func theWatchdogGivesUpAtTheCoresBudget() {
        let (engine, asked) = engine()
        defer { engine.tearDown() }
        var stopped = 0
        engine.stopper = { stopped += 1 }
        engine.reportedProgress = { 0.1 }
        engine.setOnScreen(true)
        engine.load(url)
        #expect(engine.watchdogArmed, "armed at the request")
        engine.provisionalStarted(attempt: url)
        engine.watchdogFired(elapsedMs: browserLoadGiveUpMs())
        #expect(stopped == 1)
        #expect(engine.failure == browserLoadStalled())
        #expect(engine.failureReasonKey == "explore.loadOffline")
        #expect(!engine.loading)
        #expect(!engine.watchdogArmed)
        #expect(engine.bar.host == "app.uniswap.org")
        #expect(engine.retryPendingMs == 2_000)
        #expect(browserLoadGiveUpMs() == 20_000)

        // WebKit's cancellation of the stopped load is not a second failure:
        // the panel stays.
        engine.provisionalFailed(code: NSURLErrorCancelled, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.failure == browserLoadStalled(), "the -999 keeps the panel")

        // The retry is re-armed like any request.
        engine.fireRetry()
        #expect(engine.watchdogArmed)
        #expect(asked().count == 2)
    }

    /// A slow page that is answering is never cut: WebKit's own progress
    /// past the live mark means the site is there.
    @Test func aSlowPageThatIsAnsweringIsNeverCut() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        var stopped = 0
        engine.stopper = { stopped += 1 }
        engine.reportedProgress = { 0.4 }
        engine.load(url)
        engine.provisionalStarted(attempt: url)
        engine.watchdogFired(elapsedMs: browserLoadGiveUpMs())
        #expect(engine.failure == nil)
        #expect(stopped == 0)
        #expect(engine.loading)
    }

    /// A committed load, or an older generation's timer, is never given up.
    @Test func aCommittedOrSupersededLoadIsNeverGivenUp() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.reportedProgress = { 0.1 }
        engine.load(url)
        let first = engine.loadGeneration
        engine.load("https://example.com/")
        engine.watchdogFired(generation: first, elapsedMs: 30_000)
        #expect(engine.failure == nil, "a newer request owns the watchdog")
        engine.committed()
        #expect(!engine.watchdogArmed, "a commit disarms it")
        engine.watchdogFired(elapsedMs: 30_000)
        #expect(engine.failure == nil)
    }

    /// The watchdog is dropped while the app is away and re-armed with the
    /// full budget on return.
    @Test func theWatchdogSleepsWithTheApp() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.load(url)
        #expect(engine.watchdogArmed)
        engine.setAppActive(false)
        #expect(!engine.watchdogArmed)
        engine.setAppActive(true)
        #expect(engine.watchdogArmed)
    }

    /// Stop ends the load and its watchdog; the committed page and its bar
    /// stay, and WebKit's -999 that follows draws no panel.
    @Test func stopKeepsTheCommittedPage() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        var stopped = 0
        engine.stopper = { stopped += 1 }
        engine.reportedURL = { URL(string: "https://jumper.exchange/") }
        engine.committed()
        engine.load(url)
        #expect(engine.loading)
        engine.stop()
        #expect(stopped == 1)
        #expect(!engine.loading)
        #expect(!engine.watchdogArmed)
        engine.provisionalFailed(code: NSURLErrorCancelled, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.failure == nil, "a -999 after Stop is not a failure")
        #expect(engine.bar.host == "jumper.exchange")
        #expect(engine.bar.lock == "closed")
    }

    /// RE4 (G31): -1000 is the per-app proxy refusing CONNECT — "the network
    /// is unstable", retried — never "this site does not exist". A proxy that
    /// cannot be reached (CFNetwork 306) says so, in the corpus's words.
    @Test func theAppleClassesAreTheCores() {
        let (engine, _) = engine()
        defer { engine.tearDown() }
        engine.setOnScreen(true)
        engine.load(url)
        engine.provisionalFailed(code: NSURLErrorBadURL, domain: NSURLErrorDomain, attempt: url)
        #expect(engine.failure?.class == "offline")
        #expect(engine.failureReasonKey == "explore.loadOffline")
        #expect(engine.retryPendingMs == 2_000)

        engine.load(url)
        engine.provisionalFailed(code: 306, domain: "kCFErrorDomainCFNetwork", attempt: url)
        #expect(engine.failure?.class == "proxy")
        #expect(engine.failureReasonKey == "explore.loadProxy")
        #expect(engine.retryPendingMs != nil, "a proxy failure retries like the network")
        for tag in ["zh", "en"] {
            let words = Loc(overrideTag: tag, preferredLanguages: []).t("explore.loadProxy")
            #expect(!words.isEmpty && words != "explore.loadProxy", "\(tag) echoed the key")
        }
    }

    /// The busy Retry: a spinner and "retrying" at full colour, never a
    /// dimmed button (house rule: busy is not disabled).
    @Test func theRetryIsBusyNotDisabled() throws {
        let source = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent().deletingLastPathComponent()
                .appendingPathComponent("VelaWallet/Components/Explore/BrowserWebView.swift"),
            encoding: .utf8
        )
        #expect(source.contains("loading: retrying"))
        #expect(!source.contains("enabled: !retrying"))
    }
}
