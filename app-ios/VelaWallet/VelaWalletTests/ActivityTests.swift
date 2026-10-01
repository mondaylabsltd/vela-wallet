//
//  ActivityTests.swift
//  VelaWalletTests
//
//  The receipt pipeline's shell half: the bytes that reach storage, the wire
//  translation the core reads, and the wording a row ends up wearing.
//
//  Everything that DECIDES is in Rust — which logs count, what may be admitted,
//  how rows fold into days. What is tested here is what the shell owes it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ActivityTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func freshStore() -> (VelaStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        return (VelaStore(defaults: defaults), defaults)
    }

    private func record(id: String, timestamp: Double, type: String = "receive") -> [String: Any] {
        [
            "id": id, "userOpHash": "", "txHash": "0xabc", "from": "0x1", "to": "0x2",
            "value": "1.5", "symbol": "USDC", "decimals": 6, "chainId": 100,
            "timestamp": timestamp, "status": "confirmed", "type": type, "usd": "$1.50",
        ]
    }

    // MARK: - An arrival moves the balance (#188)

    /// A NEW non-null `new_item_id` is an arrival and refreshes the balances —
    /// once. The same id again, or the glow clearing, is not another one.
    @Test func anIncomingTransferRefreshesTheBalanceOnce() {
        let (store, defaults) = freshStore()
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        let held = HeldTokens()
        let activity = ActivityStore(
            store: store, accounts: accounts, held: held,
            trust: TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        )
        var refreshes = 0
        activity.onNewItem = { refreshes += 1 }
        func feed(_ id: String?) -> FeedViewWire {
            FeedViewWire(rows: [], transactions: [], newItemId: id, toast: nil)
        }
        activity.commit(feed(nil))
        #expect(refreshes == 0, "the first pass is not an arrival")
        activity.commit(feed("rx-1"))
        activity.commit(feed("rx-1"))
        #expect(refreshes == 1)
        activity.commit(feed(nil))
        activity.commit(feed("rx-2"))
        #expect(refreshes == 2)
    }

    // MARK: - The local store

    /// The merge answers **how many were new** — the number the core celebrates
    /// on. A repeat of the same window must answer zero, or somebody sees a
    /// toast for a receipt they already saw.
    @Test func mergeCountsOnlyGenuinelyNewRecords() {
        let (store, _) = freshStore()
        #expect(TxRecords.merge([record(id: "a", timestamp: 10),
                                 record(id: "b", timestamp: 20)], store: store) == 2)
        #expect(TxRecords.merge([record(id: "b", timestamp: 20),
                                 record(id: "c", timestamp: 30)], store: store) == 1)
        #expect(TxRecords.load(store: store).count == 3)
        // Newest first, whatever order they arrived in.
        #expect(TxRecords.load(store: store).first?["id"] as? String == "c")
    }

    /// The 200 cap every client applies, and it keeps the NEWEST.
    @Test func theStoreIsCappedAtTwoHundredNewestFirst() {
        let (store, _) = freshStore()
        let many = (0..<250).map { record(id: "id-\($0)", timestamp: Double($0)) }
        #expect(TxRecords.merge(many, store: store) == 250)
        let stored = TxRecords.load(store: store)
        #expect(stored.count == TxRecords.cap)
        #expect(stored.first?["id"] as? String == "id-249")
        #expect(stored.last?["id"] as? String == "id-50")
    }

    @Test func deletingARecordRemovesOnlyThatOne() {
        let (store, _) = freshStore()
        _ = TxRecords.merge([record(id: "a", timestamp: 10), record(id: "b", timestamp: 20)],
                            store: store)
        TxRecords.delete(id: "a", store: store)
        #expect(TxRecords.load(store: store).map { $0["id"] as? String } == ["b"])
        // A missing id is a no-op, not an empty file.
        TxRecords.delete(id: "nobody", store: store)
        #expect(TxRecords.load(store: store).count == 1)
    }

    // MARK: - Stored shape → the core's vocabulary

    /// A `type` the feed has never heard of is **dropped rather than guessed
    /// at**. Inventing a kind for it would be a lie the core then acts on.
    @Test func anUnknownRecordTypeIsDroppedAndALegacyOneIsNot() {
        #expect(TxRecords.toWire(record(id: "a", timestamp: 1, type: "teleport")) == nil)
        // No `type` at all is the legacy row the core reads as `send`; it must
        // survive, with `kind: null` saying so.
        var legacy = record(id: "a", timestamp: 1)
        legacy.removeValue(forKey: "type")
        let wire = TxRecords.toWire(legacy)
        #expect(wire?["kind"] is NSNull)
    }

    /// Numbers are coerced fail-closed: the store is an unvalidated JSON parse,
    /// and a field serde could not accept would fault the core into a feed that
    /// never loads.
    @Test func malformedNumbersBecomeZeroRatherThanFaultingTheCore() {
        var broken = record(id: "a", timestamp: 1)
        broken["chainId"] = "one hundred"
        broken["decimals"] = -4
        broken["timestamp"] = "yesterday"
        let wire = TxRecords.toWire(broken)
        #expect(wire?["chain_id"] as? Int == 0)
        #expect(wire?["decimals"] as? Int == 0)
        #expect(wire?["timestamp"] as? Double == 0)
    }

    /// The grouping key the core cannot compute. A record written at 23:30
    /// local heads its own day — which is the whole reason the shell owns it.
    @Test func theDayKeyIsLocalMidnight() {
        let calendar = Calendar.current
        let lateEvening = calendar.date(bySettingHour: 23, minute: 30, second: 0, of: Date())!
        let key = TxRecords.dayStartMs(lateEvening.timeIntervalSince1970)
        let midnight = calendar.startOfDay(for: lateEvening).timeIntervalSince1970 * 1000
        #expect(key == midnight)
        // And a record thirty minutes later belongs to the NEXT day.
        let justAfter = TxRecords.dayStartMs(lateEvening.timeIntervalSince1970 + 3_600)
        #expect(justAfter > key)
    }

    // MARK: - The ingest valuation, which is the shell's

    @Test func aStablecoinIsWorthAboutADollarAndTheGlyphIsFolded() {
        #expect(ActivityExecutor.isStable("USDC"))
        #expect(ActivityExecutor.isStable("usdt"))
        // "USD₮0" is Tether's own glyph — the same coin as USDT0.
        #expect(ActivityExecutor.isStable("USD₮0"))
        #expect(!ActivityExecutor.isStable("MON"))
    }

    /// The stored string is `en-US` on every client, so a record written on the
    /// phone reads the same in the browser.
    @Test func theStoredUsdStringIsTheOneEveryClientWrites() {
        #expect(ActivityExecutor.formatUsd(1_234.5) == "$1,234.50")
        #expect(ActivityExecutor.formatUsd(1) == "$1.00")
        // Not a price of zero — "nobody could price it", which is what every
        // client has always stored and what the core reads back as unknown.
        #expect(ActivityExecutor.formatUsd(0) == "$0.00")
        #expect(ActivityExecutor.formatUsd(-3) == "$0.00")
        #expect(ActivityExecutor.formatUsd(.nan) == "$0.00")
    }

    // MARK: - Raw log → the core's wire

    /// A codec, not a policy: the log is carried through untouched, because
    /// whether it means anything is the core's call — it re-verifies
    /// `topics[2]` itself.
    @Test func aRawLogIsCarriedThroughWithAbsenceIntact() {
        let wire = TokenTrustExecutor.logToWire([
            "address": "0xtoken", "topics": ["0xddf", "0xfrom", "0xto"],
            "data": "0x01", "transactionHash": "0xhash", "logIndex": "0x2",
        ])
        #expect(wire["transaction_hash"] as? String == "0xhash")
        #expect((wire["topics"] as? [String])?.count == 3)
        // `null` means ABSENT; the core applies its own `?? 0x0`.
        #expect(wire["block_number"] is NSNull)
        #expect(wire["log_index"] as? String == "0x2")
    }

    /// The shared reader both token machines go through — `token_trust` when
    /// it admits one from a receipt, `manage_tokens` when somebody types one in.
    @Test func aStoredCustomTokenCrossesWithoutItsDisplayVocabulary() {
        let wire = CustomTokens.toWire([
            "id": "100_0xabc", "chainId": 100, "contractAddress": "0xabc",
            "symbol": "USDC", "name": "USD Coin", "decimals": 6,
            "networkName": "Gnosis",
        ])
        #expect(wire?["contract_address"] as? String == "0xabc")
        #expect(wire?["decimals"] as? Int == 6)
        // `networkName` is the shell's word for a chain and does not travel.
        #expect(wire?["networkName"] == nil)
        // A row without a contract is not a token anybody can watch.
        #expect(CustomTokens.toWire(["chainId": 100]) == nil)
    }

    @Test func hexQuantitiesReadAsNumbersOrAsNothing() {
        #expect(TokenTrustExecutor.hexToNumber("0x10") == 16)
        #expect(TokenTrustExecutor.hexToNumber("10") == 16)
        // Never a zero the core would compare against.
        #expect(TokenTrustExecutor.hexToNumber("0x") == nil)
        #expect(TokenTrustExecutor.hexToNumber("later") == nil)
    }

    // MARK: - ERC-20 metadata decoding

    /// The defect this whole path exists for: an 18-decimals guess renders a
    /// 6-decimals stablecoin as "+0 tokens", so an unreadable answer must be
    /// `nil` rather than a default.
    @Test func decimalsAreReadOrRefused() {
        let six = Data(hexString: String(repeating: "0", count: 63) + "6")!
        #expect(TokenMetadata.decodeDecimals(six) == 6)
        // Zero decimals is a real token, not a failure to read one.
        #expect(TokenMetadata.decodeDecimals(Data(repeating: 0, count: 32)) == 0)
        #expect(TokenMetadata.decodeDecimals(Data()) == nil)
        let absurd = Data(hexString: String(repeating: "f", count: 64))!
        #expect(TokenMetadata.decodeDecimals(absurd) == nil)
    }

    /// Both `symbol()` encodings — the dynamic string, and the fixed word the
    /// legacy tokens (MKR) answer with — plus a multibyte symbol surviving.
    @Test func symbolsDecodeFromBothEncodings() {
        let offset = String(repeating: "0", count: 62) + "20"
        let length = String(repeating: "0", count: 62) + "04"
        let usdc = "55534443" + String(repeating: "0", count: 56)
        #expect(TokenMetadata.decodeString(Data(hexString: offset + length + usdc)!) == "USDC")

        // bytes32: "MKR" padded with NULs.
        let mkr = "4d4b52" + String(repeating: "0", count: 58)
        #expect(TokenMetadata.decodeString(Data(hexString: mkr)!) == "MKR")

        // "USD₮0" — three bytes for the glyph, and it must survive intact.
        let tether = "5553..".replacingOccurrences(of: "..", with: "44e282ae30")
        let padded = tether + String(repeating: "0", count: 64 - tether.count)
        let dynamic = offset + String(repeating: "0", count: 62) + "08" + padded
        #expect(TokenMetadata.decodeString(Data(hexString: dynamic)!) == "USD₮0")
    }
}

// MARK: - A dApp's transactions and the empty lines (spec 082 T121, RG1–RG5)

@MainActor
struct DappActivityRowTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let now = Date().timeIntervalSince1970 * 1000

    /// The real feed core, fed the rows this app writes — the dApp's record
    /// as `SignExecutor.recordRow` stores it, read back through `toWire`.
    private func feed(_ rows: [[String: Any]], filter: Int? = nil) throws -> FeedViewWire {
        let core = ActivityFeedCore()
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": me])))
        if let filter {
            try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "chain_filter_changed", "chain_id": filter])))
        }
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

    private func dappRecord(_ id: String, status: String, value: String = "0x38d7ea4c68000") -> [String: Any] {
        SignExecutor.recordRow([
            "record_id": id, "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"\#(value)"}]"#,
            "result": "", "from": me, "chain_id": 100, "now_ms": now, "status": status,
            "user_op_hash": "0x" + String(repeating: "ab", count: 32),
            "dapp_origin": "http://192.168.50.9:8137",
            // 083 H2: the site is read from the origin the request came from.
            "dapp_url": "http://192.168.50.9:8137",
        ], nativeSymbol: "XDAI")
    }

    private struct Missing: Error {}

    private func historyFixture() throws -> HistoryModel {
        guard case .history(let model) = WalletFlowFixtures.build(.a1, loc: zh).base else { throw Missing() }
        return model
    }

    private func detailFixture() throws -> TxDetailModel {
        guard case .txDetail(let model)? = WalletFlowFixtures.build(.a3, loc: zh).sheet else { throw Missing() }
        return model
    }

    private func items(_ view: FeedViewWire) -> [FeedItemWire] {
        view.rows.compactMap { if case .item(let item) = $0 { return item } else { return nil } }
    }

    /// A pending dApp transaction is a row at once: "dApp 交易", "处理中 · the
    /// site", its native amount — from the core's kind, status and site.
    @Test func aPendingDappRowShowsItsSite() throws {
        let view = try feed([dappRecord("dapp-1-tx", status: "pending")])
        let item = try #require(items(view).first)
        #expect(item.kind == .dappTx)
        #expect(item.status == .pending)
        #expect(item.site == "192.168.50.9:8137")
        let row = WalletLive.activityRow(item, loc: zh, hidden: false)
        #expect(row.kind == .dapp)
        #expect(row.title == zh.t("history.txLabelDappTx"))
        #expect(row.subtitle == "\(zh.t("componentsTx.detail.statusPending")) · 192.168.50.9:8137")
        #expect(row.amount == "\u{2212}0.001")

        // The detail says who asked, and the row's own lifecycle.
        let detail = FlowsLive.txDetail(
            item, record: view.transactions.first,
            on: try detailFixture(), loc: zh
        )
        #expect(detail.title == zh.t("history.txLabelDappTx"))
        #expect(detail.status.text == zh.t("componentsTx.detail.statusPending"))
        #expect(detail.facts.contains { $0.label == zh.t("componentsUi.signing.siweOrigin") && $0.value == "192.168.50.9:8137" })
    }

    /// 082 X-FIRST-TAP: every row carries its record's feed id, on the home
    /// and in History alike, so a tap opens that record — not whichever one
    /// now sits at the tapped position after the feed moved.
    @Test func everyRowCarriesItsRecordsId() throws {
        let view = try feed([
            dappRecord("dapp-1-tx", status: "pending"),
            dappRecord("dapp-2-tx", status: "pending", value: "0x0"),
        ])
        let rows = WalletLive.activityGroups(view, loc: zh, hidden: false).flatMap(\.rows)
        let ids = Set(items(view).map(\.id))
        #expect(rows.count == 2)
        #expect(Set(rows.compactMap(\.itemId)) == ids, "each row names its own record")
        for row in rows {
            let id = try #require(row.itemId)
            #expect(FlowsLive.items(view).first { $0.id == id } != nil)
        }
    }

    /// A failed one says so — never a quiet "done".
    @Test func aFailedDappRowSaysFailed() throws {
        // The tracker patches a record `failed` in place; the row is written
        // pending at submit (`recordRow`).
        var failedRow = dappRecord("dapp-2-tx", status: "pending", value: "0x0")
        failedRow["status"] = "failed"
        let view = try feed([failedRow])
        let item = try #require(items(view).first)
        #expect(item.status == .failed)
        let row = WalletLive.activityRow(item, loc: zh, hidden: false)
        #expect(row.subtitle.hasPrefix(zh.t("componentsTx.detail.statusFailed") + " · "))
        #expect(row.amount.isEmpty, "a call that moved no coin shows no amount, not a −0")
    }

    /// The empty lines are the core's: "no transactions yet" on every
    /// network, and the network sentence only under a filter.
    @Test func theEmptyLinesAreTheCores() throws {
        let all = try feed([])
        #expect(all.historyEmptyKey == "history.emptyTitle")
        #expect(all.homeEmptyKey == "home.emptyNoActivity")
        let history = FlowsLive.history(
            all, on: try historyFixture(), loc: zh, hidden: false
        )
        #expect(history.emptyText == zh.t("history.emptyTitle"))
        #expect(history.emptyText != zh.t("history.emptyFilter"))

        let filtered = try feed([], filter: 100)
        #expect(filtered.historyEmptyKey == "history.emptyFilter")
        let fallback = SectionModel(title: "", action: "", mode: .empty,
                                    empty: SectionEmptyModel(title: "x", caption: "caption"))
        let home = WalletLive.homeEmpty(filtered, fallback: fallback, loc: zh)
        #expect(home.title == zh.t(filtered.homeEmptyKey))
        if filtered.homeEmptyKey != "home.emptyNoActivity" { #expect(home.caption.isEmpty) }
        #expect(WalletLive.homeEmpty(all, fallback: fallback, loc: zh).caption == "caption")
    }
}
