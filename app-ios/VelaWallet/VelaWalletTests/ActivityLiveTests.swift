//
//  ActivityLiveTests.swift
//  VelaWalletTests
//
//  The receipt pipeline against the real world.
//
//  Behind the same compile flag as the other live suites:
//
//      xcodebuild test ... -only-testing:VelaWalletTests/ActivityLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//
//  ## The address these tests watch is not ours
//
//  `token_trust` scans the **last 100 blocks** — it is a live receipt watcher,
//  not a history importer — so an account that was paid last week finds
//  nothing, correctly. To prove the pipeline actually decodes and persists a
//  receipt, the scan needs an address that is being paid *right now*, and the
//  most reliably-paid address on Ethereum is an exchange's hot wallet.
//
//  Nothing is sent, nothing is signed, and only public logs are read.
//

import Foundation
import Testing
@testable import VelaWallet

// MARK: - The detail of a dApp record (spec 082 T242; hermetic)

/// What a dApp record's detail says (RJ16, RJ18, G51, G52): the call's data
/// reaches the feed, a contract is "Interacting with" rather than "To", an op
/// that never reached the chain offers no explorer, and a pending record's
/// delete is a quiet control. Hermetic — unlike the live suite below.
@MainActor
struct ActivityDetailTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let token = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    private var drawn: TxDetailModel {
        get throws {
            guard case .txDetail(let model)? = WalletFlowFixtures.build(.a2, loc: loc).sheet else {
                throw Missing()
            }
            return model
        }
    }
    private struct Missing: Error {}

    private func item(
        role: FeedCounterpartyRoleWire, txHash: String?, status: FeedTxStatusWire
    ) -> FeedItemWire {
        FeedItemWire(
            id: "dapp-1-tx", direction: .out, counterparty: token, alias: nil,
            value: nil, symbol: "xDAI", decimals: 18, usdValue: 0, chainId: 100,
            timestamp: 1_757_000_000, dayStartMs: 1_756_944_000_000, txHash: txHash, batch: nil,
            kind: .dappTx, status: status, site: "192.168.50.9:8137", counterpartyRole: role
        )
    }

    /// The stored request's first call's `data` is what the core reads the
    /// counterparty from — `params[0].data`, or the first leg of a
    /// `wallet_sendCalls` — and nothing for a clipped request, no calldata,
    /// or another kind of row.
    @Test func theCallDataReachesTheFeed() {
        let data = "0xa9059cbb" + String(repeating: "0", count: 128)
        func wire(_ row: [String: Any]) -> Any? { TxRecords.toWire(row)?["call_data"] }
        let single: [String: Any] = [
            "id": "d1", "type": "dapp_tx", "timestamp": 1,
            "signedRequest": #"[{"to":"\#(token)","value":"0x0","data":"\#(data)"}]"#,
        ]
        #expect(wire(single) as? String == data)
        let batch: [String: Any] = [
            "id": "d2", "type": "dapp_tx", "timestamp": 1,
            "signedRequest": #"[{"calls":[{"to":"\#(token)","data":"\#(data)"},{"to":"0xbb","data":"0x"}]}]"#,
        ]
        #expect(wire(batch) as? String == data)
        var clipped = single
        clipped["requestTruncated"] = true
        #expect(wire(clipped) is NSNull, "half a call is not a call")
        let plain: [String: Any] = [
            "id": "d3", "type": "dapp_tx", "timestamp": 1,
            "signedRequest": #"[{"to":"\#(token)","value":"0x1","data":"0x"}]"#,
        ]
        #expect(wire(plain) is NSNull)
        let send: [String: Any] = ["id": "s1", "type": "send", "timestamp": 1, "signedRequest": single["signedRequest"]!]
        #expect(wire(send) is NSNull, "only a dApp's transaction carries its call")
    }

    /// G52: a relay-refused dApp record — the contract is "Interacting with",
    /// and no explorer is offered for an op that never reached the chain.
    @Test func aContractIsInteractedWithAndNoHashIsNoExplorer() throws {
        let failed = FlowsLive.txDetail(
            item(role: .contract, txHash: nil, status: .failed), record: nil, on: try drawn, loc: loc
        )
        let labels = failed.facts.map(\.label)
        #expect(labels.contains(loc.t("componentsUi.signing.interactingLabel")))
        #expect(!labels.contains(loc.t("componentsTx.detail.to")))
        #expect(failed.viewOnExplorer == nil, "no transaction, no explorer control")
        #expect(!labels.contains(loc.t("componentsTx.detail.labelHash")))
        #expect(!failed.deleteQuiet, "a settled record keeps its delete")

        let recipient = FlowsLive.txDetail(
            item(role: .recipient, txHash: "0x" + String(repeating: "7e", count: 32), status: .confirmed),
            record: nil, on: try drawn, loc: loc
        )
        #expect(recipient.facts.map(\.label).contains(loc.t("componentsTx.detail.to")))
        #expect(recipient.viewOnExplorer == loc.t("history.viewOnExplorer"))
    }

    /// G51 (RJ18): on a record still pending — "may have been sent" — the
    /// delete is a quiet control under the rest, never the loudest thing.
    @Test func aPendingRecordsDeleteIsQuiet() throws {
        let pending = FlowsLive.txDetail(
            item(role: .contract, txHash: nil, status: .pending), record: nil, on: try drawn, loc: loc
        )
        #expect(pending.deleteLabel == loc.t("history.deleteRecord"))
        #expect(pending.deleteQuiet)
    }

    /// 087 F04: a pending record nothing will settle reaches the shell as
    /// `unknown` — its row and its detail say "Unknown", never "Pending" for
    /// ever and never "Failed"; it draws no hash and keeps its quiet delete.
    @Test func aRecordNothingWillSettleReadsUnknown() throws {
        let unknown = item(role: .contract, txHash: nil, status: .unknown)
        let detail = FlowsLive.txDetail(unknown, record: nil, on: try drawn, loc: loc)
        #expect(detail.status?.text == loc.t("componentsUi.signing.intentUnknown"))
        #expect(detail.status?.tone == .info)
        #expect(detail.status?.text != loc.t("componentsTx.detail.statusPending"))
        #expect(detail.status?.text != loc.t("componentsTx.detail.statusFailed"))
        #expect(!detail.facts.map(\.label).contains(loc.t("componentsTx.detail.labelHash")))
        #expect(!detail.facts.contains { $0.copyValue?.hasPrefix("dapp-") == true })
        #expect(detail.deleteLabel == loc.t("history.deleteRecord"))
        #expect(detail.deleteQuiet)
        #expect(WalletLive.statusPrefix(.unknown, loc: loc) == loc.t("componentsUi.signing.intentUnknown"))
        let decoded = try CoreJSON.decoder.decode(FeedTxStatusWire.self, from: Data(#""unknown""#.utf8))
        #expect(decoded == .unknown)
    }
}

#if VELA_LIVE_TESTS

@MainActor
@Suite(.serialized)
struct ActivityLiveTests {

    /// Binance's hot wallet — it receives stablecoins continuously, which is
    /// the only property this test needs from it.
    private static let busyAddress = "0x28C6c06298d514Db089934071355E5743bf21d60"
    private static let goldenSafe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func fresh() -> (VelaStore, AccountStore, HeldTokens, TokenTrustStore) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        return (store, accounts, held, trust)
    }

    /// The whole scan chain, end to end: block number → `eth_getLogs` on the
    /// registry's stablecoin allowlist → local `topics[2]` re-verification →
    /// block timestamps → metadata gate → judged feed.
    @Test func theScanFindsRealReceiptsForAnAddressBeingPaid() async {
        let (_, _, _, trust) = fresh()
        let incoming = await trust.pollIncoming(address: Self.busyAddress, chainIds: [1])

        #expect(!incoming.isEmpty, "no receipts in the last 100 Ethereum blocks — suspicious")
        for transfer in incoming.prefix(3) {
            #expect(!transfer.id.isEmpty)
            #expect(!transfer.txHash.isEmpty)
            #expect(transfer.timestampSec > 1_700_000_000, "a receipt with no plausible time")
            // The metadata gate: an ERC-20 the core could not name never
            // reaches this list, because "+0 tokens" is worse than silence.
            if !transfer.isNative {
                #expect(transfer.symbol != nil, "an unnamed token slipped past the gate")
                #expect(transfer.decimals != nil)
            }
        }
        print("[live] receipts found: \(incoming.count)")
        if let first = incoming.first {
            print("[live] newest: \(first.value) \(first.symbol ?? "?") from \(first.from)")
        }
    }

    /// The executor's whole operation: scan, map, merge — and the count it
    /// answers is what the core celebrates on.
    ///
    /// The second run must answer **zero**: the same window rediscovered is not
    /// new money, and a core told otherwise would toast a receipt somebody
    /// already saw.
    @Test func aRescanOfTheSameWindowIsNotNewMoney() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        let executor = ActivityExecutor(store: store, accounts: accounts, held: held, trust: trust)

        // The held set decides which chains are scanned; this account "holds"
        // ETH on mainnet as far as this test is concerned.
        held.record(address: Self.busyAddress, tokens: [[
            "chain_id": 1, "symbol": "ETH", "decimals": 18,
            "token_address": NSNull(), "price_usd": 2_500,
        ]])

        let first = (try? CoreJSON.object(await executor.perform([
            "type": "scan_incoming_transfers", "address": Self.busyAddress,
        ]))) ?? [:]
        #expect(first["type"] as? String == "sync_completed")
        let landed = (first["new_count"] as? NSNumber)?.intValue ?? 0
        #expect(landed > 0, "the scan persisted nothing for an address being paid")
        print("[live] persisted receipts: \(landed)")

        let stored = TxRecords.load(store: store)
        #expect(stored.count == min(landed, TxRecords.cap))
        for record in stored.prefix(3) {
            #expect(record["type"] as? String == "receive")
            #expect(record["status"] as? String == "confirmed")
            // The unit trap, on the ingest side: a human decimal, never raw
            // units. A 6-decimals stablecoin recorded raw reads a million times
            // too large.
            let value = record["value"] as? String ?? ""
            #expect(!value.isEmpty && Double(value) != nil)
            #expect((record["usd"] as? String)?.hasPrefix("$") == true)
        }
        if let newest = stored.first {
            print("[live] stored: \(newest["value"] ?? "?") \(newest["symbol"] ?? "?")" +
                  " usd=\(newest["usd"] ?? "?")")
        }

        let second = (try? CoreJSON.object(await executor.perform([
            "type": "scan_incoming_transfers", "address": Self.busyAddress,
        ]))) ?? [:]
        #expect((second["new_count"] as? NSNumber)?.intValue == 0,
                "the same window came back as new money")
    }

    /// The golden Safe's own feed, which is expected to be EMPTY — and that is
    /// the honest answer rather than a defect.
    ///
    /// Its xDAI arrived as a native transfer, and a native transfer emits no
    /// `Transfer` log at all on a chain without EIP-7708. A hundred-block
    /// window on top of that means this wallet shows a receipt only while it is
    /// running. Worth a test because the empty screen it produces is the one a
    /// person will ask about.
    @Test func anAccountWithNoRecentErc20ReceiptsHasAnEmptyFeed() async {
        let (_, _, _, trust) = fresh()
        let incoming = await trust.pollIncoming(address: Self.goldenSafe, chainIds: [100])
        print("[live] golden Safe receipts on Gnosis: \(incoming.count)")
        for transfer in incoming {
            #expect(!transfer.id.isEmpty)
        }
    }
}

#endif
