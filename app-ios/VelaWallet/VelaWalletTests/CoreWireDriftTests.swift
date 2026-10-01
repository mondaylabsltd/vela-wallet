//
//  CoreWireDriftTests.swift
//  VelaWalletTests
//
//  The guard that replaces a generator.
//
//  Web reads 300-odd `ts-rs`-generated types and CI fails on drift; desktop
//  reads the Rust structs directly and drift is a compile error. Swift gets
//  JSON, and `ts-rs` has no Swift backend — so the mirrors in `*Wire.swift` are
//  hand-written, and nothing but this file notices when `vela-core` renames a
//  field.
//
//  The check is the cheapest one that actually works: drive the REAL core
//  through the REAL bridge, take the view it emits, and decode it. A renamed or
//  retyped field fails here with a `DecodingError` naming it, which is the same
//  signal `gen-core-types.mjs --check` gives the web client — one test per
//  machine, no new toolchain.
//
//  It is deliberately NOT a snapshot of the JSON. A snapshot fails on every
//  additive change, which trains people to update it without reading it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct CoreWireDriftTests {

    /// The bridge answers in TWO shapes, and confusing them is a real trap —
    /// this test caught it on its first run. `dispatch` and `resolveEffect`
    /// return `{ view, effects, cancelled_effect_ids }`; `view()` returns the
    /// view **bare**.
    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// `ContactsView` decodes, including through the fields only a populated
    /// book reaches.
    ///
    /// The `account_switched` event is what a real screen sends first, so this
    /// exercises the same path the app does rather than a state nothing
    /// produces.
    @Test func contactsViewDecodes() throws {
        let core = ContactsCore()

        let initial = try CoreJSON.decode(ContactsViewWire.self, from: try CoreJSON.object(core.view()))
        // `loaded` is false before the stores are read — a real state the
        // waiting surface renders from, not an absence.
        #expect(!initial.loaded)
        #expect(initial.contacts.isEmpty)

        let after = try CoreJSON.decode(
            ContactsViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string([
                "type": "account_switched",
                "my_address": "0x88cca07a5d3e0b1eb5f7c0f2b1f3f4a5b6c76894",
            ])))
        )
        // Still unloaded: the core has ASKED for the stores and is waiting for
        // the shell's answer, which this test deliberately does not give.
        #expect(!after.loaded)
    }

    /// `NetView` decodes — including the two tagged unions Swift cannot
    /// synthesise, `NetProbeHealth` and `NetServiceHealth`.
    ///
    /// The `started` event is what the settings route sends first, so this is
    /// the same path the screen takes.
    @Test func networkAdminViewDecodes() throws {
        let core = NetworkAdminCore()

        let initial = try CoreJSON.decode(NetViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.loaded)
        #expect(initial.wizard.phase == .idle)
        #expect(!initial.wizard.canAdd, "the add gate must be shut before anything is known")

        let after = try CoreJSON.decode(
            NetViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string(["type": "started"])))
        )
        #expect(!after.loaded, "the core has asked for the stores and is waiting")
    }

    /// The operations `network_admin` asks for on its first event are ones this
    /// build can perform.
    @Test func networkAdminAsksOnlyForOperationsThisBuildHandles() throws {
        let core = NetworkAdminCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "started"])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "`started` must ask the shell for the stores")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                NetworkAdminExecutor.operations.contains(tag),
                "the core asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// `RpcPoolView` decodes, and the pool asks for nothing this build cannot
    /// perform.
    ///
    /// The pool is the one machine whose unhandled operation would not merely
    /// blank a screen — every read in the app queues behind it, so a tag it
    /// cannot answer stalls the whole wallet.
    @Test func rpcPoolViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = RpcPoolCore()

        let initial = try CoreJSON.decode(RpcPoolViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.failedChains.isEmpty)
        #expect(initial.banned.isEmpty)

        // A call on a cold pool must ask for its config first.
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "call_requested",
            "call_id": "t1", "chain_id": 100, "kind": "rpc",
            "method": "eth_getBalance", "now_ms": 1_700_000_000_000,
        ])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a cold pool must ask for its endpoints")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                RpcPool.operations.contains(tag),
                "the pool asks for `\(tag)`, which this build cannot perform"
            )
        }
    }

    /// `FeedView` decodes, including the tagged `FeedRow` union Swift cannot
    /// synthesise — and the feed machine asks for nothing this build cannot do.
    ///
    /// `account_switched` is what the home sends first, and it is what makes
    /// the machine read the store and scan, so this is the app's own path.
    @Test func activityFeedViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = ActivityFeedCore()

        let initial = try CoreJSON.decode(FeedViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.rows.isEmpty)
        #expect(initial.toast == nil)

        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched",
            "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ])))
        _ = try CoreJSON.decode(FeedViewWire.self, from: result["view"] as? [String: Any] ?? [:])
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a switched account must read the store")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                ActivityExecutor.operations.contains(tag),
                "the feed asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// `TrustView` decodes, and the scan asks only for operations this build
    /// performs.
    ///
    /// The anti-scam core is the one machine where an unanswered operation is
    /// not merely a stall: its metadata gate would never be met, and a scan
    /// that never finishes is a wallet that never notices it was paid.
    @Test func tokenTrustViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = TokenTrustCore()

        let initial = try CoreJSON.decode(TrustViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.scanning)
        #expect(initial.incoming.isEmpty)

        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "held_chains_snapshot", "address": address, "chain_ids": [100],
        ]))
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "poll_requested", "address": address,
        ])))
        let view = try CoreJSON.decode(
            TrustViewWire.self, from: result["view"] as? [String: Any] ?? [:]
        )
        #expect(view.scanning, "a requested poll must announce itself as scanning")
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a poll must ask the chain something")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                TokenTrustExecutor.operations.contains(tag),
                "token_trust asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// The bridge's three methods behave the way `CoreDriver` assumes.
    ///
    /// Property 2 of the driver's contract: resolving an effect id the bridge
    /// does not know means the answer outlived the question, which is expected
    /// and must not throw — an executor answering a cancelled effect would
    /// otherwise crash the app.
    @Test func anAnswerThatOutlivedItsQuestionIsNotAnError() throws {
        let core = ContactsCore()
        let reply = try core.resolveEffect(
            effectId: 9_999,
            resultJson: CoreJSON.string(["type": "written"])
        )
        _ = try CoreJSON.decode(ContactsViewWire.self, from: try view(from: reply))
    }

    /// A machine that asks for something must be asked in the shape the
    /// executor switches on: an internally tagged object with a `type`.
    @Test func operationsArriveInternallyTagged() throws {
        let core = ContactsCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched", "my_address": NSNull(),
        ])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "account_switched must ask the shell for the stores")
        for effect in effects {
            let operation = effect["operation"] as? [String: Any] ?? [:]
            let tag = operation["type"] as? String ?? ""
            #expect(!tag.isEmpty, "an untagged operation cannot be dispatched")
            #expect(
                ContactsExecutor.operations.contains(tag),
                "the core asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }
}

// MARK: - Spec 082 (T103): every new variant, as the core emits it

/// Answers a bridge's effects one at a time until `answer` declines one (it
/// returns `nil`) or nothing is left — the order a `CoreDriver` would, minus
/// the concurrency. What it saw is returned for the assertions.
@MainActor
struct BridgeRun {
    private(set) var operations: [[String: Any]] = []
    private(set) var view: [String: Any] = [:]
    private var queue: [[String: Any]] = []

    var tags: [String] { operations.compactMap { $0["type"] as? String } }

    /// The operations still queued, unanswered.
    var pending: [[String: Any]] { queue.compactMap { $0["operation"] as? [String: Any] } }

    /// The effect id of the first operation of `tag` seen or queued — to
    /// answer one the drain left unanswered.
    func effectId(of tag: String) -> UInt64? {
        (asked + queue).first { ($0["operation"] as? [String: Any])?["type"] as? String == tag }
            .flatMap { ($0["id"] as? NSNumber)?.uint64Value }
    }
    private var asked: [[String: Any]] = []

    mutating func take(_ result: String) throws {
        let object = try CoreJSON.object(result)
        if let view = object["view"] as? [String: Any] { self.view = view }
        queue += object["effects"] as? [[String: Any]] ?? []
    }

    /// Runs until `answer` returns `nil` for an operation (left unanswered)
    /// or the queue is empty.
    mutating func drain(
        _ bridge: CoreBridge, limit: Int = 60, answer: ([String: Any]) -> String?
    ) throws {
        var steps = 0
        while !queue.isEmpty, steps < limit {
            steps += 1
            let effect = queue.removeFirst()
            asked.append(effect)
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let operation = effect["operation"] as? [String: Any]
            else { continue }
            operations.append(operation)
            guard let reply = answer(operation) else { return }
            try take(bridge.resolveEffect(effectId: id, resultJson: reply))
        }
    }
}

@MainActor
struct CoreWire082Tests {

    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let now = 1_757_000_000_000.0
    private let opHash = "0x" + String(repeating: "ab", count: 32)

    /// `SignView.phase` walks preparing → awaiting signature → submitting on
    /// the ceremony events, and a lost reply's `op_submitted` shows as
    /// `pendingOpMaybeSent` and on the tracker handoff with its head block.
    @Test func theSignViewCarriesItsPhaseAndAMaybeSentHandoff() throws {
        let core = SignRequestCore()
        let idle = try CoreJSON.decode(SignViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(idle.phase == .idle)
        #expect(!idle.pendingOpMaybeSent)

        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "networks_changed", "chain_ids": [100]]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": me, "credential_id": "cred-1"]],
            "active_index": 0,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived", "id": "req-1", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            "origin": "http://192.168.50.9:8137", "transport_id": "tab-1",
            "dedicated_transport": true, "per_request_chain": 100, "dapp": NSNull(),
            "granted_address": me, "requested_address": NSNull(), "request_ts_ms": NSNull(),
            "now_ms": now,
        ]))

        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "approve_tapped",
            "opts": [
                "max_fee_per_gas": NSNull(), "bundler_cost_wei": NSNull(), "gas_fee_token": NSNull(),
                "quoted_fee": ["amount": "1000", "recipient": me, "tier": "fast"],
                "fee_collector": NSNull(), "params_override_json": NSNull(), "intent": NSNull(),
                "unlimited_approved": false,
            ],
        ])))
        #expect(try CoreJSON.decode(SignViewWire.self, from: run.view).phase == .preparing)
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "check_bundler_funding": return CoreJSON.string(["type": "pre_check", "funding": NSNull()])
            case "attempt_sponsorship":
                return CoreJSON.string(["type": "sponsorship", "outcome": ["type": "denied", "reason": NSNull()]])
            default: return nil
            }
        }
        #expect(run.tags.last == "sign_and_submit")
        #expect(try CoreJSON.decode(SignViewWire.self, from: run.view).phase == .preparing,
                "the network work before the prompt is never 'waiting for your signature'")

        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_started", "id": "req-1"])))
        #expect(try CoreJSON.decode(SignViewWire.self, from: run.view).phase == .awaitingSignature)
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_done", "id": "req-1"])))
        #expect(try CoreJSON.decode(SignViewWire.self, from: run.view).phase == .submitting)

        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_submitted", "id": "req-1", "user_op_hash": opHash, "now_ms": now,
            "maybe_sent": true, "submit_block": 48_000_000,
        ])))
        let sent = try CoreJSON.decode(SignViewWire.self, from: run.view)
        #expect(sent.pendingOpMaybeSent)
        #expect(sent.pendingOpHash == opHash)
        #expect(sent.trackerHandoff?.maybeSent == true)
        #expect(sent.trackerHandoff?.submitBlock == 48_000_000)
    }

    /// A restored may-have-been-sent record is followed as `maybe_sent`, and
    /// the tracker reads the chain for the op's own event (ruling 8).
    @Test func aMaybeSentTrackerEntryDecodesWithItsFindEvent() throws {
        let core = TxTrackerCore()
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "app_resumed"])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "now": return CoreJSON.string(["type": "clock", "now_ms": now])
            case "load_pending_txs":
                return CoreJSON.string([
                    "type": "records_loaded", "now_ms": now,
                    "records": [[
                        "record_id": "dapp-1-tx", "user_op_hash": opHash, "chain_id": 100,
                        // 30 s ago: inside the wait window, past the first
                        // status interval, when the chain is read too.
                        "submitted_at_ms": now - 30_000, "maybe_sent": true, "submit_block": 48_000_000,
                    ]],
                ])
            case "poll_receipt":
                return CoreJSON.string(["type": "receipt_pending", "user_op_hash": opHash, "now_ms": now])
            case "poll_status":
                return CoreJSON.string(["type": "status_unavailable", "user_op_hash": opHash, "now_ms": now])
            default: return nil
            }
        }
        let find = try #require(run.operations.first { $0["type"] as? String == "find_op_event" })
        // The head is not known yet: the first read asks for it alone.
        #expect(find["from_block"] is NSNull)
        #expect((find["user_op_hash"] as? String) == opHash)
        #expect((find["topic0"] as? String)?.hasPrefix("0x") == true)

        let view = try CoreJSON.decode(TrackViewWire.self, from: run.view)
        let entry = try #require(view.entry(userOpHash: opHash))
        #expect(entry.outcome == "maybe_sent")
        #expect(entry.status == "pending")
        #expect(entry.relayTxHash == nil)
    }

    /// The pool's view carries the chains a first pass could not reach.
    @Test func thePoolViewCarriesUnreachedChains() throws {
        let initial = try CoreJSON.decode(RpcPoolViewWire.self, from: try CoreJSON.object(RpcPoolCore().view()))
        #expect(initial.unreachedChains.isEmpty)
    }

    /// Empty calldata is a plain send: the core's card, exact, and a zero
    /// value marked as such.
    @Test func aPlainSendDecodesWithItsCard() throws {
        func resolve(_ value: String) throws -> ClearSigningViewWire {
            let core = ClearSigningCore()
            let result = try core.dispatch(eventJson: CoreJSON.string([
                "type": "resolve_transaction",
                "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": value,
                "chain_id": 100, "locale": SigningController.defaultLocale,
            ]))
            return try CoreJSON.decode(ClearSigningViewWire.self, from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:])
        }
        let dust = try resolve("0x38d7ea4c68000")
        #expect(dust.surface == .plainSend)
        #expect(dust.plainSend?.amount == "0.001")
        #expect(dust.plainSend?.noValue == false)
        #expect(dust.plainSend?.to == (try checksumAddress(addressHex: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141")))

        let zero = try resolve("0x0")
        #expect(zero.surface == .plainSend)
        #expect(zero.plainSend?.noValue == true)
        #expect(zero.plainSend?.amount == "0")
    }

    /// A dApp transaction is a feed row with its kind, status and site, and
    /// the feed names History's empty line.
    @Test func aDappRowDecodesWithKindStatusAndSite() throws {
        let core = ActivityFeedCore()
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": me])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "read_tx_store":
                return CoreJSON.string([
                    "type": "store_loaded", "now_ms": now,
                    "read_id": (operation["read_id"] as? NSNumber)?.intValue ?? 0,
                    "records": [[
                        "id": "dapp-1-tx", "user_op_hash": opHash, "tx_hash": "", "from": me,
                        "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "to_name": NSNull(),
                        "value": "0x38d7ea4c68000", "symbol": "xDAI", "decimals": 18, "logo_urls": NSNull(),
                        "chain_id": 100, "timestamp": now / 1000, "day_start_ms": now,
                        "status": "pending", "kind": "dapp_tx", "usd": NSNull(),
                        "dapp_url": "http://192.168.50.9:8137",
                    ]],
                ])
            case "scan_incoming_transfers": return CoreJSON.string(["type": "sync_completed", "new_count": 0])
            default: return nil
            }
        }
        let view = try CoreJSON.decode(FeedViewWire.self, from: run.view)
        let item = try #require(view.rows.compactMap { row -> FeedItemWire? in
            if case .item(let item) = row { return item } else { return nil }
        }.first)
        #expect(item.kind == .dappTx)
        #expect(item.status == .pending)
        #expect(item.site == "192.168.50.9:8137")
        #expect(view.transactions.first?.dappUrl == "http://192.168.50.9:8137")
        #expect(view.historyEmptyKey == "history.emptyTitle")
        #expect(view.homeEmptyKey == "home.emptyNoActivity")
    }

    /// One variant this build has never heard of must not fail the view it
    /// sits in: each reads as its most cautious known neighbour.
    @Test func anUnknownVariantDoesNotFailTheView() throws {
        let sign = try CoreJSON.decoder.decode(SignPhaseWire.self, from: Data(#""a_future_phase""#.utf8))
        #expect(sign == .preparing)
        let surface = try CoreJSON.decoder.decode(ClearSurface.self, from: Data(#""a_future_surface""#.utf8))
        #expect(surface == .blindTransaction)
        let kind = try CoreJSON.decoder.decode(FeedTxKindWire.self, from: Data(#""a_future_kind""#.utf8))
        #expect(kind == .send)
        let status = try CoreJSON.decoder.decode(FeedTxStatusWire.self, from: Data(#""a_future_status""#.utf8))
        #expect(status == .pending)
        let tracker = try CoreJSON.decode(TrackViewWire.self, from: [
            "entries": [[
                "user_op_hash": opHash, "chain_id": 100, "record_ids": ["r"],
                "status": "a_future_status", "tx_hash": NSNull(), "polling": true,
                "submitted_at_ms": now, "outcome": "a_future_outcome", "relay_tx_hash": NSNull(),
            ]],
        ])
        #expect(tracker.entries.first?.status == "a_future_status")
    }
}


// MARK: - Spec 082 round 2 (T236): every new variant, as the core emits it

@MainActor
struct CoreWireRoundTwoTests {

    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let now = 1_757_000_000_000.0
    private let opHash = "0x" + String(repeating: "ab", count: 32)

    /// A `sign_request` core with one transaction request in flight, up to
    /// its `sign_and_submit` (left unanswered: the executor's).
    private func submitting() throws -> (SignRequestCore, BridgeRun) {
        let core = SignRequestCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "networks_changed", "chain_ids": [100]]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": me, "credential_id": "cred-1"]],
            "active_index": 0,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived", "id": "req-1", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            "origin": "http://192.168.50.9:8137", "transport_id": "tab-1",
            "dedicated_transport": true, "per_request_chain": 100, "dapp": NSNull(),
            "granted_address": me, "requested_address": NSNull(), "request_ts_ms": NSNull(),
            "now_ms": now,
        ]))
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "approve_tapped",
            "opts": [
                "max_fee_per_gas": NSNull(), "bundler_cost_wei": NSNull(), "gas_fee_token": NSNull(),
                "quoted_fee": ["amount": "1000", "recipient": me, "tier": "fast"],
                "fee_collector": NSNull(), "params_override_json": NSNull(), "intent": NSNull(),
                "unlimited_approved": false,
            ],
        ])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "check_bundler_funding": return CoreJSON.string(["type": "pre_check", "funding": NSNull()])
            case "attempt_sponsorship":
                return CoreJSON.string(["type": "sponsorship", "outcome": ["type": "denied", "reason": NSNull()]])
            default: return nil
            }
        }
        #expect(run.tags.last == "sign_and_submit")
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_started", "id": "req-1"])))
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_done", "id": "req-1"])))
        return (core, run)
    }

    /// RJ1: `op_signed` → the record ahead (`persist_record`, may have been
    /// sent) and its hand-off → on `record_persisted`, `clear_to_post` — every
    /// tag one `SignExecutor` handles, and the view decodes throughout. Then
    /// the relay's yes: `update_record{admitted}` and an admitted hand-off.
    @Test func theWriteAheadAndItsAcceptanceDecode() throws {
        let (core, started) = try submitting()
        var run = started
        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_signed", "id": "req-1", "user_op_hash": opHash,
            "submit_block": 48_000_000, "now_ms": now,
        ])))
        let ahead = try CoreJSON.decode(SignViewWire.self, from: run.view)
        #expect(ahead.trackerHandoff?.maybeSent == true)
        #expect(ahead.trackerHandoff?.admitted == false)
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "persist_record": return CoreJSON.string(["type": "record_persisted"])
            default: return nil
            }
        }
        let persist = try #require(run.operations.first { $0["type"] as? String == "persist_record" })
        #expect((persist["record"] as? [String: Any])?["maybe_sent"] as? Bool == true)
        #expect(run.tags.last == "clear_to_post", "\(run.tags)")
        let clear = try #require(run.operations.last)
        #expect(clear["user_op_hash"] as? String == opHash)
        #expect(clear["id"] as? String == "req-1")
        for tag in run.tags {
            #expect(SignExecutor.operations.contains(tag), "the core asks for `\(tag)`")
        }

        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_submitted", "id": "req-1", "user_op_hash": opHash, "now_ms": now,
            "maybe_sent": false, "submit_block": 48_000_000,
        ])))
        let accepted = try CoreJSON.decode(SignViewWire.self, from: run.view)
        #expect(accepted.trackerHandoff?.admitted == true)
        #expect(accepted.trackerHandoff?.maybeSent == false)
        #expect(!accepted.pendingOpMaybeSent)
        try run.drain(core) { operation in
            operation["type"] as? String == "update_record" ? CoreJSON.string(["type": "record_updated"]) : nil
        }
        let update = try #require(run.operations.last { $0["type"] as? String == "update_record" })
        #expect((update["close"] as? [String: Any])?["type"] as? String == "admitted")
    }

    /// RJ1 + RJ3: proven not sent after the write-ahead — `delete_record`
    /// and a `tracker_withdraw` in the view; a relay refusal carries
    /// `failure_refused`. Both decode.
    @Test func aRefusalAfterTheWriteAheadWithdrawsAndSaysRefused() throws {
        let (core, started) = try submitting()
        var run = started
        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_signed", "id": "req-1", "user_op_hash": opHash,
            "submit_block": NSNull(), "now_ms": now,
        ])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "persist_record": return CoreJSON.string(["type": "record_persisted"])
            default: return nil
            }
        }
        // The executor's own answer to `sign_and_submit`, a refusal.
        let submitId = try #require(run.effectId(of: "sign_and_submit"))
        try run.take(core.resolveEffect(effectId: submitId, resultJson: CoreJSON.string([
            "type": "submit",
            "outcome": SignExecutor.failed("AA23 reverted", refused: true),
            "now_ms": now,
        ])))
        let view = try CoreJSON.decode(SignViewWire.self, from: run.view)
        #expect(view.failureRefused, "the sheet reads the refusal")
        #expect(view.error?.kind == .submitFailed)
        let withdraw = try #require(view.trackerWithdraw)
        #expect(withdraw.userOpHash == opHash)
        #expect(!withdraw.recordIds.isEmpty)
        #expect(run.pending.contains { $0["type"] as? String == "delete_record" }, "\(run.pending)")
        #expect(SignExecutor.operations.contains("delete_record"))
        // The page is answered the core's fixed refusal, once.
        let answer = try #require(run.pending.first { $0["type"] as? String == "send_response" })
        let payload = try #require(answer["payload"] as? [String: Any])
        #expect(payload["message"] as? String == userOpRefusedDappDetail())
        #expect((payload["code"] as? NSNumber)?.intValue == -32603)
    }

    /// RJ4: the tracker's `Refused` ending decodes, and `op_tracked` is an
    /// event the core takes (the answer follows the tracker).
    @Test func theRefusedEndingAndTheTrackedEventDecode() throws {
        let still = try #require(SigningAftercare.of(
            method: "eth_sendTransaction", chainId: 100,
            payload: ["type": "ok", "result": opHash], submittedUserOp: opHash
        ))
        let entry = TrackEntryWire(
            userOpHash: opHash, chainId: 100, recordIds: ["r"], status: "rejected", txHash: nil,
            polling: false, submittedAtMs: now, outcome: "final"
        )
        #expect(still.state(track: entry) == .refused)
        let core = SignRequestCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_tracked", "user_op_hash": opHash, "status": "confirmed",
            "tx_hash": "0x" + String(repeating: "7e", count: 32), "now_ms": now,
        ])))
        _ = try CoreJSON.decode(SignViewWire.self, from: result["view"] as? [String: Any] ?? [:])
    }

    /// RJ16: a dApp row whose call is a contract call names the contract,
    /// `contract`; a token `transfer` names its recipient. The record's
    /// `call_data` decodes back.
    @Test func theFeedCarriesCallDataAndTheCounterpartyRole() throws {
        func row(callData: String?) throws -> (FeedItemWire, FeedTxRecordWire?) {
            let core = ActivityFeedCore()
            var run = BridgeRun()
            try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": me])))
            try run.drain(core) { operation in
                switch operation["type"] as? String {
                case "read_tx_store":
                    return CoreJSON.string([
                        "type": "store_loaded", "now_ms": now,
                        "read_id": (operation["read_id"] as? NSNumber)?.intValue ?? 0,
                        "records": [[
                            "id": "dapp-1-tx", "user_op_hash": opHash, "tx_hash": opHash, "from": me,
                            "to": "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83", "to_name": NSNull(),
                            "value": "0x0", "symbol": "xDAI", "decimals": 18, "logo_urls": NSNull(),
                            "chain_id": 100, "timestamp": now / 1000, "day_start_ms": now,
                            "status": "failed", "kind": "dapp_tx", "usd": NSNull(),
                            "dapp_origin": "http://192.168.50.9:8137",
                            "call_data": callData.map { $0 as Any } ?? NSNull(),
                        ]],
                    ])
                case "scan_incoming_transfers": return CoreJSON.string(["type": "sync_completed", "new_count": 0])
                default: return nil
                }
            }
            let view = try CoreJSON.decode(FeedViewWire.self, from: run.view)
            let item = try #require(view.rows.compactMap { row -> FeedItemWire? in
                if case .item(let item) = row { return item } else { return nil }
            }.first)
            return (item, view.transactions.first)
        }
        let recipient = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
        let transfer = "0xa9059cbb" + String(repeating: "0", count: 24) + String(recipient.dropFirst(2))
            + String(repeating: "0", count: 60) + "03e8"
        let (paid, record) = try row(callData: transfer)
        #expect(paid.counterpartyRole == .recipient)
        #expect(paid.counterparty?.lowercased() == recipient)
        #expect(record?.callData == transfer)
        #expect(paid.txHash == nil, "an op hash is never a tx hash")

        let (called, _) = try row(callData: "0x095ea7b3" + String(repeating: "0", count: 128))
        #expect(called.counterpartyRole == .contract)
        #expect(called.counterparty?.lowercased() == "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83")

        let unknown = try CoreJSON.decoder.decode(FeedCounterpartyRoleWire.self, from: Data(#""a_future_role""#.utf8))
        #expect(unknown == .recipient)
    }
}
