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
