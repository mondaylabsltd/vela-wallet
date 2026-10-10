//
//  ReceiveTimeTests.swift
//  VelaWalletTests
//
//  A receipt's time is its block's time, never "now" (PR 3, fix A).
//
//  On the owner's iPhone three receipts of 0.001 xDAI, really received on
//  2026-09-29, stood under 「今天」 on 2026-10-10: the scan could not read
//  their block, the core stamped them with the clock THIS shell handed it
//  (`block_timestamp.now_ms`), and this shell stored that for good.
//
//  The rule is the core's (`token_trust` invariant ⑨, `activity_feed`'s
//  repair). What is tested here is what this shell owes it, on its own
//  wiring: the trust executor's answer carries no clock; a receipt stored
//  from the feed is marked; the mark reaches the core and its absence marks
//  a record for repair; and the repair's two operations — read a receipt's
//  block time through the pool, rewrite one stored record — do exactly that
//  and nothing else. The real `token_trust` and `activity_feed` cores, the
//  real executors and the real store; only the chain is scripted.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ReceiveTimeTests {

    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let peer = "0x1111111111111111111111111111111111111111"
    /// 2026-09-29 09:30:00 UTC — the day the three receipts really landed.
    private let blockTime = 1_790_674_200.0
    private let blockTimeHex = "0x6abb8518"

    // MARK: - The chain, scripted

    /// What the feed executor's reads are answered with, and what it asked.
    @MainActor
    final class Chain {
        var receipt: RpcOutcome = .ok(["blockNumber": "0x2a1b3c"] as [String: Any])
        var block: RpcOutcome = .ok(["timestamp": "0x6abb8518"] as [String: Any])
        private(set) var asked: [(chainId: Int, method: String, params: [Any])] = []

        func read(_ chainId: Int, _ method: String, _ params: [Any]) -> RpcOutcome {
            asked.append((chainId, method, params))
            switch method {
            case "eth_getTransactionReceipt": return receipt
            case "eth_getBlockByNumber": return block
            default: return .failed(rateLimited: false)
            }
        }

        var methods: [String] { asked.map(\.method) }
    }

    private struct Rig {
        let store: VelaStore
        let defaults: UserDefaults
        let executor: ActivityExecutor
        let chain: Chain
    }

    private func rig() -> Rig {
        let defaults = UserDefaults(suiteName: "vela.tests.receive-time.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let held = HeldTokens()
        // Never booted, offline: nothing here reaches a network.
        let pool = RpcPool(store: store, accounts: accounts, offline: true)
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        let chain = Chain()
        let executor = ActivityExecutor(
            store: store, accounts: accounts, held: held, trust: trust,
            read: { chainId, method, params in chain.read(chainId, method, params) }
        )
        return Rig(store: store, defaults: defaults, executor: executor, chain: chain)
    }

    /// A `receive` as this app stored one before PR 3: no mark.
    private func oldReceive(_ id: String, tx: String, timestamp: Double) -> [String: Any] {
        [
            "id": id, "userOpHash": "", "txHash": tx, "from": peer, "to": me,
            "value": "0.001", "symbol": "xDAI", "decimals": 18, "chainId": 100,
            "timestamp": timestamp, "status": "confirmed", "type": "receive", "usd": "$0.00",
            "logoUrls": ["https://example.invalid/chainlogos/eip155-100.png"],
        ]
    }

    private func sent(_ id: String, timestamp: Double) -> [String: Any] {
        [
            "id": id, "userOpHash": "0x" + String(repeating: "ab", count: 32), "txHash": "0xfeed",
            "from": me, "to": peer, "toName": "Ana", "value": "2.5", "symbol": "USDC", "decimals": 6,
            "chainId": 100, "timestamp": timestamp, "status": "confirmed", "type": "send", "usd": "$2.50",
            "logoUrls": [String](),
        ]
    }

    /// One record as canonical JSON — what "byte-identical" is checked on.
    private func bytes(_ record: [String: Any], without keys: [String] = []) throws -> String {
        var record = record
        for key in keys { record.removeValue(forKey: key) }
        return String(
            decoding: try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys]),
            as: UTF8.self
        )
    }

    private func stored(_ rig: Rig, _ id: String) throws -> [String: Any] {
        try #require(TxRecords.load(store: rig.store).first { $0["id"] as? String == id }, "no record \(id)")
    }

    private func answer(_ rig: Rig, _ operation: [String: Any]) async throws -> [String: Any] {
        try CoreJSON.object(await rig.executor.perform(operation))
    }

    // MARK: - (a) The store rewrite

    /// `write_receive_time` rewrites ONE record: the block's time and the
    /// mark. Every other field of it is byte-identical, every other record
    /// untouched and where it was — and the next `read_tx_store` hands the
    /// core `time_verified: true` for it, and nothing for the others.
    @Test func theRewriteGivesOneRecordItsBlocksTimeAndItsMarkAndTouchesNothingElse() async throws {
        let rig = rig()
        let today = Date().timeIntervalSince1970.rounded(.down)
        TxRecords.writeRecords([
            oldReceive("rx-wrong", tx: "0xa1", timestamp: today),
            sent("tx-1", timestamp: today - 60),
            oldReceive("rx-other", tx: "0xa2", timestamp: today - 120),
        ], store: rig.store)
        let before = TxRecords.load(store: rig.store)
        #expect(before.map { $0["id"] as? String } == ["rx-wrong", "tx-1", "rx-other"])
        #expect(before.allSatisfy { $0["timeVerified"] == nil }, "an old record carries no mark")

        let written = try await answer(rig, [
            "type": "write_receive_time", "id": "rx-wrong", "timestamp_sec": blockTime,
        ])
        #expect(written["type"] as? String == "receive_time_written")
        #expect(written["id"] as? String == "rx-wrong")
        #expect(written["ok"] as? Bool == true)
        #expect(Set(written.keys) == ["type", "id", "ok"])

        let after = TxRecords.load(store: rig.store)
        #expect(after.map { $0["id"] as? String } == ["rx-wrong", "tx-1", "rx-other"],
                "no record moved, none was added, none was lost")
        let repaired = try stored(rig, "rx-wrong")
        #expect((repaired["timestamp"] as? NSNumber)?.doubleValue == blockTime)
        #expect(repaired["timeVerified"] as? Bool == true)
        let rest = try bytes(repaired, without: ["timestamp", "timeVerified"])
        let restBefore = try bytes(before[0], without: ["timestamp"])
        #expect(rest == restBefore, "nothing else of the record changed")
        let send = try (bytes(after[1]), bytes(before[1]))
        #expect(send.0 == send.1, "the send is byte-identical")
        let other = try (bytes(after[2]), bytes(before[2]))
        #expect(other.0 == other.1, "the other receive is byte-identical")

        // The mapping: the mark reaches the core, and only where it is stored.
        let read = try await answer(rig, ["type": "read_tx_store", "read_id": 7])
        let records = try #require(read["records"] as? [[String: Any]])
        let wire = try #require(records.first { $0["id"] as? String == "rx-wrong" })
        #expect(wire["time_verified"] as? Bool == true)
        #expect((wire["timestamp"] as? NSNumber)?.doubleValue == blockTime)
        #expect((wire["day_start_ms"] as? NSNumber)?.doubleValue == TxRecords.dayStartMs(blockTime),
                "it heads its own day, not today's")
        for id in ["tx-1", "rx-other"] {
            let unmarked = try #require(records.first { $0["id"] as? String == id })
            #expect(unmarked["time_verified"] == nil, "\(id): absent stays absent — that is what asks for a repair")
        }
    }

    /// No record has that id: `ok: false`, and not a byte of the store is
    /// written. A missing time is refused the same way.
    @Test func anUnknownIdAnswersNotOkAndWritesNothing() async throws {
        let rig = rig()
        TxRecords.writeRecords([oldReceive("rx-1", tx: "0xa1", timestamp: 1_800_000_000)], store: rig.store)
        let before = rig.defaults.string(forKey: VelaStore.Key.transactionHistory)

        let unknown = try await answer(rig, [
            "type": "write_receive_time", "id": "rx-nobody", "timestamp_sec": blockTime,
        ])
        #expect(unknown["type"] as? String == "receive_time_written")
        #expect(unknown["id"] as? String == "rx-nobody")
        #expect(unknown["ok"] as? Bool == false)

        let timeless = try await answer(rig, ["type": "write_receive_time", "id": "rx-1"])
        #expect(timeless["ok"] as? Bool == false, "no time to write is no write")

        #expect(rig.defaults.string(forKey: VelaStore.Key.transactionHistory) == before)
        let kept = try stored(rig, "rx-1")
        #expect(kept["timeVerified"] == nil)
    }

    /// A later scan's record with the same id never writes over a repaired
    /// time: the merge skips an id it holds, whole.
    @Test func aRepairedTimeIsNotWrittenOverByALaterScan() throws {
        let rig = rig()
        TxRecords.writeRecords([oldReceive("rx-1", tx: "0xa1", timestamp: 1_800_000_000)], store: rig.store)
        #expect(TxRecords.writeReceiveTime(id: "rx-1", timestampSec: blockTime, store: rig.store))
        var again = oldReceive("rx-1", tx: "0xa1", timestamp: 1_800_000_999)
        again["timeVerified"] = true
        #expect(TxRecords.merge([again], store: rig.store) == 0, "the same receipt is not new money")
        let kept = try stored(rig, "rx-1")
        #expect((kept["timestamp"] as? NSNumber)?.doubleValue == blockTime)
    }

    // MARK: - (b) The block's time, through the pool

    /// Receipt → its block → that block's `timestamp`, in seconds: two reads,
    /// the second by the block the first named, and no third.
    @Test func aReceiptsTimeIsReadFromItsReceiptThenItsBlock() async throws {
        let rig = rig()
        let read = try await answer(rig, [
            "type": "read_receive_time", "id": "rx-1", "chain_id": 100, "tx_hash": "0xa1",
        ])
        #expect(read["type"] as? String == "receive_time_read")
        #expect(read["id"] as? String == "rx-1")
        #expect((read["timestamp_sec"] as? NSNumber)?.doubleValue == blockTime)
        #expect(Set(read.keys) == ["type", "id", "timestamp_sec"], "nothing rides beside the block's time")

        #expect(rig.chain.methods == ["eth_getTransactionReceipt", "eth_getBlockByNumber"])
        #expect(rig.chain.asked.allSatisfy { $0.chainId == 100 })
        #expect(rig.chain.asked[0].params.first as? String == "0xa1")
        #expect(rig.chain.asked[1].params.first as? String == "0x2a1b3c")
        #expect(rig.chain.asked[1].params.last as? Bool == false, "the header alone, no transactions")
    }

    /// Whenever either read gave no usable answer the time is `null` — never
    /// the clock's, never a guess — and nothing is asked twice.
    @Test func aTimeNobodyCouldReadIsNullNeverTheClocks() async throws {
        let noReceipt: [(String, RpcOutcome)] = [
            ("the node has no receipt", .ok(nil)),
            ("a null receipt", .ok(NSNull())),
            ("the receipt read failed", .failed(rateLimited: false)),
            ("the node refused the receipt", .rpcError(code: -32000, message: "not found")),
            ("a receipt with no block", .ok(["status": "0x1"] as [String: Any])),
            ("a block number nobody can read", .ok(["blockNumber": "pending"] as [String: Any])),
        ]
        for (name, outcome) in noReceipt {
            let rig = rig()
            rig.chain.receipt = outcome
            let read = try await answer(rig, [
                "type": "read_receive_time", "id": "rx-1", "chain_id": 100, "tx_hash": "0xa1",
            ])
            #expect(read["timestamp_sec"] is NSNull, "\(name): \(read)")
            #expect(read["id"] as? String == "rx-1", "\(name)")
            #expect(Set(read.keys) == ["type", "id", "timestamp_sec"], "\(name)")
            #expect(rig.chain.methods == ["eth_getTransactionReceipt"], "\(name): no block is asked, and nothing twice")
        }

        let noBlock: [(String, RpcOutcome)] = [
            ("the block read failed", .failed(rateLimited: true)),
            ("the node refused the block", .rpcError(code: -32000, message: "header not found")),
            ("no such block", .ok(nil)),
            ("a header with no time", .ok(["number": "0x2a1b3c"] as [String: Any])),
            ("a time nobody can read", .ok(["timestamp": "0xzz"] as [String: Any])),
            ("a time of zero", .ok(["timestamp": "0x0"] as [String: Any])),
        ]
        for (name, outcome) in noBlock {
            let rig = rig()
            rig.chain.block = outcome
            let read = try await answer(rig, [
                "type": "read_receive_time", "id": "rx-1", "chain_id": 100, "tx_hash": "0xa1",
            ])
            #expect(read["timestamp_sec"] is NSNull, "\(name): \(read)")
            #expect(Set(read.keys) == ["type", "id", "timestamp_sec"], "\(name)")
            #expect(rig.chain.methods == ["eth_getTransactionReceipt", "eth_getBlockByNumber"],
                    "\(name): one pass, no retry")
        }

        // A record with nothing to ask about asks nothing.
        let rig = rig()
        let empty = try await answer(rig, ["type": "read_receive_time", "id": "rx-1", "chain_id": 100, "tx_hash": ""])
        #expect(empty["timestamp_sec"] is NSNull)
        #expect(rig.chain.asked.isEmpty)
    }

    // MARK: - (d) + (c) The scan: no clock, and a stored receipt is marked

    /// The real `token_trust` core, polled once up to its block reads.
    private func pollToBlockReads(_ core: TokenTrustCore, latest: Int, log: [String: Any]) throws -> BridgeRun {
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "poll_requested", "address": me])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "read_custom_tokens":
                return CoreJSON.string(["type": "custom_tokens", "tokens": [[String: Any]]()])
            case "rpc_block_number":
                return CoreJSON.string([
                    "type": "block_number", "address": me, "chain_id": 100,
                    "block_hex": "0x" + String(latest, radix: 16),
                ])
            case "rpc_get_logs":
                return CoreJSON.string([
                    "type": "logs", "address": me, "chain_id": 100,
                    "outcome": ["type": "ok", "logs": [log]],
                ])
            case "rpc_get_safe_received_logs":
                return CoreJSON.string([
                    "type": "safe_received_logs", "address": me, "chain_id": 100,
                    "outcome": ["type": "ok", "logs": [[String: Any]]()],
                ])
            default:
                return nil
            }
        }
        return run
    }

    /// The trust executor's answer for a block it could not read carries no
    /// clock, the core withholds the transfer on the strength of it, and
    /// when the block is read the receipt is stored with the BLOCK's time
    /// and the mark — which then spares it the feed's repair.
    @Test func anUnreadBlockIsAnsweredWithNoClockAndItsReceiptIsStoredMarkedOnceRead() async throws {
        let rig = rig()
        let pool = RpcPool(store: rig.store, accounts: AccountStore(defaults: rig.defaults), offline: true)
        let trustExecutor = TokenTrustExecutor(
            store: rig.store, pool: pool, metadata: TokenMetadata(store: rig.store, pool: pool)
        )
        let core = TokenTrustCore()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "held_chains_snapshot", "address": me, "chain_ids": [100],
        ]))
        // The chain's own log of native money paid to the wallet (EIP-7708).
        let topic = { (address: String) in "0x" + String(repeating: "0", count: 24) + address.dropFirst(2) }
        let log: [String: Any] = [
            "address": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "topics": ["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef", topic(peer), topic(me)],
            "data": "0x" + String(repeating: "0", count: 51) + "38d7ea4c68000",
            "transaction_hash": "0xa1", "block_number": "0x3e7", "log_index": "0x1",
        ]

        // (d) Poll one: the block cannot be read (the pool is offline).
        var run = try pollToBlockReads(core, latest: 1_000, log: log)
        let blockRead = try #require(run.operations.last)
        #expect(blockRead["type"] as? String == "rpc_get_block_by_number", "\(run.tags)")
        let unread = try CoreJSON.object(await trustExecutor.perform(blockRead))
        #expect(unread["type"] as? String == "block_timestamp")
        #expect(unread["timestamp_sec"] is NSNull, "an unread block has no time")
        #expect(unread["now_ms"] == nil, "no clock crosses into the machine")
        #expect(Set(unread.keys) == ["type", "address", "chain_id", "block_number", "timestamp_sec"])
        #expect((unread["block_number"] as? NSNumber)?.intValue == 999)
        let readId = try #require(run.effectId(of: "rpc_get_block_by_number"))
        try run.take(core.resolveEffect(effectId: readId, resultJson: CoreJSON.string(unread)))
        let withheld = try CoreJSON.decode(TrustViewWire.self, from: run.view)
        #expect(!withheld.scanning, "the poll ended")
        #expect(withheld.incoming.isEmpty, "a transfer with no block time is withheld — it is given none")
        #expect(rig.executor.ingest(withheld.incoming, address: me) == 0)
        #expect(TxRecords.load(store: rig.store).isEmpty, "nothing is stored under a time nobody read")

        // (c) Poll two: the same block, read this time — the executor's own
        // answer with the block's seconds in it.
        run = try pollToBlockReads(core, latest: 1_001, log: log)
        #expect(run.operations.last?["type"] as? String == "rpc_get_block_by_number", "asked again: \(run.tags)")
        var readNow = unread
        readNow["timestamp_sec"] = blockTime
        let againId = try #require(run.effectId(of: "rpc_get_block_by_number"))
        try run.take(core.resolveEffect(effectId: againId, resultJson: CoreJSON.string(readNow)))
        let landed = try CoreJSON.decode(TrustViewWire.self, from: run.view)
        let transfer = try #require(landed.incoming.first, "the transfer reaches the feed once its block is read")
        #expect(transfer.timestampSec == blockTime)

        #expect(rig.executor.ingest(landed.incoming, address: me) == 1)
        let record = try #require(TxRecords.load(store: rig.store).first)
        #expect(record["type"] as? String == "receive")
        #expect((record["timestamp"] as? NSNumber)?.doubleValue == blockTime, "the block's time, not today's")
        #expect(record["timeVerified"] as? Bool == true)
        #expect(abs(blockTime - Date().timeIntervalSince1970) > 86_400, "the fixture is not today")

        // The mark reaches the feed core, which asks for no repair of it.
        let feed = ActivityFeedCore()
        var feedRun = BridgeRun()
        try feedRun.take(feed.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": me])))
        let storeReadOp = try #require(feedRun.pending.first { $0["type"] as? String == "read_tx_store" })
        let loaded = await rig.executor.perform(storeReadOp)
        let handed = try CoreJSON.object(loaded)["records"] as? [[String: Any]]
        #expect(handed?.first?["time_verified"] as? Bool == true)
        let storeRead = try #require(feedRun.effectId(of: "read_tx_store"))
        try feedRun.take(feed.resolveEffect(effectId: storeRead, resultJson: loaded))
        #expect(!feedRun.pending.contains { $0["type"] as? String == "read_receive_time" },
                "a marked receipt is not asked about: \(feedRun.pending)")
    }

    // MARK: - The repair, end to end

    /// The owner's three rows: stored "today" with no mark, really of
    /// 2026-09-29. The real feed core finds them after a store read, this
    /// executor reads each one's block time and rewrites it, and the feed —
    /// read once more — stands them under their own day.
    @Test func threeReceiptsStoredTodayMoveToTheDayTheirBlocksSay() async throws {
        let rig = rig()
        let now = Date().timeIntervalSince1970.rounded(.down)
        TxRecords.writeRecords([
            oldReceive("rx-1", tx: "0xa1", timestamp: now - 10),
            oldReceive("rx-2", tx: "0xa2", timestamp: now - 20),
            oldReceive("rx-3", tx: "0xa3", timestamp: now - 30),
        ], store: rig.store)

        let feed = ActivityFeedCore()
        var view: [String: Any] = [:]
        var queue: [[String: Any]] = []
        var asked: [String] = []
        func take(_ result: String) throws {
            let object = try CoreJSON.object(result)
            if let next = object["view"] as? [String: Any] { view = next }
            queue += object["effects"] as? [[String: Any]] ?? []
        }
        try take(feed.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": me])))
        var firstDays: Set<Double>?
        var steps = 0
        while !queue.isEmpty, steps < 40 {
            steps += 1
            let effect = queue.removeFirst()
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let operation = effect["operation"] as? [String: Any],
                  let tag = operation["type"] as? String
            else { continue }
            asked.append(tag)
            let reply: String
            switch tag {
            // Hermetic: the scan found nothing new. Everything else is this
            // executor's own answer.
            case "scan_incoming_transfers": reply = CoreJSON.string(["type": "sync_completed", "new_count": 0])
            default: reply = await rig.executor.perform(operation)
            }
            try take(feed.resolveEffect(effectId: id, resultJson: reply))
            if tag == "read_tx_store", firstDays == nil { firstDays = days(view) }
        }

        let today = TxRecords.dayStartMs(now)
        let theirs = TxRecords.dayStartMs(blockTime)
        #expect(firstDays == [today], "before the repair they stand under today: \(String(describing: firstDays))")
        #expect(asked.filter { $0 == "read_receive_time" }.count == 3, "\(asked)")
        #expect(asked.filter { $0 == "write_receive_time" }.count == 3, "\(asked)")
        #expect(asked.filter { $0 == "read_tx_store" }.count == 2, "one more read once a time moved: \(asked)")
        #expect(rig.chain.methods.filter { $0 == "eth_getTransactionReceipt" }.count == 3)
        #expect(rig.chain.methods.filter { $0 == "eth_getBlockByNumber" }.count == 3)

        for record in TxRecords.load(store: rig.store) {
            #expect((record["timestamp"] as? NSNumber)?.doubleValue == blockTime, "\(record["id"] ?? "?")")
            #expect(record["timeVerified"] as? Bool == true)
        }
        #expect(days(view) == [theirs], "the rows stand under 2026-09-29, none under today: \(days(view))")
        let decoded = try CoreJSON.decode(FeedViewWire.self, from: view)
        #expect(decoded.transactions.count == 3)
        #expect(decoded.transactions.allSatisfy { $0.timestamp == blockTime })
        #expect(decoded.newItemId == nil && decoded.toast == nil, "a repair celebrates nothing")
    }

    /// The local-midnight keys of the feed's day headers.
    private func days(_ view: [String: Any]) -> Set<Double> {
        Set((view["rows"] as? [[String: Any]] ?? []).compactMap { row in
            row["type"] as? String == "header" ? (row["day_start_ms"] as? NSNumber)?.doubleValue : nil
        })
    }
}
