//
//  ParityTests.swift
//  VelaWalletTests
//
//  Spec 058 — the differences the two phones had, asserted so they stay closed.
//
//  Every case here is something that was DRAWN on iOS and did nothing, or that
//  Android said and iOS could not. The views are decoded from the real cores
//  and patched, the way `SendPickFixture` does, so a fixture here cannot drift
//  away from the wire it stands for.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ParityTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - 转账: the receipt says which of three things is happening

    private func sendView(
        txStatus: String,
        receiptStatus: String? = nil,
        holdReason: String? = nil
    ) -> SendViewWire {
        let core = SendCore()
        var object = try! CoreJSON.object(core.view())
        object["tx_status"] = txStatus
        if let receiptStatus {
            object["receipt"] = [
                "status": receiptStatus,
                "hold_reason": holdReason ?? NSNull(),
                "kind": NSNull(),
                "transfers": [],
                "amount": "0.001",
                "usd_value": 0,
                "submitted_at_ms": NSNull(),
                "typical_inclusion_s": NSNull(),
            ] as [String: Any]
        }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func receipt(_ view: SendViewWire) -> SendReceiptModel {
        guard case .sendReceipt(let drawn) = WalletFlowFixtures.build(.sd4a, loc: loc).base else {
            Issue.record("sd4a is not a receipt")
            fatalError("unreachable")
        }
        return SendLive.receipt(view, display: .usd, on: drawn, loc: loc)
    }

    /// One sentence for three states told a person the phone was submitting
    /// while it was waiting for their finger.
    @Test func theReceiptNamesPreparingSigningAndSubmittingSeparately() {
        let preparing = receipt(sendView(txStatus: "idle")).title
        let signing = receipt(sendView(txStatus: "signing")).title
        let submitting = receipt(sendView(txStatus: "submitting")).title

        #expect(preparing == loc.t("send.txPreparing"))
        #expect(signing == loc.t("send.txSigning"))
        #expect(submitting == loc.t("send.txSubmitting"))
        #expect(Set([preparing, signing, submitting]).count == 3)
    }

    /// While the passkey prompt is up the one honest button is 取消 — and it
    /// must not navigate, because the prompt has to be answered where it is.
    @Test func theCeremonysButtonCancelsAndDoesNotLeave() {
        let signing = receipt(sendView(txStatus: "signing"))
        #expect(signing.cta == loc.t("componentsUi.funding.cancel"))
        #expect(signing.ctaCancels)

        let idle = receipt(sendView(txStatus: "idle"))
        #expect(idle.cta == loc.t("send.txCloseBackground"))
        #expect(!idle.ctaCancels)
    }

    /// `hold_reason` was on the wire and read by nothing: a payment waiting for
    /// fees to settle looked exactly like one that had failed.
    @Test func aFeeHoldExplainsItselfAndAFeeRejectionSaysNothingWasSent() {
        let held = receipt(sendView(txStatus: "submitted", receiptStatus: "submitted",
                                    holdReason: "fee_hold"))
        #expect(held.captions.contains(loc.t("send.txHeldFees")))

        let rejected = receipt(sendView(txStatus: "idle", receiptStatus: "failed",
                                        holdReason: "fee_rejected"))
        #expect(rejected.captions == [loc.t("send.txRejectedFees")])

        // A plain failure is still a plain failure.
        let failed = receipt(sendView(txStatus: "idle", receiptStatus: "failed"))
        #expect(!failed.captions.contains(loc.t("send.txRejectedFees")))
    }

    // MARK: - 收款: a code for one asset

    private func qr(asset: PaymentRequestAssetWire?) -> ReceiveQrModel {
        guard case .receiveQr(let drawn)? = WalletFlowFixtures.build(.r3, loc: loc).sheet else {
            Issue.record("r3 has no code sheet")
            fatalError("unreachable")
        }
        return FlowsLive.receiveQr(
            "0x88cCA0dE2Ee0aB4E4C0b0dB4bA6b1d8b2b6d6894",
            name: "Vela",
            chain: ChainCatalog.meta(100),
            asset: asset,
            on: drawn, loc: loc
        )
    }

    @Test func aTokensCodeNamesTheTokenAndPrintsItsContract() {
        let contract = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
        let sheet = qr(asset: PaymentRequestAssetWire(
            chainId: 100, tokenAddress: contract, symbol: "USDC",
            decimals: 6, networkName: "Gnosis"
        ))
        #expect(sheet.title == loc.t("receive.qrTitleAsset",
                                     vars: ["symbol": "USDC", "network": "Gnosis"]))
        #expect(sheet.centre.ticker == "USDC")
        // The WHOLE contract is what a copy has to produce.
        #expect(sheet.contract?.copyValue == contract)
    }

    /// A network's code has no contract line — leaving R3's up would print a
    /// stranger's contract over a code for the chain's own coin.
    @Test func aNetworksCodeHasNoContractLine() {
        let sheet = qr(asset: PaymentRequestAssetWire(
            chainId: 100, tokenAddress: nil, symbol: "XDAI",
            decimals: 18, networkName: "Gnosis"
        ))
        #expect(sheet.contract == nil)
        #expect(sheet.title == loc.t("receive.qrTitleNetwork", vars: ["network": "Gnosis"]))
        #expect(qr(asset: nil).contract == nil)
    }

    // MARK: - Copies that copy

    /// Every copy affordance in the app showed a checkmark and left the
    /// clipboard alone. The model is where the fix is provable: an ellipsed
    /// value must carry the whole one.
    @Test func anEllipsedFactCarriesTheWholeValueToCopy() throws {
        let counterparty = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
        let hash = "0xabc1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcd"
        let item = try Self.feedItem(direction: "out", counterparty: counterparty, hash: hash)

        guard case .txDetail(let drawn)? = WalletFlowFixtures.build(.a3, loc: loc).sheet else {
            Issue.record("a3 has no detail sheet")
            return
        }
        let detail = FlowsLive.txDetail(item, record: nil, on: drawn, loc: loc)
        let copyable = detail.facts.filter { $0.copy != nil }
        #expect(!copyable.isEmpty)
        for fact in copyable {
            let value = try #require(fact.copyValue)
            // The point of the field: what is copied is never the ellipsis.
            #expect(!value.contains("…"))
        }
        #expect(copyable.contains { $0.copyValue == counterparty })
        #expect(copyable.contains { $0.copyValue == hash })
    }

    /// 删除记录 was drawn on the web, never labelled anywhere, and reachable on
    /// no client. The corpus sentence has been there the whole time.
    @Test func theTransactionDetailOffersToDeleteTheLocalRecord() throws {
        let item = try Self.feedItem(direction: "in", counterparty: nil, hash: nil)
        guard case .txDetail(let drawn)? = WalletFlowFixtures.build(.a2, loc: loc).sheet else {
            Issue.record("a2 has no detail sheet")
            return
        }
        let detail = FlowsLive.txDetail(item, record: nil, on: drawn, loc: loc)
        #expect(detail.deleteLabel == loc.t("history.deleteRecord"))
        #expect(detail.deleteLabel != "history.deleteRecord")
    }

    /// One feed row, in the core's own wire shape. Decoded rather than built
    /// with an initialiser so a field that changes name in the core breaks this
    /// file rather than passing with a default.
    private static func feedItem(
        direction: String,
        counterparty: String?,
        hash: String?
    ) throws -> FeedItemWire {
        try CoreJSON.decode(FeedItemWire.self, from: [
            "id": "t1",
            "direction": direction,
            "counterparty": counterparty ?? NSNull(),
            "alias": NSNull(),
            "value": "0.001",
            "symbol": "XDAI",
            "decimals": 18,
            "usd_value": 0,
            "chain_id": 100,
            "timestamp": 1_700_000_000,
            "day_start_ms": 1_700_000_000_000,
            "tx_hash": hash ?? NSNull(),
            "batch": NSNull(),
        ])
    }

    // MARK: - 设置: measured, not drawn

    @Test func storageWeighsTheStoresOwnKeysAndClearsExactlyThose() {
        let defaults = UserDefaults(suiteName: "vela.parity.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        store.writeList(VelaStore.Key.transactionHistory, [["id": "a"], ["id": "b"]])
        store.writeString(VelaStore.Key.balanceCache, #"{"x":1}"#)

        let before = DeviceStorage.measure(store)
        #expect(before.item("transactions")?.records == 2)
        #expect((before.item("transactions")?.bytes ?? 0) > 0)
        #expect(before.totalBytes == before.items.reduce(0) { $0 + $1.bytes })
        // A cache key is filed under the cache group, which is what the ring's
        // three fractions are drawn from.
        #expect(before.bytes(of: .cache) > 0)

        DeviceStorage.clear(store, item: "transactions")
        let after = DeviceStorage.measure(store)
        #expect(after.item("transactions")?.bytes == 0)
        // And nothing else went with it.
        #expect(after.bytes(of: .cache) == before.bytes(of: .cache))
    }

    /// A value that is not a list has no record count — the row shows a size
    /// alone rather than an invented "1".
    @Test func onlyListsAreCounted() {
        #expect(DeviceStorage.records(in: #"[1,2,3]"#) == 3)
        #expect(DeviceStorage.records(in: #"{"items":[1,2]}"#) == 2)
        #expect(DeviceStorage.records(in: #""just a string""#) == nil)
        #expect(DeviceStorage.records(in: "not json") == nil)
    }

    @Test func theAboutPageNamesTheRunningBuild() {
        let model = SettingsLive.withAbout(
            version: "9.9", commit: "deadbee", networkCount: 3,
            on: SettingsFixtures.build(.st14, loc: loc), loc: loc
        )
        #expect(model.about.version.contains("9.9"))
        #expect(model.about.version.contains("deadbee"))
        // The technical row is this device's count, not the drawing's twelve.
        #expect(model.about.rows.contains { $0.value.contains("3") })
        #expect(!model.about.version.contains("6ab8f"))
    }

    // MARK: - The hero's status line

    private func balance(rateLimited: [Int], failed: [Int], hidden: Bool = false) -> BalanceViewWire {
        let core = BalanceDashboardCore()
        var object = try! CoreJSON.object(core.view())
        object["rate_limited_chain_ids"] = rateLimited
        object["banner_chain_ids"] = failed
        object["hidden"] = hidden
        object["display_total_usd"] = 12.5
        object["tokens"] = [[
            "chain_id": 1, "symbol": "ETH", "name": "Ether", "balance": "0.005",
            "decimals": 18, "token_address": NSNull(), "price_usd": 2500, "spam": false,
        ]]
        return try! CoreJSON.decode(BalanceViewWire.self, from: object)
    }

    /// A rate-limited chain gets no button because it resolves itself; an
    /// unreachable one gets 立即重试 because it does not.
    @Test func theBreakdownOffersRetryOnlyWhereRetryingIsTheAnswer() {
        let model = SettingsLive.withBalanceDetail(
            balance(rateLimited: [137], failed: [100]),
            display: .usd,
            on: SettingsFixtures.build(.sr3, loc: loc),
            loc: loc
        )
        let limited = model.balanceDetail.pending.first { $0.id == "137" }
        let dead = model.balanceDetail.pending.first { $0.id == "100" }
        #expect(limited?.action == nil)
        #expect(limited?.tone == .neutral)
        #expect(dead?.action == loc.t("home.balanceDetailRetry"))
        #expect(dead?.tone == .error)
        // The chain that answered is in the settled list, with its figure.
        #expect(model.balanceDetail.done.contains { $0.id == "1" })
    }

    /// A person who hid the figure did not agree to have it broken out.
    @Test func aHiddenBalanceStaysHiddenInTheBreakdown() {
        let model = SettingsLive.withBalanceDetail(
            balance(rateLimited: [], failed: [], hidden: true),
            display: .usd,
            on: SettingsFixtures.build(.sr3, loc: loc),
            loc: loc
        )
        #expect(model.balanceDetail.done.allSatisfy { ($0.amount ?? "").allSatisfy { $0 == "•" } })
        #expect(model.balanceDetail.summary.contains("••••"))
    }

    /// The fix names the chain that failed, and shows what is stored for it —
    /// the drawing's Polygon was whichever chain the mock chose.
    @Test func theRpcFixIsAboutTheChainThatFailed() {
        let model = SettingsLive.withRpcFix(
            chainId: 100, endpoint: "https://rpc.gnosischain.example",
            on: SettingsFixtures.build(.sr2, loc: loc), loc: loc
        )
        #expect(model.rpcFix.name == "Gnosis")
        #expect(model.rpcFix.field.value == "https://rpc.gnosischain.example")
    }

    // MARK: - 语言

    /// The stored choice decides the app's language. It was written by the
    /// settings page and read by nothing, on any launch.
    @Test func theStoredLanguageIsTheOneTheAppSpeaks() {
        let target = Loc(overrideTag: nil, preferredLanguages: ["en-US"])
        #expect(target.resolvedLanguage == "en")

        target.apply("ja")
        #expect(target.resolvedLanguage == "ja")
        // And the words change with it — the same key, two languages.
        let japanese = target.t("componentsTx.receipt.done")
        target.apply("en")
        #expect(target.resolvedLanguage == "en")
        #expect(japanese != target.t("componentsTx.receipt.done"))
    }

    /// `auto` means the device decides, which is what every other client
    /// stores for "system".
    @Test func autoFollowsTheDevice() {
        let target = Loc(overrideTag: nil, preferredLanguages: ["ja-JP"])
        #expect(target.resolvedLanguage == "ja")
        target.apply("de")
        #expect(target.resolvedLanguage == "de")
        target.apply("auto")
        #expect(target.resolvedLanguage == "ja")
    }
}
