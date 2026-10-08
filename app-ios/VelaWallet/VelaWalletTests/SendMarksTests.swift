//
//  SendMarksTests.swift
//  VelaWalletTests
//
//  The send flow's marks (DESIGN L): a NETWORK wears the network's own mark,
//  a COIN wears the coin's — the core's kind rule — and no mark is built on
//  chain 0, on a fixture's network, or from an empty ticker. The views are the
//  core's own starting view with a few fields patched, as the other send
//  tests build them.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendMarksTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private static let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let alice = "0x1111111111111111111111111111111111111111"
    /// USDC on Base, by its real contract.
    private static let usdcBase = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"

    private func sendView(_ patch: [String: Any]) -> SendViewWire {
        var object = try! CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func token(
        _ symbol: String, chainId: Int, address: String? = nil, balance: String = "1"
    ) -> [String: Any] {
        [
            "symbol": symbol, "network": "chain-\(chainId)", "chain_id": chainId,
            "token_address": address.map { $0 as Any } ?? NSNull(), "decimals": address == nil ? 18 : 6,
            "balance": balance, "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
    }

    private func drawnPick() -> SendPickModel {
        guard case .sendPick(let model) = WalletFlowFixtures.build(.sd1b, loc: loc).base
        else { fatalError("SD1b does not draw the picker") }
        return model
    }

    private func drawnConfirm(_ state: FlowStateId) -> SendConfirmModel {
        guard case .sendConfirm(let model) = WalletFlowFixtures.build(state, loc: loc).base
        else { fatalError("\(state) does not draw the confirm page") }
        return model
    }

    private func isNetwork(_ mark: TokenMarkModel?, _ chainId: Int) -> Bool {
        guard let mark else { return false }
        return mark.logoURLs == [Marks.chainLogoURL(chainId)].compactMap { $0 } && mark.badgeHidden
    }

    // MARK: - Notices name a network in the network's mark

    /// The sweep's notice ("1 token · Base") wears the chain it is locked
    /// to — never the drawing's Ethereum "ETH", never a blank disc.
    @Test func theSweepNoticeWearsTheLockedChainsOwnMark() throws {
        let view = sendView([
            "multi_select_mode": true, "multi_chain_id": 8453,
            "multi_selected_ids": ["chain-8453_native_ETH"],
            "tokens": [token("ETH", chainId: 8453)],
        ])
        let notice = try #require(SendLive.pick(view, on: drawnPick(), picking: true, loc: loc).notice)
        #expect(isNetwork(notice.mark, 8453), "Base's logo, no badge")
        #expect(notice.mark?.glyph == "ETH", "its coin's letters under it")
        #expect(notice.text.contains("Base"))
    }

    /// A scanned code's network wears that network's mark.
    @Test func aScannedCodesNetworkWearsItsOwnMark() throws {
        let view = sendView(["request_chain_id": 56, "tokens": [token("BNB", chainId: 56)]])
        let notice = try #require(SendLive.requestNotice(view, loc: loc))
        #expect(isNetwork(notice.mark, 56))
        #expect(notice.mark?.glyph == "BNB")
    }

    /// A notice that names no network wears no mark: these were an empty grey
    /// disc. A code for a chain the wallet cannot name has no letters to fall
    /// back on, so it gets none either; a chain it can name gets its own.
    @Test func aNoticeThatNamesNoNetworkWearsNoMark() throws {
        let unknown = sendView(["lock_error": ["type": "network", "chain_id": 424242]])
        #expect(try #require(SendLive.lockNotice(unknown, loc: loc)).mark == nil)
        let token = sendView(["lock_error": ["type": "token"]])
        #expect(try #require(SendLive.lockNotice(token, loc: loc)).mark == nil)
        for tag in ["net_not_found", "net_not_compatible", "net_add_error"] {
            let failed = sendView(["add_network_msg": ["type": tag]])
            #expect(try #require(SendLive.lockNotice(failed, loc: loc)).mark == nil, "\(tag)")
        }
        let known = sendView(["lock_error": ["type": "network", "chain_id": 100]])
        #expect(isNetwork(try #require(SendLive.lockNotice(known, loc: loc)).mark, 100))
    }

    // MARK: - Coins wear the coin's mark

    private func drawnForm(_ state: FlowStateId = .sd2) -> SendFormModel {
        guard case .sendForm(let model) = WalletFlowFixtures.build(state, loc: loc).base
        else { fatalError("\(state) does not draw the form") }
        return model
    }

    /// The picker's rows wear each coin's own mark, as the home's rows do:
    /// BNB on BNB Chain is BNB's logo with no badge (it was "BNB" letters and
    /// a yellow dot); USDC on Base is USDC's asset logo, checksummed first,
    /// with Base's badge.
    @Test func thePickerRowsWearEachCoinsOwnMark() throws {
        let view = sendView(["tokens": [
            token("BNB", chainId: 56),
            token("USDC", chainId: 8453, address: Self.usdcBase),
        ]])
        let rows = SendLive.pick(view, on: drawnPick(), loc: loc).rows
        let bnb = try #require(rows.first?.mark)
        #expect(bnb.logoURLs == [Marks.chainLogoURL(56)].compactMap { $0 })
        #expect(bnb.badgeHidden)
        let usdc = try #require(rows.last?.mark)
        #expect(usdc.logoURLs.first?
            .hasSuffix("/assets/eip155-8453/0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913/logo.png") == true)
        #expect(usdc.logoURLs.count == 2, "the lowercase spelling second")
        #expect(!usdc.badgeHidden)
        #expect(usdc.badgeLogoURL == Marks.chainLogoURL(8453))
        #expect(rows.last?.chain == "Base")
    }

    /// The URLs the core already named for a coin come first.
    @Test func aCoinsNamedLogoComesFirst() throws {
        var usdc = token("USDC", chainId: 8453, address: Self.usdcBase)
        usdc["logo_urls"] = ["https://logos.example/usdc.png"]
        let rows = SendLive.pick(sendView(["tokens": [usdc]]), on: drawnPick(), loc: loc).rows
        #expect(try #require(rows.first?.mark).logoURLs.first == "https://logos.example/usdc.png")
    }

    /// A sweep's rows carry each coin's mark to the form (the body used to
    /// drop it and draw letters with a dot).
    @Test func aSweepsRowsWearEachCoinsOwnMark() {
        let view = sendView([
            "multi_select_mode": true, "multi_chain_id": 8453, "recipient": Self.alice,
            "multi_selected_ids": ["chain-8453_native_ETH", "chain-8453_\(Self.usdcBase)_USDC"],
            "tokens": [token("ETH", chainId: 8453), token("USDC", chainId: 8453, address: Self.usdcBase)],
        ])
        let form = SendLive.form(view, fee: nil, display: .usd, on: drawnForm(), loc: loc)
        #expect(form.sweepRows.map(\.symbol) == ["ETH", "USDC"])
        #expect(form.sweepRows[0].mark.logoURLs == [Marks.chainLogoURL(1)].compactMap { $0 })
        #expect(form.sweepRows[0].mark.badgeLogoURL == Marks.chainLogoURL(8453))
        #expect(form.sweepRows[1].mark.logoURLs.first?.contains("/assets/eip155-8453/") == true)
    }

    // MARK: - The fee coin

    /// A fee estimate as the send machine holds it, paid in `asset`.
    private func estimate(chainId: Int, asset: [String: Any], tier: String = "fast") -> [String: Any] {
        [
            "chain_id": chainId, "total_wei": "2100000000000000", "max_fee_per_gas": "2000000000",
            "network_fee_per_gas": "1000000000", "relayer_fee_per_gas": "1000000000",
            "bundler_gas_price": "1000000000", "in_band_gas_basis": "0",
            "effective_gas_price": NSNull(), "max_gas_price": NSNull(), "total_gas": "21000",
            "deployed": true, "tier": tier, "quoted": true, "fee_asset": asset,
            "fee_recipient": "0xfee",
        ]
    }

    private static let usdcFee: [String: Any] = [
        "type": "erc20", "token": usdcBase, "decimals": 6, "amount": "50000", "symbol": "USDC",
    ]

    /// The fee session in force while it measures: no figure, pricing in
    /// `feeToken` (`nil` = the chain's own coin), its rows not back yet.
    private func measuring(feeToken: String?, options: [FeeOptionWire] = []) -> FeeViewWire {
        FeeViewWire(busy: true, failed: nil, fee: nil, stale: false, feeToken: feeToken,
                    options: options, confirmFeeReady: false)
    }

    /// The speed control with `tier` in force — picked by the person, its own
    /// figure not in yet.
    private func speed(_ tier: String) -> SendLive.SpeedInputs {
        SendLive.SpeedInputs(
            view: FeeSpeedViewWire(
                tier: tier, preferred: "fast", previews: [], open: false, picked: true,
                free: false, freeNote: false, single: false, gasPriceLine: false, options: []
            ),
            feeView: { _ in nil }
        )
    }

    /// A USDC fee wears USDC's own logo — by its contract, which the row
    /// left out, so it would have worn the chain's logo by the native-coin
    /// rule — with the chain's badge.
    @Test func anERC20FeeWearsItsOwnContractsLogo() {
        let view = sendView([
            "selected_token": token("ETH", chainId: 8453),
            "fee": estimate(chainId: 8453, asset: [
                "type": "erc20", "token": Self.usdcBase, "decimals": 6, "amount": "50000", "symbol": "USDC",
            ]),
        ])
        let row = SendLive.form(view, fee: nil, display: .usd, on: drawnForm(), loc: loc).fee
        #expect(row.mark.ticker == "USDC")
        #expect(row.mark.logoURLs.first?
            .hasSuffix("/assets/eip155-8453/0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913/logo.png") == true)
        #expect(!row.mark.logoURLs.contains { $0.contains("/chainlogos/") })
        #expect(row.mark.badgeLogoURL == Marks.chainLogoURL(8453))
    }

    /// With no estimate in hand (a quote out, or one that failed), the row
    /// wears the send's chain's own coin — BNB on a BNB Chain send — never
    /// the drawing's ETH.
    @Test func withNoEstimateTheFeeRowWearsTheChainsOwnCoin() {
        let view = sendView(["selected_token": token("BNB", chainId: 56)])
        let row = SendLive.form(view, fee: nil, display: .usd, on: drawnForm(), loc: loc).fee
        #expect(row.mark.ticker == "BNB")
        #expect(row.mark.logoURLs == [Marks.chainLogoURL(56)].compactMap { $0 })
        #expect(row.mark.badgeHidden)
        // No token at all: no coin to name, and no drawing's coin either.
        let none = SendLive.form(sendView([:]), fee: nil, display: .usd, on: drawnForm(), loc: loc).fee
        #expect(none.mark.ticker.isEmpty)
        #expect(none.mark.logoURLs.isEmpty)
    }

    /// A USDC fee keeps USDC's own logo while a newly picked speed is
    /// measured (the desktop's rule): the estimate in hand is the speed just
    /// left, the session in force has no figure yet, and the coin does not
    /// change with the speed — so the row never drops to the chain's coin for
    /// that moment, as the web's did.
    @Test func theFeeCoinKeepsItsLogoWhileANewSpeedIsMeasured() throws {
        let view = sendView([
            "selected_token": token("ETH", chainId: 8453),
            "fee": estimate(chainId: 8453, asset: Self.usdcFee, tier: "fast"),
        ])
        let usdc = TokenMarkModel.of(chainId: 8453, symbol: "USDC", tokenAddress: Self.usdcBase,
                                     color: SendLive.chainColor(8453))
        // Fast is the estimate's own speed; slow is a pick still measuring.
        for tier in ["fast", "slow"] {
            let row = SendLive.form(view, fee: measuring(feeToken: Self.usdcBase), display: .usd,
                                    on: drawnForm(), loc: loc, speed: speed(tier)).fee
            #expect(row.mark.ticker == "USDC", "\(tier)")
            #expect(row.mark.logoURLs == usdc.logoURLs, "\(tier)")
            #expect(row.mark.badgeLogoURL == usdc.badgeLogoURL, "\(tier)")
        }
        let slow = SendLive.form(view, fee: measuring(feeToken: Self.usdcBase), display: .usd,
                                 on: drawnForm(), loc: loc, speed: speed("slow")).fee
        #expect(slow.value == loc.t("send.estimatingFee"), "the old speed's figure never shows")
    }

    /// With no estimate in hand at all, the row names the coin the fee
    /// session is pricing in — the coin that will pay — named by the form's
    /// holdings when the session's rows are not back; with no coin chosen,
    /// the chain's own.
    @Test func withNoEstimateTheRowNamesTheCoinBeingPriced() {
        let view = sendView([
            "selected_token": token("ETH", chainId: 8453),
            "tokens": [token("ETH", chainId: 8453), token("USDC", chainId: 8453, address: Self.usdcBase)],
        ])
        let usdc = SendLive.form(view, fee: measuring(feeToken: Self.usdcBase), display: .usd,
                                 on: drawnForm(), loc: loc, speed: speed("slow")).fee
        #expect(usdc.mark.ticker == "USDC")
        #expect(usdc.mark.logoURLs.first?
            .hasSuffix("/assets/eip155-8453/0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913/logo.png") == true)
        let native = SendLive.form(view, fee: measuring(feeToken: nil), display: .usd,
                                   on: drawnForm(), loc: loc, speed: speed("slow")).fee
        #expect(native.mark.ticker == "ETH")
        #expect(native.mark.logoURLs == TokenMarkModel.of(chainId: 8453, symbol: "ETH",
                                                           color: SendLive.chainColor(8453)).logoURLs)
    }

    /// A quote that failed on BNB Chain — the simulator's "ETH —" for the
    /// golden Safe — names BNB, the chain's own coin, beside its "—"; a coin
    /// the person chose (USDT) is named instead. Never the drawing's ETH.
    @Test func aFailedQuoteNamesTheChainsCoinOrTheChosenOne() {
        let usdt = "0x55d398326f99059ff775485246999027b3197955"
        let view = sendView([
            "selected_token": token("BNB", chainId: 56),
            "tokens": [token("BNB", chainId: 56), token("USDT", chainId: 56, address: usdt)],
        ])
        func failed(_ feeToken: String?) -> FeeViewWire {
            FeeViewWire(busy: false, failed: "quote_unavailable", fee: nil, stale: false,
                        feeToken: feeToken, options: [], confirmFeeReady: false)
        }
        let bnb = SendLive.form(view, fee: failed(nil), display: .usd, on: drawnForm(), loc: loc).fee
        #expect(bnb.mark.ticker == "BNB")
        #expect(bnb.mark.logoURLs == [Marks.chainLogoURL(56)].compactMap { $0 })
        #expect(bnb.value == "—")
        let chosen = SendLive.form(view, fee: failed(usdt), display: .usd, on: drawnForm(), loc: loc).fee
        #expect(chosen.mark.ticker == "USDT")
        #expect(chosen.mark.logoURLs.first?.contains("/assets/eip155-56/") == true)
        #expect(chosen.mark.badgeLogoURL == Marks.chainLogoURL(56))
        #expect(chosen.value == "—")
        // And before the fee session exists at all, the chain's coin too.
        let none = SendLive.form(view, fee: nil, display: .usd, on: drawnForm(), loc: loc).fee
        for row in [bnb, chosen, none] {
            #expect(row.mark.ticker != "ETH" && row.mark.glyph != "ETH")
        }
    }

    /// The fee-coin sheet before any quote was asked draws its chrome and no
    /// coins — never the drawing's ETH, USDC and USDT at its own balances,
    /// which stood in on a live send.
    @Test func theFeeCoinSheetBeforeAnyQuoteDrawsNoDrawnCoins() {
        guard case .feeToken(let drawn)? = WalletFlowFixtures.build(.sd2f, loc: loc).sheet
        else { fatalError("SD2f does not draw the fee sheet") }
        #expect(drawn.rows.contains { $0.symbol == "ETH" }, "the drawing that used to stand in")
        let reading = SendLive.feeSheetReading(on: drawn)
        #expect(reading.rows.isEmpty)
        #expect(reading.title == drawn.title)
        #expect(reading.hint == drawn.hint)
    }

    /// The fee-coin sheet with no estimate names the SEND's chain — never 0.
    @Test func theFeeCoinSheetNeverAsksForChainZero() {
        func option(_ symbol: String, contract: String?) -> FeeOptionWire {
            FeeOptionWire(
                symbol: symbol, contract: contract, decimals: 18, balance: "0", recipient: "0x1",
                usdBalance: "0", usdPrice: "1", amount: nil, insufficient: false, selected: contract == nil
            )
        }
        let fee = FeeViewWire(
            busy: true, failed: nil, fee: nil, stale: false, feeToken: nil,
            options: [option("BNB", contract: nil), option("USDT", contract: "0x55d398326f99059ff775485246999027b3197955")],
            confirmFeeReady: false
        )
        guard case .feeToken(let drawn)? = WalletFlowFixtures.build(.sd2f, loc: loc).sheet
        else { fatalError("SD2f does not draw the fee sheet") }
        let sheet = SendLive.feeSheet(fee, on: drawn, loc: loc, chainId: 56)
        let urls = sheet.rows.flatMap { $0.mark.logoURLs + [$0.mark.badgeLogoURL].compactMap { $0 } }
        #expect(!urls.isEmpty)
        #expect(!urls.contains { $0.contains("eip155-0") })
        #expect(sheet.rows[0].mark.logoURLs == [Marks.chainLogoURL(56)].compactMap { $0 })
        #expect(sheet.rows[1].mark.logoURLs.first?.contains("/assets/eip155-56/") == true)
        // With no chain at all: letters, and nothing asked of anybody.
        let blind = SendLive.feeSheet(fee, on: drawn, loc: loc)
        #expect(blind.rows.allSatisfy { $0.mark.logoURLs.isEmpty && $0.mark.badgeHidden })
    }

    /// A sweep's receipt with no chain in hand draws its coins' named logos,
    /// never a URL on chain 0.
    @Test func aReceiptWithNoChainNeverAsksForChainZero() {
        let view = sendView([
            "receipt": [
                "status": "submitted", "hold_reason": NSNull(), "kind": "multi_select",
                "transfers": [[String: Any]](), "amount": "", "usd_value": 0,
                "submitted_at_ms": NSNull(), "typical_inclusion_s": NSNull(),
                "coins": [
                    ["symbol": "ETH", "amount": "0.1", "token_address": NSNull(),
                     "logo_urls": [String](), "usd_value": 0],
                    ["symbol": "USDC", "amount": "5", "token_address": Self.usdcBase,
                     "logo_urls": ["https://logos.example/usdc.png"], "usd_value": 0],
                ],
            ],
        ])
        let parts = SendLive.receiptParts(view, symbol: "", loc: loc)
        #expect(parts.rows.count == 2)
        let urls = parts.rows.flatMap { ($0.lead?.logoURLs ?? []) + [$0.lead?.badgeLogoURL].compactMap { $0 } }
        #expect(!urls.contains { $0.contains("eip155-0") })
        #expect(parts.rows[1].lead?.logoURLs == ["https://logos.example/usdc.png"])
    }

    // MARK: - The confirm page

    /// The confirm page's From row has the account's face, and its Network
    /// row the NETWORK's own mark: ETH sent on Base is "Base" beside Base's
    /// logo — never Ethereum's, the coin's.
    @Test func theConfirmsFromHasAFaceAndItsNetworkTheNetworksMark() {
        let view = sendView([
            "stage": "confirm", "recipient": Self.alice, "confirm_amount": "0.1",
            "selected_token": token("ETH", chainId: 8453),
        ])
        let model = SendLive.confirm(view, from: (Self.me, nil), display: .usd,
                                     on: drawnConfirm(.sd3), loc: loc)
        guard case .identicon(let seed)? = model.facts[0].lead else {
            Issue.record("the From row has no face")
            return
        }
        #expect(seed == Self.me)
        #expect(model.facts[2].value == "Base")
        guard case .token(let mark)? = model.facts[2].lead else {
            Issue.record("the Network row has no mark")
            return
        }
        #expect(isNetwork(mark, 8453))
        #expect(!mark.logoURLs.contains { $0.hasSuffix("eip155-1.png") }, "the coin's logo on a network row")
        // The coin itself, above, is still the coin: Ethereum's logo with
        // Base's badge.
        #expect(model.mark?.logoURLs == [Marks.chainLogoURL(1)].compactMap { $0 })
        #expect(model.mark?.badgeHidden == false)
    }

    /// A sweep's Network row is the chain it is locked to.
    @Test func aSweepsConfirmNamesTheLockedChain() {
        let view = sendView([
            "stage": "confirm", "multi_select_mode": true, "recipient": Self.alice,
            "multi_chain_id": 8453, "multi_selected_ids": ["chain-8453_native_ETH"],
            "tokens": [token("ETH", chainId: 8453)],
        ])
        let model = SendLive.confirm(view, from: (Self.me, nil), display: .usd,
                                     on: drawnConfirm(.sd3c), loc: loc)
        let network = model.facts.first { $0.value == "Base" }
        guard case .token(let mark)? = network?.lead else {
            Issue.record("the sweep's Network row has no mark")
            return
        }
        #expect(isNetwork(mark, 8453))
    }
}
