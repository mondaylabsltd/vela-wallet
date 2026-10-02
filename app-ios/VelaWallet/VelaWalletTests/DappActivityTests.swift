//
//  DappActivityTests.swift
//  VelaWalletTests
//
//  Spec 093 — every dApp interaction in Activity, the iOS wiring.
//
//  One test per wiring point, each through the REAL core where the core is
//  in the loop: what the approve copies (`record_intent`, `token_meta`, the
//  sheet's judged balance changes), what `persist_record` leaves on the disk
//  (`SignExecutor.recordRow`), what the store hands back (`TxRecords.toWire`),
//  how the feed's new fields decode (`ActivityWire`), and how a row and a
//  detail say them (`WalletLive`, `FlowsLive`). The words are the corpus's;
//  nothing here decides what a request was.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// The three interactions the spec's screenshots show — a swap on Uniswap, a
/// Permit2 permit, a Sign-In with Ethereum — as the core's `SignRecord`s, the
/// way `persist_record` hands them to `SignExecutor`.
@MainActor
enum DappActivityFixture {
    static let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    static let router = "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad"
    static let permit2 = "0x000000000022d473030f116ddee9f6b43ac78ba3"
    static let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    static let txHash = "0x" + String(repeating: "7e", count: 32)
    static let opHash = "0x" + String(repeating: "ab", count: 32)
    static let site = "https://app.uniswap.org"

    static let swapRequest = #"[{"to":"\#(router)","value":"0x0","data":"0x3593564c"}]"#
    static let permitRequest = #"["\#(me)","{\"primaryType\":\"PermitSingle\",\"domain\":{\"name\":\"Permit2\",\"chainId\":1}}"]"#

    static func swap(nowMs: Double) -> [String: Any] {
        [
            "record_id": "dapp-swap-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": swapRequest, "result": txHash, "from": me, "chain_id": 1,
            "now_ms": nowMs, "status": "confirmed", "user_op_hash": opHash,
            "dapp_origin": site, "dapp_url": site, "intent": "Swap",
            "summary": ["action": "call", "calls": 1, "contract": router],
            "balance_changes": [
                ["type": "erc20_trusted", "token": usdc, "delta": "-100000000",
                 "symbol": "USDC", "decimals": 6, "in_trusted_set": true],
                ["type": "native", "delta": "30000000000000000"],
            ],
            "stored_request": swapRequest, "request_truncated": false,
        ]
    }

    static func permit(nowMs: Double) -> [String: Any] {
        [
            "record_id": "dapp-permit-sig", "kind": "sign_typed_data", "method": "eth_signTypedData_v4",
            "params_json": permitRequest, "result": "", "from": me, "chain_id": 1,
            "now_ms": nowMs, "status": "confirmed", "user_op_hash": "",
            "dapp_origin": site, "dapp_url": site,
            "summary": [
                "action": "permit", "contract": permit2, "spender": router, "token": usdc,
                "symbol": "USDC", "decimals": 6, "unlimited": true, "primary_type": "PermitSingle",
            ],
            "stored_request": permitRequest, "request_truncated": false,
        ]
    }

    static func signIn(nowMs: Double) -> [String: Any] {
        [
            "record_id": "dapp-siwe-msg", "kind": "sign_message", "method": "personal_sign",
            "params_json": #"["0x6170702e756e69737761702e6f7267","\#(me)"]"#, "result": "", "from": me,
            "chain_id": 1, "now_ms": nowMs, "status": "confirmed", "user_op_hash": "",
            "dapp_origin": site, "dapp_url": site,
            "summary": ["action": "sign_in", "signin_domain": "app.uniswap.org"],
            "stored_request": #"["0x6170702e756e69737761702e6f7267","\#(me)"]"#, "request_truncated": false,
        ]
    }

    /// The three, stored as this app stores them.
    static func stored(nowMs: Double) -> [[String: Any]] {
        [swap(nowMs: nowMs), permit(nowMs: nowMs - 60_000), signIn(nowMs: nowMs - 120_000)]
            .map { SignExecutor.recordRow($0, nativeSymbol: "ETH") }
    }
}

@MainActor
struct DappActivityTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let now = Date().timeIntervalSince1970 * 1000
    private typealias F = DappActivityFixture

    // MARK: - Helpers

    /// The real feed core, fed stored rows through `TxRecords.toWire`.
    private func feed(_ rows: [[String: Any]]) throws -> FeedViewWire {
        let core = ActivityFeedCore()
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": F.me])))
        let wire = rows.compactMap(TxRecords.toWire)
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "read_tx_store":
                return CoreJSON.string([
                    "type": "store_loaded", "now_ms": now,
                    "read_id": (operation["read_id"] as? NSNumber)?.intValue ?? 0,
                    "records": wire,
                ])
            case "scan_incoming_transfers": return CoreJSON.string(["type": "sync_completed", "new_count": 0])
            default: return nil
            }
        }
        return try CoreJSON.decode(FeedViewWire.self, from: run.view)
    }

    private func items(_ view: FeedViewWire) -> [FeedItemWire] { FlowsLive.items(view) }

    private func item(_ id: String, in view: FeedViewWire) throws -> FeedItemWire {
        try #require(items(view).first { $0.id == id })
    }

    private var drawnDetail: TxDetailModel {
        get throws {
            guard case .txDetail(let model)? = WalletFlowFixtures.build(.a3, loc: zh).sheet else {
                throw Missing()
            }
            return model
        }
    }
    private struct Missing: Error {}

    // MARK: - Approve (SigningController.approveOpts)

    /// The verb the record keeps is the core's `record_intent` — never the
    /// sheet's result's intent — and the guard's token and the drawn balance
    /// changes are copied verbatim.
    @Test func theApproveCopiesRecordIntentTokenMetaAndTheDrawnChanges() throws {
        // A plain send, read by the real core: no decoded result, and the
        // record still says "Send".
        let core = ClearSigningCore()
        let result = try core.dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "data": "0x", "value": "0x1",
            "chain_id": 100, "locale": SigningController.defaultLocale,
        ]))
        let clear = try CoreJSON.decode(
            ClearSigningViewWire.self, from: CoreJSON.object(result)["view"] as? [String: Any] ?? [:]
        )
        #expect(clear.result == nil)
        #expect(clear.recordIntent == "Send")

        let judgments = try CoreJSON.decoder.decode([TrustSimJudgmentWire].self, from: Data(#"""
        [{"type":"erc20_trusted","token":"\#(F.usdc)","delta":"-100000000","symbol":"USDC","decimals":6,"in_trusted_set":true},
         {"type":"native","delta":"30000000000000000"},
         {"type":"erc20_unverified","token":null,"delta":"5"}]
        """#.utf8))
        let opts = SigningController.approveOpts(
            fee: nil, clear: clear, guard: .empty, balanceChanges: judgments
        )
        #expect(opts["intent"] as? String == "Send")
        let meta = try #require(opts["token_meta"] as? [String: Any])
        #expect(meta["symbol"] as? String == GuardViewWire.empty.meta.symbol)
        #expect(meta["decimals"] as? Int == GuardViewWire.empty.meta.decimals)
        #expect(meta["verified"] as? Bool == false)
        #expect(meta["loading"] as? Bool == false)
        let changes = try #require(opts["balance_changes"] as? [[String: Any]])
        #expect(changes.count == 3)
        #expect(changes[0]["in_trusted_set"] as? Bool == true, "the trust bit survives the round trip")
        #expect(changes[1]["type"] as? String == "native")
        #expect(changes[2]["token"] is NSNull)

        // Nothing drawn, nothing recorded.
        let none = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect(none["intent"] is NSNull)
        #expect(none["balance_changes"] is NSNull)
    }

    /// The changes the record keeps are the ones the sheet DREW: this
    /// request's simulation answered, its judgments ready, and some — never
    /// a notice's request, a previous request's, or "nothing moves".
    @Test func theRecordKeepsOnlyTheChangesTheSheetDrew() throws {
        let judged = try CoreJSON.decoder.decode(TrustSimViewWire.self, from: Data(#"""
        {"ready":true,"judgments":[{"type":"native","delta":"-1"}]}
        """#.utf8))
        #expect(SigningController.drawnChanges(judged, simulation: .answered)?.count == 1)
        #expect(SigningController.drawnChanges(judged, simulation: .pending) == nil)
        #expect(SigningController.drawnChanges(
            judged, simulation: .notice(risk: "danger", key: "k", reason: nil)
        ) == nil)
        let notReady = try CoreJSON.decoder.decode(TrustSimViewWire.self, from: Data(#"""
        {"ready":false,"judgments":[{"type":"native","delta":"-1"}]}
        """#.utf8))
        #expect(SigningController.drawnChanges(notReady, simulation: .answered) == nil)
        let empty = try CoreJSON.decoder.decode(TrustSimViewWire.self, from: Data(#"{"ready":true,"judgments":[]}"#.utf8))
        #expect(SigningController.drawnChanges(empty, simulation: .answered) == nil)
        #expect(SigningController.drawnChanges(nil, simulation: .answered) == nil)
    }

    // MARK: - Persist (SignExecutor.recordRow), through the real signing core

    /// The real `sign_request` core, given this app's approve opts for a
    /// request over 8 KB: its `persist_record` carries the summary read from
    /// the WHOLE request, the request clipped by the core, the recorded verb
    /// and the drawn changes — and `recordRow` stores each verbatim, which
    /// `toWire` hands back.
    @Test func aLongRequestIsSummarisedWholeAndStoredAsTheCoreCutIt() throws {
        let data = "0x3593564c" + String(repeating: "ab", count: 6_000)
        let params = #"[{"to":"\#(F.router)","value":"0x0","data":"\#(data)"}]"#
        #expect(params.utf8.count > 8 * 1024)
        let core = SignRequestCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "networks_changed", "chain_ids": [1]]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": F.me, "credential_id": "cred-1"]], "active_index": 0,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived", "id": "req-1", "method": "eth_sendTransaction",
            "params_json": params, "origin": F.site, "transport_id": "tab-1",
            "dedicated_transport": true, "per_request_chain": 1, "dapp": NSNull(),
            "granted_address": F.me, "requested_address": NSNull(), "request_ts_ms": NSNull(),
            "now_ms": now,
        ]))
        var clear = ClearSigningViewWire.empty
        clear.recordIntent = "Swap"
        let judgments = try CoreJSON.decoder.decode([TrustSimJudgmentWire].self, from: Data(#"""
        [{"type":"erc20_trusted","token":"\#(F.usdc)","delta":"-100000000","symbol":"USDC","decimals":6,"in_trusted_set":true},
         {"type":"native","delta":"30000000000000000"}]
        """#.utf8))
        var opts = SigningController.approveOpts(
            fee: nil, clear: clear, guard: .empty, balanceChanges: judgments
        )
        // The fee is not what this test is about; the core needs one to go on.
        opts["quoted_fee"] = ["amount": "1000", "recipient": F.me, "tier": "fast"]
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "approve_tapped", "opts": opts])))
        try run.drain(core) { operation in
            switch operation["type"] as? String {
            case "check_bundler_funding": return CoreJSON.string(["type": "pre_check", "funding": NSNull()])
            case "attempt_sponsorship":
                return CoreJSON.string(["type": "sponsorship", "outcome": ["type": "denied", "reason": NSNull()]])
            default: return nil
            }
        }
        #expect(run.tags.last == "sign_and_submit", "\(run.tags)")
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_started", "id": "req-1"])))
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "ceremony_done", "id": "req-1"])))
        try run.take(core.dispatch(eventJson: CoreJSON.string([
            "type": "op_signed", "id": "req-1", "user_op_hash": F.opHash,
            "submit_block": NSNull(), "now_ms": now,
        ])))
        try run.drain(core) { operation in
            operation["type"] as? String == "persist_record" ? CoreJSON.string(["type": "record_persisted"]) : nil
        }
        let persist = try #require(run.operations.first { $0["type"] as? String == "persist_record" })
        let record = try #require(persist["record"] as? [String: Any])
        let stored = try #require(record["stored_request"] as? String)
        #expect(stored.utf8.count <= 8 * 1024)
        #expect(record["request_truncated"] as? Bool == true)
        let summary = try #require(record["summary"] as? [String: Any])
        #expect(summary["action"] as? String == "call")
        #expect(summary["contract"] as? String == F.router)
        #expect(record["intent"] as? String == "Swap")
        #expect((record["balance_changes"] as? [[String: Any]])?.count == 2)

        let row = SignExecutor.recordRow(record, nativeSymbol: "ETH")
        #expect(row["signedRequest"] as? String == stored, "the core's cut, verbatim")
        #expect(row["requestTruncated"] as? Bool == true)
        #expect((row["dappSummary"] as? NSDictionary) == (summary as NSDictionary))
        #expect((row["balanceChanges"] as? NSArray) == (record["balance_changes"] as? NSArray))
        #expect(row["intent"] as? String == "Swap")

        let wire = try #require(TxRecords.toWire(row))
        #expect((wire["summary"] as? NSDictionary) == (summary as NSDictionary))
        #expect((wire["balance_changes"] as? [[String: Any]])?.count == 2)
        #expect(wire["intent"] as? String == "Swap")
        #expect(wire["call_data"] is NSNull, "a clipped request is not a call")
    }

    /// A signature's record keeps that it was given — its `result` is empty —
    /// with its summary and the request as the core cut it.
    @Test func aSignatureRecordKeepsItsSummaryAndNoSignature() throws {
        let row = SignExecutor.recordRow(F.permit(nowMs: now), nativeSymbol: "ETH")
        #expect(row["type"] as? String == "sign_typed_data")
        #expect((row["txHash"] as? String)?.isEmpty == true)
        #expect(row["signedRequest"] as? String == F.permitRequest)
        #expect(row["requestTruncated"] as? Bool == false)
        let summary = try #require(row["dappSummary"] as? [String: Any])
        #expect(summary["action"] as? String == "permit")
        #expect(summary["unlimited"] as? Bool == true)
        #expect(row["balanceChanges"] == nil)
    }

    // MARK: - Read (TxRecords.toWire)

    /// dApp records map their origin (`dappUrl`, else — on a record from
    /// before 083 — `dappOrigin`, which this app only ever wrote from the
    /// browser), intent, summary and changes; a send maps none of them.
    @Test func theStoreMapsADappRecordsFieldsAndFallsBackToItsOrigin() throws {
        let rows = F.stored(nowMs: now)
        for row in rows {
            let wire = try #require(TxRecords.toWire(row))
            #expect(wire["dapp_url"] as? String == F.site)
            #expect(wire["summary"] is [String: Any])
        }
        #expect(TxRecords.toWire(rows[0])?["intent"] as? String == "Swap")
        #expect((TxRecords.toWire(rows[0])?["balance_changes"] as? [[String: Any]])?.count == 2)
        #expect(TxRecords.toWire(rows[0])?["call_data"] as? String == "0x3593564c")
        #expect(TxRecords.toWire(rows[1])?["call_data"] is NSNull, "only a transaction carries a call")

        // A record from before 083: no `dappUrl`, the origin in `dappOrigin`.
        let legacy: [String: Any] = [
            "id": "dapp-old-msg", "type": "sign_message", "timestamp": 1, "from": F.me,
            "dappOrigin": "https://old.example", "txHash": "0x" + String(repeating: "11", count: 65),
        ]
        #expect(TxRecords.toWire(legacy)?["dapp_url"] as? String == "https://old.example")
        #expect(TxRecords.toWire(legacy)?["summary"] == nil)
        var empty = legacy
        empty["dappUrl"] = ""
        #expect(TxRecords.toWire(empty)?["dapp_url"] as? String == "https://old.example")

        // Not a dApp's: none of it, whatever the record holds.
        let send: [String: Any] = [
            "id": "s1", "type": "send", "timestamp": 1, "dappOrigin": "https://x.test",
            "intent": "Swap", "dappSummary": ["action": "call"],
        ]
        let sendWire = try #require(TxRecords.toWire(send))
        #expect(sendWire["dapp_url"] is NSNull)
        #expect(sendWire["intent"] == nil)
        #expect(sendWire["summary"] == nil)

        // A malformed stored value is absent, never a feed that cannot load.
        var bent = rows[0]
        bent["dappSummary"] = "call"
        bent["balanceChanges"] = ["x"]
        let bentWire = try #require(TxRecords.toWire(bent))
        #expect(bentWire["summary"] == nil)
        #expect(bentWire["balance_changes"] == nil)
    }

    /// The detail reads the stored request by record id — the whole stored
    /// text, or nothing when the record kept none: the core's `""` for a
    /// request whose shape alone was past its budget is "not recorded"
    /// (`connect.detail.contentMissing`), never an empty block.
    @Test func theStoredRequestIsReadByRecordId() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        var permit = F.permit(nowMs: now)
        permit["stored_request"] = ""
        permit["request_truncated"] = true
        var rows = F.stored(nowMs: now)
        rows[1] = SignExecutor.recordRow(permit, nativeSymbol: "ETH")
        #expect(rows[1]["signedRequest"] as? String == "")
        #expect(rows[1]["requestTruncated"] as? Bool == true)
        TxRecords.writeRecords(rows, store: store)
        #expect(TxRecords.storedRequest(id: "dapp-swap-tx", store: store) == F.swapRequest)
        #expect(TxRecords.storedRequest(id: "dapp-permit-sig", store: store) == nil)
        #expect(TxRecords.storedRequest(id: "nope", store: store) == nil)

        // The detail, wired to the store as `RootView` wires it.
        let view = try feed(rows)
        let detail = FlowsLive.txDetail(
            try item("dapp-permit-sig", in: view), record: nil, on: try drawnDetail, loc: zh,
            readRequest: { TxRecords.storedRequest(id: $0, store: store) }
        )
        let content = try #require(detail.technical?.lines.compactMap { line -> TxContentModel? in
            if case .content(let content) = line { return content } else { return nil }
        }.first)
        #expect(content.read() == nil)
        #expect(content.missing == zh.t("connect.detail.contentMissing"))
    }

    /// A summary this build cannot read (an action from a newer build) never
    /// stops the feed: the core reads that record by its kind, and the rest
    /// of the feed is as it was.
    @Test func aSummaryThisBuildCannotReadNeverStopsTheFeed() throws {
        var rows = F.stored(nowMs: now)
        rows[1]["dappSummary"] = ["action": "a_future_action", "calls": 1]
        rows[0]["balanceChanges"] = [["type": "a_future_judgment", "delta": "-1"]]
        let view = try feed(rows)
        #expect(items(view).count == 3)
        let permit = try item("dapp-permit-sig", in: view)
        #expect(permit.dapp?.action == .typedData, "read by its kind")
        #expect(permit.dapp?.allowance == nil)
        let swap = try item("dapp-swap-tx", in: view)
        #expect(swap.dapp?.changes.isEmpty == true)
        #expect(swap.dapp?.place == "Uniswap", "its summary still reads")
    }

    /// Who got the money is the detail's recipient (`componentsTx.detail.to`)
    /// — a plain send's, named as the row names them, or the one a token
    /// `transfer` names — never the contract the transfer was called on; a
    /// call that paid nobody names its contract with the noun.
    @Test func aCallThatPaidSomebodyNamesTheRecipient() throws {
        let alice = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
        let bob = "0x00000000000000000000000000000000000b0b0b"
        func record(_ id: String, to: String, data: String?) -> [String: Any] {
            let call = data.map { #"[{"to":"\#(to)","value":"0x0","data":"\#($0)"}]"# }
                ?? #"[{"to":"\#(to)","value":"0x38d7ea4c68000"}]"#
            return SignExecutor.recordRow([
                "record_id": id, "kind": "dapp_tx", "method": "eth_sendTransaction",
                "params_json": call, "stored_request": call, "request_truncated": false,
                "result": F.txHash, "from": F.me, "chain_id": 1, "now_ms": now,
                "status": "confirmed", "user_op_hash": F.opHash,
                "dapp_origin": F.site, "dapp_url": F.site,
                "summary": ["action": "call", "calls": 1, "contract": to],
            ], nativeSymbol: "ETH")
        }
        var send = record("dapp-send-tx", to: alice, data: nil)
        send["toName"] = "Alice"
        let transfer = "0xa9059cbb" + String(repeating: "0", count: 24) + String(bob.dropFirst(2))
            + String(repeating: "0", count: 56) + "05f5e100"
        let view = try feed([send, record("dapp-transfer-tx", to: F.usdc, data: transfer)])

        let paid = FlowsLive.txDetail(
            try item("dapp-send-tx", in: view), record: nil, on: try drawnDetail, loc: en
        )
        let to = try #require(paid.facts.first { $0.label == en.t("componentsTx.detail.to") })
        #expect(to.value == "Alice", "named as the row names them")
        #expect(to.copyValue?.lowercased() == alice)
        #expect(!paid.facts.contains { $0.label == en.t("tokenDetail.labelContract") })

        let token = FlowsLive.txDetail(
            try item("dapp-transfer-tx", in: view), record: nil, on: try drawnDetail, loc: en
        )
        let recipient = try #require(token.facts.first { $0.label == en.t("componentsTx.detail.to") })
        #expect(recipient.copyValue?.lowercased() == bob)
        #expect(recipient.value == AddressText.short(recipient.copyValue ?? ""))
        #expect(!token.facts.contains { $0.copyValue?.lowercased() == F.usdc },
                "never the contract the transfer was called on")
    }

    // MARK: - Decode (ActivityWire)

    /// The feed's spec-093 fields decode from the real core: the verb as a
    /// term, the place, the allowance, off-chain, facts and technical lines,
    /// the money with its "≈" and the coin back, and every row's subtitle.
    @Test func theCoresDappFieldsDecode() throws {
        let view = try feed(F.stored(nowMs: now))
        let swap = try item("dapp-swap-tx", in: view)
        let swapDapp = try #require(swap.dapp)
        #expect(swap.kind == .dappTx)
        #expect(swapDapp.intentTerm == "intentSwap")
        #expect(swapDapp.place == "Uniswap")
        #expect(swapDapp.estimated)
        #expect(swap.value == "100")
        #expect(swap.symbol == "USDC")
        #expect(swapDapp.received?.symbol == "ETH")
        #expect(swapDapp.received?.value == "0.03")
        #expect(swapDapp.received?.exact == false)
        #expect(!swapDapp.offChain)
        #expect(swapDapp.changes.count == 2)
        #expect(swap.subtitle == [.site("app.uniswap.org"), .network(chainId: 1)])
        #expect(swapDapp.technical.contains(.hash(txHash: F.txHash)))
        #expect(swapDapp.technical.contains(.userOpHash(F.opHash)))

        let permit = try item("dapp-permit-sig", in: view)
        let permitDapp = try #require(permit.dapp)
        #expect(permit.kind == .signTypedData)
        #expect(permitDapp.action == .permit)
        #expect(permitDapp.intentTerm == "permitIntent")
        #expect(permitDapp.place == "Uniswap")
        #expect(permitDapp.offChain)
        #expect(permitDapp.allowance?.unlimited == true)
        #expect(permitDapp.allowance?.symbol == "USDC")
        #expect(permitDapp.facts.first == .site("app.uniswap.org"))
        #expect(permitDapp.facts.contains(.expires(at: nil)))
        #expect(permitDapp.technical.first == .operation(.typedDataSignature))
        #expect(permitDapp.technical.contains(.content(.typedData)))
        #expect(permitDapp.technical.contains(.primaryType("PermitSingle")))
        #expect(permit.txHash == nil)

        let signIn = try item("dapp-siwe-msg", in: view)
        #expect(signIn.kind == .signMessage)
        #expect(signIn.dapp?.intentTerm == "signInIntent")
        #expect(signIn.dapp?.place == "app.uniswap.org")
        #expect(signIn.subtitle == [.network(chainId: 1)], "the title named the site already")
        #expect(signIn.dapp?.allowance == nil)
    }

    /// A tag this build has never heard of is `unknown` and is left out — it
    /// never fails the feed it sits in.
    @Test func anUnknownTagDoesNotFailTheFeed() throws {
        let json = #"""
        {"id":"x","direction":"out","counterparty":null,"alias":null,"value":null,"symbol":"",
         "decimals":null,"usd_value":0,"chain_id":1,"timestamp":1,"day_start_ms":0,"tx_hash":null,
         "batch":null,"kind":"a_future_kind","status":"confirmed","site":"a.example",
         "counterparty_role":"recipient",
         "subtitle":[{"type":"a_future_line"},{"type":"site","site":"a.example"}],
         "dapp":{"site":"a.example","intent":null,"intent_term":null,"action":"a_future_action",
                 "place":null,"allowance":null,"off_chain":true,
                 "facts":[{"type":"a_future_fact","x":1},{"type":"date","timestamp":1}],
                 "technical":[{"type":"operation","operation":{"type":"a_future_op"}},
                              {"type":"content","content":"a_future_content"}]}}
        """#
        let item = try CoreJSON.decoder.decode(FeedItemWire.self, from: Data(json.utf8))
        #expect(item.subtitle == [.unknown, .site("a.example")])
        #expect(item.dapp?.action == .call)
        #expect(item.dapp?.facts == [.unknown, .date(timestamp: 1)])
        #expect(item.dapp?.technical == [.operation(.unknown), .content(.unknown)])
        #expect(WalletLive.subtitleText(item.subtitle, loc: en) == "a.example")
        let detail = FlowsLive.txDetail(item, record: nil, on: try drawnDetail, loc: en)
        #expect(detail.facts.map(\.label) == [en.t("componentsTx.detail.labelDate")])
        #expect(detail.technical == nil, "no line it can say")
    }

    // MARK: - Rows (WalletLive)

    /// Each row is titled by the core's verb at its place, with the core's
    /// second line: 「在 Uniswap 兑换」 ≈ −100 USDC with ≈ +0.03 ETH back;
    /// 「在 Uniswap 授权签名」 无限额 USDC in the danger tone; 「在
    /// app.uniswap.org 登录」 with no figure.
    @Test func theRowsSayTheCoresWords() throws {
        let view = try feed(F.stored(nowMs: now))
        for loc in [zh, en] {
            func title(_ verb: String, _ place: String) -> String {
                loc.t("history.dappRowTitle", vars: ["intent": loc.t("componentsUi.signing.\(verb)"), "place": place])
            }
            let swap = WalletLive.activityRow(try item("dapp-swap-tx", in: view), loc: loc, hidden: false)
            #expect(swap.kind == .dapp)
            #expect(swap.title == title("intentSwap", "Uniswap"))
            #expect(swap.subtitle == "app.uniswap.org · \(WalletLive.chainName(1))")
            #expect(swap.amount == "≈ \u{2212}100")
            #expect(swap.unit == "USDC")
            #expect(swap.received == "≈ +0.03 ETH")
            #expect(!swap.danger)

            let permit = WalletLive.activityRow(try item("dapp-permit-sig", in: view), loc: loc, hidden: false)
            #expect(permit.kind == .dapp)
            #expect(permit.title == title("permitIntent", "Uniswap"))
            #expect(permit.subtitle == "app.uniswap.org · \(WalletLive.chainName(1))")
            #expect(permit.amount == loc.t("componentsUi.signingApprove.unlimitedValue"))
            #expect(permit.unit == "USDC")
            #expect(permit.danger)
            #expect(permit.received == nil)

            let signIn = WalletLive.activityRow(try item("dapp-siwe-msg", in: view), loc: loc, hidden: false)
            #expect(signIn.kind == .dapp)
            #expect(signIn.title == title("signInIntent", "app.uniswap.org"))
            #expect(signIn.subtitle == WalletLive.chainName(1))
            #expect(signIn.amount.isEmpty && signIn.unit.isEmpty, "a sign-in moves nothing")
        }
        #expect(WalletLive.activityRow(try item("dapp-swap-tx", in: view), loc: zh, hidden: false).title == "在 Uniswap 兑换")
    }

    /// Privacy masks a figure and keeps its unit — the swap, the coin back
    /// and a finite allowance — but never "Unlimited", which is a risk to see.
    @Test func privacyMasksFiguresButNotUnlimited() throws {
        var rows = F.stored(nowMs: now)
        var finite = rows[1]
        finite["id"] = "dapp-permit-finite"
        finite["dappSummary"] = [
            "action": "permit", "contract": F.permit2, "spender": F.router, "token": F.usdc,
            "symbol": "USDC", "decimals": 6, "amount": "100000000",
        ]
        rows.append(finite)
        let view = try feed(rows)
        let swap = WalletLive.activityRow(try item("dapp-swap-tx", in: view), loc: en, hidden: true)
        #expect(swap.amount == WalletFixtures.mask)
        #expect(swap.masked)
        #expect(swap.received == "≈ +\(WalletFixtures.mask) ETH")
        let unlimited = WalletLive.activityRow(try item("dapp-permit-sig", in: view), loc: en, hidden: true)
        #expect(unlimited.amount == en.t("componentsUi.signingApprove.unlimitedValue"))
        #expect(!unlimited.masked)
        let capped = try item("dapp-permit-finite", in: view)
        #expect(WalletLive.activityRow(capped, loc: en, hidden: false).amount == "100")
        let masked = WalletLive.activityRow(capped, loc: en, hidden: true)
        #expect(masked.amount == WalletFixtures.mask)
        #expect(masked.unit == "USDC")
        #expect(!masked.danger)
        let signIn = WalletLive.activityRow(try item("dapp-siwe-msg", in: view), loc: en, hidden: true)
        #expect(signIn.amount.isEmpty && !signIn.masked, "nothing to show, nothing to mask")
    }

    // MARK: - Detail (FlowsLive)

    /// A permit's detail: no status chip — the off-chain note instead — the
    /// core's facts in its order, and "Technical details" whose stored
    /// request is read from the store only when it is opened.
    @Test func aPermitsDetailIsTheCoresFactsAndReadsTheRequestOnlyWhenOpened() throws {
        let view = try feed(F.stored(nowMs: now))
        var reads: [String] = []
        let detail = FlowsLive.txDetail(
            try item("dapp-permit-sig", in: view), record: view.transactions.first { $0.id == "dapp-permit-sig" },
            on: try drawnDetail, loc: en,
            readRequest: { id in reads.append(id); return F.permitRequest }
        )
        #expect(detail.title == en.t("history.dappRowTitle", vars: [
            "intent": en.t("componentsUi.signing.permitIntent"), "place": "Uniswap",
        ]))
        #expect(detail.status == nil, "a signature has nothing to settle")
        #expect(detail.note == en.t("connect.detail.offChainNote"))
        #expect(detail.amount == "\(en.t("componentsUi.signingApprove.unlimitedValue")) USDC")
        #expect(detail.amountDanger)
        #expect(detail.viewOnExplorer == nil)
        #expect(detail.facts.map(\.label) == [
            en.t("connect.detail.labelApp"),
            en.t("componentsTx.detail.labelChain"),
            en.t("componentsUi.signing.labelSpender"),
            en.t("componentsUi.signingApprove.spendingCap"),
            en.t("componentsUi.signingApprove.expiresLabel"),
            en.t("componentsTx.detail.labelDate"),
        ])
        #expect(detail.facts[0].value == "app.uniswap.org")
        #expect(detail.facts[2].copyValue == F.router)
        #expect(detail.facts[3].danger)
        #expect(detail.facts[4].value == en.t("componentsUi.signingApprove.noExpiry"))

        let technical = try #require(detail.technical)
        #expect(technical.title == en.t("componentsUi.signing.advancedToggle"))
        #expect(reads.isEmpty, "nothing is read while the detail is built")
        var labels: [String] = []
        for line in technical.lines {
            switch line {
            case .fact(let fact): labels.append("\(fact.label)=\(fact.value)")
            case .content(let content):
                labels.append(content.label)
                #expect(content.missing == en.t("connect.detail.contentMissing"))
                #expect(content.read() == F.permitRequest)
            }
        }
        #expect(reads == ["dapp-permit-sig"], "read by record id, when opened")
        #expect(labels == [
            "\(en.t("componentsTx.detail.labelOperation"))=\(en.t("componentsTx.detail.opTypedDataSignature"))",
            en.t("connect.detail.contentTypedData"),
            "\(en.t("componentsUi.signing.typeLabel"))=PermitSingle",
        ])
    }

    /// A swap's detail keeps its status chip and explorer, states the lines it
    /// moved in the sheet's form, and puts both hashes under the disclosure.
    @Test func aSwapsDetailKeepsItsChipAndStatesWhatItMoved() throws {
        let view = try feed(F.stored(nowMs: now))
        let detail = FlowsLive.txDetail(
            try item("dapp-swap-tx", in: view), record: nil, on: try drawnDetail, loc: zh
        )
        #expect(detail.title == "在 Uniswap 兑换")
        #expect(detail.status?.text == zh.t("componentsTx.detail.statusSucceeded"))
        #expect(detail.note == nil)
        #expect(detail.amount == "≈ \u{2212}100 USDC")
        #expect(detail.received == "≈ +0.03 ETH")
        #expect(detail.viewOnExplorer != nil)
        let changes = try #require(detail.facts.first { $0.label == zh.t("componentsUi.signing.balanceChangesTitle") })
        #expect(changes.lines.map { "\($0.delta) \($0.symbol)" } == ["≈ \u{2212}100 USDC", "≈ +0.03 ETH"])
        #expect(changes.lines.map(\.tone) == [.neutral, .success])
        // A call that paid nobody names its contract — with the noun (083 F3
        // review), never "Interacting with", and never "To".
        #expect(detail.facts.contains { $0.label == zh.t("tokenDetail.labelContract") && $0.copyValue == F.router })
        #expect(!detail.facts.contains { $0.label == zh.t("componentsUi.signing.interactingLabel") })
        #expect(!detail.facts.contains { $0.label == zh.t("componentsTx.detail.to") })
        let technical = try #require(detail.technical)
        let facts = technical.lines.compactMap { line -> FactRowModel? in
            if case .fact(let fact) = line { return fact } else { return nil }
        }
        #expect(facts.contains { $0.label == zh.t("componentsTx.detail.labelHash") && $0.copyValue == F.txHash })
        #expect(facts.contains { $0.label == zh.t("componentsTx.receipt.userOpHash") && $0.copyValue == F.opHash })
        #expect(facts.first?.value == zh.t("componentsTx.detail.opContractInteraction"))
        #expect(!detail.facts.contains { $0.label == zh.t("componentsTx.detail.labelHash") },
                "the hash is a technical detail")
    }
}
