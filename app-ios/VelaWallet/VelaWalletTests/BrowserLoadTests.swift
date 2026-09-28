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
}
