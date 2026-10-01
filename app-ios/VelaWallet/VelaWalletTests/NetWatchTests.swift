//
//  NetWatchTests.swift
//  VelaWalletTests
//
//  Spec 082 T113 (RE3, W5): when the network comes back nothing waits for a
//  tap. The pool's call outcomes feed the core's `netHealthStep` — three
//  unthrottled give-ups are "offline", the first answer after them is the
//  edge — and the system path's unsatisfied → satisfied edge counts too,
//  through a seam. Hermetic.
//
//  Spec 082 T241 (RJ14, G53): health counts SOURCES, not calls. Offline
//  needs the misses from at least two chains and nothing reached for 10 s;
//  one faulted chain while the others answer never flaps "offline".
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A path the test drives.
@MainActor
final class ScriptedPath: NetPathSource {
    private var onChange: (@MainActor (Bool) -> Void)?
    func start(_ onChange: @escaping @MainActor (Bool) -> Void) { self.onChange = onChange }
    func cancel() { onChange = nil }
    func send(_ satisfied: Bool) { onChange?(satisfied) }
}

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct NetWatchTests {

    /// Three misses, then an answer: one edge — and one retry of the page in
    /// front that failed while the network was gone.
    @Test func threeMissesThenAReachIsOneRetry() {
        let watch = NetWatch(debounceMs: 0)
        let engine = BrowserEngine(id: "tab-\(UUID().uuidString)")
        defer { engine.tearDown() }
        var asked: [URL] = []
        engine.loader = { request in if let url = request.url { asked.append(url) } }
        engine.stopper = {}
        engine.setOnScreen(true)
        engine.load("https://app.uniswap.org/")
        engine.provisionalFailed(code: NSURLErrorNotConnectedToInternet, domain: NSURLErrorDomain,
                                 attempt: "https://app.uniswap.org/")
        // Three automatic attempts, spent while the network was still gone.
        for _ in 0..<3 {
            engine.fireRetry()
            engine.provisionalFailed(code: NSURLErrorNotConnectedToInternet, domain: NSURLErrorDomain,
                                     attempt: "https://app.uniswap.org/")
        }
        #expect(engine.retryPendingMs == nil, "the schedule is spent")
        let before = asked.count

        watch.onCameBack = { engine.networkCameBack() }
        // Three misses from two chains, the last past the 10 s quiet window.
        let t0 = 1_757_000_000_000.0
        watch.observe(.failed(rateLimited: false), chainId: 1, nowMs: t0)
        watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: t0 + 4_000)
        #expect(watch.online, "two misses are not offline yet")
        watch.observe(.failed(rateLimited: false), chainId: 1, nowMs: t0 + 11_000)
        #expect(!watch.online)
        #expect(watch.cameBackCount == 0)

        watch.observe(.ok("0x1"), chainId: 100, nowMs: t0 + 12_000)
        #expect(watch.online)
        #expect(watch.cameBackCount == 1)
        #expect(asked.count == before + 1, "the page in front is asked for again, once")
        #expect(engine.retrying)

        // Another answer is no second edge.
        watch.observe(.ok("0x2"), chainId: 1, nowMs: t0 + 13_000)
        #expect(watch.cameBackCount == 1)
    }

    /// G53: one chain giving up over and over while another answers is that
    /// chain's own notice — never "offline", never a "came back" flap.
    @Test func oneFaultedChainWhileOthersAnswerNeverFlaps() {
        let watch = NetWatch(debounceMs: 0)
        let t0 = 1_757_000_000_000.0
        for step in 0..<12 {
            let at = t0 + Double(step) * 5_000
            watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: at)
            watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: at + 1_000)
            watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: at + 2_000)
            watch.observe(.ok("0x1"), chainId: 1, nowMs: at + 2_500)
        }
        #expect(watch.online, "one faulted chain is not the network")
        #expect(watch.cameBackCount == 0, "and nothing flapped")

        // Many misses from ONE chain, nothing else answering, long past the
        // quiet window: still one source — not "offline".
        let alone = NetWatch(debounceMs: 0)
        for step in 0..<6 {
            alone.observe(.failed(rateLimited: false), chainId: 100, nowMs: t0 + Double(step) * 5_000)
        }
        #expect(alone.online)
    }

    /// Two chains missing, but something answered inside the 10 s window: the
    /// quiet rule holds "offline" back until nothing has answered for 10 s.
    @Test func offlineWaitsForTenQuietSeconds() {
        let watch = NetWatch(debounceMs: 0)
        let t0 = 1_757_000_000_000.0
        watch.observe(.ok("0x1"), chainId: 1, nowMs: t0)
        watch.observe(.failed(rateLimited: false), chainId: 1, nowMs: t0 + 1_000)
        watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: t0 + 2_000)
        watch.observe(.failed(rateLimited: false), chainId: 1, nowMs: t0 + 3_000)
        #expect(watch.online, "something answered 3 s ago")
        watch.observe(.failed(rateLimited: false), chainId: 100, nowMs: t0 + 10_500)
        #expect(!watch.online, "three-plus misses from two chains, nothing for 10 s")
    }

    /// A 429 is the server answering: never a miss. A refusal is an answer.
    @Test func aRateLimitOrARefusalIsAReach() {
        let watch = NetWatch(debounceMs: 0)
        for _ in 0..<5 { watch.observe(.failed(rateLimited: true), chainId: 1) }
        #expect(watch.online)
        #expect(watch.misses == 0)
        watch.observe(.failed(rateLimited: false), chainId: 1)
        watch.observe(.rpcError(code: -32000, message: "no"), chainId: 1)
        #expect(watch.misses == 0)
    }

    /// The path's unsatisfied → satisfied edge, through the seam, after the
    /// debounce — and a flap inside it is not an edge.
    @Test func thePathEdgeThroughTheSeam() async {
        let watch = NetWatch(debounceMs: 20)
        let path = ScriptedPath()
        watch.watchPath(path)
        path.send(true)
        #expect(watch.cameBackCount == 0, "satisfied from the start is no edge")
        path.send(false)
        path.send(true)
        path.send(false)   // a flap inside the debounce
        try? await Task.sleep(nanoseconds: 80_000_000)
        #expect(watch.cameBackCount == 0)
        path.send(true)
        await Wait.until { watch.cameBackCount == 1 }
        #expect(watch.online)
        #expect(watch.misses == 0)
    }

    /// A failed page whose class a returning network cannot clear (a wrong
    /// name) is not asked for again; its count still starts over.
    @Test func aWrongNameIsNotRetriedWhenTheNetworkReturns() {
        let engine = BrowserEngine(id: "tab-\(UUID().uuidString)")
        defer { engine.tearDown() }
        var asked = 0
        engine.loader = { _ in asked += 1 }
        engine.stopper = {}
        engine.setOnScreen(true)
        engine.load("https://nope.invalid/")
        engine.provisionalFailed(code: NSURLErrorCannotFindHost, domain: NSURLErrorDomain,
                                 attempt: "https://nope.invalid/")
        engine.networkCameBack()
        #expect(asked == 1)
        #expect(!engine.retrying)
    }
}
