//
//  TrackerFindOpEventTests.swift
//  VelaWalletTests
//
//  Spec 082 T107 and T183 (ruling 8, RA4, RA7, RE8): an op whose submit
//  reply was lost is followed to its end, relay or no relay.
//
//  - `find_op_event` reads the EntryPoint's `UserOperationEvent` through the
//    pool and answers what came back — logs, the endpoint's error, the head —
//    judging none of it (the core decides a range error, T180).
//  - The relay's status is asked by the ONE method the relay serves, and
//    parsed by the core (G13's -32601 on every poll).
//  - A may-have-been-sent record written at submit comes back into the core
//    as one after a relaunch, and the chain's own event closes it.
//
//  Hermetic: a scripted port, the real tracker core.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A port whose `eth_getLogs` answers the op's own `UserOperationEvent` for
/// whatever filter it is asked — the topic comes from the question, so the
/// test never hard-codes the event's hash.
@MainActor
final class EventChainPort: RelayPort {
    var head: UInt64 = 48_000_120
    /// The event's `success` word.
    var success = true
    let txHash = "0x" + String(repeating: "7e", count: 32)
    private(set) var filters: [[String: Any]] = []
    private(set) var calls: [String] = []

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        calls.append(method)
        switch method {
        case "eth_blockNumber":
            return .ok("0x" + String(head, radix: 16))
        case "eth_getLogs":
            guard let filter = params.first as? [String: Any],
                  let topics = filter["topics"] as? [String], topics.count == 2
            else { return .ok([Any]()) }
            filters.append(filter)
            let word = { (value: Int) in String(repeating: "0", count: 63) + String(value) }
            return .ok([[
                "address": filter["address"] as? String ?? "",
                "topics": [topics[0], topics[1], "0x" + String(repeating: "0", count: 64),
                           "0x" + String(repeating: "0", count: 64)],
                "data": "0x" + word(0) + word(success ? 1 : 0) + word(5) + word(7),
                "transactionHash": txHash,
                "blockNumber": "0x" + String(head - 3, radix: 16),
                "removed": false,
            ] as [String: Any]])
        default:
            // The relay is mute: no receipt, no status.
            return .failed(rateLimited: false)
        }
    }

    func bundlerBase(chainId: Int) async -> String? { "https://relay.test" }
    func bestRpcUrl(chainId: Int) async -> String? { nil }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer { .failed }
}

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct TrackerFindOpEventTests {

    private let op = "0x" + String(repeating: "c3", count: 32)
    private let entryPoint = "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
    private let topic = "0x" + String(repeating: "49", count: 32)

    private func store() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
    }

    private func findOp(from: UInt64?, to: UInt64?) -> [String: Any] {
        [
            "type": "find_op_event", "chain_id": 100, "entry_point": entryPoint,
            "topic0": topic, "user_op_hash": op,
            "from_block": from.map { $0 as Any } ?? NSNull(),
            "to_block": to.map { $0 as Any } ?? NSNull(),
        ]
    }

    private func executor(_ port: RelayPort) -> TrackerExecutor {
        TrackerExecutor(store: store(), relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0))
    }

    // MARK: - The read, as it came

    /// The logs go back as the pool answered them, with the head; the filter
    /// is the EntryPoint, the event and this op, over the core's window.
    @Test func theLogsGoBackAsThePoolAnsweredThem() async throws {
        let port = EventChainPort()
        let reply = try CoreJSON.object(await executor(port).perform(findOp(from: 100, to: 200)))
        #expect(reply["type"] as? String == "op_event")
        #expect(reply["user_op_hash"] as? String == op)
        #expect(reply["error_json"] is NSNull)
        #expect((reply["head_block"] as? NSNumber)?.uint64Value == port.head)
        let logs = try #require(reply["logs_json"] as? String)
        let parsed = try #require(try JSONSerialization.jsonObject(with: Data(logs.utf8)) as? [[String: Any]])
        #expect(parsed.first?["transactionHash"] as? String == port.txHash)

        let filter = try #require(port.filters.first)
        #expect(filter["address"] as? String == entryPoint)
        #expect(filter["topics"] as? [String] == [topic, op])
        #expect(filter["fromBlock"] as? String == "0x64")
        #expect(filter["toBlock"] as? String == "0xc8")
    }

    /// A range limit is the endpoint's own error member, untouched: the core
    /// halves its window on it (T180). The shell never decides that.
    @Test func aRangeErrorGoesBackUntouched() async throws {
        let port = ScriptedRelayPort()
        port.rpc["eth_blockNumber"] = .ok("0x2dc6c78")
        let member = #"{"code":-32005,"message":"query returned more than 10000 results"}"#
        port.detailed["eth_getLogs"] = [RpcCallResult(
            outcome: .rpcError(code: -32005, message: "query returned more than 10000 results"),
            maybeDelivered: false, heldErrorJson: member
        )]
        let reply = try CoreJSON.object(await executor(port).perform(findOp(from: 1, to: 2_000)))
        #expect(reply["error_json"] as? String == member)
        #expect(reply["logs_json"] is NSNull)
        #expect((reply["head_block"] as? NSNumber)?.uint64Value == 0x2dc6c78)
        // And the core does read it as a range limit.
        #expect(port.calls.contains("eth_getLogs"))
    }

    /// No answer is neither logs nor an error: the core asks the same window
    /// on the next tick.
    @Test func noAnswerIsNeitherLogsNorAnError() async throws {
        let port = ScriptedRelayPort()   // everything unscripted ⇒ .failed
        let reply = try CoreJSON.object(await executor(port).perform(findOp(from: 1, to: 2)))
        #expect(reply["logs_json"] is NSNull)
        #expect(reply["error_json"] is NSNull)
        #expect(reply["head_block"] is NSNull)
    }

    /// `from_block` absent asks for the head alone — no logs are read.
    @Test func aHeadOnlyAskReadsNoLogs() async throws {
        let port = EventChainPort()
        let reply = try CoreJSON.object(await executor(port).perform(findOp(from: nil, to: nil)))
        #expect((reply["head_block"] as? NSNumber)?.uint64Value == port.head)
        #expect(reply["logs_json"] is NSNull)
        #expect(!port.calls.contains("eth_getLogs"))
    }

    // MARK: - The relay's status (T106, RA7, G13)

    /// The relay serves `pimlico_getUserOperationStatus`; the `eth_` name
    /// this client used to ask answered -32601 on every poll. The name and
    /// the parser are the core's, and the bundle hash rides along.
    @Test func theStatusIsAskedByTheCoresMethodAndParsedByTheCore() async throws {
        let port = ScriptedRelayPort()
        port.rpc[userOpStatusMethod()] = .ok([
            "status": "included", "transactionHash": "0xabc", "last_executor_stage": "sent",
        ] as [String: Any])
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let answer = try #require(await relay.userOpStatus(chainId: 100, userOpHash: op))
        #expect(answer.status == "included")
        #expect(answer.txHash == "0xabc")
        #expect(userOpStatusMethod() == "pimlico_getUserOperationStatus")
        #expect(port.calls == ["pimlico_getUserOperationStatus"])

        let reply = try CoreJSON.object(await TrackerExecutor(store: store(), relay: relay).perform([
            "type": "poll_status", "user_op_hash": op, "chain_id": 100,
        ]))
        #expect(reply["type"] as? String == "status")
        #expect(reply["tx_hash"] as? String == "0xabc")
    }

    /// The live probe's answer (evidence/relay-status-probe.txt) parses; a
    /// status the core does not know is no verdict.
    @Test func theProbesAnswerParses() throws {
        let probe = #"{"status":"not_found","transactionHash":null}"#
        let answer = try #require(parseUserOpStatus(json: probe))
        #expect(answer.status == "not_found")
        #expect(answer.txHash == nil)
        #expect(parseUserOpStatus(json: #"{"status":"thinking"}"#) == nil)
    }

    // MARK: - A relaunch (T183)

    /// The row a may-have-been-sent submit wrote goes back into the core as
    /// one; a row from before 082 reads false / absent.
    @Test func aStoredMaybeSentRowIsAMaybeSentWire() throws {
        let row = SignExecutor.recordRow([
            "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xbb","value":"0x1"}]"#, "user_op_hash": op, "chain_id": 100,
            "now_ms": 1_757_000_000_000.0, "maybe_sent": true, "submit_block": 48_000_000,
        ], nativeSymbol: "XDAI")
        let wire = try #require(TrackerExecutor.pendingWire(row))
        #expect(wire["maybe_sent"] as? Bool == true)
        #expect((wire["submit_block"] as? NSNumber)?.uint64Value == 48_000_000)

        let send = SendExecutor.feedRow([
            "id": "s1", "user_op_hash": op, "chain_id": 100, "timestamp_s": 1_757_000_000,
            "maybe_sent": true, "submit_block": 48_000_000,
        ])
        let sendWire = try #require(TrackerExecutor.pendingWire(send))
        #expect(sendWire["maybe_sent"] as? Bool == true)
        #expect((sendWire["submit_block"] as? NSNumber)?.uint64Value == 48_000_000)

        let old: [String: Any] = [
            "id": "old", "userOpHash": op, "chainId": 100, "timestamp": 1_757_000_000, "status": "pending",
        ]
        let oldWire = try #require(TrackerExecutor.pendingWire(old))
        #expect(oldWire["maybe_sent"] as? Bool == false)
        #expect(oldWire["submit_block"] == nil)
    }

    /// Force-quit while "may have been sent" shows, relaunch with the relay
    /// still mute: the row is picked up as may-have-been-sent, the chain's
    /// own event is read from the block written at submit, and it closes the
    /// row confirmed with the event's transaction — no relay needed. Then the
    /// balance is read again (G26).
    @Test func aRelaunchFollowsAMaybeSentRowToTheChainsOwnEvent() async throws {
        let store = store()
        let submitBlock: UInt64 = 48_000_000
        TxRecords.writeRecords([SignExecutor.recordRow([
            "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xbb","value":"0x1"}]"#, "result": "", "from": "0x1",
            "user_op_hash": op, "chain_id": 100,
            // Half a minute ago: inside the wait window, past the first poll.
            "now_ms": Date().timeIntervalSince1970 * 1000 - 30_000,
            "status": "pending", "dapp_origin": "http://127.0.0.1:8137",
            "maybe_sent": true, "submit_block": submitBlock,
        ], nativeSymbol: "XDAI")], store: store)

        let port = EventChainPort()
        var moved: [Int] = []
        var confirmed: [String] = []
        let executor = TrackerExecutor(
            store: store,
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            ports: TrackerExecutor.Ports(
                notifyConfirmed: { hash, _, _ in confirmed.append(hash) },
                holdingsMoved: { moved.append($0) }
            )
        )
        let tracker = TrackerStore(executor: executor)
        tracker.boot()

        await Wait.until({ TxRecords.pending(store: store).isEmpty }, orIdle: { tracker.isIdle })
        let row = try #require(TxRecords.load(store: store).first)
        #expect(row["status"] as? String == "confirmed")
        #expect(row["txHash"] as? String == port.txHash)
        #expect(confirmed == [op])
        #expect(moved == [100], "the balance is read again once the op landed")
        // The scan started at the block written at submit.
        let filter = try #require(port.filters.first)
        #expect(filter["fromBlock"] as? String == "0x" + String(submitBlock, radix: 16))
        let entry = try #require(tracker.view?.entry(userOpHash: op))
        #expect(entry.status == "confirmed")
    }

    /// The chain's event says it reverted: the row fails with the event's
    /// transaction — gas was spent, so the balance is read again too.
    @Test func aRevertedEventFailsTheRowAndStillMovesTheBalance() async throws {
        let store = store()
        TxRecords.writeRecords([SignExecutor.recordRow([
            "record_id": "dapp-2-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xbb","value":"0x1"}]"#, "user_op_hash": op, "chain_id": 100,
            "now_ms": Date().timeIntervalSince1970 * 1000 - 30_000, "status": "pending",
            "maybe_sent": true, "submit_block": 48_000_000,
        ], nativeSymbol: "XDAI")], store: store)
        let port = EventChainPort()
        port.success = false
        var moved: [Int] = []
        let tracker = TrackerStore(executor: TrackerExecutor(
            store: store,
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            ports: TrackerExecutor.Ports(holdingsMoved: { moved.append($0) })
        ))
        tracker.boot()
        await Wait.until({ TxRecords.pending(store: store).isEmpty }, orIdle: { tracker.isIdle })
        #expect(TxRecords.load(store: store).first?["status"] as? String == "failed")
        #expect(moved == [100])
        #expect(tracker.view?.entry(userOpHash: op)?.status == "dropped")
    }
}
