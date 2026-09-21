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
import SwiftUI
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

    /// #199: the core says WHEN the op was handed over; the receipt counts
    /// against the chain's usual time instead of sitting on a still clock.
    @Test func aSubmittedReceiptCountsDownThenUpThenSaysItIsSlow() throws {
        func view(submittedAt: Any) throws -> SendViewWire {
            var object = try CoreJSON.object(SendCore().view())
            object["tx_status"] = "submitted"
            object["receipt"] = [
                "status": "submitted", "hold_reason": NSNull(), "kind": NSNull(), "transfers": [],
                "amount": "1", "usd_value": 0, "submitted_at_ms": submittedAt, "typical_inclusion_s": 10,
            ] as [String: Any]
            return try CoreJSON.decode(SendViewWire.self, from: object)
        }
        let model = receipt(try view(submittedAt: 1_000_000))
        let eta = try #require(model.eta)
        #expect(model.captions == [loc.t("send.txWaitingConfirm")], "the typical line moves into the clock")
        #expect(eta.lines(nowMs: 1_004_000)[1] == loc.t("send.txRemaining", vars: ["remaining": "6"]))
        #expect(eta.lines(nowMs: 1_013_000)[1] == loc.t("send.txElapsed", vars: ["elapsed": "13"]))
        #expect(eta.lines(nowMs: 1_025_000)[1] == loc.t("send.txSlowConfirm"))
        #expect(eta.lines(nowMs: 1_000_000)[0].contains("10"))
        #expect(eta.progress(nowMs: 1_010_000) > 0.6 && eta.progress(nowMs: 9_000_000) <= 0.92)

        // Without the moment, the typical line is still said, as before.
        let untimed = receipt(try view(submittedAt: NSNull()))
        #expect(untimed.eta == nil)
        #expect(untimed.captions.count == 2)
    }

    /// #261: a split's receipt names its count and every person in it, from
    /// the core's frozen transfers — not the first draft's address.
    @Test func aSplitReceiptListsEveryRecipient() throws {
        var object = try CoreJSON.object(SendCore().view())
        object["tx_status"] = "confirmed"
        func leg(_ to: String, _ name: Any, _ amount: String) -> [String: Any] {
            ["to": to, "to_name": name, "amount": amount, "symbol": "USDC", "logo_urls": [], "usd_value": 0]
        }
        object["receipt"] = [
            "status": "confirmed", "hold_reason": NSNull(), "kind": "split",
            "transfers": [
                leg("0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "Alice", "2"),
                leg("0x88cca0eedbf2c4426110bbfc998f048689266894", NSNull(), "3"),
            ],
            "amount": "5", "usd_value": 0, "submitted_at_ms": NSNull(), "typical_inclusion_s": NSNull(),
        ] as [String: Any]
        let model = receipt(try CoreJSON.decode(SendViewWire.self, from: object))
        let title = loc.t("send.recipientCount_other", vars: ["count": "2"])
        #expect(model.breakdownTitle == title)
        #expect(model.breakdown.map(\.label) == ["Alice", "0x88cc…6894"])
        #expect(model.breakdown.map(\.value) == ["2 USDC", "3 USDC"])
        #expect(model.captions.first?.hasPrefix(title) == true)
    }

    /// With no estimate the confirm's fee fact says so, as the form's row does
    /// — never the drawing's "~0.0021 ETH · ≈$0.55", which is somebody else's send.
    @Test func theConfirmFeeFactNeverShowsTheDrawingsFigure() throws {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            Issue.record("sd3 is not the confirm page")
            return
        }
        func built(busy: Bool) throws -> SendConfirmModel {
            var object = try CoreJSON.object(SendCore().view())
            object["fee"] = NSNull()
            object["fee_busy"] = busy
            object["estimating_gas"] = false
            return SendLive.confirm(
                try CoreJSON.decode(SendViewWire.self, from: object),
                from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
                display: .usd, on: drawn, loc: loc
            )
        }
        #expect(try built(busy: true).facts[3].value == loc.t("send.estimatingFee"))
        #expect(try built(busy: false).facts[3].value == "—")
        #expect(try built(busy: false).facts[3].value != drawn.facts[3].value)
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

    // MARK: - 分组

    /// The groups header offered 管理 — a page this client does not have — and
    /// nothing happened. Android's says 新建分组 and creates one.
    @Test func theGroupsHeaderOffersToCreateAGroup() throws {
        let group = try CoreJSON.decode(ContactGroupWire.self, from: [
            "id": "g1", "name": "Team", "color": NSNull(), "members": [],
        ])
        // With a contact in the book: an EMPTY book draws its own state and no
        // section headers at all — which is its own finding, below.
        let view = try CoreJSON.decode(ContactsViewWire.self, from: [
            "loaded": true,
            "contacts": [[
                "address": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "name": "Alice", "resolved_name": NSNull(), "resolved_source": NSNull(),
                "kind": "eoa", "favorite": false, "note": NSNull(), "tx_count": 0,
                "last_used_ms": 0, "first_seen_ms": 0, "source": "manual",
            ]],
            "sections": [], "groups": [
                ["id": "g1", "name": "Team", "color": NSNull(), "members": []],
            ],
            "last_import": NSNull(), "import_failure": NSNull(),
            "export": NSNull(), "recipient": NSNull(),
        ])
        let model = ContactsLive.home(view, loc: loc)
        #expect(model.groupsHeader?.action == loc.t("contacts.groupNew"))
        #expect(model.groupsHeader?.action != loc.t("contacts.manage"))
        #expect(group.name == "Team")
    }

    // MARK: - Logos (058, the founder's ask)

    /// A native coin wears its OWN chain's logo — ETH on Base is Ethereum's —
    /// and its badge disappears where it would repeat the coin.
    @Test func aNativeCoinWearsItsOwnChainsLogoAndNoRedundantBadge() {
        let eth = TokenMarkModel.of(chainId: 8453, symbol: "ETH", color: .clear)
        #expect(eth.logoURLs.first?.contains("eip155-1.png") == true)
        // On Base the badge is Base's: the coin and the chain differ.
        #expect(eth.badgeHidden == false)
        #expect(eth.badgeLogoURL?.contains("eip155-8453") == true)

        let xdai = TokenMarkModel.of(chainId: 100, symbol: "XDAI", color: .clear)
        #expect(xdai.badgeHidden)
        #expect(xdai.badgeLogoURL == nil)
    }

    /// A token's logo is its contract's path, checksummed first and lowercase
    /// second — the index carries both spellings and neither is guaranteed.
    @Test func aTokenOffersBothSpellingsOfItsContract() {
        let contract = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
        let mark = TokenMarkModel.of(chainId: 100, symbol: "USDC",
                                     tokenAddress: contract, color: .clear)
        #expect(mark.logoURLs.count == 2)
        #expect(mark.logoURLs[0] != mark.logoURLs[1])
        #expect(mark.logoURLs.allSatisfy { $0.lowercased().contains(contract) })
        // A token on a chain is badged with that chain.
        #expect(!mark.badgeHidden)
    }

    /// The core's own `logo_urls` win over any path this client guessed.
    @Test func aStatedLogoOutranksAGuessedPath() {
        let stated = "https://example.test/vela.png"
        let mark = TokenMarkModel.of(
            chainId: 1, symbol: "VELA",
            tokenAddress: "0x1111111111111111111111111111111111111111",
            color: .clear, named: [stated]
        )
        #expect(mark.logoURLs.first == stated)
    }

    // MARK: - The event the fan-out sends

    /// **The core must accept the snapshot this app actually sends.**
    ///
    /// It did not. `chain_assets_arrived` went without the `address` the core
    /// requires — "a stale account's stream can never paint the new account
    /// (invariant ⑤)" — so every mid-fetch snapshot was refused:
    ///
    ///     balance_dashboard fault: invalid event from shell: missing field `address`
    ///
    /// once per chain, per refresh, since 051. The progressive total the
    /// fan-out exists to paint never arrived. Found in the device console.
    ///
    /// The test hands the REAL core the REAL event: rebuilding the shape here
    /// would have passed throughout.
    @Test func theCoreAcceptsTheChainSnapshotThisAppSends() throws {
        let core = BalanceDashboardCore()
        let token: [String: Any] = [
            "chain_id": 100, "symbol": "XDAI", "name": "xDai", "balance": "0.5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "spam": false,
        ]
        let accepted = try CoreJSON.object(core.dispatch(eventJson: WalletStore.chainAssetsEvent(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", tokens: [token]
        )))
        // A refused event comes back as a fault, not a view.
        #expect(accepted["view"] != nil)

        // And the field really is required — which is what makes the fix a fix
        // rather than a coincidence.
        let refused = try? core.dispatch(eventJson: CoreJSON.string([
            "type": "chain_assets_arrived", "tokens": [token],
        ]))
        #expect(refused == nil)
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

    /// `VELA_LANG` pins the language, and a stored preference does not
    /// override it — that pin is how the screenshot sweep and the acceptance
    /// tests ask for one, and it broke the moment `apply` landed.
    @Test func anExplicitPinOutranksTheStoredChoice() {
        let pinned = Loc(overrideTag: "zh", preferredLanguages: ["en-US"])
        #expect(pinned.resolvedLanguage == "zh")
        pinned.apply("ja")
        #expect(pinned.resolvedLanguage == "zh")
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
