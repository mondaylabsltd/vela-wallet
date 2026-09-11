//
//  WalletFixtures.swift
//  VelaWallet
//
//  Canonical wallet-home fixtures (spec 015, data-model.md — the single
//  canon all four platforms port; web reference: src/lib/wallet/fixtures.ts).
//  Content is verbatim from the docs/design/wallet mocks (FR-012); builders merge
//  it with corpus strings (Loc) into display-ready view models. Pure data +
//  assembly: no fetching, no formatting rules, no business state.
//
//  Chain colors are fixture DATA, hosted as named values in
//  DesignSystem/ChainPalette (the sanctioned home of literal colors).
//

import SwiftUI

enum WalletFixtures {
    // MARK: - Canon

    static let mask = "••••"
    static let balanceMask = "••••••"
    static let networkCount = 8

    struct Identity {
        let name: String
        let longName: String
        let addressDisplay: String
        let addressFull: String
    }

    static let identity = Identity(
        name: "大表哥",
        longName: "这是一个非常长",
        addressDisplay: "0x14fB1f…D1eA5c",
        addressFull: "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
    )

    /// Identicon-board seeds (US3): cross-platform eyeball parity set.
    /// Empty string exercises the placeholder path.
    static let identiconBoardSeeds: [String] = [
        identity.addressFull,
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "alice",
        "bob",
        "0x9F3c00000000000000000000000000000000021aE",
        "",
    ]

    private enum Direction {
        case to(String)
        case from(String)
        case plain(String)
    }

    private enum Day: Equatable {
        case today, yesterday
        case literal(String)
    }

    private struct ActivityFixture {
        let kind: ActivityKind
        let direction: Direction
        let day: Day
        var clock: String?
        let amount: String
        let unit: String
        let positive: Bool
        let badgeColor: Color
    }

    private static let defaultActivity: [ActivityFixture] = [
        ActivityFixture(kind: .sent, direction: .to("hold on"), day: .today, clock: "14:02",
                        amount: "−2", unit: "POL", positive: false, badgeColor: ChainPalette.polygon),
        ActivityFixture(kind: .received, direction: .from("0x9F3c…21aE"), day: .today, clock: "11:20",
                        amount: "+120", unit: "USDT", positive: true, badgeColor: ChainPalette.ethereum),
        ActivityFixture(kind: .dapp, direction: .plain("PancakeSwap · BNB Chain"), day: .today, clock: "09:41",
                        amount: "−0.05", unit: "BNB", positive: false, badgeColor: ChainPalette.bnb),
        ActivityFixture(kind: .received, direction: .from("Alice"), day: .yesterday, clock: "20:15",
                        amount: "+50", unit: "USDC", positive: true, badgeColor: ChainPalette.base),
    ]

    private static let extremeActivity: [ActivityFixture] = [
        ActivityFixture(kind: .sent, direction: .to("Alexandra"), day: .today,
                        amount: "−1234.5678", unit: "POL", positive: false, badgeColor: ChainPalette.polygon),
        ActivityFixture(kind: .dapp, direction: .plain("app.uniswap.org · BNB"), day: .today,
                        amount: "−0.0000001", unit: "BNB", positive: false, badgeColor: ChainPalette.bnb),
    ]

    private struct AssetFixture {
        let ticker: String
        let chain: String
        let badgeColor: Color
        let balance: String
        /// nil → 无价格 (H4 partial-price failure).
        let fiat: String?
    }

    private static let defaultAssets: [AssetFixture] = [
        AssetFixture(ticker: "BNB", chain: "BNB Chain", badgeColor: ChainPalette.bnb, balance: "0.8533", fiat: "$496.46"),
        AssetFixture(ticker: "ETH", chain: "Arbitrum", badgeColor: ChainPalette.arbitrum, balance: "0.2253", fiat: "$422.62"),
        AssetFixture(ticker: "ETH", chain: "Ethereum", badgeColor: ChainPalette.ethereum, balance: "0.0689", fiat: "$129.25"),
        AssetFixture(ticker: "XDAI", chain: "Gnosis", badgeColor: ChainPalette.gnosis, balance: "74.3965", fiat: "$74.38"),
        AssetFixture(ticker: "USDT", chain: "Ethereum", badgeColor: ChainPalette.ethereum, balance: "53.4836", fiat: "$53.48"),
        AssetFixture(ticker: "USDC", chain: "Polygon", badgeColor: ChainPalette.polygon, balance: "12.04", fiat: "$12.04"),
    ]

    private static let partialPriceAssets: [AssetFixture] = [
        defaultAssets[0],
        defaultAssets[1],
        AssetFixture(ticker: "CAKE", chain: "BNB Chain", badgeColor: ChainPalette.bnb, balance: "18.20", fiat: nil),
    ]

    private static let extremeAssets: [AssetFixture] = [
        AssetFixture(ticker: "WBTC", chain: "以太坊主网 Ethereum", badgeColor: ChainPalette.ethereum, balance: "0.00000042", fiat: "$0.03"),
        AssetFixture(ticker: "USDT", chain: "Ethereum", badgeColor: ChainPalette.ethereum, balance: "1,234,567.8901", fiat: "$1,234,567.89"),
    ]

    private struct ChainFixture {
        let name: String
        let dot: Color
        let count: Int
    }

    private static let chains: [ChainFixture] = [
        ChainFixture(name: "BNB Chain", dot: ChainPalette.bnb, count: 1),
        ChainFixture(name: "Ethereum", dot: ChainPalette.ethereum, count: 3),
        ChainFixture(name: "Arbitrum", dot: ChainPalette.arbitrum, count: 1),
        ChainFixture(name: "Gnosis", dot: ChainPalette.gnosis, count: 1),
        ChainFixture(name: "Base", dot: ChainPalette.base, count: 1),
        ChainFixture(name: "Polygon", dot: ChainPalette.polygon, count: 1),
    ]

    // MARK: - Assembly

    private static func subtitle(_ f: ActivityFixture, loc: Loc) -> String {
        switch f.direction {
        case .to(let name): loc.t("history.toName", vars: ["name": name])
        case .from(let name): loc.t("history.fromName", vars: ["name": name])
        case .plain(let text): text
        }
    }

    private static func title(_ kind: ActivityKind, loc: Loc) -> String {
        switch kind {
        case .sent: loc.t("history.labelSent")
        case .received: loc.t("history.labelReceived")
        case .dapp: loc.t("history.txLabelDappTx")
        }
    }

    private static func dayLabel(_ day: Day, loc: Loc) -> String {
        switch day {
        case .today: loc.t("componentsUi.dayGroup.today")
        case .yesterday: loc.t("componentsUi.dayGroup.yesterday")
        case .literal(let text): text
        }
    }

    private static func activityRow(_ f: ActivityFixture, loc: Loc, masked: Bool = false) -> ActivityRowModel {
        ActivityRowModel(
            kind: f.kind,
            title: title(f.kind, loc: loc),
            subtitle: subtitle(f, loc: loc),
            amount: masked ? mask : f.amount,
            unit: f.unit,
            positive: f.positive,
            masked: masked,
            badgeColor: f.badgeColor
        )
    }

    private static func groupByDay(_ fixtures: [ActivityFixture], loc: Loc, masked: Bool = false) -> [ActivityGroupModel] {
        var groups: [(label: String, rows: [ActivityRowModel])] = []
        for f in fixtures {
            let label = dayLabel(f.day, loc: loc)
            let row = activityRow(f, loc: loc, masked: masked)
            if let last = groups.indices.last, groups[last].label == label {
                groups[last].rows.append(row)
            } else {
                groups.append((label, [row]))
            }
        }
        return groups.map { ActivityGroupModel(label: $0.label, rows: $0.rows) }
    }

    private static func assetRow(_ f: AssetFixture, loc: Loc, masked: Bool = false) -> AssetRowModel {
        let fiat: AssetFiatModel = if masked {
            .masked
        } else if let value = f.fiat {
            .value(value)
        } else {
            .noPrice(loc.t("home.balanceDetailNoPrice"))
        }
        return AssetRowModel(
            ticker: f.ticker,
            chain: f.chain,
            badgeColor: f.badgeColor,
            balance: masked ? mask : f.balance,
            fiat: fiat,
            masked: masked
        )
    }

    private static func chainRows(loc: Loc) -> [ChainRowModel] {
        [ChainRowModel(name: loc.t("componentsUi.networkFilter.allNetworks"), dot: .all, count: networkCount, selected: true)]
            + chains.map { ChainRowModel(name: $0.name, dot: .color($0.dot), count: $0.count, selected: false) }
    }

    private static func balance(
        _ state: BalanceStateKind,
        loc: Loc,
        integer: String? = nil,
        decimals: String? = nil,
        status: BalanceStatusModel? = nil
    ) -> BalanceModel {
        BalanceModel(
            label: loc.t("home.totalBalance"),
            currency: "USD",
            state: state,
            integer: state == .hidden ? balanceMask : integer,
            decimals: state == .hidden ? nil : decimals,
            liveText: state == .zeroLive ? loc.t("home.liveIndicator") : nil,
            status: status,
            a11yHide: loc.t("home.a11yHideBalance"),
            a11yShow: loc.t("home.a11yShowBalance")
        )
    }

    private static func header(loc: Loc, long: Bool = false) -> WalletHeaderModel {
        WalletHeaderModel(
            name: long ? identity.longName : identity.name,
            addressDisplay: identity.addressDisplay,
            identiconSeed: identity.addressFull
        )
    }

    private static func section(_ titleKey: String, loc: Loc, mode: SectionMode, emptyKeys: (title: String, caption: String)) -> SectionModel {
        SectionModel(
            title: loc.t(titleKey),
            action: loc.t("history.filterAll"),
            mode: mode,
            empty: SectionEmptyModel(title: loc.t(emptyKeys.title), caption: loc.t(emptyKeys.caption))
        )
    }

    private static func activitySection(loc: Loc, mode: SectionMode) -> SectionModel {
        section("home.tabActivity", loc: loc, mode: mode,
                emptyKeys: ("home.emptyNoActivity", "home.emptySubtitle"))
    }

    private static func assetsSection(loc: Loc, mode: SectionMode) -> SectionModel {
        section("assets.sectionTitle", loc: loc, mode: mode,
                emptyKeys: ("assets.emptyTitle", "assets.emptySubtext"))
    }

    /// Assemble the mobile home view model for one H-state (FR-002).
    static func buildMobileState(_ state: MobileStateId, loc: Loc) -> WalletHomeModel {
        let long = state == .h7 || state == .h7x
        let pill: NetworkPillModel = long
            ? .single(dot: ChainPalette.bnb, label: "BNB Chain")
            : .all(dots: ChainPalette.pillDots, label: loc.t("componentsUi.networkFilter.pillAll"))
        let tabs = TabsModel(
            wallet: loc.t("componentsUi.mainNav.wallet"),
            contacts: loc.t("componentsUi.mainNav.contacts"),
            explore: loc.t("componentsUi.mainNav.explore"),
            settings: loc.t("componentsUi.mainNav.settings")
        )
        let actions = ActionsModel(
            receive: loc.t("componentsUi.dock.receive"),
            send: loc.t("componentsUi.dock.send"),
            scan: loc.t("componentsUi.dock.scan")
        )

        var model = WalletHomeModel(
            state: state,
            header: header(loc: loc, long: long),
            pill: pill,
            balance: balance(.normal, loc: loc, integer: "$1,383", decimals: "28"),
            actions: actions,
            activitySection: activitySection(loc: loc, mode: .rows),
            // H1 first screen shows 今天 rows 1–2 (data-model.md); H1s shows all.
            activityGroups: groupByDay(Array(defaultActivity.prefix(2)), loc: loc),
            assetsSection: assetsSection(loc: loc, mode: .rows),
            assetRows: defaultAssets.map { assetRow($0, loc: loc) },
            tabs: tabs,
            sheet: nil,
            textScale: state == .h7x ? 1.35 : 1
        )

        switch state {
        case .h1:
            return model
        case .h1s:
            model = replacing(model, activityGroups: groupByDay(defaultActivity, loc: loc))
            return model
        case .h2:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.zeroLive, loc: loc, integer: "$0", decimals: "00"),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .empty),
                activityGroups: [],
                assetsSection: assetsSection(loc: loc, mode: .empty),
                assetRows: [],
                tabs: tabs, sheet: nil, textScale: 1
            )
        case .h3:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.loading, loc: loc),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .loading),
                activityGroups: [],
                assetsSection: assetsSection(loc: loc, mode: .loading),
                assetRows: [],
                tabs: tabs, sheet: nil, textScale: 1
            )
        case .h4:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.normal, loc: loc, integer: "$1,383", decimals: "46",
                                 status: BalanceStatusModel(kind: .warning, text: loc.t("home.balanceUnpriced"))),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .rows),
                activityGroups: groupByDay(Array(defaultActivity.prefix(2)), loc: loc),
                assetsSection: assetsSection(loc: loc, mode: .rows),
                assetRows: partialPriceAssets.map { assetRow($0, loc: loc) },
                tabs: tabs, sheet: nil, textScale: 1
            )
        case .h5:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.hidden, loc: loc),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .rows),
                activityGroups: groupByDay(defaultActivity, loc: loc, masked: true),
                assetsSection: assetsSection(loc: loc, mode: .rows),
                assetRows: defaultAssets.map { assetRow($0, loc: loc, masked: true) },
                tabs: tabs, sheet: nil, textScale: 1
            )
        case .h6:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.normal, loc: loc, integer: "$1,383", decimals: "28",
                                 status: BalanceStatusModel(kind: .refreshing, text: loc.t("home.balanceStale"))),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .rows),
                activityGroups: groupByDay(Array(defaultActivity.prefix(2)), loc: loc),
                assetsSection: assetsSection(loc: loc, mode: .rows),
                assetRows: defaultAssets.map { assetRow($0, loc: loc) },
                tabs: tabs, sheet: nil, textScale: 1
            )
        case .h7, .h7x:
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: balance(.normal, loc: loc, integer: "$1,234,567", decimals: "89"),
                actions: actions,
                activitySection: activitySection(loc: loc, mode: .rows),
                activityGroups: groupByDay(extremeActivity, loc: loc),
                assetsSection: assetsSection(loc: loc, mode: .rows),
                assetRows: extremeAssets.map { assetRow($0, loc: loc) },
                tabs: tabs, sheet: nil, textScale: state == .h7x ? 1.35 : 1
            )
        case .h8:
            model = replacing(model, activityGroups: groupByDay(defaultActivity, loc: loc))
            return WalletHomeModel(
                state: state, header: model.header, pill: pill,
                balance: model.balance, actions: actions,
                activitySection: model.activitySection,
                activityGroups: model.activityGroups,
                assetsSection: model.assetsSection,
                assetRows: model.assetRows,
                tabs: tabs,
                sheet: ChainSheetModel(title: loc.t("componentsUi.networkFilter.selectChain"), rows: chainRows(loc: loc)),
                textScale: 1
            )
        }
    }

    private static func replacing(_ model: WalletHomeModel, activityGroups: [ActivityGroupModel]) -> WalletHomeModel {
        WalletHomeModel(
            state: model.state, header: model.header, pill: model.pill,
            balance: model.balance, actions: model.actions,
            activitySection: model.activitySection,
            activityGroups: activityGroups,
            assetsSection: model.assetsSection,
            assetRows: model.assetRows,
            tabs: model.tabs, sheet: model.sheet, textScale: model.textScale
        )
    }
}
