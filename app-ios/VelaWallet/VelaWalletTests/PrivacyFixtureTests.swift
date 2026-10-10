//
//  PrivacyFixtureTests.swift
//  VelaWalletTests
//
//  PR 2, hide balance: ONE masking rule in the core, and the shared fixture
//  every shell replays — `rust/crates/vela-core/tests/fixtures/privacy-hidden.json`,
//  a real balance and feed driven through the core, shown and hidden.
//
//  Replayed through this shell's own surface builders (the ones `RootView`
//  calls), it holds the four claims the fixture states:
//
//    1. shown — every forbidden digit run appears on some surface (so the
//       hidden check below is looking at real figures, not at nothing);
//    2. hidden — no masked surface's output contains one, and the masked
//       figures read the mask (the hero's own, longer mask on the total);
//    3. hidden — Send still draws its figures (the balance keeps them);
//    4. a feed row masks its figure exactly when the core's
//       `figure_maskable` says so.
//
//  Before this round iOS leaked on four of them: the holdings' quantities,
//  the token detail, the transfer and dApp details, and the switcher's total
//  (an empty, withheld list summed to $0.00).
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct PrivacyFixtureTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private struct Fixture: @unchecked Sendable {
        let raw: [String: Any]
        let forbidden: [String]
        let mask: String
        let balanceMask: String
        let figureMaskable: [String: Bool]
        let masked: [String]
        let visible: [String]
    }

    private static let fixture: Fixture = {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
        let url = repoRoot.appendingPathComponent("rust/crates/vela-core/tests/fixtures/privacy-hidden.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: Any] ?? [:]
        return Fixture(
            raw: json,
            forbidden: json["forbidden"] as? [String] ?? [],
            mask: json["mask"] as? String ?? "",
            balanceMask: json["balance_mask"] as? String ?? "",
            figureMaskable: json["figure_maskable"] as? [String: Bool] ?? [:],
            masked: json["masked_surfaces"] as? [String] ?? [],
            visible: json["visible_surfaces"] as? [String] ?? []
        )
    }()

    private func views(_ side: String) throws -> (BalanceViewWire, FeedViewWire) {
        let half = try #require(Self.fixture.raw[side] as? [String: Any])
        let balance = try CoreJSON.decode(BalanceViewWire.self, from: try #require(half["balance"] as? [String: Any]))
        let feed = try CoreJSON.decode(FeedViewWire.self, from: try #require(half["feed"] as? [String: Any]))
        return (balance, feed)
    }

    // MARK: - The surfaces, as RootView builds them

    private var home: WalletHomeModel { WalletFixtures.buildMobileState(.h1, loc: loc) }

    private var drawnAssets: AssetsModel {
        guard case .assets(let model) = WalletFlowFixtures.build(.t1, loc: loc).base
        else { fatalError("T1 does not draw the assets list") }
        return model
    }

    private var drawnHistory: HistoryModel {
        guard case .history(let model) = WalletFlowFixtures.build(.a1, loc: loc).base
        else { fatalError("A1 does not draw the history") }
        return model
    }

    private var drawnTxDetail: TxDetailModel {
        guard case .txDetail(let model)? = WalletFlowFixtures.build(.a2, loc: loc).sheet
        else { fatalError("A2 does not draw the transaction sheet") }
        return model
    }

    private var drawnTokenDetail: TokenDetailModel {
        guard case .tokenDetail(let model)? = WalletFlowFixtures.build(.t2, loc: loc).sheet
        else { fatalError("T2 does not draw the token sheet") }
        return model
    }

    /// The two accounts the fixture's switcher totals belong to.
    private func session(_ balance: BalanceViewWire) throws -> SessionView {
        try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": balance.address ?? "",
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [
                ["index": 0, "account": ["name": "Mine", "address": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]],
                ["index": 1, "account": ["name": "Spare", "address": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]],
            ],
        ])
    }

    /// Every masked surface's drawn text, by the fixture's surface names.
    private func maskedSurfaces(_ balance: BalanceViewWire, _ feed: FeedViewWire) throws -> [String: [String]] {
        let applied = WalletLive.apply(balance, feed: feed, feedRead: true, on: home, loc: loc)
        let hidden = feed.hidden
        let items = FlowsLive.items(feed)
        var out: [String: [String]] = [:]
        out["home_total"] = Self.texts(applied.balance)
        out["holdings"] = Self.texts(applied.assetRows)
        out["assets"] = Self.texts(FlowsLive.assets(balance, currency: nil, on: drawnAssets, loc: loc).rows)
        out["token_detail"] = balance.tokens.flatMap { token in
            Self.texts(FlowsLive.tokenDetail(
                token, feed: feed, display: .usd, on: drawnTokenDetail, loc: loc,
                hidden: balance.hidden || feed.hidden
            ))
        }
        out["home_activity"] = Self.texts(applied.activityGroups)
        out["history"] = Self.texts(FlowsLive.history(feed, on: drawnHistory, loc: loc, hidden: hidden).groups)
        func detail(_ item: FeedItemWire) -> [String] {
            Self.texts(FlowsLive.txDetail(
                item, record: feed.transactions.first { $0.id == item.id }, on: drawnTxDetail, loc: loc,
                hidden: hidden
            ))
        }
        out["transfer_detail"] = items.filter { $0.dapp == nil }.flatMap(detail)
        out["dapp_detail"] = items.filter { $0.dapp != nil }.flatMap(detail)
        out["contact_activity"] = Self.texts(feed.contactRows.map {
            WalletLive.activityRow($0, loc: loc, hidden: hidden)
        })
        out["account_switcher"] = Self.texts(SettingsLive.withAccounts(
            session: try session(balance), balances: balance.switcher.balances,
            hidden: balance.switcher.hidden, display: .usd,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).accountsSheet)
        out["balance_detail"] = Self.texts(SettingsLive.withBalanceDetail(
            balance, display: .usd, on: SettingsFixtures.build(.sr3, loc: loc), loc: loc
        ).balanceDetail) + Self.texts(SettingsLive.withUnreachable(
            balance, display: .usd, on: SettingsFixtures.build(.sr3, loc: loc), loc: loc
        ).unreachable)
        out["network_picker"] = Array(WalletLive.networkHoldings(balance, display: .usd).values)
        out["receipt_toast"] = feed.toast.map { [$0.value] } ?? []
        return out
    }

    /// Every `String` a model holds, walked by reflection — what the screen
    /// can draw from it.
    private static func texts(_ value: Any) -> [String] {
        if let text = value as? String { return [text] }
        let mirror = Mirror(reflecting: value)
        if mirror.displayStyle == .optional {
            return mirror.children.first.map { texts($0.value) } ?? []
        }
        return mirror.children.flatMap { texts($0.value) }
    }

    private func leaks(_ texts: [String]) -> [String] {
        texts.filter { text in Self.fixture.forbidden.contains { text.contains($0) } }
    }

    // MARK: - The four claims

    @Test func theFixtureIsHere() {
        #expect(!Self.fixture.forbidden.isEmpty, "the shared fixture did not load")
        #expect(Self.fixture.mask == WalletFixtures.mask)
        #expect(Self.fixture.balanceMask == WalletFixtures.balanceMask)
    }

    /// 1. Shown, the figures are really there — on some surface each.
    @Test func shownEveryForbiddenFigureAppearsSomewhere() throws {
        let (balance, feed) = try views("shown")
        #expect(!balance.hidden && !feed.hidden)
        let surfaces = try maskedSurfaces(balance, feed)
        let all = surfaces.values.flatMap { $0 }
        for run in Self.fixture.forbidden {
            #expect(all.contains { $0.contains(run) }, "\(run) is on no surface — the check below would prove nothing")
        }
    }

    /// 2. Hidden, no masked surface draws a figure, and the figures read the
    /// mask.
    @Test func hiddenNoMaskedSurfaceDrawsAFigure() throws {
        let (balance, feed) = try views("hidden")
        #expect(balance.hidden && feed.hidden && balance.switcher.hidden)
        let surfaces = try maskedSurfaces(balance, feed)
        #expect(Set(surfaces.keys) == Set(Self.fixture.masked), "this shell builds every masked surface")
        for (name, texts) in surfaces.sorted(by: { $0.key < $1.key }) {
            #expect(leaks(texts).isEmpty, "\(name) leaks \(leaks(texts))")
        }
        // Masked, not blank: the figures read the mask.
        let applied = WalletLive.apply(balance, feed: feed, feedRead: true, on: home, loc: loc)
        // The hero draws the balance mask as its six dots (`state: .hidden`),
        // with no figure to split — nor to leak into its accessibility label.
        #expect(applied.balance.state == .hidden)
        #expect(applied.balance.integer == nil)
        #expect(WalletGeometry.hiddenDotCount == Self.fixture.balanceMask.count)
        #expect(applied.assetRows.allSatisfy { $0.balance == Self.fixture.mask && $0.masked })
        #expect(surfaces["account_switcher"]?.contains { $0.contains(Self.fixture.mask) } == true,
                "the switcher's total is not the mask")
        let sheet = SettingsLive.withAccounts(
            session: try session(balance), balances: balance.switcher.balances,
            hidden: balance.switcher.hidden, display: .usd,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).accountsSheet
        #expect(sheet.rows.allSatisfy { $0.amount == Self.fixture.mask })
        #expect(!sheet.summary.contains("$0.00"), "an empty, withheld list summed to zero: \(sheet.summary)")
        let token = try #require(balance.tokens.first)
        let card = FlowsLive.tokenDetail(token, feed: feed, display: .usd, on: drawnTokenDetail, loc: loc,
                                         hidden: true)
        #expect(card.balance.hasPrefix(Self.fixture.mask))
        #expect(card.fiat == Self.fixture.mask)
        #expect(feed.toast == nil, "the core withholds the receipt toast")
    }

    /// 3. Hidden, Send keeps its figures: the balance it draws from keeps the
    /// amounts (the core masks none of them), and its formatter prints them.
    @Test func hiddenSendStillDrawsItsFigures() throws {
        let (balance, _) = try views("hidden")
        #expect(Self.fixture.visible.contains("send"))
        let amounts = balance.tokens.map { WalletLive.tokenAmountText($0.balance) }
        #expect(amounts.contains { $0.contains("418") } && amounts.contains { $0.contains("376") },
                "Send's holdings lost their amounts: \(amounts)")
    }

    /// 4. A row masks its figure exactly when the core says it is money.
    @Test func aRowMasksExactlyWhenFigureMaskable() throws {
        let (_, feed) = try views("hidden")
        let items = FlowsLive.items(feed)
        #expect(!Self.fixture.figureMaskable.isEmpty)
        for (id, maskable) in Self.fixture.figureMaskable {
            let item = try #require(items.first { $0.id == id }, "no row \(id) in the feed")
            #expect(item.figureMaskable == maskable, "\(id): the wire said otherwise")
            let row = WalletLive.activityRow(item, loc: loc, hidden: true)
            #expect(row.masked == maskable, "\(id) masked=\(row.masked), the core says \(maskable)")
            if maskable { #expect(row.amount == Self.fixture.mask, "\(id)") }
            // Shown, nothing masks.
            #expect(!WalletLive.activityRow(item, loc: loc, hidden: false).masked)
        }
    }

    /// 5. The home draws the core's cut (issue #469): the newest three, the
    /// same rows History opens with; History draws every row.
    @Test func theHomeDrawsTheCoresNewestThreeAndHistoryEveryRow() throws {
        let (balance, feed) = try views("shown")
        let all = FlowsLive.items(feed)
        let homeItems: [FeedItemWire] = feed.homeRows.compactMap {
            if case .item(let item) = $0 { return item } else { return nil }
        }
        #expect(all.count > 3, "the fixture must hold more than the home draws")
        #expect(homeItems.map(\.id) == all.prefix(3).map(\.id))
        let applied = WalletLive.apply(balance, feed: feed, feedRead: true, on: home, loc: loc)
        #expect(applied.activityGroups.flatMap(\.rows).compactMap(\.itemId) == homeItems.map(\.id))
        #expect(applied.activityGroups.allSatisfy { !$0.rows.isEmpty }, "a header with no row under it")
        let history = FlowsLive.history(feed, on: drawnHistory, loc: loc, hidden: false)
        #expect(history.groups.flatMap(\.rows).count == all.count)
    }

    /// 6. PR 3 (item 12) — a hidden figure keeps its UNIT. "•••• xDAI" says
    /// what kind of money without saying how much; "••••" alone drops the
    /// one fact a person hiding the amount still wants. One rule on every
    /// detail: a transfer's, a token's page, and a dApp row's figure or the
    /// allowance it granted.
    @Test func aHiddenFigureKeepsItsUnit() throws {
        let (balance, feed) = try views("hidden")
        let mask = Self.fixture.mask
        let items = FlowsLive.items(feed)
        func detail(_ item: FeedItemWire) -> TxDetailModel {
            FlowsLive.txDetail(
                item, record: feed.transactions.first { $0.id == item.id }, on: drawnTxDetail, loc: loc,
                hidden: true
            )
        }

        let transfers = items.filter { $0.dapp == nil && $0.figureMaskable && !$0.symbol.isEmpty }
        #expect(!transfers.isEmpty, "the fixture has no plain transfer to check")
        for item in transfers {
            #expect(detail(item).amount == "\(mask) \(item.symbol)", "\(item.id) reads \(detail(item).amount)")
        }

        let dapps = items.filter { $0.dapp != nil && $0.figureMaskable }
        #expect(!dapps.isEmpty, "the fixture has no dApp row with a figure")
        for item in dapps {
            let amount = detail(item).amount
            #expect(amount.hasPrefix("\(mask) "), "\(item.id) reads \(amount)")
            #expect(amount.count > mask.count + 1, "\(item.id) lost its unit: \(amount)")
        }

        for token in balance.tokens {
            let card = FlowsLive.tokenDetail(token, feed: feed, display: .usd, on: drawnTokenDetail, loc: loc,
                                             hidden: true)
            #expect(card.balance == "\(mask) \(token.symbol)")
        }
        // The fiat worth is a figure in a currency: masked whole, no unit to keep.
        #expect(transfers.allSatisfy { [mask, ""].contains(detail($0).fiat) })
    }
}
