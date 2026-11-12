//
//  ParentSafeGuardTests.swift
//  VelaWalletTests
//
//  VELA-2026-002 (the 0.9.9 hotfix): a site never makes this account act for
//  a Safe it owns. The rule is the core's (`parent_safe_guard`); what is
//  proven here is this shell's half:
//
//  - the vector `tests/vectors/rules/parent-safe-guard.json` answers through
//    this shell's door to the rule (uniffi `parentSafeCheck`), case for case;
//  - `SignExecutor` answers `read_safe_owner` with one `eth_call` as it came
//    (the node's result verbatim, its JSON-RPC error, or `unreachable`), and
//    decides nothing;
//  - the in-app browser's dApp entry — the only page entry into the sign
//    machine on iOS — is read first (`read_safe_owner` → `eth_call`
//    `isOwner(signer)`): the vector's takeover `execTransaction(swapOwner)`,
//    v=1, on a Safe the chain says this account owns opens the sheet refused
//    (`blocked.parent`), signs nothing, and closing it answers the page
//    -32603 `parent_safe_blocked`; the confirm is shut while it reads, and an
//    unreadable answer is refused "couldn't check";
//  - the sheet says it in the catalog's words.
//
//  Hermetic: no network, no ceremony, no device.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - The vector

enum ParentSafeVector {
    static let url = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json")

    static func load() throws -> [String: Any] {
        let data = try Data(contentsOf: url)
        return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    static func cases() throws -> [[String: Any]] {
        try #require(load()["cases"] as? [[String: Any]])
    }

    /// One case's `input`, by its name.
    static func input(_ name: String) throws -> [String: Any] {
        let found = try cases().first { $0["name"] as? String == name }
        let named = try #require(found, "no vector case '\(name)'")
        return try #require(named["input"] as? [String: Any])
    }

    /// One case's request params, as the page would send them.
    static func paramsJson(_ name: String) throws -> String {
        let given = try input(name)
        let params = try #require(given["params"])
        let data = try JSONSerialization.data(withJSONObject: params)
        return String(decoding: data, as: UTF8.self)
    }

    static let takeover = "a: execTransaction(swapOwner) on a Safe this account owns, v=1"
    static let unknownOwner = "unknown Safe, isOwner true: refused"
}

struct ParentSafeGuardVectorTests {

    /// Every case, through `parentSafeCheck`: deep JSON equality with `expect`.
    @Test func everyVectorCaseAnswersThroughThisShellsDoor() throws {
        let cases = try ParentSafeVector.cases()
        #expect(cases.count >= 70, "the vector has its cases: \(cases.count)")
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let input = try #require(c["input"] as? [String: Any], "\(name)")
            let expect = try #require(c["expect"] as? [String: Any], "\(name)")
            let got = try CoreJSON.object(parentSafeCheck(inputJson: CoreJSON.string(input)))
            #expect((got as NSDictionary).isEqual(to: expect), "\(name): got \(got), expected \(expect)")
        }
    }

    /// An input that does not read is an error, never a verdict.
    @Test func anUnreadableInputIsAnError() throws {
        let got = try CoreJSON.object(parentSafeCheck(inputJson: "{"))
        #expect(got["error"] is String)
        #expect(got["verdict"] == nil)
    }
}

// MARK: - The executor's read

@MainActor
struct SafeOwnerReadTests {

    private let safe = "0x7777777777777777777777777777777777777777"
    private let signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

    private var isOwnerData: String { "0x2f54bf6e" + String(repeating: "0", count: 24) + String(signer.dropFirst(2)) }

    private func executor(
        pool: RpcPool? = nil, seconds: Double = SignExecutor.ownerReadDefaultSeconds,
        call: (@MainActor (_ chainId: Int, _ params: [Any]) async -> RpcOutcome)? = nil
    ) -> SignExecutor {
        let relay = RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let store = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.p0g.\(UUID().uuidString)")!)
        var ports = SignExecutor.Ports()
        ports.ethCall = call
        return SignExecutor(
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            relay: relay, store: store, pool: pool, ports: ports, ownerReadSeconds: seconds
        )
    }

    private var operation: [String: Any] {
        ["type": "read_safe_owner", "id": "req-7", "chain_id": 8453, "safe": safe, "account": signer, "data": isOwnerData]
    }

    private func answered(_ executor: SignExecutor) async throws -> [String: Any] {
        let result = try CoreJSON.object(await executor.perform(operation))
        #expect(result["type"] as? String == "safe_owner_read")
        #expect(result["id"] as? String == "req-7", "the id echoed")
        #expect((result["chain_id"] as? NSNumber)?.intValue == 8453, "the chain echoed")
        #expect(result["safe"] as? String == safe, "the Safe echoed")
        return try #require(result["answer"] as? [String: Any])
    }

    @Test func itIsAnOperationThisExecutorAnswers() {
        #expect(SignExecutor.operations.contains("read_safe_owner"))
    }

    /// One `eth_call` of the core's calldata to the Safe, at `latest`, on
    /// the operation's chain — and the node's result handed back verbatim.
    @Test func theNodesResultIsHandedBackVerbatim() async throws {
        var asked: [(Int, [Any])] = []
        let word = "0x" + String(repeating: "0", count: 63) + "1"
        let answer = try await answered(executor { chainId, params in
            asked.append((chainId, params))
            return .ok(word)
        })
        #expect(answer["type"] as? String == "returned")
        #expect(answer["data"] as? String == word)
        #expect(asked.count == 1)
        #expect(asked.first?.0 == 8453)
        let call = asked.first?.1.first as? [String: Any]
        #expect(call?["to"] as? String == safe)
        #expect(call?["data"] as? String == isOwnerData)
        #expect(asked.first?.1.last as? String == "latest")
    }

    /// A JSON-RPC error is the node's, with its code — or none.
    @Test func aNodesErrorIsItsErrorObject() async throws {
        let reverted = try await answered(executor { _, _ in .rpcError(code: 3, message: "execution reverted") })
        #expect(reverted["type"] as? String == "rpc_error")
        #expect((reverted["code"] as? NSNumber)?.intValue == 3)
        #expect(reverted["message"] as? String == "execution reverted")

        let codeless = try await answered(executor { _, _ in .rpcError(code: nil, message: "boom") })
        #expect(codeless["type"] as? String == "rpc_error")
        #expect(codeless["code"] is NSNull, "no code is said as null")
    }

    /// No node answered, a rate limit, a result that is no string, a read
    /// that runs out of time, and no pool at all: each is `unreachable`.
    @Test func noAnswerIsUnreachable() async throws {
        for outcome: RpcOutcome in [.failed(rateLimited: false), .failed(rateLimited: true), .ok(nil), .ok(5)] {
            let answer = try await answered(executor { _, _ in outcome })
            #expect(answer["type"] as? String == "unreachable", "\(outcome)")
            #expect(answer["detail"] is String)
        }
        let late = try await answered(executor(seconds: 0.05) { _, _ in
            try? await Task.sleep(nanoseconds: 5_000_000_000)
            return .ok("0x" + String(repeating: "0", count: 64))
        })
        #expect(late["type"] as? String == "unreachable", "a read past its bound is no answer")
        #expect(late["detail"] as? String == "timed out")

        #expect(try await answered(executor())["type"] as? String == "unreachable")
    }

    /// The app's own pool, with no network: nobody answered.
    @Test func throughThePoolWithNoNetworkItIsUnreachable() async throws {
        let defaults = UserDefaults(suiteName: "vela.tests.p0g.pool.\(UUID().uuidString)")!
        let pool = RpcPool(store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults), offline: true)
        let answer = try await answered(executor(pool: pool))
        #expect(answer["type"] as? String == "unreachable")
    }

    /// The neutral answer (a drifted operation) is "couldn't check", which
    /// the core refuses — never a clear.
    @Test func theNeutralAnswerIsUnreachable() throws {
        let result = try CoreJSON.object(SignExecutor.neutralAnswer(operation))
        #expect(result["type"] as? String == "safe_owner_read")
        #expect(result["id"] as? String == "req-7")
        #expect((result["answer"] as? [String: Any])?["type"] as? String == "unreachable")
    }
}

// MARK: - The sign machine, as the app drives it

/// A signing sheet as `RootView.openSigningRequest` builds one, for the
/// vector's signer on Base: the relay scripted, the pool offline, the clock
/// stopped, the passkey counted — and the owner read the test's.
@MainActor
final class ParentSafeRig {
    static let signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    static let parent = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
    static let unknown = "0x7777777777777777777777777777777777777777"
    static let chain = 8453

    let controller: SigningController
    let signer = CountingSigner()
    let port = ScriptedRelayPort()
    var answers: [(transportId: String, id: String, payload: [String: Any])] = []
    var reads: [(chainId: Int, params: [Any])] = []
    /// What the owner read answers, once let go (`held`).
    var readAnswer: RpcOutcome = .failed(rateLimited: false)
    var holdRead = false
    private var held: [CheckedContinuation<Void, Never>] = []

    init(respond: ((String, String, [String: Any]) -> Void)? = nil) {
        port.rpc["eth_getCode"] = .ok("0x")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let defaults = UserDefaults(suiteName: "vela.tests.p0g.sheet.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let signer = self.signer
        var answer: ((String, String, [String: Any]) -> Void)?
        var read: (@MainActor (Int, [Any]) async -> RpcOutcome)?
        controller = SigningController(
            wallet: (address: Self.signer, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { signer }),
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true),
            ports: SigningController.Ports(
                respond: { transportId, id, payload, _ in answer?(transportId, id, payload) },
                knownChains: { [Self.chain] },
                ethCall: { chainId, params in await read?(chainId, params) ?? .failed(rateLimited: false) }
            ),
            timers: .stopped
        )
        answer = { [weak self] transportId, id, payload in
            self?.answers.append((transportId, id, payload))
            respond?(transportId, id, payload)
        }
        read = { [weak self] chainId, params in
            guard let self else { return .failed(rateLimited: false) }
            reads.append((chainId, params))
            if holdRead { await withCheckedContinuation { self.held.append($0) } }
            return readAnswer
        }
    }

    func release() {
        let waiting = held
        held.removeAll()
        waiting.forEach { $0.resume() }
    }

    var readsHeld: Int { held.count }

    /// Until `condition` holds, or the sheet's machines have nothing out
    /// that could still make it hold (`isIdle`: an owner read out, held or
    /// not, keeps it busy) — a lost answer fails at once instead of waiting.
    /// Capped at two minutes so a broken path fails rather than hangs: under
    /// the whole suite every test shares one main actor, and a wall-clock
    /// bound of seconds was a guess at how busy it is.
    func until(_ condition: () -> Bool) async {
        let deadline = Date().addingTimeInterval(120)
        var quietTurns = 0
        while !condition(), Date() < deadline {
            quietTurns = controller.isIdle ? quietTurns + 1 : 0
            if quietTurns > 20 { return }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    /// Until the sheet's machines have nothing out: whatever a tap could
    /// have started has run.
    func settle() async {
        await Task.yield()
        await until { false }
    }

    /// Nothing was signed and nothing was sent.
    var nothingSigned: Bool {
        signer.calls == 0 && !port.calls.contains("eth_sendUserOperation")
    }

    static func incoming(_ case: String, id: String = "req-1") throws -> SigningController.Incoming {
        SigningController.Incoming(
            id: id, method: try ParentSafeVector.input(`case`)["method"] as? String ?? "eth_sendTransaction",
            paramsJson: try ParentSafeVector.paramsJson(`case`),
            origin: "https://evil.example", transportId: "tab-1", chainId: chain, grantedAddress: signer
        )
    }
}

@MainActor
struct ParentSafeSheetTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func payloadKind(_ rig: ParentSafeRig) -> (code: Int?, kind: String?) {
        let payload = rig.answers.last?.payload
        return ((payload?["code"] as? NSNumber)?.intValue, payload?["kind"] as? String)
    }

    /// The vector's takeover on a Safe the chain says this account owns:
    /// read, then refused — no confirm, nothing signed — and its close
    /// answers -32603 `parent_safe_blocked`.
    @Test func aTakeoverOfAnOwnedSafeIsRefusedOnceRead() async throws {
        let rig = ParentSafeRig()
        rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
        rig.controller.open(try ParentSafeRig.incoming(ParentSafeVector.takeover))
        await rig.until { rig.controller.sign.blocked != nil }

        let blocked = try #require(rig.controller.sign.blocked, "the sheet is refused once read")
        #expect(blocked.parent == ParentSafeRig.parent)
        #expect(blocked.reason == .owner)
        #expect(blocked.function == "execTransaction")
        #expect(rig.controller.sign.error?.kind == .parentSafeBlocked)
        #expect(!rig.controller.confirmState.enabled, "no confirm")
        #expect(rig.reads.count == 1, "one isOwner read of the Safe")

        // The person tries anyway: nothing happens.
        rig.controller.approve()
        await rig.settle()
        #expect(rig.nothingSigned)
        #expect(rig.answers.isEmpty, "the answer waits for the close")

        rig.controller.reject()
        await rig.until { !rig.answers.isEmpty }
        #expect(rig.answers.count == 1)
        let (code, kind) = payloadKind(rig)
        #expect(code == -32603)
        #expect(kind == "parent_safe_blocked")
        #expect(rig.nothingSigned)
    }

    /// Every named hotfix shape a page sends as a transaction: refused once
    /// read owned, nothing signed.
    @Test func everyHotfixShapeIsRefusedOnceReadOwned() async throws {
        for short in ["exec-swap", "exec-add", "exec-thr", "approveHash", "MultiSend-nested"] {
            let rig = ParentSafeRig()
            rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
            rig.controller.open(try ParentSafeRig.incoming("hotfix \(short): isOwner true, refused"))
            await rig.until { rig.controller.sign.blocked != nil }
            #expect(rig.controller.sign.blocked?.parent == ParentSafeRig.unknown, "\(short)")
            #expect(rig.controller.sign.blocked?.reason == .owner, "\(short)")
            rig.controller.approve()
            await rig.settle()
            #expect(rig.nothingSigned, "\(short)")
        }
    }

    /// The same takeover on a Safe this account does not own: read, and no
    /// refusal — the request goes on as before.
    @Test func aSafeThisAccountDoesNotOwnGoesOnAsBefore() async throws {
        let rig = ParentSafeRig()
        rig.readAnswer = .ok("0x" + String(repeating: "0", count: 64))
        rig.controller.open(try ParentSafeRig.incoming("hotfix exec-swap: unknown Safe, not owner, allowed"))
        await rig.until { !rig.reads.isEmpty && rig.controller.confirmState.block != "reading" }
        #expect(rig.reads.count == 1)
        #expect(rig.controller.sign.blocked == nil, "not this account's Safe: no refusal")
        #expect(rig.answers.isEmpty)
    }

    /// Any Safe: `read_safe_owner` → the executor's `eth_call`
    /// of `isOwner(signer)` on the request's chain. The confirm is shut while
    /// it reads; the chain saying "owner" refuses it.
    @Test func anUnknownSafeIsReadAndAnOwnerIsRefused() async throws {
        let rig = ParentSafeRig()
        rig.holdRead = true
        rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
        rig.controller.open(try ParentSafeRig.incoming(ParentSafeVector.unknownOwner))
        await rig.until { rig.readsHeld == 1 }

        #expect(rig.controller.sign.blocked == nil)
        #expect(!rig.controller.confirmState.enabled)
        #expect(rig.controller.confirmState.block == "reading")
        let read = try #require(rig.reads.first)
        #expect(read.chainId == ParentSafeRig.chain)
        let call = read.params.first as? [String: Any]
        #expect(call?["to"] as? String == ParentSafeRig.unknown)
        #expect(call?["data"] as? String == "0x2f54bf6e" + String(repeating: "0", count: 24) + String(ParentSafeRig.signer.dropFirst(2)))
        #expect(read.params.last as? String == "latest")

        rig.release()
        await rig.until { rig.controller.sign.blocked != nil }
        let blocked = try #require(rig.controller.sign.blocked)
        #expect(blocked.parent == ParentSafeRig.unknown)
        #expect(blocked.reason == .owner)
        #expect(!rig.controller.confirmState.enabled)

        rig.controller.approve()
        await rig.settle()
        #expect(rig.nothingSigned)
        rig.controller.reject()
        await rig.until { !rig.answers.isEmpty }
        let (code, kind) = payloadKind(rig)
        #expect(code == -32603 && kind == "parent_safe_blocked")
    }

    /// No node could say: refused all the same, "couldn't check".
    @Test func anUnreadableSafeIsRefusedUnchecked() async throws {
        let rig = ParentSafeRig()
        rig.readAnswer = .failed(rateLimited: false)
        rig.controller.open(try ParentSafeRig.incoming(ParentSafeVector.unknownOwner))
        await rig.until { rig.controller.sign.blocked != nil }

        let blocked = try #require(
            rig.controller.sign.blocked,
            "reads \(rig.reads.count), confirm \(rig.controller.confirmState), idle \(rig.controller.isIdle)"
        )
        #expect(blocked.parent == ParentSafeRig.unknown)
        #expect(blocked.reason == .unchecked)
        #expect(rig.reads.count == 1)
        rig.controller.approve()
        await rig.settle()
        #expect(rig.nothingSigned)
        rig.controller.reject()
        await rig.until { !rig.answers.isEmpty }
        let (code, kind) = payloadKind(rig)
        #expect(code == -32603 && kind == "parent_safe_blocked")
        let message = rig.answers.last?.payload["message"] as? String ?? ""
        #expect(message.contains("could not be checked"), "\(message)")
    }

    /// 0.9.10: the vector's arrival refusals — the Safe's address written
    /// without its `0x` (or `0X`, or under-width), as a `to`, a batch leg or
    /// a `verifyingContract` — are answered -32602 before any read or sheet,
    /// nothing signed.
    @Test func aNonCanonicalAddressIsAnsweredInvalidParamsBeforeAnyRead() async throws {
        let arrivals = try ParentSafeVector.cases().filter {
            ($0["expect"] as? [String: Any])?["verdict"] as? String == "invalid"
        }
        #expect(arrivals.count >= 7, "the arrival cases: \(arrivals.count)")
        for c in arrivals {
            let name = try #require(c["name"] as? String)
            let rig = ParentSafeRig()
            rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
            rig.controller.open(try ParentSafeRig.incoming(name))
            await rig.until { !rig.answers.isEmpty }
            #expect(rig.answers.count == 1, "\(name)")
            let (code, kind) = payloadKind(rig)
            #expect(code == -32602, "\(name)")
            #expect(kind == "invalid_params", "\(name)")
            #expect(rig.reads.isEmpty, "\(name): never read")
            #expect(rig.controller.sign.blocked == nil, "\(name)")
            #expect(rig.nothingSigned, "\(name)")
        }
    }

    /// 0.9.10: a page's `SafeMessage` / `SafeOp` for a Safe v1.5.0 this
    /// account owns is its approval — read, refused, nothing signed.
    @Test func aSafeMessageOrSafeOpForAnOwnedV150SafeIsRefusedOnceRead() async throws {
        for (name, function) in [
            ("0.9.10 v1.5.0: SafeMessage under an owned Safe v1.5.0's domain, isOwner true: refused", "SafeMessage"),
            ("0.9.10 v1.5.0: SafeOp for an owned Safe v1.5.0 (its `safe`), under the 4337 module's domain: refused", "SafeOp"),
        ] {
            let rig = ParentSafeRig()
            rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
            rig.controller.open(try ParentSafeRig.incoming(name))
            await rig.until { rig.controller.sign.blocked != nil }
            let blocked = try #require(rig.controller.sign.blocked, "\(name)")
            #expect(blocked.parent == ParentSafeRig.parent, "\(name)")
            #expect(blocked.reason == .owner, "\(name)")
            #expect(blocked.function == function, "\(name)")
            #expect(rig.reads.count == 1, "\(name)")
            rig.controller.approve()
            await rig.settle()
            #expect(rig.nothingSigned, "\(name)")
            rig.controller.reject()
            await rig.until { !rig.answers.isEmpty }
            let (code, kind) = payloadKind(rig)
            #expect(code == -32603 && kind == "parent_safe_blocked", "\(name)")
        }
    }

    /// The sheet's words: the catalog's `parentSafeBlocked` /
    /// `parentSafeUnchecked`, with the call and the Safe, under the refusal's
    /// title — never the self-call sentence.
    @Test func theSheetSaysItInTheCatalogsWords() async throws {
        let rig = ParentSafeRig()
        rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
        rig.controller.open(try ParentSafeRig.incoming(ParentSafeVector.takeover))
        await rig.until { rig.controller.sign.blocked != nil }
        let said = texts(SigningLive.statusBlocks(sign: rig.controller.sign, loc: loc))
        let safe = AddressText.short(ParentSafeRig.parent)
        let sentence = loc.t("componentsUi.signing.parentSafeBlocked", vars: ["function": "execTransaction", "safe": safe])
        #expect(sentence != "componentsUi.signing.parentSafeBlocked", "the catalog has the key")
        #expect(said.contains(sentence), "\(said)")
        #expect(said.contains(loc.t("componentsUi.signing.selfCallBlockedTitle")))
        #expect(!said.contains(loc.t("componentsUi.signing.selfCallBlockedBody", vars: ["function": "execTransaction"])))

        // The same request, "couldn't check": its own sentence.
        let unread = ParentSafeRig()
        unread.readAnswer = .failed(rateLimited: false)
        unread.controller.open(try ParentSafeRig.incoming(ParentSafeVector.takeover))
        await unread.until { unread.controller.sign.blocked != nil }
        #expect(unread.controller.sign.blocked?.reason == .unchecked)
        let uncheckedSentence = loc.t("componentsUi.signing.parentSafeUnchecked", vars: ["function": "execTransaction", "safe": safe])
        #expect(uncheckedSentence != "componentsUi.signing.parentSafeUnchecked")
        #expect(texts(SigningLive.statusBlocks(sign: unread.controller.sign, loc: loc)).contains(uncheckedSentence))
    }

    /// The words of the sheet's status blocks.
    private func texts(_ blocks: [SigningBlock]) -> [String] {
        blocks.compactMap { block in
            switch block {
            case .intent(let text, _), .sentence(let text, _), .warning(_, let text): text
            default: nil
            }
        }
    }

    /// A view without the new fields (an older core) still decodes: no
    /// parent, the self-call wording.
    @Test func aBlockedViewWithoutAParentDecodes() throws {
        let view = try CoreJSON.decoder.decode(
            SignBlockedViewWire.self,
            from: Data(#"{"function":"addOwnerWithThreshold","selector":"0x0d582f13","leg_index":null,"nested":false}"#.utf8)
        )
        #expect(view.parent == nil && view.reason == nil)
        let kind = try CoreJSON.decoder.decode([SignErrorKind].self, from: Data(#"["parent_safe_blocked"]"#.utf8))
        #expect(kind == [.parentSafeBlocked])
    }
}

// MARK: - The in-app browser's entry, end to end below the web view

@MainActor
struct ParentSafeBrowserEntryTests {

    private let site = "https://evil.example"

    /// The page asks; the REAL browser core forwards it; the sheet is built
    /// from the forward exactly as `RootView.openBrowserSigning` builds it;
    /// the chain says this account owns the Safe; its answer goes back
    /// through the browser core to the page. The page is told -32603.
    @Test func aPagesTakeoverRequestIsRefusedAndThePageToldSo() async throws {
        let signer = ParentSafeRig.signer
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(self.site)", BrowserHarness.grant(self.site, signer, chain: ParentSafeRig.chain))
            store.writeString("vela.chain.\(self.site)", "\(ParentSafeRig.chain)")
        })
        h.browser.startConnections()
        h.browser.networksChanged([ParentSafeRig.chain])
        h.browser.accountsChanged(addresses: [signer], active: signer)
        await h.until { h.browser.dbr.ready }
        await h.hello("t1", doc: "d1", origin: site)

        let takeover = try ParentSafeVector.input(ParentSafeVector.takeover)
        let params = try #require(takeover["params"] as? [Any])
        await h.ask("t1", doc: "d1", origin: site, id: "x1", method: "eth_sendTransaction", params: params)
        await h.until { !h.forwards.isEmpty }
        let forward = try #require(h.forwards.first, "the browser forwards it to the sheet")
        #expect(forward.grantedAddress.lowercased() == signer)
        #expect(forward.chainId == ParentSafeRig.chain)

        let browser = h.browser
        let rig = ParentSafeRig { transportId, id, payload in
            browser.signingAnswered(tab: transportId, id: id, payload: payload, userOpHash: nil)
        }
        rig.readAnswer = .ok("0x" + String(repeating: "0", count: 63) + "1")
        // As `RootView.openBrowserSigning` builds it from the forward.
        rig.controller.open(SigningController.Incoming(
            id: forward.id,
            method: forward.method,
            paramsJson: forward.paramsJson,
            origin: forward.origin,
            transportId: forward.tab,
            chainId: forward.chainId,
            grantedAddress: forward.grantedAddress
        ))
        await rig.until { rig.controller.sign.blocked != nil }

        let blocked = try #require(rig.controller.sign.blocked, "the sheet is refused once read")
        #expect(blocked.parent == ParentSafeRig.parent)
        #expect(blocked.reason == .owner)
        #expect(rig.reads.count == 1)
        #expect(!rig.controller.confirmState.enabled)
        rig.controller.approve()
        await rig.settle()
        #expect(rig.nothingSigned, "nothing signed")
        #expect(h.answer("t1", id: "x1") == nil, "the page waits for the close")

        rig.controller.reject()
        // The sheet's answer is an effect a turn later; then the browser's.
        await rig.until { !rig.answers.isEmpty }
        await h.until { h.answer("t1", id: "x1") != nil }
        let answer = h.answer("t1", id: "x1")
        let error = try #require(answer?.message["error"] as? [String: Any], "\(String(describing: answer?.message)) \(rig.answers)")
        #expect((error["code"] as? NSNumber)?.intValue == -32603, "\(error)")
        #expect(rig.answers.last?.payload["kind"] as? String == "parent_safe_blocked")
        #expect(rig.nothingSigned)
    }
}
