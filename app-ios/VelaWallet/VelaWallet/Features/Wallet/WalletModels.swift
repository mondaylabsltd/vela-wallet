//
//  WalletModels.swift
//  VelaWallet
//
//  Wallet view models (spec 015, data-model.md). Components consume ONLY
//  these display-ready shapes — no service types, no formatting, no
//  fetching (FR-005 / SC-005). A later "real data" feature replaces the
//  fixture layer that builds them and nothing else. Mirrors the web
//  reference `src/lib/wallet/model.ts`.
//

import SwiftUI

/// The nine mobile home states plus the chain-select sheet (H8).
enum MobileStateId: String, CaseIterable, Identifiable {
    case h1, h1s, h2, h3, h4, h5, h6, h7, h7x, h8
    var id: String { rawValue }

    /// Gallery chip label — mock naming, not translatable copy.
    var label: String {
        switch self {
        case .h1: "H1"
        case .h1s: "H1s"
        case .h2: "H2"
        case .h3: "H3"
        case .h4: "H4"
        case .h5: "H5"
        case .h6: "H6"
        case .h7: "H7"
        case .h7x: "H7x"
        case .h8: "H8"
        }
    }
}

struct WalletHeaderModel {
    let name: String
    let addressDisplay: String
    /// Raw seed — IdenticonAvatar normalizes through vela-core (FR-006).
    let identiconSeed: String
}

enum NetworkPillModel {
    case all(dots: [Color], label: String)
    case single(dot: Color, label: String)
}

enum BalanceStateKind {
    case normal, zeroLive, loading, hidden
}

struct BalanceStatusModel {
    enum Kind { case warning, refreshing }
    let kind: Kind
    let text: String
}

struct BalanceModel {
    let label: String
    let currency: String
    let state: BalanceStateKind
    /// e.g. "$1,383" — absent while loading; mask string when hidden.
    var integer: String?
    /// e.g. "28" — rendered de-emphasised after the separator.
    var decimals: String?
    /// zero-live only (实时 · 监听收款中).
    var liveText: String?
    var status: BalanceStatusModel?
    let a11yHide: String
    let a11yShow: String
}

enum ActivityKind {
    case sent, received, dapp
}

struct ActivityRowModel: Identifiable {
    let id = UUID()
    let kind: ActivityKind
    let title: String
    let subtitle: String
    let amount: String
    let unit: String
    let positive: Bool
    let masked: Bool
    let badgeColor: Color
}

struct ActivityGroupModel: Identifiable {
    let id = UUID()
    let label: String
    let rows: [ActivityRowModel]
}

enum AssetFiatModel {
    case value(String)
    /// Orange 无价格 marker (H4).
    case noPrice(String)
    case masked
    /// Spec 021 SD2d: the row has no fiat line at all. Distinct from `masked`,
    /// which HIDES a figure that exists — a sweep row is an editable amount,
    /// and dots under it read as a concealed second number.
    case none
}

struct AssetRowModel: Identifiable {
    let id = UUID()
    let ticker: String
    let chain: String
    let badgeColor: Color
    let balance: String
    let fiat: AssetFiatModel
    let masked: Bool
    /// The whole mark, where the builder knows the chain and contract (058).
    /// `nil` keeps the lettermark, which is what every fixture row draws.
    var mark: TokenMarkModel?
}

enum SectionMode {
    case rows, empty, loading
}

struct SectionEmptyModel {
    let title: String
    let caption: String
}

struct SectionModel {
    let title: String
    let action: String
    let mode: SectionMode
    var empty: SectionEmptyModel?
}

enum ChainDot {
    /// 所有网络 row — neutral dot resolved from the theme at render time.
    case all
    case color(Color)
}

struct ChainRowModel: Identifiable {
    let id = UUID()
    let name: String
    let dot: ChainDot
    let count: Int
    let selected: Bool
    /// Which chain this row picks; `nil` is the 所有网络 row. Absent in the
    /// fixtures, which are a picture of the list rather than a picker.
    var chainId: Int?
}

struct ChainSheetModel {
    let title: String
    let rows: [ChainRowModel]
}

struct TabsModel {
    let wallet: String
    let contacts: String
    let explore: String
    let settings: String
}

struct ActionsModel {
    let receive: String
    let send: String
    let scan: String
}

struct WalletHomeModel {
    let state: MobileStateId
    var header: WalletHeaderModel
    let pill: NetworkPillModel
    var balance: BalanceModel
    let actions: ActionsModel
    // `var` since spec 051 phase 3: the activity section is a machine's now,
    // and `WalletLive` swaps it the way it already swaps the balance.
    var activitySection: SectionModel
    var activityGroups: [ActivityGroupModel]
    let assetsSection: SectionModel
    var assetRows: [AssetRowModel]
    let tabs: TabsModel
    var sheet: ChainSheetModel?
    /// 1 or 1.35 — multiplies wallet type roles via walletTextScale (FR-011).
    let textScale: CGFloat

    /// Swap in the signed-in wallet's real address (spec 019).
    ///
    /// The rest of this screen is still the spec-015 fixture layer, and that is
    /// the point of doing it here rather than inside the fixtures: an address is
    /// the ONE thing on the home screen a person acts on, and showing a fixture
    /// address after a real create would be the app telling them their money is
    /// somewhere it is not. Everything else on the page is
    /// visibly-placeholder balance data; an address is not.
    ///
    /// An empty address changes nothing — the developer routes reach this
    /// screen with no session at all.
    func withAddress(_ address: String) -> WalletHomeModel {
        guard !address.isEmpty else { return self }
        var copy = self
        copy.header = WalletHeaderModel(
            name: header.name,
            addressDisplay: Self.shorten(address),
            identiconSeed: address
        )
        return copy
    }

    /// Swap in the signed-in wallet's real NAME (spec 019).
    ///
    /// Same argument as the address, and found the same way — on a device.
    /// The header drew the fixture's 大表哥 over a real address and a real
    /// identicon, so the one line on the screen that was wrong was the one
    /// line a person reads as "this is my wallet".
    ///
    /// An empty name changes nothing: the developer routes reach this screen
    /// with no session, and a blank header is worse than a placeholder one.
    func withName(_ name: String) -> WalletHomeModel {
        guard !name.isEmpty else { return self }
        var copy = self
        copy.header = WalletHeaderModel(
            name: name,
            addressDisplay: header.addressDisplay,
            identiconSeed: header.identiconSeed
        )
        return copy
    }

    /// `0x1234…cdef` — the house short form, matching the other three clients.
    private static func shorten(_ address: String) -> String {
        guard address.count > 10 else { return address }
        return "\(address.prefix(6))\u{2026}\(address.suffix(4))"
    }
}
