//
//  MultisigShellTests.swift
//  VelaWalletTests
//
//  The multisig shell (native multisig M1–M6): every `MultisigOperation` the
//  five machines can ask is answered by exactly one result the core reads —
//  the web's `shell/handler.test.ts`, operation for operation — and the real
//  machines, over uniffi, run on it: the accounts machine reads its store
//  and says `loaded`, a draft gives an address, the proposals machine
//  follows nothing until told.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A simulated world behind the shell.
@MainActor
final class FakeMultisigPorts: MultisigPorts {
    var store: [String: String] = [:]
    var rpcReply: MultisigRpcReply = .result("0x1")
    var rpcDelayMs: UInt64 = 0
    var httpAnswer: Result<(Int, String?, String), Error> = .success((200, nil, "{}"))
    var notices: [[String: Any]] = []
    var risk = false
    var asked: [String] = []
    let peer: MultisigPeerTransport = FakePeer()

    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws -> (status: Int, retryAfter: String?, body: String) {
        asked.append("http \(method) \(url)")
        let (status, retry, body) = try httpAnswer.get()
        return (status, retry, body)
    }
    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply {
        asked.append("rpc \(chainId) \(method)")
        if rpcDelayMs > 0 { try? await Task.sleep(nanoseconds: rpcDelayMs * 1_000_000) }
        return rpcReply
    }
    func storeRead(_ key: String) -> String?? { .some(store[key]) }
    func storeWrite(_ key: String, _ value: String?) -> Bool { store[key] = value; return true }
    func nowMs() -> Double { 1_700_000_000_000 }
    func random(_ bytes: Int) -> Data { Data(repeating: 0xab, count: bytes) }
    func passkey(_ op: [String: Any]) async -> [String: Any] { ["type": "passkey_cancelled"] }
    func askRisk(_ op: [String: Any]) async -> Bool { risk }
    func sendCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "calls_cancelled"] }
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] { ["type": "quote_failed", "detail": "x"] }
    func simulate(_ op: [String: Any]) async -> String? { #"{"result":[]}"# }
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) { (false, true) }
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? { ("included", "0xtx") }
    func wallet(method: String, params: Any) async throws -> Any {
        throw MultisigWalletRefusal(code: 4001, message: "refused")
    }
    func notice(_ notice: [String: Any]) async { notices.append(notice) }
}

@MainActor
final class FakePeer: MultisigPeerTransport {
    var sent: [Any] = []
    func open(relay: String?, invite: String?, pairing: String?) async throws -> (room: String, invite: String?, pairing: String?) {
        ("room-1", invite == nil && pairing == nil ? "inv" : nil, "rec")
    }
    func send(room: String, message: Any) async { sent.append(message) }
    func receive(room: String) async -> MultisigPeerArrival { .closed("left") }
    func close(room: String, cancel: Bool, end: Bool) async {}
}

@MainActor
struct MultisigShellTests {

    private func run(_ ports: FakeMultisigPorts, _ op: [String: Any]) async -> [String: Any] {
        await MultisigExecutor(ports: ports).answer(op)
    }

    /// Every operation of the vocabulary (`MultisigOperation`, tag `type`).
    static let operations = [
        "http", "rpc", "load_store", "write_store", "wait", "now", "random", "passkey",
        "ask_risk", "send_calls", "quote_calls", "simulate", "recipient_risk",
        "user_op_status", "peer_open", "peer_send", "peer_receive", "peer_close",
        "wallet_request", "notice",
    ]

    /// The result each answers with when it works (one of them).
    static let answers: [String: Set<String>] = [
        "http": ["http_answered", "http_unreachable"],
        "rpc": ["rpc_result", "rpc_error", "rpc_unanswered"],
        "load_store": ["store_loaded", "store_failed"],
        "write_store": ["store_written", "store_failed"],
        "wait": ["waited"], "now": ["now"], "random": ["random"],
        "passkey": ["asserted", "page_approved", "passkey_cancelled", "passkey_failed"],
        "ask_risk": ["risk_answered"],
        "send_calls": ["calls_sent", "calls_failed", "calls_cancelled"],
        "quote_calls": ["quoted", "quote_failed"],
        "simulate": ["simulated"], "recipient_risk": ["recipient_facts"],
        "user_op_status": ["user_op_status"],
        "peer_open": ["peer_opened", "peer_closed"], "peer_send": ["ack"],
        "peer_receive": ["peer_message", "peer_closed"], "peer_close": ["ack"],
        "wallet_request": ["wallet_answered", "wallet_refused"], "notice": ["ack"],
    ]

    @Test func everyOperationIsAnsweredOnceInItsOwnWords() async {
        let ports = FakeMultisigPorts()
        for op in Self.operations {
            let answer = await run(ports, ["type": op, "keys": ["k"], "key": "k", "ms": 0, "bytes": 4, "notice": ["kind": "activated"]])
            let type = answer["type"] as? String ?? ""
            #expect(Self.answers[op]?.contains(type) == true, "\(op) answered \(type)")
        }
    }

    /// The failure each operation answers when its port could not: never a
    /// hang (the web's `operationFailure`).
    @Test func everyOperationHasItsOwnFailure() {
        for op in Self.operations {
            let failure = MultisigExecutor.failure(for: ["type": op], detail: "boom")
            let type = failure["type"] as? String ?? ""
            #expect(Self.answers[op]?.contains(type) == true, "\(op) failed as \(type)")
        }
        // An operation this build does not know is still answered.
        #expect(MultisigExecutor.failure(for: ["type": "brand_new"], detail: "x")["type"] as? String == "ack")
    }

    @Test func anHttpAnswerIsReportedAsItCame() async {
        let ports = FakeMultisigPorts()
        ports.httpAnswer = .success((429, "7", "slow down"))
        let answer = await run(ports, ["type": "http", "method": "POST", "url": "http://s/x", "body": "{}", "timeout_ms": 1000])
        #expect(answer["type"] as? String == "http_answered")
        #expect(answer["status"] as? Int == 429)
        #expect(answer["retry_after"] as? String == "7")
        #expect(answer["body"] as? String == "slow down")
        ports.httpAnswer = .failure(URLError(.notConnectedToInternet))
        let down = await run(ports, ["type": "http", "method": "GET", "url": "http://s/x", "timeout_ms": 1000])
        #expect(down["type"] as? String == "http_unreachable")
    }

    @Test func aNodeAnswerIsReportedAsTheNodeGaveIt() async {
        let ports = FakeMultisigPorts()
        ports.rpcReply = .error(code: -32000, message: "execution reverted")
        let error = await run(ports, ["type": "rpc", "chain_id": 100, "method": "eth_call", "params": []])
        #expect(error["type"] as? String == "rpc_error")
        #expect(error["code"] as? Int == -32000)
        #expect(error["message"] as? String == "execution reverted")
        ports.rpcReply = .unanswered("down")
        let down = await run(ports, ["type": "rpc", "chain_id": 100, "method": "eth_call", "params": []])
        #expect(down["type"] as? String == "rpc_unanswered")
        ports.rpcReply = .result(nil)
        let null = await run(ports, ["type": "rpc", "chain_id": 100, "method": "eth_getCode", "params": []])
        #expect(null["type"] as? String == "rpc_result")
        #expect(null["result"] is NSNull)
    }

    /// A read past its bound never holds the machine.
    @Test func aBoundedReadThatDoesNotAnswerIsUnanswered() async {
        let ports = FakeMultisigPorts()
        // The read takes far longer than the bound; the answer comes at the
        // bound — measured against the read, not a wall clock a busy suite
        // stretches.
        ports.rpcDelayMs = 300_000
        let started = Date()
        let answer = await run(ports, ["type": "rpc", "chain_id": 100, "method": "eth_call", "params": [], "bound_ms": 50])
        #expect(answer["type"] as? String == "rpc_unanswered")
        #expect(Date().timeIntervalSince(started) < 150)
    }

    @Test func theStoreIsReadAndWrittenByKey() async {
        let ports = FakeMultisigPorts()
        ports.store["vela.multisigAccounts"] = "[]"
        let loaded = await run(ports, ["type": "load_store", "keys": ["vela.multisigAccounts", "vela.activeMultisig"]])
        let entries = loaded["entries"] as? [String: Any] ?? [:]
        #expect(entries["vela.multisigAccounts"] as? String == "[]")
        #expect(entries["vela.activeMultisig"] is NSNull)
        _ = await run(ports, ["type": "write_store", "key": "vela.activeMultisig", "value": "0xabc"])
        #expect(ports.store["vela.activeMultisig"] == "0xabc")
        _ = await run(ports, ["type": "write_store", "key": "vela.activeMultisig", "value": NSNull()])
        #expect(ports.store["vela.activeMultisig"] == nil)
    }

    @Test func randomIsHexOfTheBytesAsked() async {
        let answer = await run(FakeMultisigPorts(), ["type": "random", "bytes": 3])
        #expect(answer["hex"] as? String == "0xababab")
    }

    @Test func aRefusedWalletIsSaidWithItsCode() async {
        let answer = await run(FakeMultisigPorts(), ["type": "wallet_request", "method": "eth_accounts", "params": []])
        #expect(answer["type"] as? String == "wallet_refused")
        #expect(answer["code"] as? Int == 4001)
    }

    @Test func aNoticeIsRoutedAndAcked() async {
        let ports = FakeMultisigPorts()
        let answer = await run(ports, ["type": "notice", "notice": ["kind": "mailbox_took", "safe": "0xs"]])
        #expect(answer["type"] as? String == "ack")
        #expect(ports.notices.first?["kind"] as? String == "mailbox_took")
    }

    @Test func recipientFactsAndRelayStatusCarryTheirFields() async {
        let ports = FakeMultisigPorts()
        let facts = await run(ports, ["type": "recipient_risk", "chain_id": 100, "address": "0x1"])
        #expect(facts["is_contract"] as? Bool == false)
        #expect(facts["first_time"] as? Bool == true)
        let status = await run(ports, ["type": "user_op_status", "chain_id": 100, "user_op_hash": "0x1"])
        #expect(status["status"] as? String == "included")
        #expect(status["tx_hash"] as? String == "0xtx")
    }

    @Test func aPeerRoomOpensWithItsInvite() async {
        let answer = await run(FakeMultisigPorts(), ["type": "peer_open", "relay": "ws://r", "invite": NSNull(), "pairing": NSNull()])
        #expect(answer["type"] as? String == "peer_opened")
        #expect(answer["room"] as? String == "room-1")
        #expect(answer["invite"] as? String == "inv")
    }

    // MARK: - The shell's failure words

    @Test func aShellFailureRoundTripsThroughItsDetail() {
        let detail = MultisigShellFailure.passkey(failure: "cancelled", timedOut: true, message: nil).detail
        let read = MultisigShellFailure.read(detail)
        #expect(read?["kind"] as? String == "passkey")
        #expect(read?["timedOut"] as? Bool == true)
        #expect(MultisigShellFailure.read("the core's own words") == nil)
        #expect(MultisigShellFailure.cannotPay(need: "10", have: "3").object["need"] as? String == "10")
    }

    @Test func aSpineRefusalIsSaidInTheShellsWords() {
        #expect(MultisigLive.passkeyAnswer(.passkeyCancelled)["type"] as? String == "passkey_cancelled")
        let timed = MultisigLive.passkeyAnswer(.passkeyTimedOut)
        #expect(timed["type"] as? String == "passkey_failed")
        #expect(MultisigShellFailure.read(timed["detail"] as? String)?["timedOut"] as? Bool == true)
        #expect(MultisigLive.shellFailure(.notSent) == .notSent)
        #expect(MultisigLive.shellFailure(.trustedSigner(.closed)) == .pageDeclined)
        #expect(MultisigLive.shellFailure(.trustedSigner(.timeout)) == .pageSignFailed(page: "unanswered", message: nil))
    }

    @Test func anExecutionsPageIsToldWhatItRuns() {
        let params = MultisigLive.executeParams(["kind": "execute", "tx": ["nonce": "1"], "multisig": "0xm", "name": "Team", "threshold": 2])
        #expect(params["multisig"] as? String == "0xm")
        #expect(params["threshold"] as? Int == 2)
        #expect(MultisigLive.executeParams(nil).isEmpty)
    }

    @Test func amountsCompareInBaseUnits() {
        let have = UInt128Text(hex: "0xde0b6b3a7640000")!  // 1e18
        #expect(have.decimal == "1000000000000000000")
        #expect(UInt128Text.zero.adding("999999999999999999").less(than: have))
        #expect(!have.less(than: UInt128Text.zero.adding("1000000000000000000")))
        #expect(MultisigLive.decimal("0x10") == "16")
        #expect(MultisigLive.decimal(NSNumber(value: 7)) == "7")
        #expect(MultisigLive.bytes("0x0aff") == Data([0x0a, 0xff]))
        #expect(MultisigLive.bytes("0xf") == nil)
    }

    // MARK: - The real machines on this shell

    @Test func theAccountsMachineReadsItsStoreAndSaysLoaded() async {
        let ports = FakeMultisigPorts()
        let machine = MultisigMachine(core: MultisigAccountsCore(), ports: ports, name: "accounts")
        machine.dispatch(["type": "start", "service_url": "http://127.0.0.1:1", "accounts": [], "networks": [100]])
        let loaded = await machine.until(timeout: 10) { $0["loaded"] as? Bool == true ? true : nil }
        #expect(loaded == true)
        #expect(machine.accountsView.loaded)
        #expect(machine.accountsView.accounts.isEmpty)
        machine.dispose()
    }

    @Test func theProposalsMachineFollowsNothingUntilTold() {
        let machine = MultisigMachine(core: MultisigProposalsCore(), ports: FakeMultisigPorts(), name: "proposals")
        let view = machine.proposalsView
        #expect(view.followed == nil)
        #expect(view.paceMs == 12_000)
        #expect(view.items.isEmpty)
        machine.dispose()
    }

    @Test func theRulesAnswerThroughTheDoors() {
        #expect(MsRules.plainUnits("1500000", decimals: 6) == "1.5")
        #expect(MsRules.parseUnits("1.5", decimals: 6) == "1500000")
        #expect(MsRules.executedRowId(chainId: 100, safeTxHash: "0x" + String(repeating: "a", count: 64), index: 0)
            == "multisig:100:0x\(String(repeating: "a", count: 64)):0")
        #expect(MsWords.validName("Team fund"))
        #expect(!MsWords.validName(" padded"))
        #expect(addressEntry(address: "0x1234") == "malformed")
    }
}
