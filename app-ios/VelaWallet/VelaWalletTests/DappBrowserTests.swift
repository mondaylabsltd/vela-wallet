//
//  DappBrowserTests.swift
//  VelaWalletTests
//
//  The in-app browser on the core's engine (spec 070), end to end below the
//  web view.
//
//  The REAL `DappBrowserCore`, through the REAL `BrowserController` and
//  `DbrExecutor`, with a fake page (the messages WebKit would have reported,
//  with the tab they came from), a fake delivery sink, a fake pool and a real
//  `VelaStore` on a private suite. What is asserted is what a page would see:
//  which tab, which document, which answer — and what reached the store, the
//  pool and the signing sheet.
//
//  The routing table itself is the core's and is pinned by its own suite
//  (`tests/app_dapp_browser.rs`); these tests are about the shell keeping its
//  half of the contract: never answering the wrong tab, never falling back to
//  the front one, and answering every operation.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

private let dapp = "https://dapp.example"
private let other = "https://other.example"
private let a1 = "0x1111111111111111111111111111111111111111"
private let a2 = "0x2222222222222222222222222222222222222222"
private let t0: Double = 1_757_000_000_000

/// The browser, with every port a recorder.
@MainActor
final class BrowserHarness {
    let store: VelaStore
    let browser: BrowserController

    struct Delivery {
        let tab: String
        let message: [String: Any]
        var doc: String { message["doc"] as? String ?? "" }
        var id: String? { message["id"] as? String }
        var event: String? { message["event"] as? String }
        var errorCode: Int? { ((message["error"] as? [String: Any])?["code"] as? NSNumber)?.intValue }
        var result: Any? { message["result"] }
    }

    var delivered: [Delivery] = []
    var forwards: [DbrForward] = []
    var cancels: [(tab: String, id: String)] = []
    var reads: [(chainId: Int, method: String, params: [Any], bundler: Bool)] = []
    var readAnswer: [String: Any]? = ["result": "0x10"]
    var holdReads = false
    private var held: [CheckedContinuation<Void, Never>] = []
    var receiptLookups: [(chainId: Int, hash: String)] = []
    var receiptAnswer: String?
    var records: [[String: Any]] = []

    init(seed: (VelaStore) -> Void = { _ in }, signingSheet: Bool = true) {
        let suite = "vela.tests.dbr.\(UUID().uuidString)"
        UserDefaults().removePersistentDomain(forName: suite)
        store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        seed(store)
        browser = BrowserController(store: store, now: { t0 })
        browser.ports = BrowserController.Ports(
            poolCall: { [weak self] chainId, method, params, bundler in
                guard let self else { return nil }
                reads.append((chainId, method, params, bundler))
                if holdReads { await withCheckedContinuation { self.held.append($0) } }
                return readAnswer
            },
            resolveUserOp: { [weak self] chainId, hash in
                self?.receiptLookups.append((chainId, hash))
                return self?.receiptAnswer
            },
            onForwardToSigning: signingSheet ? { [weak self] forward in self?.forwards.append(forward) } : nil,
            onCancelSigning: { [weak self] tab, id in self?.cancels.append((tab, id)) },
            writeRecords: { [weak self] rows in self?.records += rows }
        )
        browser.deliverForTesting { [weak self] tab, json in
            let message = (try? CoreJSON.object(json)) ?? [:]
            self?.delivered.append(Delivery(tab: tab, message: message))
        }
    }

    /// Started, two accounts, `a1` active, chains 1 / 100 / 8453, sites read.
    func boot() async {
        browser.startConnections()
        browser.networksChanged([1, 100, 8453])
        browser.accountsChanged(addresses: [a1, a2], active: a1)
        await until { self.browser.dbr.ready }
    }

    func releaseReads() {
        let waiting = held
        held.removeAll()
        waiting.forEach { $0.resume() }
    }

    // MARK: - The fake page

    func hello(_ tab: String, doc: String, origin: String) async {
        browser.pageMessageForTesting(tab: tab, frameOrigin: origin, body: CoreJSON.string(["t": "hello", "doc": doc]))
        await settle()
    }

    func ask(
        _ tab: String, doc: String, origin: String, id: String, method: String,
        params: [Any] = [], mainFrame: Bool = true
    ) async {
        browser.pageMessageForTesting(
            tab: tab, frameOrigin: origin, isMainFrame: mainFrame,
            body: CoreJSON.string(["t": "req", "doc": doc, "id": id, "method": method, "params": params])
        )
        await settle()
    }

    /// Every answer (not event) delivered to `tab`.
    func answers(_ tab: String) -> [Delivery] {
        delivered.filter { $0.tab == tab && $0.message["dir"] as? String == "res" }
    }

    func answer(_ tab: String, id: String) -> Delivery? {
        answers(tab).last { $0.id == id }
    }

    func events(_ tab: String) -> [Delivery] {
        delivered.filter { $0.tab == tab && $0.message["dir"] as? String == "evt" }
    }

    /// Let the core's effect tasks run to quiescence.
    func settle() async {
        for _ in 0..<20 { await Task.yield() }
        try? await Task.sleep(for: .milliseconds(20))
        for _ in 0..<20 { await Task.yield() }
    }

    func until(_ condition: () -> Bool, timeout: Duration = .seconds(3)) async {
        let clock = ContinuousClock()
        let deadline = clock.now + timeout
        while !condition(), clock.now < deadline {
            try? await Task.sleep(for: .milliseconds(5))
        }
    }

    static func grant(_ origin: String, _ address: String, chain: Int) -> String {
        CoreJSON.string([
            "origin": origin, "address": address, "chain_id": chain, "granted_at_ms": t0,
        ])
    }
}

@MainActor
struct DappBrowserTests {

    // MARK: - Delivery goes to the tab — and the document — that asked

    @Test func anAnswerReachesOnlyTheTabThatAsked() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.hello("t2", doc: "e1", origin: other)

        await h.ask("t2", doc: "e1", origin: other, id: "1", method: "eth_chainId")
        await h.until { h.answer("t2", id: "1") != nil }

        let answer = h.answer("t2", id: "1")
        #expect(answer?.doc == "e1", "addressed to the DOCUMENT that asked")
        #expect(answer?.result as? String == "0x1", "a new origin starts on Ethereum (research R5)")
        #expect(h.delivered.allSatisfy { $0.tab == "t2" }, "the other tab heard nothing")
    }

    /// A message from a subframe is ignored: it cannot be answered without
    /// speaking as the top page.
    @Test func aSubframeMessageIsNeverAnswered() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: "https://ads.example", id: "x", method: "eth_accounts",
                    mainFrame: false)
        #expect(h.delivered.isEmpty)
    }

    // MARK: - A background tab's navigation leaves the other tabs alone

    @Test func aBackgroundNavigationSettlesOnlyItsOwnTab() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.hello("t2", doc: "e1", origin: other)

        h.holdReads = true
        await h.ask("t1", doc: "d1", origin: dapp, id: "a", method: "eth_blockNumber")
        await h.ask("t2", doc: "e1", origin: other, id: "b", method: "eth_blockNumber")
        await h.until { h.reads.count == 2 }

        // t1 navigates: a new document commits and says hello.
        h.browser.navigationForTesting(tab: "t1", url: dapp + "/next", finished: false)
        await h.hello("t1", doc: "d2", origin: dapp)
        h.browser.navigationForTesting(tab: "t1", url: dapp + "/next", finished: true)
        await h.settle()

        let settled = h.answer("t1", id: "a")
        #expect(settled?.errorCode == 4900, "the old document's request is settled — never 4001")
        #expect(settled?.doc == "d1", "and addressed to the old document, which the bridge drops")
        #expect(h.answers("t2").isEmpty, "the other tab's request is still open")

        h.releaseReads()
        await h.until { h.answer("t2", id: "b") != nil }
        await h.settle()
        #expect(h.answer("t2", id: "b")?.result as? String == "0x10")
        #expect(h.answers("t1").filter { $0.id == "a" }.count == 1, "exactly one answer per request")
    }

    // MARK: - Signing

    /// The forward carries the SITE's chain and the GRANTED address, and the
    /// answer comes back through the core to the page.
    @Test func aSignatureIsForwardedWithTheSitesChainAndGrantedAddress() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
            store.writeString("vela.chain.\(dapp)", "8453")
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "personal_sign",
                    params: ["0x68656c6c6f", a1])
        await h.until { !h.forwards.isEmpty }

        let forward = h.forwards.first
        #expect(forward?.tab == "t1")
        #expect(forward?.id == "s1")
        #expect(forward?.origin == dapp)
        #expect(forward?.chainId == 8453, "the site's own chain, not a global one")
        #expect(forward?.grantedAddress == a1)
        #expect(h.browser.dbr.signing == DbrSigningViewWire(tab: "t1", id: "s1"))

        h.browser.signingAnswered(tab: "t1", id: "s1",
                                  payload: ["type": "ok", "result": "0x5167"], userOpHash: nil)
        await h.until { h.answer("t1", id: "s1") != nil }
        #expect(h.answer("t1", id: "s1")?.result as? String == "0x5167")
        #expect(h.browser.dbr.signing == nil)
    }

    /// A refusal's words are the core's, from the kind.
    @Test func aRefusedSignatureIsAnsweredWithTheCoresWords() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "personal_sign", params: ["0x00", a1])
        await h.until { !h.forwards.isEmpty }
        h.browser.signingAnswered(
            tab: "t1", id: "s1",
            payload: ["type": "err", "code": 4001, "kind": "user_rejected", "message": NSNull()],
            userOpHash: nil
        )
        await h.until { h.answer("t1", id: "s1") != nil }
        let refused = h.answer("t1", id: "s1")?.message["error"] as? [String: Any]
        #expect((refused?["code"] as? NSNumber)?.intValue == 4001)
        #expect(refused?["message"] as? String == "User rejected the request")
    }

    /// The page behind a sheet navigates: it is settled 4900 by the core, and
    /// the shell is told to close that sheet.
    @Test func navigatingAwayCancelsTheSheet() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "eth_sendTransaction",
                    params: [["to": a2, "value": "0x1"]])
        await h.until { !h.forwards.isEmpty }

        await h.hello("t1", doc: "d2", origin: dapp)
        await h.until { !h.cancels.isEmpty }
        #expect(h.cancels.first?.tab == "t1")
        #expect(h.cancels.first?.id == "s1")
        #expect(h.answer("t1", id: "s1")?.errorCode == 4900)

        // The sheet's answer, arriving late, reaches nobody.
        let before = h.delivered.count
        h.browser.signingAnswered(tab: "t1", id: "s1", payload: ["type": "ok", "result": "0xabc"], userOpHash: nil)
        await h.settle()
        #expect(h.delivered.count == before)
    }

    /// Closing a tab mid-sign cancels its sheet too.
    @Test func closingTheTabCancelsTheSheet() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "personal_sign", params: ["0x00", a1])
        await h.until { !h.forwards.isEmpty }
        h.browser.tabClosedForTesting(tab: "t1")
        await h.until { !h.cancels.isEmpty }
        #expect(h.cancels.map(\.id) == ["s1"])
        #expect(h.browser.dbr.tab("t1") == nil)
    }

    /// No sheet to show it on: the page is told -32002 at once, never left
    /// waiting.
    @Test func noSigningSheetIsABusyAnswerNotASilence() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        }, signingSheet: false)
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "personal_sign", params: ["0x00", a1])
        await h.until { h.answer("t1", id: "s1") != nil }
        #expect(h.answer("t1", id: "s1")?.errorCode == -32002)
    }

    /// A page answered with a user-operation hash can ask for its receipt by
    /// that hash: the relay is asked which transaction carried it, and the
    /// node is asked about THAT.
    @Test func aReceiptPollForAUserOperationIsTranslated() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "eth_sendTransaction",
                    params: [["to": a2, "value": "0x1"]])
        await h.until { !h.forwards.isEmpty }
        h.browser.signingAnswered(tab: "t1", id: "s1", payload: ["type": "ok", "result": "0xOP"],
                                  userOpHash: "0xOP")
        await h.until { h.answer("t1", id: "s1") != nil }

        h.receiptAnswer = "0x7a"
        await h.ask("t1", doc: "d1", origin: dapp, id: "r1", method: "eth_getTransactionReceipt",
                    params: ["0xop"])
        await h.until { h.answer("t1", id: "r1") != nil }
        #expect(h.receiptLookups.first?.hash == "0xop")
        #expect(h.receiptLookups.first?.chainId == 100)
        #expect(h.reads.last?.method == "eth_getTransactionReceipt")
        #expect(h.reads.last?.params.first as? String == "0x7a", "the node is asked about the TRANSACTION")
    }

    // MARK: - Reads

    @Test func aReadGoesThroughThePoolOnTheSitesChain() async {
        let h = BrowserHarness(seed: { store in store.writeString("vela.chain.\(dapp)", "100") })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "1", method: "eth_call",
                    params: [["to": a2, "data": "0x70a08231"], "latest"])
        await h.until { h.answer("t1", id: "1") != nil }

        #expect(h.reads.first?.chainId == 100)
        #expect(h.reads.first?.method == "eth_call")
        #expect(h.reads.first?.bundler == false)
        #expect(h.reads.first?.params.last as? String == "latest")
        #expect(h.answer("t1", id: "1")?.result as? String == "0x10")

        // A bundler method goes to the bundler.
        await h.ask("t1", doc: "d1", origin: dapp, id: "2", method: "eth_getUserOperationReceipt",
                    params: ["0xabc"])
        await h.until { h.answer("t1", id: "2") != nil }
        #expect(h.reads.last?.bundler == true)
    }

    /// The node's refusal reaches the page verbatim; no endpoint at all is
    /// -32603.
    @Test func aNodesRefusalAndASilenceAreDifferentAnswers() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        h.readAnswer = ["error": ["code": 3, "message": "execution reverted: nope"]]
        await h.ask("t1", doc: "d1", origin: dapp, id: "1", method: "eth_call", params: [["to": a2], "latest"])
        await h.until { h.answer("t1", id: "1") != nil }
        let refused = h.answer("t1", id: "1")?.message["error"] as? [String: Any]
        #expect((refused?["code"] as? NSNumber)?.intValue == 3)
        #expect(refused?["message"] as? String == "execution reverted: nope")

        h.readAnswer = nil
        await h.ask("t1", doc: "d1", origin: dapp, id: "2", method: "eth_blockNumber")
        await h.until { h.answer("t1", id: "2") != nil }
        #expect(h.answer("t1", id: "2")?.errorCode == -32603)
    }

    // MARK: - Per-origin chains and grants reach the store

    @Test func aSwitchIsPerOriginAndPersisted() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.hello("t2", doc: "e1", origin: other)
        await h.ask("t1", doc: "d1", origin: dapp, id: "1", method: "wallet_switchEthereumChain",
                    params: [["chainId": "0x2105"]])
        await h.until { h.answer("t1", id: "1") != nil && h.store.readString("vela.chain.\(dapp)") != nil }

        #expect(h.store.readString("vela.chain.\(dapp)") == "8453")
        #expect(h.events("t1").contains { $0.event == "chainChanged" && $0.message["data"] as? String == "0x2105" })
        #expect(h.events("t2").isEmpty, "another site's tab hears nothing")
        #expect(h.browser.dbr.tab("t2")?.chainId == 1)
        #expect(h.browser.dbr.tab("t1")?.chainId == 8453)
    }

    /// Connect → consent → approve: the grant is written, the feed row is
    /// written, and the page is answered.
    @Test func aConsentedConnectionIsStoredAndRecorded() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        await h.ask("t1", doc: "d1", origin: dapp, id: "c", method: "eth_requestAccounts")
        #expect(h.browser.dbr.consent?.origin == dapp)
        #expect(h.browser.dbr.consent?.tab == "t1")

        h.browser.consentApproved()
        await h.until { h.answer("t1", id: "c") != nil && !h.records.isEmpty }
        #expect(h.answer("t1", id: "c")?.result as? [String] == [a1])
        let stored = DpermGrantWire.read(h.store.readString("vela.perm.\(dapp)"))
        #expect(stored?.address == a1)
        #expect(h.records.first?["type"] as? String == "connect")
        #expect(h.records.first?["dappOrigin"] as? String == dapp)
        #expect(h.browser.dbr.tab("t1")?.connectedAddress == a1)
        #expect(h.browser.dbr.sites.map(\.origin) == [dapp])
    }

    /// Settings' Disconnect-all revokes in the LIVE core: the open page hears
    /// it and the key is gone.
    @Test func revokeAllReachesTheOpenPage() async {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        })
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        h.browser.revokeAll()
        await h.until { h.events("t1").count >= 2 && h.store.readString("vela.perm.\(dapp)") == nil }
        #expect(h.events("t1").map(\.event) == ["accountsChanged", "disconnect"])
        #expect(h.store.readString("vela.perm.\(dapp)") == nil)
        #expect(h.browser.dbr.sites.isEmpty)
    }

    /// The renderer dies: every open request of that tab is settled, and the
    /// tab shows as crashed until its next document says hello.
    @Test func aRendererDeathSettlesItsTabAndIsDrawn() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: dapp)
        h.holdReads = true
        await h.ask("t1", doc: "d1", origin: dapp, id: "a", method: "eth_blockNumber")
        h.browser.rendererGoneForTesting(tab: "t1")
        await h.settle()
        #expect(h.browser.dbr.tab("t1")?.crashed == true)
        await h.hello("t1", doc: "d2", origin: dapp)
        #expect(h.browser.dbr.tab("t1")?.crashed == false)
        h.releaseReads()
        await h.settle()
        // The crash settled the request inside the core; nothing is written
        // into a dead renderer, and the read that finishes late answers
        // nobody — least of all the NEXT document.
        #expect(h.answers("t1").filter { $0.id == "a" }.isEmpty)
    }

    // MARK: - list_sites, from the store

    /// Every `vela.perm.*` and `vela.chain.*`, merged by origin. A damaged
    /// grant is left out rather than making the core refuse the whole list.
    @Test func listSitesReadsGrantsAndChains() async {
        let suite = "vela.tests.dbr.list.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        store.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
        store.writeString("vela.chain.\(dapp)", "8453")
        store.writeString("vela.chain.\(other)", "137")
        store.writeString("vela.perm.https://broken.example", "{not json")
        store.writeString("vela.chain.https://zero.example", "0")

        let sites = DbrExecutor(store: store).listSites()
        #expect(sites.map { $0["origin"] as? String } == [dapp, other])
        let first = sites.first ?? [:]
        #expect((first["grant"] as? [String: Any])?["address"] as? String == a1)
        #expect((first["chain_id"] as? NSNumber)?.intValue == 8453)
        #expect(sites.last?["grant"] is NSNull)

        // …and the core reads exactly that.
        let h = BrowserHarness(seed: { seeded in
            seeded.writeString("vela.perm.\(dapp)", BrowserHarness.grant(dapp, a1, chain: 100))
            seeded.writeString("vela.chain.\(dapp)", "8453")
            seeded.writeString("vela.perm.https://broken.example", "{not json")
        })
        await h.boot()
        #expect(h.browser.dbr.sites == [DbrSiteViewWire(origin: dapp, address: a1, chainId: 8453, grantedAtMs: t0)])
    }

    // MARK: - Every operation is answered

    @Test func everyOperationIsAnswered() async throws {
        let suite = "vela.tests.dbr.ops.\(UUID().uuidString)"
        let executor = DbrExecutor(store: VelaStore(defaults: UserDefaults(suiteName: suite)!))
        #expect(DbrExecutor.operations.count == 10)
        for name in DbrExecutor.operations {
            let reply = try CoreJSON.object(await executor.perform(["type": name]))
            #expect(!(reply["type"] as? String ?? "").isEmpty, "no answer for `\(name)`")
        }
        // The neutral answers are the contract's.
        let read = try CoreJSON.object(DbrExecutor.neutralAnswer(["type": "read"]))
        #expect(read["type"] as? String == "read_answered")
        #expect(read["body_json"] is NSNull)
        let sites = try CoreJSON.object(DbrExecutor.neutralAnswer(["type": "list_sites"]))
        #expect((sites["sites"] as? [Any])?.isEmpty == true)
        let receipt = try CoreJSON.object(DbrExecutor.neutralAnswer(["type": "resolve_user_op"]))
        #expect(receipt["tx_hash"] is NSNull)
        #expect(try CoreJSON.object(DbrExecutor.neutralAnswer(["type": "nonsense"]))["type"] as? String == "ack")
    }

    // MARK: - The signing sheet's half

    /// The user-operation hash rides with the answer only when it IS the
    /// answer — a late receipt — never when the page got a transaction hash.
    @Test func theOpHashRidesOnlyWhenItIsTheAnswer() {
        #expect(SigningController.opHashAnswer(["type": "ok", "result": "0xAB"], submitted: "0xab") == "0xab")
        #expect(SigningController.opHashAnswer(["type": "ok", "result": "0xtx"], submitted: "0xab") == nil)
        #expect(SigningController.opHashAnswer(["type": "err", "code": 4001], submitted: "0xab") == nil)
        #expect(SigningController.opHashAnswer(["type": "ok", "result": "0xab"], submitted: nil) == nil)
    }
}
