//
//  R22SigningTruthTests.swift
//  VelaWalletTests
//
//  Round 22 "signing truth" (docs/multisig-machines.md §36), the iOS shell's
//  half. Every rule is the core's; these prove the shell asks it, hands it
//  what it reads, and draws its answer:
//
//    1. the pool's corroborated reads — both nodes' bodies back, an ordinary
//       call unchanged, a chain-id probe outside any race answered;
//    2. the shared vector `rules/signing-truth.json` through the binding
//       (`sim_outcome` over the envelope), the multisig's `rpc_corroborated`
//       and the §31 owner read's witness;
//    3. the confirm's posture (`anyway`, `danger`, `hold_ms`), "Sign anyway",
//       and the hold component — hold, early release, the accessible path;
//    4. the multisig door's `hold_ms`, "Refuse blind signing" and the
//       guard's uncorroborated / disagreed lines;
//    5. "Refuse blind signing": off by default, on refuses every "… anyway";
//    6. WYSIWYS: a drifted operation is refused and nothing is signed;
//    7. r21-1 an expired execution says it expired, calmly; r21-2 a fee is
//       the core's figure, rounded up.
//

import CryptoKit
import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

private let repoRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent()

private func vector(_ path: String) throws -> [String: Any] {
    let data = try Data(contentsOf: repoRoot.appendingPathComponent("rust/crates/vela-core/tests/vectors/\(path)"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}

// MARK: - 1. The pool: corroborated reads

/// A network of scripted nodes behind the real routing core.
@MainActor
final class ScriptedNodes {
    var endpoints: [CollectedEndpoint]
    var ownOnly = false
    /// What each URL answers (`{"result": …}` or `{"error": …}`).
    var answers: [String: [String: Any]] = [:]
    /// The chain each URL says it serves; absent: it says nothing.
    var chainOf: [String: Int] = [:]
    private(set) var posted: [String] = []
    private(set) var probed: [String] = []

    init(_ endpoints: [CollectedEndpoint]) { self.endpoints = endpoints }

    var transport: RpcPool.Transport {
        RpcPool.Transport(
            endpoints: { [unowned self] _ in (self.endpoints, self.ownOnly) },
            post: { [unowned self] url, _, _ in
                self.posted.append(url)
                guard let body = self.answers[url] else { return .network }
                return .response(body)
            },
            chainId: { [unowned self] url in
                self.probed.append(url)
                return self.chainOf[url]
            }
        )
    }
}

@MainActor
@Suite(.hangLimit)
struct R22CorroboratedPoolTests {

    private static let chain = 4242
    private let a = "https://rpc.alpha-nodes.example/v1"
    private let b = "https://eth.beta-infra.example/rpc"

    private func pool(_ nodes: ScriptedNodes) -> RpcPool {
        let defaults = UserDefaults(suiteName: "vela.tests.r22.pool.\(UUID().uuidString)")!
        let pool = RpcPool(store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults),
                           transport: nodes.transport)
        pool.boot()
        return pool
    }

    private func nodes() -> ScriptedNodes {
        let nodes = ScriptedNodes([
            CollectedEndpoint(url: a, source: "default"),
            CollectedEndpoint(url: b, source: "public"),
        ])
        nodes.answers[a] = ["jsonrpc": "2.0", "id": 1, "result": "0xaaaa"]
        nodes.answers[b] = ["jsonrpc": "2.0", "id": 1, "result": "0xbbbb"]
        nodes.chainOf = [a: Self.chain, b: Self.chain]
        return nodes
    }

    /// A corroborated read is answered with BOTH nodes' bodies — the one
    /// the core named and its independent witness — each endpoint having
    /// said which chain it serves first (a probe outside any race, answered).
    @Test func aCorroboratedCallResolvesWithBothBodies() async throws {
        let nodes = nodes()
        let pool = pool(nodes)
        let read = await pool.callCorroborated(chainId: Self.chain, method: "eth_call", params: [])
        let results = [read.primary?["result"] as? String, read.witness?["result"] as? String]
        #expect(Set(results.compactMap { $0 }) == ["0xaaaa", "0xbbbb"], "both bodies: \(results)")
        #expect(!read.ownNode)
        #expect(Set(nodes.probed) == [a, b], "each source said its chain first — outside any race")
        #expect(Set(nodes.posted) == [a, b])
        // The envelope is the core's helper's.
        let envelope = try CoreJSON.object(read.envelopeJson)
        #expect(envelope["result"] is String)
        #expect((envelope["witness"] as? [String: Any])?["result"] is String)
    }

    /// An ordinary call is as it was: one answer, no witness, no probe.
    @Test func anOrdinaryCallIsUnchanged() async throws {
        let nodes = nodes()
        let pool = pool(nodes)
        let routed = await pool.callDetailed(chainId: Self.chain, method: "eth_call", params: [])
        guard case .ok(let value) = routed.outcome else {
            Issue.record("no answer: \(routed.outcome)")
            return
        }
        #expect(["0xaaaa", "0xbbbb"].contains(value as? String))
        #expect(routed.corroborated == nil)
        #expect(nodes.posted.count == 1, "one POST")
        #expect(nodes.probed.isEmpty, "no chain-id probe for a plain read")
    }

    /// A node that names another chain is no source: never asked the read.
    @Test func aWrongChainNodeIsNeverAWitness() async throws {
        let nodes = nodes()
        nodes.chainOf[b] = 1
        let pool = pool(nodes)
        let read = await pool.callCorroborated(chainId: Self.chain, method: "eth_call", params: [])
        #expect(read.primary?["result"] as? String == "0xaaaa")
        #expect(read.witness == nil, "nobody independent answered")
        #expect(!nodes.posted.contains(b))
        // One node's word: the core believes no plain verdict on it.
        let envelope = try CoreJSON.object(read.envelopeJson)
        #expect(envelope["witness"] == nil)
    }

    /// "Only use my node": the person's node alone, trusted by their choice
    /// and said (`own_node`).
    @Test func onlyMyNodeIsTrustedAloneAndSaid() async throws {
        let own = "https://my-node.example:8545"
        let nodes = ScriptedNodes([CollectedEndpoint(url: own, source: "user"), CollectedEndpoint(url: b, source: "public")])
        nodes.ownOnly = true
        nodes.answers[own] = ["jsonrpc": "2.0", "id": 1, "result": "0x01"]
        nodes.chainOf = [own: Self.chain]
        let pool = pool(nodes)
        let read = await pool.callCorroborated(chainId: Self.chain, method: "eth_call", params: [])
        #expect(read.ownNode)
        #expect(read.witness == nil)
        #expect(nodes.posted == [own], "nobody else is asked")
        #expect(try CoreJSON.object(read.envelopeJson)["ownNode"] as? Bool == true)
    }

    /// The pool giving up is no answer at all: `{"unreachable": true}`.
    @Test func thePoolGivingUpIsUnreachable() async throws {
        let nodes = nodes()
        nodes.answers = [:]
        let pool = pool(nodes)
        let read = await pool.callCorroborated(chainId: Self.chain, method: "eth_call", params: [])
        #expect(read.primary == nil)
        #expect(read.envelopeJson == #"{"unreachable":true}"#)
        // An offline pool says the same.
        let defaults = UserDefaults(suiteName: "vela.tests.r22.off.\(UUID().uuidString)")!
        let offline = RpcPool(store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults), offline: true)
        #expect(await offline.callCorroborated(chainId: 1, method: "eth_call").envelopeJson == #"{"unreachable":true}"#)
    }
}

// MARK: - 2. The shared vector, through the binding

@MainActor
struct R22SimVectorTests {

    /// Every `sim` case of `rules/signing-truth.json` through `sim_outcome`
    /// (what the sheet reads the pool's envelope with): its kind, who stands
    /// behind it, its notice, its moves and its source line.
    @Test func theSigningTruthSimCasesReplayThroughTheBinding() throws {
        let file = try vector("rules/signing-truth.json")
        let user = try #require(file["user"] as? String)
        let cases = try #require(file["sim"] as? [[String: Any]])
        #expect(cases.count >= 13)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let envelope = try #require(c["envelope"] as? [String: Any])
            let expect = try #require(c["expect"] as? [String: Any])
            let record = simOutcome(user: user, replyJson: CoreJSON.string(envelope))
            #expect(record.kind == expect["kind"] as? String, "\(name): kind \(record.kind)")
            #expect(record.source == expect["source"] as? String, "\(name): source \(String(describing: record.source))")
            #expect(record.noticeKey == expect["notice_key"] as? String, "\(name): notice_key")
            #expect(record.sourceKey == expect["source_key"] as? String, "\(name): source_key")
            let deltas = (try JSONSerialization.jsonObject(with: Data(record.deltasJson.utf8)) as? [[String: Any]]) ?? []
            #expect(deltas.compactMap { $0["delta"] as? String } == (expect["deltas"] as? [String] ?? []), "\(name): deltas")
            // The sheet's own reading of it: anything but a checked answer is
            // a notice, and one node's word asks again (`recheck`).
            if record.kind == "unconfirmed" {
                #expect(SigningController.simulation(of: record) != .answered, "\(name)")
                #expect(record.recheck, "\(name): Check again")
            }
        }
    }

    /// The sheet's verdict JSON (what the confirm's gate and the agent read)
    /// carries who stood behind it.
    @Test func theVerdictCarriesItsSource() throws {
        let file = try vector("rules/signing-truth.json")
        let user = try #require(file["user"] as? String)
        let own = try #require((file["sim"] as? [[String: Any]])?.first { ($0["expect"] as? [String: Any])?["source"] as? String == "own_node" })
        let record = simOutcome(user: user, replyJson: CoreJSON.string(try #require(own["envelope"] as? [String: Any])))
        let verdict = SigningController.simVerdictObject(of: record)
        #expect(verdict["source"] as? String == "own_node")
        #expect(verdict["source_key"] as? String == "componentsUi.signing.simOwnNode")
        #expect(verdict["deltas"] is [Any], "deltas as the parsed array")
    }
}

// MARK: - The multisig's reads and §31's owner read

@MainActor
final class CorroboratingPorts: MultisigPorts {
    var envelope: [String: Any] = ["result": "0x01", "witness": ["result": "0x01"]]
    private(set) var asked: [(method: String, corroborated: Bool)] = []
    let peer: MultisigPeerTransport = NoRooms()

    final class NoRooms: MultisigPeerTransport {
        func open(relay: String?, invite: String?, pairing: String?) async throws -> (room: String, invite: String?, pairing: String?) {
            throw MultisigWalletRefusal(code: nil, message: "no rooms")
        }
        func send(room: String, message: Any) async {}
        func receive(room: String) async -> MultisigPeerArrival { .closed("none") }
        func close(room: String, cancel: Bool, end: Bool) async {}
    }

    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws -> (status: Int, retryAfter: String?, body: String) {
        (status: 500, retryAfter: nil, body: "")
    }
    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply {
        asked.append((method, false))
        return .result("0x02")
    }
    func rpcCorroborated(chainId: Int, method: String, params: Any) async -> [String: Any] {
        asked.append((method, true))
        return envelope
    }
    func storeRead(_ key: String) -> String?? { .some(nil) }
    func storeWrite(_ key: String, _ value: String?) -> Bool { true }
    func nowMs() -> Double { 0 }
    func random(_ bytes: Int) -> Data { Data(count: bytes) }
    func passkey(_ op: [String: Any]) async -> [String: Any] { ["type": "passkey_cancelled"] }
    func askRisk(_ op: [String: Any]) async -> Bool { false }
    func sendCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "calls_cancelled"] }
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "quote_failed"] }
    func simulate(_ op: [String: Any]) async -> String? { nil }
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) { (nil, nil) }
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? { nil }
    func wallet(method: String, params: Any) async throws -> Any { throw MultisigWalletRefusal(code: 4900, message: "none") }
    func notice(_ notice: [String: Any]) async {}
}

@MainActor
struct R22MultisigReadTests {

    /// `Rpc { corroborate: true }` → `rpc_corroborated` with the envelope as
    /// an OBJECT; `corroborate: false` → exactly as before.
    @Test func aGuardsReadIsAnsweredWithBothNodes() async throws {
        let ports = CorroboratingPorts()
        let executor = MultisigExecutor(ports: ports)
        let corroborated = try CoreJSON.object(await executor.perform([
            "type": "rpc", "chain_id": 100, "method": "eth_getCode", "params": ["0x1", "latest"], "corroborate": true,
        ]))
        #expect(corroborated["type"] as? String == "rpc_corroborated")
        let envelope = try #require(corroborated["envelope"] as? [String: Any], "the envelope is an object")
        #expect((envelope["witness"] as? [String: Any])?["result"] as? String == "0x01")

        let plain = try CoreJSON.object(await executor.perform([
            "type": "rpc", "chain_id": 100, "method": "eth_getCode", "params": ["0x1", "latest"],
        ]))
        #expect(plain["type"] as? String == "rpc_result")
        #expect(plain["result"] as? String == "0x02")
        #expect(ports.asked.map(\.corroborated) == [true, false])
    }

    /// A port with no corroborating pool answers one node's word, honestly.
    @Test func aPortWithoutAPoolAnswersOneNodesWord() async throws {
        final class Single: CorroboratingPortsBase {}
        let envelope = await Single().rpcCorroborated(chainId: 1, method: "eth_call", params: [])
        #expect(envelope["result"] as? String == "0x02")
        #expect(envelope["witness"] == nil)
    }

    /// §31: an unlisted Safe's `isOwner`, as two nodes answered it — the
    /// first's answer, the witness's in the same shape, and `own_node`.
    @Test func theOwnerReadCarriesItsWitness() throws {
        let op: [String: Any] = ["type": "read_safe_owner", "id": "r", "chain_id": 8453, "safe": "0x7", "account": "0xa", "data": "0x"]
        let no = "0x" + String(repeating: "0", count: 64)
        let read = RpcCorroborated(primary: ["result": no], witness: ["result": no], ownNode: false)
        let both = try CoreJSON.object(SignExecutor.safeOwnerRead(op, read: read))
        #expect((both["answer"] as? [String: Any])?["data"] as? String == no)
        #expect((both["witness"] as? [String: Any])?["type"] as? String == "returned")
        #expect(both["own_node"] as? Bool == false)

        let alone = try CoreJSON.object(SignExecutor.safeOwnerRead(op, read: .single(.ok(no))))
        #expect(alone["witness"] is NSNull, "nobody independent answered")
        let mine = try CoreJSON.object(SignExecutor.safeOwnerRead(op, read: RpcCorroborated(primary: ["result": no], witness: nil, ownNode: true)))
        #expect(mine["own_node"] as? Bool == true)
        let late = try CoreJSON.object(SignExecutor.safeOwnerRead(op, read: nil))
        #expect((late["answer"] as? [String: Any])?["type"] as? String == "unreachable")
    }
}

/// `CorroboratingPorts` without its own `rpcCorroborated`: the protocol's.
@MainActor
class CorroboratingPortsBase: MultisigPorts {
    let peer: MultisigPeerTransport = CorroboratingPorts.NoRooms()
    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws -> (status: Int, retryAfter: String?, body: String) {
        (status: 500, retryAfter: nil, body: "")
    }
    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply { .result("0x02") }
    func storeRead(_ key: String) -> String?? { .some(nil) }
    func storeWrite(_ key: String, _ value: String?) -> Bool { true }
    func nowMs() -> Double { 0 }
    func random(_ bytes: Int) -> Data { Data(count: bytes) }
    func passkey(_ op: [String: Any]) async -> [String: Any] { ["type": "passkey_cancelled"] }
    func askRisk(_ op: [String: Any]) async -> Bool { false }
    func sendCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "calls_cancelled"] }
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "quote_failed"] }
    func simulate(_ op: [String: Any]) async -> String? { nil }
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) { (nil, nil) }
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? { nil }
    func wallet(method: String, params: Any) async throws -> Any { throw MultisigWalletRefusal(code: 4900, message: "none") }
    func notice(_ notice: [String: Any]) async {}
}

// MARK: - 3. The confirm's posture, and the hold

@MainActor
struct R22ConfirmTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func view(_ bridge: CoreBridge, _ patch: [String: Any]) throws -> String {
        var object = try CoreJSON.object(bridge.view())
        for (key, value) in patch { object[key] = value }
        return CoreJSON.string(object)
    }

    private func gate(surface: String, sim: String?, refuseBlind: Bool = false) throws -> SignConfirmStateWire {
        let sign = try view(SignRequestCore(), ["surface": "sheet", "confirm_gate_open": true, "confirm_block": NSNull()])
        let guardJson = try view(ApprovalGuardCore(), ["confirm_allowed": true, "surface": "none"])
        let clear = try view(ClearSigningCore(), ["surface": surface, "resolving": false, "resolved": true])
        // A transaction's fee, settled at the speed in force.
        let fee = try view(FeePolicyCore(), [
            "busy": false, "confirm_fee_ready": true,
            "fee": [
                "chain_id": 100, "total_wei": "2100000000000000", "max_fee_per_gas": "2000000000",
                "network_fee_per_gas": "1000000000", "relayer_fee_per_gas": "1000000000",
                "bundler_gas_price": "1000000000", "in_band_gas_basis": "0",
                "effective_gas_price": NSNull(), "max_gas_price": NSNull(), "total_gas": "21000",
                "deployed": true, "tier": "fast", "quoted": true, "fee_asset": ["type": "native"],
                "fee_recipient": "0xfee",
            ] as [String: Any],
        ])
        return SignConfirmStateWire.of(sign: sign, guard: guardJson, clear: clear, fee: fee, speedTier: "fast",
                                       sim: sim, refuseBlind: refuseBlind)
    }

    private static let user = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func verdict(_ envelope: String) -> String {
        CoreJSON.string(SigningController.simVerdictObject(of: simOutcome(user: Self.user, replyJson: envelope)))
    }

    private var checked: String {
        let empty = #"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#
        return verdict(#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}],"witness":\#(empty)}"#)
    }

    /// A transaction whose own simulation two nodes agreed on: a plain press.
    /// One node's word, or no verdict yet: "… anyway", held.
    @Test func theConfirmsPostureIsTheCores() throws {
        let plain = try gate(surface: "clear_sign", sim: checked)
        #expect(plain.enabled && !plain.anyway && plain.holdMs == 0)

        let one = verdict(#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#)
        let unconfirmed = try gate(surface: "clear_sign", sim: one)
        #expect(unconfirmed.enabled && unconfirmed.anyway && unconfirmed.dangerStyle)
        #expect(unconfirmed.holdMs == Int(holdToConfirmMs()))

        let none = try gate(surface: "clear_sign", sim: nil)
        #expect(none.anyway, "a transaction without a verdict is unread")
        #expect(holdToConfirmMs() == 1500 && confirmAgainWindowMs() == 5000)
    }

    /// "Refuse blind signing" on: every "… anyway" is refused, its line said.
    @Test func refuseBlindRefusesEveryAnyway() throws {
        let one = verdict(#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#)
        let refused = try gate(surface: "clear_sign", sim: one, refuseBlind: true)
        #expect(!refused.enabled)
        #expect(refused.block == "blind_refused")
        #expect(refused.key == "componentsUi.signing.confirmBlock.blindRefused")
        #expect(loc.t(refused.key ?? "") != refused.key)
        // A plain request is open as ever.
        #expect(try gate(surface: "clear_sign", sim: checked, refuseBlind: true).enabled)
    }

    /// The sheet's confirm: "Sign anyway" exactly when the core says
    /// `anyway`, in the danger style, held — and the words beside it.
    @Test func theSheetSaysSignAnywayAndHoldsWhereTheCoreSays() throws {
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        let clear = ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil,
            surface: .clearSign, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
        func model(_ gate: SignConfirmStateWire, ownNode: Bool = false) -> SigningModel {
            var context = SigningLive.Context(
                loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
                walletName: "Me", walletAddress: Self.user
            )
            context.simOwnNodeHint = ownNode
            return SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: clear, guard: .empty, fee: nil, context: context, gate: gate
            )
        }
        let anyway = model(SignConfirmStateWire(enabled: true, block: nil, key: nil, anyway: true, danger: false, holdMs: 1500))
        #expect(anyway.confirm?.action == loc.t("componentsUi.signing.certain.signAnyway"))
        #expect(anyway.confirmPress.danger && anyway.confirmPress.holdMs == 1500)
        #expect(anyway.confirmPress.holdHint == loc.t("componentsUi.signing.holdToConfirm"))
        #expect(anyway.confirmPress.confirmAgain == "Press again to confirm")

        let plain = model(SignConfirmStateWire(enabled: true, block: nil, key: nil))
        #expect(plain.confirm?.action != loc.t("componentsUi.signing.certain.signAnyway"))
        #expect(!plain.confirmPress.held && !plain.confirmPress.danger)

        let danger = model(SignConfirmStateWire(enabled: true, block: nil, key: nil, anyway: false, danger: true, holdMs: 1500))
        #expect(danger.confirmPress.danger && danger.confirmPress.held)

        // One node's word: how to check with a node of their own, and the way there.
        let hinted = model(SignConfirmStateWire(enabled: true, block: nil, key: nil, anyway: true, danger: false, holdMs: 1500), ownNode: true)
        #expect(hinted.simOwnNode?.hint == loc.t("componentsUi.signing.ownNodeHint"))
        #expect(hinted.simOwnNode?.action == loc.t("home.nodeFallbackOpen"))
    }

    /// The own node's quiet line under the balance changes.
    @Test func theOwnNodeLineIsSaidUnderTheBalances() {
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: Self.user
        )
        context.simulation = .answered
        context.sim = TrustSimViewWire(ready: true, judgments: [], noChangeKey: "componentsUi.signing.simResultNoChange")
        context.simSourceKey = "componentsUi.signing.simOwnNode"
        let blocks = SigningLive.balanceBlocks(isTransaction: true, context: context)
        let said = blocks.contains {
            if case .source(let text, _) = $0 { return text == "Checked by your own node only" }
            return false
        }
        #expect(said, "\(blocks.map(\.id))")
    }
}

@MainActor
struct R22HoldTests {

    private let hold = 1500.0
    private let window = Double(confirmAgainWindowMs())

    /// Held for `hold_ms`: the confirm fires, once.
    @Test func aHoldThatCompletesConfirms() {
        var state = HoldConfirmState()
        state.pressBegan(nowMs: 1000)
        #expect(state.holdReached(nowMs: 1000 + hold, holdMs: hold) == .confirmed)
        #expect(state.holdReached(nowMs: 1000 + hold * 2, holdMs: hold) == .nothing, "once")
    }

    /// Released early: nothing, and the next hold starts over.
    @Test func anEarlyReleaseDoesNothing() {
        var state = HoldConfirmState()
        state.pressBegan(nowMs: 0)
        #expect(state.pressEnded() == .nothing)
        #expect(state.holdReached(nowMs: hold, holdMs: hold) == .nothing, "the released press never fires")
        state.pressBegan(nowMs: 5000)
        #expect(state.holdReached(nowMs: 5000 + hold - 1, holdMs: hold) == .nothing, "not before its time")
        #expect(state.holdReached(nowMs: 5000 + hold, holdMs: hold) == .confirmed)
    }

    /// The accessible path: the first activation arms (and is announced),
    /// a second within the window confirms.
    @Test func theAssistivePathArmsThenConfirms() {
        var state = HoldConfirmState()
        #expect(state.activate(nowMs: 0, windowMs: window) == .armed)
        #expect(state.armed(nowMs: 100, windowMs: window))
        #expect(state.activate(nowMs: 4000, windowMs: window) == .confirmed)
        #expect(!state.armed(nowMs: 4001, windowMs: window), "disarmed once it fired")
    }

    /// Past the window it disarms: the next activation arms again.
    @Test func theWindowExpiresAndDisarms() {
        var state = HoldConfirmState()
        #expect(state.activate(nowMs: 0, windowMs: window) == .armed)
        state.expire(nowMs: window + 1, windowMs: window)
        #expect(!state.armed(nowMs: window + 1, windowMs: window))
        #expect(state.activate(nowMs: window + 2, windowMs: window) == .armed, "armed again, never a confirm")
    }

    /// A plain press is a plain button; the door's press reads its hold.
    @Test func thePressIsTheDoors() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(!ConfirmPress.plain.held)
        let held = ConfirmPress.door(holdMs: 1500, danger: true, loc: loc)
        #expect(held.held && held.danger)
        #expect(held.holdHint == loc.t("componentsUi.signing.holdToConfirm"))
        #expect(!ConfirmPress.door(holdMs: 0, danger: false, loc: loc).held)
    }
}

// MARK: - 4. The multisig door and the guard's lines

@MainActor
struct R22MultisigDoorTests {

    private let words = MsWords(loc: Loc(overrideTag: "en", preferredLanguages: []), hub: nil)

    private static let anyway = MsJSON(any: ["sign": ["allowed": true, "anyway": true], "concerns": [[String: Any]]()] as [String: Any])
    private static let ready = MsJSON(any: ["kind": "ready"])

    /// An "… anyway" review is held; with "Refuse blind signing" it is
    /// refused, its line said above the review.
    @Test func theDoorHoldsAnywayAndRefusesItWhenTheyAsk() {
        let held = MsRules.reviewDoor(state: Self.ready, review: Self.anyway, refuseBlind: false)
        #expect(held.enabled && held.anyway && held.holdMs == 1500)

        let refused = MsRules.reviewDoor(state: Self.ready, review: Self.anyway, refuseBlind: true)
        #expect(!refused.enabled && refused.blindRefused)
        #expect(refused.unchecked, "said where the unchecked line goes")
        #expect(MsReviewPending.uncheckedLine(refused, words: words).hasPrefix("Vela won’t sign this here"))
    }

    /// The pref reaches every door by default.
    @Test func theDoorReadsThePreference() {
        let before = MsRules.refuseBlind
        defer { MsRules.refuseBlind = before }
        MsRules.refuseBlind = { true }
        #expect(!MsRules.reviewDoor(state: Self.ready, review: Self.anyway).enabled)
        MsRules.refuseBlind = { false }
        #expect(MsRules.reviewDoor(state: Self.ready, review: Self.anyway).enabled)
    }

    /// The state guard could not be confirmed: one node's word, or two that
    /// disagree — each said as what it is.
    @Test func theGuardsUnreadLineSaysWhy() {
        let network = "Gnosis"
        func line(_ reason: String?) -> String {
            var verdict: [String: Any] = ["kind": "unavailable"]
            if let reason { verdict["reason"] = reason }
            return MsReviewBody.unreadStateLine(MsJSON(any: verdict), network: network, words: words)
        }
        #expect(line("uncorroborated").hasPrefix("Only one node on Gnosis answered"))
        #expect(line("disagreed").hasPrefix("Two independent nodes on Gnosis disagree"))
        #expect(line("unreachable") == words.t("review.stateUnread", ["network": network]))
    }

    /// A simulation the review could not confirm is its notice; the own
    /// node's is the quiet line.
    @Test func theReviewsSimulationSaysItsSource() {
        let unconfirmed = MsJSON(any: ["simulation": [
            "kind": "unconfirmed", "notice_key": "componentsUi.signing.simUnconfirmed", "notice_risk": "caution",
        ]])
        #expect(MsReviewBody.movesSaid(unconfirmed) == .line(key: "componentsUi.signing.simUnconfirmed", reason: nil, tone: .warning))
        let own = MsJSON(any: ["simulation": [
            "kind": "deltas", "deltas": [[String: Any]](), "no_change_key": "componentsUi.signing.simResultNoChange",
            "source": "own_node", "source_key": "componentsUi.signing.simOwnNode",
        ]])
        #expect(MsReviewBody.sourceLine(own, words: words) == "Checked by your own node only")
        #expect(MsReviewBody.sourceLine(unconfirmed, words: words) == nil)
    }
}

// MARK: - 5. "Refuse blind signing"

@MainActor
struct R22RefuseBlindPrefTests {

    @Test func itIsOffByDefaultAndStoredByTheCoresKey() {
        let defaults = UserDefaults(suiteName: "vela.tests.r22.pref.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        #expect(refuseBlindSigningKey() == "vela.refuseBlindSigning")
        #expect(!Preferences.refuseBlind(store), "off by default")
        let prefs = Preferences(store: store)
        prefs.boot()
        #expect(!prefs.refuseBlindSigning)
        prefs.setRefuseBlindSigning(true)
        #expect(store.rawValue(refuseBlindSigningKey()) == "on")
        #expect(Preferences.refuseBlind(store))
        prefs.setRefuseBlindSigning(false)
        #expect(store.rawValue(refuseBlindSigningKey()) == "off")
        #expect(!Preferences.refuseBlind(store))
    }

    /// Settings draws it in a "Security" section, as stored.
    @Test func settingsDrawsItInASecuritySection() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let model = SettingsLive.withSecurity(
            refuseBlind: true, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        #expect(model.security?.label == "Security")
        #expect(model.security?.title == "Refuse blind signing")
        #expect(model.security?.on == true)
    }
}

// MARK: - 6. What the sheet showed is what is signed

@MainActor
@Suite(.hangLimit)
struct R22DappAttestationTests {

    private let fixture = TrustedSignerFixture()
    private let gnosis = 100
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"

    private func review(to: String, value: String) -> String {
        CoreJSON.string([
            "kind": "calls", "chain_id": gnosis, "account": fixture.account,
            "calls": [["to": to, "value": value, "data": "0x"]],
            "fee": ["recipient": payee],
        ] as [String: Any])
    }

    private func submit(review: String?) async -> (Result<UserOpSpine.Submitted, Error>, P256AssertionSigner, WindowCapture) {
        let port = WindowCapture()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        let signer = P256AssertionSigner(fixture)
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }, now: { 1_793_000_000_000 }
        )
        do {
            let submitted = try await spine.submit(
                chainId: gnosis, account: fixture.account,
                calls: [UserOpCall(to: payee, value: "1000", data: "0x")],
                gasFeeToken: nil, quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: payee),
                dappReview: UserOpSpine.DappReview(json: review),
                writeAhead: { _, _ in true }
            )
            return (.success(submitted), signer, port)
        } catch {
            return (.failure(error), signer, port)
        }
    }

    /// The reviewed request is signed and sent.
    @Test func theReviewedRequestIsSigned() async throws {
        let (result, signer, port) = await submit(review: review(to: payee, value: "1000"))
        _ = try result.get()
        #expect(signer.challenges.count == 1)
        #expect(port.posted.count == 1)
    }

    /// A drifted operation (another call than the sheet showed), or no
    /// review at all: refused before any prompt — nothing signed, nothing sent.
    @Test func aDriftedOperationIsRefusedAndNothingIsSigned() async throws {
        for drifted in [review(to: payee, value: "999999"), review(to: "0x" + String(repeating: "1", count: 40), value: "1000"), nil] {
            let (result, signer, port) = await submit(review: drifted)
            guard case .failure(let error) = result else {
                Issue.record("a drifted request was signed: \(String(describing: drifted))")
                continue
            }
            #expect((error as? UserOpSpine.Refused)?.failure == .other(UserOpSpine.notReviewedSentence), "\(error)")
            #expect(signer.challenges.isEmpty, "no passkey prompt")
            #expect(port.posted.isEmpty, "nothing sent")
        }
        // The sheet says it as a failed sign, never a success.
        let outcome = SignExecutor.refusedOutcome(.other(UserOpSpine.notReviewedSentence), method: "eth_sendTransaction", chainId: 100)
        #expect(outcome["type"] as? String == "failed")
    }

    /// A signature wraps only the digest the sheet read.
    @Test func aSignatureWrapsOnlyTheDigestShown() throws {
        let digest = Data(repeating: 0xab, count: 32)
        let shown = CoreJSON.string(["kind": "message", "chain_id": gnosis, "account": fixture.account,
                                     "digest": "0x" + String(repeating: "ab", count: 32)] as [String: Any])
        #expect(try dappMessageHash(originalHash: digest, account: fixture.account, reviewJson: shown) == digest)
        #expect(throws: (any Error).self) {
            try dappMessageHash(originalHash: Data(repeating: 0xcd, count: 32), account: fixture.account, reviewJson: shown)
        }
    }
}

// MARK: - 7. r21-1 expired, r21-2 the fee figure

@MainActor
struct R22FollowUpTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private func failed(_ detail: String) throws -> MsTagged {
        try JSONDecoder().decode(MsTagged.self, from: Data(CoreJSON.string(["kind": "failed", "detail": detail]).utf8))
    }

    /// An execution the relay refused with AA22 says the Send's words:
    /// "Not sent", calmly, why — never "Something went wrong".
    @Test func anExpiredExecutionSaysItExpired() throws {
        for detail in [
            MultisigShellFailure.expired.detail,
            MultisigShellFailure.unknown("SUBMIT REFUSED by simulation: AA22 expired or not due").detail,
            #"{"kind":"not_sent","message":"AA22 expired or not due"}"#,
        ] {
            let state = try failed(detail)
            #expect(MsWords.expired(state), "\(detail)")
            let text = try #require(words.actionFailure(state, chainId: 100))
            #expect(text.hasPrefix(loc.t("componentsTx.receipt.statusNotSent")), "\(text)")
            #expect(text.contains(loc.t("componentsUi.signing.expired")), "\(text)")
            #expect(!text.contains(words.t("proposal.reason.unknown")))
        }
        #expect(!MsWords.expired(try failed(#"{"kind":"not_sent"}"#)))
        // The spine's `expired` is the multisig's own kind.
        #expect(MultisigLive.shellFailure(.expired) == .expired)
    }

    /// A fee figure is the core's, rounded UP at its places — never below
    /// the charge (6 for a chain's coin, 4 for a token).
    @Test func aFeeIsTheCoresFigureRoundedUp() {
        #expect(SendLive.feeFromBase("410400290875302", decimals: 18) == "0.000411")
        #expect(SendLive.feeFromBase("1234561", decimals: 6, shown: SendLive.tokenFeePlaces) == "1.2346")
        #expect(SendLive.feeFromBase("1000001", decimals: 6, shown: SendLive.tokenFeePlaces) == "1.0001")
        #expect(SendLive.feeFromBase("123456789", decimals: 18) == feeFigure(units: "123456789", decimals: 18, shown: 6))
        #expect(SendLive.feeFromBase("0", decimals: 18) == "0")
    }
}
