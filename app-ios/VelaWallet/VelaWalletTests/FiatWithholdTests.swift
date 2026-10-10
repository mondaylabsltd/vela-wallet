//
//  FiatWithholdTests.swift
//  VelaWalletTests
//
//  PR 3 notes 9 and 27 — the core's withhold rule (`display_currency.rs`):
//  until `CurrencyView.committed` the view is the USD/1 placeholder, which is
//  not the person's currency, and **no fiat figure is drawn on ANY surface**.
//  The rule was once applied to five surfaces and missed the rest (the token
//  page, the balance detail sheet, Send's "≈" lines, the send form, the
//  signing fee), because each surface multiplied by the rate itself.
//
//  The surfaces are the core's `FIAT_SURFACES`. This test stack cannot read
//  the core's constant, so the twelve names are pinned here, in the core's
//  order, and each has a case below:
//
//      home_total, holdings, account_switcher, token_detail, assets,
//      balance_detail, activity_row, activity_detail, send_coin_list,
//      send_form, signing_sheet, settings_total
//
//  Each case replays the surface's PRODUCTION builder twice — the currency on
//  its way, then committed (CNY at 7.1) — and holds three things:
//
//    1. on its way, nothing the surface draws is a fiat figure: no currency
//       glyph beside a digit, no "≈" before one — and the token amounts
//       ("0.5 ETH"), which are not fiat, are drawn as always;
//    2. committed, the figure is there, in the person's money;
//    3. the withheld figure KEEPS ITS ROOM: a figure that is a line of its
//       own is that line with nothing on it (`Display.withheldLine`, never an
//       empty string — an empty `Text` has no height), and a figure inside a
//       longer line leaves the rest of that line where it was. The same rows,
//       the same lines, before and after.
//
//  And one structural check: every fiat figure is written by ONE formatter
//  (`WalletLive.Display.fiat`) — no other file multiplies by the rate.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct FiatWithholdTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The core's `FIAT_SURFACES` (`rust/crates/vela-core/src/app/
    /// display_currency.rs`), in its order.
    private static let surfaces = [
        "home_total", "holdings", "account_switcher", "token_detail", "assets",
        "balance_detail", "activity_row", "activity_detail", "send_coin_list",
        "send_form", "signing_sheet", "settings_total",
    ]

    // MARK: - The two displays

    /// The stored choice's rate is on its way: nothing committed.
    private let waitingView = CurrencyViewWire(code: "USD", rate: 1, committed: false, pending: "CNY")
    private let committedView = CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
    private var waiting: WalletLive.Display { .from(waitingView) }
    private var committed: WalletLive.Display { .from(committedView) }

    /// A fiat figure: a currency glyph the catalog knows beside a digit, or
    /// an "≈" in front of one. "≈ +0.03 ETH" (a token amount the wallet
    /// cannot vouch for) is not — its "≈" is followed by a sign.
    private static let glyphs: [String] = {
        Set(CurrencyCatalog.entries.map(\.glyph).filter { !$0.isEmpty }).sorted()
    }()

    private static func isFiat(_ text: String) -> Bool {
        let scalars = Array(text)
        for (index, character) in scalars.enumerated() {
            let next = scalars.dropFirst(index + 1).first { $0 != " " && $0 != "\u{00A0}" }
            if glyphs.contains(String(character)), next?.isNumber == true { return true }
            if character == "≈", let next, next.isNumber || glyphs.contains(String(next)) { return true }
        }
        return false
    }

    /// Every `String` a model holds — what the screen can draw from it.
    private static func texts(_ value: Any) -> [String] {
        if let text = value as? String { return [text] }
        let mirror = Mirror(reflecting: value)
        if mirror.displayStyle == .optional {
            return mirror.children.first.map { texts($0.value) } ?? []
        }
        return mirror.children.flatMap { texts($0.value) }
    }

    private func expectNoFiat(_ value: Any, _ surface: String, sourceLocation: SourceLocation = #_sourceLocation) {
        let drawn = Self.texts(value).filter(Self.isFiat)
        #expect(drawn.isEmpty, "\(surface) draws a fiat figure before the currency commits: \(drawn)",
                sourceLocation: sourceLocation)
    }

    private func expectFiat(_ value: Any, _ surface: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(Self.texts(value).contains { $0.contains("¥") && Self.isFiat($0) },
                "\(surface) draws no figure in the person's currency once it commits",
                sourceLocation: sourceLocation)
    }

    // MARK: - Fixtures

    private static let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func token(
        _ symbol: String, chainId: Int = 100, balance: String, price: Double?
    ) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chainId, symbol: symbol, name: symbol, balance: balance,
                         decimals: 18, tokenAddress: nil, priceUsd: price, spam: false)
    }

    private var balance: BalanceViewWire {
        var view = BalanceViewWire(
            address: Self.address,
            displayTotalUsd: 1_383.28, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [token("xDAI", balance: "412.5", price: 1), token("ETH", chainId: 8453, balance: "0.5", price: 1_941.56)],
            unpricedTokens: [], failedChainIds: [56],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [
                BalanceCacheEntryWire(address: Self.address, usd: 1_383.28),
            ])
        )
        view.unreachableNetworks = [
            UnreachableNetworkWire(chainId: 56, lastKnown: "held", lastSeenUsd: 157, lineKey: "assets.lastSeen"),
            UnreachableNetworkWire(chainId: 137, lastKnown: "not_read", lastSeenUsd: nil, lineKey: "assets.notReadYet"),
        ]
        view.unreachableKey = "assets.unreachableMany"
        return view
    }

    private var home: WalletHomeModel { WalletFixtures.buildMobileState(.h1, loc: loc) }

    private var drawnTokenDetail: TokenDetailModel {
        guard case .tokenDetail(let model)? = WalletFlowFixtures.build(.t2, loc: loc).sheet
        else { fatalError("T2 does not draw the token sheet") }
        return model
    }

    private var drawnTxDetail: TxDetailModel {
        guard case .txDetail(let model)? = WalletFlowFixtures.build(.a2, loc: loc).sheet
        else { fatalError("A2 does not draw the transaction sheet") }
        return model
    }

    private var drawnAssets: AssetsModel {
        guard case .assets(let model) = WalletFlowFixtures.build(.t1, loc: loc).base
        else { fatalError("T1 does not draw the assets list") }
        return model
    }

    /// The shared privacy fixture's feed, shown, as JSON: real rows and
    /// their records.
    private func feedJSON() throws -> [String: Any] {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let url = repoRoot.appendingPathComponent("rust/crates/vela-core/tests/fixtures/privacy-hidden.json")
        let json = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let shown = try #require(json["shown"] as? [String: Any])
        return try #require(shown["feed"] as? [String: Any])
    }

    private func feed() throws -> FeedViewWire {
        try CoreJSON.decode(FeedViewWire.self, from: try feedJSON())
    }

    /// One of the fixture's stored records, with the worth a shell writes
    /// when it stores one ("$237.50") — the fixture's dApp record keeps none.
    private func record(_ id: String, usd: String) throws -> FeedTxRecordWire {
        let records = try #require(try feedJSON()["transactions"] as? [[String: Any]])
        var record = try #require(records.first { $0["id"] as? String == id })
        record["usd"] = usd
        return try CoreJSON.decode(FeedTxRecordWire.self, from: record)
    }

    private let xdai: [String: Any] = [
        "symbol": "xDAI", "network": "Gnosis", "chain_id": 100,
        "token_address": NSNull(), "decimals": 18, "balance": "412.5",
        "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    private func sendView(_ patch: [String: Any]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    /// A settled native quote of 0.01 xDAI, and the relay's row that prices it.
    private let quote = FeeEstimateWire(
        chainId: 100, totalWei: "10000000000000000", maxFeePerGas: "1",
        totalGas: "1", deployed: true, quoted: true, feeAsset: .native,
        feeRecipient: nil
    )

    private var pricedFee: FeeViewWire {
        FeeViewWire(
            busy: false, failed: nil, fee: quote, stale: false, feeToken: nil,
            options: [
                FeeOptionWire(
                    symbol: "XDAI", contract: nil, decimals: 18,
                    balance: "480000000000000000", recipient: "0x1",
                    usdBalance: "0.48", usdPrice: "1", amount: "10000000000000000",
                    insufficient: false, selected: true
                ),
            ],
            confirmFeeReady: true
        )
    }

    // MARK: - The list, and the one formatter

    /// The twelve surfaces, as the core lists them — and every one has a
    /// case in this file (the `@Test`s below are named for them).
    @Test func theSurfacesAreTheCoresTwelve() {
        #expect(Self.surfaces.count == 12)
        #expect(Set(Self.surfaces).count == 12)
        // The core's own list, read from its source: a surface added there
        // without a case here fails this, in the file that has to grow.
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let source = (try? String(
            contentsOf: repoRoot.appendingPathComponent("rust/crates/vela-core/src/app/display_currency.rs"),
            encoding: .utf8
        )) ?? ""
        let declared = source.components(separatedBy: "pub const FIAT_SURFACES").dropFirst().first?
            .components(separatedBy: "];").first?
            .components(separatedBy: "\"").enumerated().compactMap { $0.offset % 2 == 1 ? $0.element : nil } ?? []
        #expect(declared == Self.surfaces, "the core's FIAT_SURFACES moved: \(declared)")
    }

    /// The one formatter: a figure in the person's money once the currency
    /// is committed, `nil` — withheld — before; and its own line keeps a
    /// line's height with nothing on it.
    @Test func theOneFormatterWithholdsUntilTheCurrencyCommits() {
        #expect(waiting.fiat(1_383.28) == nil)
        #expect(waiting.fiatParts(1_383.28) == nil)
        #expect(WalletLive.Display.live(nil).fiat(1) == nil, "before the machine has answered at all")
        #expect(WalletLive.Display.from(.unread).fiat(1) == nil)
        #expect(committed.fiat(1_383.28) == "¥9,821.29")
        #expect(committed.fiatParts(1_383.28)?.integer == "¥9,821")
        #expect(committed.fiatParts(1_383.28)?.decimals == "29")
        // A committed currency nobody could price is honestly dollars.
        #expect(WalletLive.Display.from(CurrencyViewWire(code: "CNY", rate: nil, committed: true)).fiat(2) == "$2.00")
        // A drawing with no machine behind it is settled dollars, as before.
        #expect(WalletLive.Display.usd.fiat(2) == "$2.00")

        // A withheld line is a LINE: not empty (an empty `Text` has no
        // height, and what is under it would move), and nothing to read.
        #expect(waiting.fiatLine(2, prefix: "≈ ") == WalletLive.Display.withheldLine)
        #expect(!WalletLive.Display.withheldLine.isEmpty)
        #expect(WalletLive.Display.withheldLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        #expect(!Self.isFiat(WalletLive.Display.withheldLine))
        #expect(committed.fiatLine(2, prefix: "≈ ") == "≈ ¥14.20")
    }

    /// No other file multiplies by the rate or glues a currency glyph to a
    /// number: every surface asks `Display.fiat`. (The glyph is private to
    /// the formatter's file, which the compiler holds; the rate leaves that
    /// file by one door — `Display.sendContext`, the send machine's own
    /// input, at its open and at every change after it — so no other file
    /// reads it at all.)
    @Test func noOtherFileFormatsFiatItself() throws {
        let app = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet")
        let files = try #require(FileManager.default.enumerator(at: app, includingPropertiesForKeys: nil))
        var offenders: [String] = []
        var scanned = 0
        for case let url as URL in files where url.pathExtension == "swift" {
            // The formatter's own file, and the boards (fixture data).
            if url.lastPathComponent == "WalletLive.swift" || url.path.contains("/Gallery/") { continue }
            scanned += 1
            let source = try String(contentsOf: url, encoding: .utf8)
            for (number, line) in source.components(separatedBy: "\n").enumerated() {
                let code = line.components(separatedBy: "//").first ?? line
                guard code.contains("display.rate") || code.contains("display.glyph") else { continue }
                offenders.append("\(url.lastPathComponent):\(number + 1): \(code.trimmingCharacters(in: .whitespaces))")
            }
        }
        #expect(scanned > 200, "the scan found no sources (\(scanned))")
        #expect(offenders.isEmpty, "fiat is formatted outside the one helper:\n\(offenders.joined(separator: "\n"))")
    }

    // MARK: - home_total, holdings, assets

    @Test func homeTotal() {
        let before = WalletLive.balance(balance, display: waiting, fallback: home.balance, loc: loc)
        #expect(before.integer == nil && before.decimals == nil)
        #expect(before.state == .loading, "the hero keeps its skeleton, at the figure's size")
        #expect(before.currency == "CNY", "named by the stored choice on its way, never USD first")
        expectNoFiat(before, "home_total")
        let after = WalletLive.balance(balance, display: committed, fallback: home.balance, loc: loc)
        #expect(after.state == .normal && after.integer == "¥9,821" && after.decimals == "29")
        // The line under the figure is the same line before and after.
        #expect(before.status?.text == after.status?.text)
    }

    @Test func holdings() {
        let before = WalletLive.assetRows(balance, display: waiting)
        let after = WalletLive.assetRows(balance, display: committed)
        #expect(before.count == after.count && !before.isEmpty)
        for (waitingRow, settledRow) in zip(before, after) {
            guard case .pending = waitingRow.fiat else {
                Issue.record("a holding's worth is drawn before the currency commits: \(waitingRow.fiat)")
                continue
            }
            guard case .value(let figure) = settledRow.fiat else {
                Issue.record("a priced holding has no worth once committed: \(settledRow.fiat)")
                continue
            }
            #expect(figure.hasPrefix("¥"))
            // The token amount is not fiat: the same, before and after.
            #expect(waitingRow.balance == settledRow.balance && !waitingRow.balance.isEmpty)
        }
        expectNoFiat(before, "holdings")
        // The network picker's per-network worth, the same holdings.
        #expect(WalletLive.networkHoldings(balance, display: waiting).isEmpty)
        #expect(WalletLive.networkHoldings(balance, display: committed)[100] == "¥2,928.75")
    }

    @Test func assets() {
        let before = FlowsLive.assets(balance, currency: waitingView, on: drawnAssets, loc: loc)
        let after = FlowsLive.assets(balance, currency: committedView, on: drawnAssets, loc: loc)
        #expect(before.rows.count == after.rows.count && !before.rows.isEmpty)
        expectNoFiat(before.rows, "assets")
        expectFiat(after.rows, "assets")
        #expect(before.rows.map(\.balance) == after.rows.map(\.balance))
        // The live app before its currency machine has answered: `.unread`.
        expectNoFiat(FlowsLive.assets(balance, currency: .unread, on: drawnAssets, loc: loc).rows, "assets")
    }

    // MARK: - account_switcher, settings_total

    private func session() throws -> SessionView {
        try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.address,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Mine", "address": Self.address]]],
        ])
    }

    private func accounts(_ display: WalletLive.Display) throws -> AccountsSheetModel {
        SettingsLive.withAccounts(
            session: try session(), balances: balance.switcher.balances, display: display,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).accountsSheet
    }

    @Test func accountSwitcher() throws {
        let before = try accounts(waiting)
        let after = try accounts(committed)
        #expect(before.rows.count == after.rows.count)
        #expect(before.rows.allSatisfy { $0.amount.isEmpty }, "a row's total is drawn: \(before.rows.map(\.amount))")
        #expect(after.rows.first?.amount == "¥9,821.29")
        // The row is its name and address either way: same rows, same order.
        #expect(before.rows.map(\.name) == after.rows.map(\.name))
        expectNoFiat(before.rows, "account_switcher")
    }

    @Test func settingsTotal() throws {
        let before = try accounts(waiting)
        let after = try accounts(committed)
        // The count alone, on the same line; the total joins it.
        #expect(before.summary == loc.t("home.switcherAccountCount", count: 1))
        #expect(!before.summary.contains("switcherAccountCount"), "the count's key echoed: \(before.summary)")
        #expect(after.summary.hasPrefix(before.summary))
        #expect(after.summary.contains("¥9,821.29"))
        expectNoFiat(before.summary, "settings_total")
        // Settings' own currency row: no figure, and no "USD" that turns
        // into another code — the stored choice named alone, else nothing.
        #expect(SettingsLive.currencyRowValue(waitingView) == "CNY")
        #expect(SettingsLive.currencyRowValue(.unread).isEmpty)
        #expect(SettingsLive.currencyRowValue(committedView).hasPrefix("CNY · ¥"))
    }

    // MARK: - token_detail

    @Test func tokenDetail() throws {
        let held = try #require(balance.tokens.last)
        func card(_ display: WalletLive.Display) -> TokenDetailModel {
            FlowsLive.tokenDetail(held, feed: nil, display: display, on: drawnTokenDetail, loc: loc)
        }
        let before = card(waiting)
        let after = card(committed)
        expectNoFiat(before, "token_detail")
        // The holding's worth: its own line, kept — Receive and Send under
        // it do not move when the figure lands.
        #expect(before.fiat == WalletLive.Display.withheldLine, "the worth's line is \(before.fiat.debugDescription)")
        #expect(after.fiat == "¥6,892.54")
        // The price: its row stays, with its label; the value lands beside it.
        let price = loc.t("tokenDetail.labelPrice")
        #expect(before.facts.map(\.label) == after.facts.map(\.label), "a fact row came or went with the currency")
        #expect(before.facts.first { $0.label == price }?.value == "")
        #expect(after.facts.first { $0.label == price }?.value.contains("¥13,785.08") == true)
        // The token amount is not fiat.
        #expect(before.balance == after.balance && before.balance == "0.5 ETH")
        // An unpriced coin has no worth line in either state: nothing to wait for.
        let unpriced = token("CAKE", balance: "18.2", price: nil)
        #expect(FlowsLive.tokenDetail(unpriced, feed: nil, display: waiting, on: drawnTokenDetail, loc: loc).fiat == "")
    }

    // MARK: - balance_detail

    @Test func balanceDetail() {
        func sheet(_ display: WalletLive.Display) -> BalanceDetailModel {
            SettingsLive.withBalanceDetail(balance, display: display, on: SettingsFixtures.build(.sr3, loc: loc), loc: loc)
                .balanceDetail
        }
        let before = sheet(waiting)
        let after = sheet(committed)
        expectNoFiat(before, "balance_detail")
        // The total's line is kept, empty; the rows are the same rows.
        #expect(before.summary == WalletLive.Display.withheldLine)
        #expect(after.summary == loc.t("assets.switcherTotal", vars: ["amount": "¥9,821.29"]))
        #expect(before.done.map(\.id) == after.done.map(\.id) && !before.done.isEmpty)
        #expect(before.done.allSatisfy { $0.amount == nil })
        #expect(after.done.allSatisfy { $0.amount?.hasPrefix("¥") == true })
        #expect(before.pending.map(\.id) == after.pending.map(\.id))

        // The unreachable list's "last seen": the row keeps its line.
        func list(_ display: WalletLive.Display) -> UnreachableModel {
            SettingsLive.withUnreachable(balance, display: display, on: SettingsFixtures.build(.sr3, loc: loc), loc: loc)
                .unreachable
        }
        let unreachableBefore = list(waiting)
        let unreachableAfter = list(committed)
        expectNoFiat(unreachableBefore, "balance_detail (unreachable)")
        #expect(unreachableBefore.rows.map(\.id) == unreachableAfter.rows.map(\.id))
        #expect(unreachableBefore.rows.first?.line == WalletLive.Display.withheldLine)
        #expect(unreachableAfter.rows.first?.line == loc.t("assets.lastSeen", vars: ["amount": "¥1,114.70"]))
        // A line with no figure in it is said as always.
        #expect(unreachableBefore.rows.last?.line == loc.t("assets.notReadYet"))
        // Hidden is the mask, in either state: nothing to wait for.
        var hidden = balance
        hidden.unreachableNetworks = [
            UnreachableNetworkWire(chainId: 56, lastKnown: "held", lastSeenUsd: nil, lineKey: "assets.lastSeen"),
        ]
        let masked = SettingsLive.withUnreachable(
            BalanceViewWire(
                address: hidden.address, displayTotalUsd: nil, balanceUnknown: false, balancePartial: false,
                notice: nil, hidden: true, refreshing: false, lastRefreshedAtMs: nil, tokens: [],
                unpricedTokens: [], failedChainIds: [56], rateLimitedChainIds: [],
                unreachableNetworks: hidden.unreachableNetworks, unreachableKey: "assets.unreachableOne",
                holdingsLoading: false, cachedTotalUsd: nil,
                switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [], hidden: true)
            ),
            display: waiting, on: SettingsFixtures.build(.sr3, loc: loc), loc: loc
        ).unreachable
        #expect(masked.rows.first?.line == loc.t("assets.lastSeen", vars: ["amount": WalletFixtures.mask]))
    }

    // MARK: - activity_row, activity_detail

    /// A row states the TOKEN amount and its coin — iOS draws no fiat on an
    /// activity row, in any state. Held here so one cannot be added without
    /// meeting the rule.
    @Test func activityRow() throws {
        let rows = FlowsLive.items(try feed()).map { WalletLive.activityRow($0, loc: loc, hidden: false) }
        #expect(!rows.isEmpty)
        expectNoFiat(rows, "activity_row")
        #expect(rows.contains { $0.amount == "\u{2212}163.25" && $0.unit == "USDC" })
    }

    @Test func activityDetail() throws {
        let feed = try feed()
        let items = FlowsLive.items(feed)
        // The dApp record with the worth a shell stores beside it.
        let swapRecord = try record("swap", usd: "$237.50")
        func detail(_ item: FeedItemWire, _ display: WalletLive.Display) -> TxDetailModel {
            FlowsLive.txDetail(
                item, record: item.id == "swap" ? swapRecord : feed.transactions.first { $0.id == item.id },
                on: drawnTxDetail, loc: loc, display: display
            )
        }
        // A plain transfer and a dApp row, each with a stored worth.
        let transfer = try #require(items.first { $0.id == "sent" })
        let swap = try #require(items.first { $0.id == "swap" })
        #expect(swap.dapp != nil && transfer.dapp == nil)
        for item in [transfer, swap] {
            let before = detail(item, waiting)
            let after = detail(item, committed)
            #expect(after.fiat.hasPrefix("≈ "), "\(item.id) has no stored worth to withhold: \(after.fiat)")
            // The worth's line is kept, with nothing on it.
            #expect(before.fiat == WalletLive.Display.withheldLine, "\(item.id): \(before.fiat.debugDescription)")
            #expect(!Self.isFiat(before.fiat))
            // Everything else is the same detail: the figure, its facts.
            #expect(before.amount == after.amount && !before.amount.isEmpty)
            #expect(before.facts.map(\.label) == after.facts.map(\.label))
            #expect(before.title == after.title)
        }
        // A record that kept no worth has no line in either state.
        let signature = try #require(items.first { $0.id == "signature" })
        #expect(detail(signature, waiting).fiat == "" && detail(signature, committed).fiat == "")
        // Hidden is the mask, whatever the currency is doing.
        #expect(FlowsLive.storedFiat("$163.25", hidden: true, display: waiting) == WalletFixtures.mask)
        #expect(FlowsLive.storedFiat("$163.25", hidden: false, display: committed) == "≈ $163.25")
        #expect(FlowsLive.storedFiat(nil, hidden: false, display: waiting) == "")
    }

    // MARK: - send_coin_list, send_form

    /// Send's coin list states what is HELD — "412.5 xDAI" — and no worth:
    /// choosing an asset is the picker's job. Held here like the rows above.
    @Test func sendCoinList() throws {
        let view = try sendView(["tokens": [xdai], "stage": "select_token"])
        guard case .sendPick(let drawn) = WalletFlowFixtures.build(.sd1, loc: loc).base else {
            Issue.record("sd1 is not the coin list")
            return
        }
        let pick = SendLive.pick(view, on: drawn, loc: loc)
        #expect(pick.rows.count == 1)
        expectNoFiat(pick.rows, "send_coin_list")
        #expect(pick.rows.first?.balance == "412.5 xDAI")
    }

    @Test func sendForm() throws {
        let view = try sendView([
            "stage": "enter_details", "tokens": [xdai], "selected_token": xdai,
            "amount": "5", "token_amount": "5", "confirm_amount": "5",
            "recipient": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            "fee": [
                "chain_id": 100, "total_wei": "10000000000000000", "max_fee_per_gas": "1",
                "total_gas": "1", "deployed": true, "quoted": true,
                "fee_asset": ["type": "native"], "fee_recipient": NSNull(), "tier": "standard",
            ] as [String: Any],
        ])
        guard case .sendForm(let drawnForm) = WalletFlowFixtures.build(.sd2, loc: loc).base,
              case .sendConfirm(let drawnConfirm) = WalletFlowFixtures.build(.sd3, loc: loc).base
        else {
            Issue.record("sd2 / sd3 are not the form and the confirm")
            return
        }

        // The form: the "≈" line under the amount, and the fee row.
        func form(_ display: WalletLive.Display) -> SendFormModel {
            SendLive.form(view, fee: pricedFee, display: display, on: drawnForm, loc: loc)
        }
        let before = form(waiting)
        let after = form(committed)
        expectNoFiat(before, "send_form")
        #expect(before.amount?.fiat == WalletLive.Display.withheldLine, "the ≈ line is \(String(describing: before.amount?.fiat))")
        #expect(after.amount?.fiat == "≈ ¥35.50")
        // What is typed and what is held are not fiat: drawn as always.
        #expect(before.amount?.value == "5" && after.amount?.value == "5")
        #expect(before.token?.detail == after.token?.detail)
        // The fee row is the coin amount on its one line; the "≈" half joins it.
        #expect(before.fee.value == "0.01 xDAI", "the fee row reads \(before.fee.value)")
        #expect(after.fee.value == "0.01 xDAI · ≈¥0.07")

        // The confirm: the same two figures on the page that signs.
        func confirm(_ display: WalletLive.Display) -> SendConfirmModel {
            SendLive.confirm(view, from: (Self.address, nil), display: display, on: drawnConfirm, loc: loc,
                             fee: pricedFee)
        }
        let confirmBefore = confirm(waiting)
        let confirmAfter = confirm(committed)
        expectNoFiat(confirmBefore, "send_form (confirm)")
        #expect(confirmBefore.subline == WalletLive.Display.withheldLine)
        #expect(confirmAfter.subline == "≈ ¥35.50")
        #expect(confirmBefore.amount == confirmAfter.amount && confirmBefore.amount == "5")
        // The same fact rows, in the same order: only the fee's value grows.
        #expect(confirmBefore.facts.map(\.label) == confirmAfter.facts.map(\.label))
        #expect(confirmBefore.facts.contains { $0.value == "~0.01 xDAI" })
        #expect(confirmAfter.facts.contains { $0.value == "~0.01 xDAI · ≈¥0.07" })
    }

    // MARK: - signing_sheet

    /// The signing sheet's fee row is the send screens' own line through the
    /// send screens' own formatter; iOS draws a balance change as its token
    /// amount, never a worth. Withheld, the row is the coin that pays — which
    /// is what is signed — and the gate does not wait on the currency.
    @Test func signingSheet() throws {
        let clear = try CoreJSON.decode(ClearSigningViewWire.self, from: CoreJSON.object(ClearSigningCore().view()))
        func row(_ display: WalletLive.Display) -> String? {
            var context = SigningLive.Context(
                loc: loc, chainName: "Gnosis", chainDot: SettingsLive.chainColor(100), nativeSymbol: "xDAI",
                walletName: "Mine", walletAddress: Self.address
            )
            context.chainId = 100
            context.display = display
            guard case .onchain(_, let value, _, _, _) = SigningLive.feeModel(clear: clear, fee: pricedFee, context: context)
            else { return nil }
            return value
        }
        #expect(row(waiting) == "~0.01 xDAI")
        #expect(row(committed) == "~0.01 xDAI · ≈¥0.07")
        #expect(!Self.isFiat(row(waiting) ?? ""))
        // The line itself, as every fee row on the send screens draws it.
        #expect(SendLive.feeLine(quote, view: nil, fee: pricedFee, display: waiting) == "0.01 xDAI")
        #expect(SendLive.feeLine(quote, view: nil, fee: pricedFee, display: committed) == "0.01 xDAI · ≈¥0.07")
        #expect(SendLive.feeLine(quote, view: nil, fee: pricedFee, display: .usd) == "0.01 xDAI · ≈$0.01")
    }
}
