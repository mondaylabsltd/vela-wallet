//
//  SettingsModels.swift
//  VelaWallet
//
//  Settings view models (spec 023). Components consume ONLY these
//  display-ready shapes — no preference store, no RPC probing, no storage
//  accounting, no formatting. The later "wire real settings" feature replaces
//  the fixture layer that builds them and nothing else. Mirrors the web
//  reference `src/lib/settings/model.ts`.
//
//  The forty mocks in `design/settings/` are a small vocabulary re-dealt,
//  which is why this file is short: a row, a segmented control, a select row,
//  a status pill, a callout, a URL field and a confirm sheet cover almost all
//  of them.
//

import SwiftUI

/// One id per mock in `design/settings/`.
enum SettingsStateId: String, CaseIterable, Identifiable {
    case st1, st1b, st2, st3, st3b, st4, st5, st6, st7, st8
    case st9, st9b, st10, st10b, st10c, st11, st12, st13, st13b, st14, st15, st16
    case sr1, sr2, sr2b, sr3, sr4, sr5

    var id: String { rawValue }

    /// Gallery chip label — mock/state code, not translatable copy.
    var label: String { rawValue.uppercased().replacingOccurrences(of: "B", with: "b") }
}

/// Which page the settings surface is showing (`home` plus the pushed pages).
enum SettingsPage: Equatable {
    case home, networks, networkDetail, addNetwork, rpcProviders, endpoints, storage, about
}

/// Which sheet is over it. `none` is a real state, not an absence of one.
enum SettingsOverlay: Equatable, Identifiable {
    case none, accounts, signOut, language, currency, numberFormat, dateFormat, timeFormat
    case clearCaches, eraseDevice, feedback, rpcFix, balanceDetail, relayer

    var id: String { String(describing: self) }
}

/// Status-pill tone. `neutral` is unset/idle, not failed.
enum SettingsTone { case ok, warn, error, neutral }

struct StatusPillModel {
    let tone: SettingsTone
    let label: String
    var dot: Bool = true
}

/// Callout tone. `success` swaps the triangle for a check.
enum CalloutTone { case warning, danger, info, success }

struct CalloutModel {
    let tone: CalloutTone
    let text: String
}

/// Row emphasis. `danger` is the red 退出登录 / 清理数据 family.
enum RowTone { case standard, accent, danger }

/// What sits at the end of a settings row.
enum RowTrailing { case chevron, external, none }

struct SettingsRowModel: Identifiable {
    /// Action-sink id — routed by the screen, never by the component.
    let id: String
    let title: String
    var icon: LucideGlyph?
    var subtitle: String?
    /// Right-aligned current value — "简体中文 · 系统", "12 个网络".
    var value: String?
    var trailing: RowTrailing = .chevron
    var tone: RowTone = .standard
}

struct SettingsSectionModel: Identifiable {
    let id = UUID()
    var rows: [SettingsRowModel]
    var label: String?
    /// ST1b: 高级 is a disclosure, and it remembers being open.
    var collapsible: Bool = false
    /// ST1: the appearance block ends in three CONTROLS rather than rows.
    /// Marking the section says so in the data, instead of the screen
    /// counting indices.
    var appearanceControls: Bool = false
}

/// ST1's identity block: avatar, name, address, and a trailing text action.
struct SettingsAccountRowModel {
    var name: String
    var addressDisplay: String
    /// Raw seed — IdenticonAvatar normalizes through vela-core; never
    /// lowercased at the call site (spec 003 rule).
    var addressFull: String
    let action: String
}

struct SegmentModel: Identifiable {
    let id: String
    let label: String
    var icon: LucideGlyph?
}

struct SegmentedModel {
    let label: String
    let segments: [SegmentModel]
    let selected: String
}

/// The A ——●—— A slider.
struct TextScaleModel {
    let label: String
    let steps: Int
    let index: Int
}

/// One choice in a picker (语言/货币/数字/日期/时间).
struct SelectRowModel: Identifiable {
    let id: String
    let label: String
    /// Right-aligned note — "系统 · 简体中文", "印度计数".
    var note: String?
    /// Leading circular badge — the currency sheet's ¥ / $ / €.
    var glyph: String?
    /// Secondary label after the primary one — the currency sheet's 美元.
    var caption: String?
    var selected: Bool = false
    /// Mono face — every number/date/time sample wants it.
    var mono: Bool = false
}

struct SelectSheetModel {
    let title: String
    let rows: [SelectRowModel]
    var subtitle: String?
    var searchPlaceholder: String?
    var footerNote: String?
    var footerLink: String?
}

struct AccountsSheetRowModel: Identifiable {
    var id: String { addressFull }
    var name: String
    var addressDisplay: String
    var addressFull: String
    let amount: String
    let selected: Bool
}

struct AccountsSheetModel {
    let title: String
    /// "3 个账户 · 总计 $3,262.40".
    let summary: String
    var rows: [AccountsSheetRowModel]
    let primary: String
    let secondary: String
}

/// ST3 / ST13b / ST16 share this; only the tone and the callout differ.
struct ConfirmSheetModel {
    let title: String
    let body: String
    let confirm: String
    let cancel: String
    let danger: Bool
    /// Second, quieter paragraph — the sign-out sheet's "keeps" line.
    var note: String?
    var callout: CalloutModel?
}

/// A chain's circular avatar: a letter over a fixture-supplied brand colour.
struct ChainMarkModel {
    let letter: String
    let color: Color
}

struct SettingsNetworkRowModel: Identifiable {
    let id: String
    /// The core's key for this chain (spec 050). `nil` on a fixture row, whose
    /// taps go nowhere by design — the id above is a slug, and a slug is not
    /// something `network_admin` can be asked about.
    var chainId: Int?
    let mark: ChainMarkModel
    let name: String
    /// "链 1" — the chain-id line under the name.
    let meta: String
    var badge: StatusPillModel?
    /// ST9: custom networks carry a 自定义 tag and a bin.
    var tag: String?
    var removable: Bool = false
}

struct UrlFieldModel: Identifiable {
    let id: String
    let label: String
    let value: String
    var placeholder: String?
    var hint: String?
    var badge: StatusPillModel?
    var tone: SettingsTone?
    /// The blue action inside the field — 检查密钥 / 获取密钥.
    var action: String?
}

struct NetworkDetailModel {
    let title: String
    /// "链 1 · ETH".
    let subtitle: String
    let mark: ChainMarkModel
    let name: String
    let note: String
    let badge: StatusPillModel
    let rpc: UrlFieldModel
    let explorer: UrlFieldModel
    var callout: CalloutModel?
}

struct CheckItemModel: Identifiable {
    var id: String { label }
    let label: String
    let ok: Bool
}

struct AddNetworkModel {
    let title: String
    var subtitle: String
    let searchPlaceholder: String
    var results: [SettingsNetworkRowModel] = []
    var candidate: SettingsNetworkRowModel?
    var checksTitle: String?
    var checks: [CheckItemModel] = []
    var customRpc: UrlFieldModel?
    var callout: CalloutModel?
    var primary: String?
    var secondary: String?
    var recheck: String?
}

struct ProviderCardModel: Identifiable {
    let id: String
    let name: String
    let badge: StatusPillModel
    let field: UrlFieldModel
    var support: String?
    var link: String?
}

struct RpcProvidersModel {
    let title: String
    let subtitle: String
    let description: String
    let providers: [ProviderCardModel]
}

struct EndpointsModel {
    let title: String
    let description: String
    let fields: [UrlFieldModel]
    let reset: String
}

struct StorageSegmentModel: Identifiable {
    let id: String
    let label: String
    let fraction: Double
    let color: Color
}

struct StorageItemModel: Identifiable {
    let id: String
    let label: String
    /// "200 条 · 1.0 MB" — already joined by the fixture layer.
    let meta: String
    let action: String
    var destructive: Bool = false
}

struct StorageGroupModel: Identifiable {
    var id: String { label }
    let label: String
    let items: [StorageItemModel]
    /// The 清除全部缓存 link under the cache group.
    var action: String?
}

struct StorageModel {
    let title: String
    let subtitle: String
    /// "2.4" and "MB", split so the number can carry the display type.
    let amount: String
    let unit: String
    let summary: String
    let segments: [StorageSegmentModel]
    let groups: [StorageGroupModel]
}

struct KeyValueRowModel: Identifiable {
    var id: String { label }
    let label: String
    let value: String
    var mono: Bool = false
    var external: Bool = false
}

struct AboutModel {
    let title: String
    let tagline: String
    let version: String
    let sectionTechnical: String
    let rows: [KeyValueRowModel]
    let links: [KeyValueRowModel]
    let footer: String
}

struct FeedbackModel {
    let title: String
    let subtitle: String
    let placeholder: String
    let addSteps: String
    let previewToggle: String
    let previewLines: [String]
    let consent: String
    let send: String
    let githubLink: String
}

/// SR1: the amber "these networks are down" banner and its per-chain fixes.
struct RpcBannerChipModel: Identifiable {
    let id: String
    let mark: ChainMarkModel
    let name: String
    let action: String
}

struct RpcBannerModel {
    let text: String
    let chips: [RpcBannerChipModel]
}

struct RpcFixModel {
    let title: String
    let mark: ChainMarkModel
    let name: String
    /// "链 137 · POL".
    let meta: String
    let badge: StatusPillModel
    let callout: CalloutModel
    let field: UrlFieldModel
    let primary: String
    var providersLabel: String?
    var providers: [String] = []
    var report: String?
}

/// SR3: the quiet rate-limited balance breakdown.
struct BalanceDetailRowModel: Identifiable {
    let id: String
    let mark: ChainMarkModel
    let name: String
    var status: String?
    var tone: SettingsTone = .neutral
    var action: String?
    var amount: String?
}

struct BalanceDetailModel {
    let title: String
    let summary: String
    let sectionPending: String
    let pendingNote: String
    let pending: [BalanceDetailRowModel]
    let sectionDone: String
    let done: [BalanceDetailRowModel]
}

/// SR4: fund this chain's bundler treasury.
struct RelayerModel {
    let title: String
    let lead: String
    let mark: ChainMarkModel
    let name: String
    let amountHint: String
    let qrCaption: String
    let addressDisplay: String
    let copyLabel: String
    let callout: CalloutModel
    let primary: String
}

/// SR5: the passkey index is unreachable, and onboarding needs it.
struct IndexDownModel {
    let title: String
    let subtitle: String
    let callout: CalloutModel
    let field: UrlFieldModel
    let primary: String
    let secondary: String
    let footer: String
}

/// Everything one settings state needs.
struct SettingsScreenModel {
    let state: SettingsStateId
    let title: String
    let page: SettingsPage
    let overlay: SettingsOverlay
    /// SR states sit on the 钱包 tab, over another screen.
    let rescue: Bool
    let tabs: TabsModel
    var account: SettingsAccountRowModel
    var sections: [SettingsSectionModel]
    var theme: SegmentedModel
    var avatar: SegmentedModel
    var textScale: TextScaleModel
    let signOutLabel: String
    let eraseTitle: String
    let eraseSubtitle: String
    let networksTitle: String
    let networksSubtitle: String
    var networks: [SettingsNetworkRowModel]
    let addNetworkLabel: String
    var networkDetail: NetworkDetailModel
    /// One detail per network, keyed by row id (spec 050).
    ///
    /// Empty for the fixtures, which draw a single detail page and reach it
    /// by state pin rather than by tapping a row. A live list fills it, so
    /// tapping Gnosis opens Gnosis.
    var networkDetails: [String: NetworkDetailModel] = [:]
    var addNetwork: AddNetworkModel
    let rpcProviders: RpcProvidersModel
    let endpoints: EndpointsModel
    let storage: StorageModel
    let about: AboutModel
    var accountsSheet: AccountsSheetModel
    let signOutSheet: ConfirmSheetModel
    var languageSheet: SelectSheetModel
    var currencySheet: SelectSheetModel
    var numberSheet: SelectSheetModel
    var dateSheet: SelectSheetModel
    var timeSheet: SelectSheetModel
    let clearCachesSheet: ConfirmSheetModel
    let eraseSheet: ConfirmSheetModel
    let feedback: FeedbackModel
    let rpcBanner: RpcBannerModel?
    let rpcFix: RpcFixModel
    let balanceDetail: BalanceDetailModel
    let relayer: RelayerModel
    let indexDown: IndexDownModel
    /// Scrim title behind a rescue sheet — "钱包", "转账", "设备存储".
    let backdropTitle: String
    let closeLabel: String

    /// The signed-in identity, swapped over the fixture account (spec 019's
    /// rule: a fixture name over a real address tells somebody they are signed
    /// in as a stranger).
    func withIdentity(name: String, address: String, display: String) -> SettingsScreenModel {
        var copy = self
        copy.account.name = name
        copy.account.addressFull = address
        copy.account.addressDisplay = display
        // Only the ACTIVE row: the other two are fixtures, and there is no
        // honest way to make them real without an account list the core does
        // not expose yet.
        copy.accountsSheet.rows = accountsSheet.rows.enumerated().map { index, row in
            guard index == 0 else { return row }
            var updated = row
            updated.name = name
            updated.addressFull = address
            updated.addressDisplay = display
            return updated
        }
        return copy
    }
}
