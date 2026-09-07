//
//  FlowModels.swift
//  VelaWallet
//
//  Wallet-flow view models (spec 021 — the iOS port of the web's
//  `src/lib/flows/model.ts`). Components consume ONLY these display-ready
//  shapes: no service types, no formatting, no fetching. Every string
//  arrives resolved and every number arrives as text, so the later "real
//  data" feature replaces the fixture layer and nothing else.
//
//  Shapes spec 015 already defined (ActivityRowModel, AssetRowModel) are
//  reused rather than restated — the send token picker and the assets list
//  render the SAME row as the wallet home, and a parallel type would be the
//  first step towards a parallel component.
//

import SwiftUI

/// The thirty mobile states — spec.md's matrix, stable across all four clients.
enum FlowStateId: String, CaseIterable, Identifiable {
    case r1, r2, r2x, r3, r4
    case s1
    case a1, a2, a3
    case t1, t2, t3, t3b, t4, t5, t5b
    case sd1, sd1b, sd2, sd2b, sd2c, sd2d, sd2e, sd2f
    case sd3, sd3b, sd3c
    case sd4a, sd4b, sd4c

    var id: String { rawValue }

    /// Gallery chip label — mock naming, not translatable copy.
    var label: String { rawValue.uppercased() }
}

// MARK: - Chrome

/// A screen's own top bar: back, title, and at most one trailing text action.
struct FlowHeaderModel {
    let title: String
    let backLabel: String
    /// e.g. T1's 添加. Absent on screens whose header carries no action.
    var action: String?
    /// The network pill, where the screen filters by chain (A1, T1, SD1).
    var pill: FlowPillModel?
}

struct FlowPillModel {
    let dots: [Color]
    let label: String
}

// MARK: - Shared

/// A token's circular mark: three-letter glyph plus its chain colour.
struct TokenMarkModel {
    let ticker: String
    let badgeColor: Color
}

/// Leading art on a fact row's value side.
enum FactLead {
    case dot(Color)
    case token(TokenMarkModel)
    case identicon(String)
}

/// A label/value row. The single label-value primitive for the whole feature —
/// A2's transaction facts, SD3's summary, T2's token facts and T3b's chain
/// facts are the same row with different leading art.
struct FactRowModel: Identifiable {
    let id = UUID()
    let label: String
    let value: String
    var lead: FactLead?
    /// Renders the value in the mono face (addresses, hashes).
    var mono = false
    /// Shows a copy affordance under this accessible name.
    var copy: String?
}

enum StatusTone {
    case success, warning, error, info
}

struct StatusChipModel {
    let text: String
    let tone: StatusTone
}

// MARK: - Receive

/// One row of R1: a network, the address on it, and the two things you do.
struct NetworkRowModel: Identifiable {
    let id = UUID()
    let name: String
    let code: String
    let badgeColor: Color
    let addressDisplay: String
    let copyLabel: String
    let qrLabel: String
}

struct ReceiveListModel {
    let header: FlowHeaderModel
    /// "One address across all 8 networks".
    var subtitle: String
    let searchPlaceholder: String
    /// Shown in place of the rows when the search matches nothing.
    let emptyText: String
    var rows: [NetworkRowModel]
}

/// The account card that sits above every QR: whose address this is.
struct AddressCardModel {
    let name: String
    let identiconSeed: String
    /// The full address, pre-split into the two lines the mocks wrap it into.
    let lines: [String]
    let copyLabel: String
}

struct ContractLineModel {
    let label: String
    let value: String
    let copyLabel: String
}

struct ReceiveQrModel {
    var title: String
    let closeLabel: String
    /// R3 only: the token's contract, above the account card.
    var contract: ContractLineModel?
    var account: AddressCardModel
    /// The mark drawn in the middle of the code — the token, or the network.
    var centre: TokenMarkModel
    let warning: String
    let saveImage: String
    let viewOnExplorer: String
    /// The real code's modules, when there is a real address to encode.
    /// `nil` keeps the drawn demo pattern — see `QrCode`.
    var modules: [[Bool]]?
}

/// R4 — the image "Save image" produces, not a screen someone navigates to.
struct ShareCardModel {
    let headline: String
    let name: String
    let lines: [String]
    let networkNote: String
    let networkMark: TokenMarkModel
    let identiconSeed: String
    let wordmark: String
}

// MARK: - Scan

enum ScanTool: String, Identifiable {
    case gallery, torch, flip
    var id: String { rawValue }
}

struct ScanToolModel: Identifiable {
    let id: ScanTool
    let label: String
}

struct ScanModel {
    let title: String
    let hint: String
    let closeLabel: String
    let tools: [ScanToolModel]
}

// MARK: - Activity

enum HistoryMode {
    case rows, empty, loading
}

struct HistoryModel {
    let header: FlowHeaderModel
    let mode: HistoryMode
    let emptyText: String
    let groups: [ActivityGroupModel]
}

/// A2 / A3 — one transaction, opened from a history row.
struct TxDetailModel {
    let title: String
    let status: StatusChipModel
    let closeLabel: String
    let amount: String
    let fiat: String
    let positive: Bool
    let facts: [FactRowModel]
    let viewOnExplorer: String
}

// MARK: - Assets

struct AssetsEmptyModel {
    let title: String
    let caption: String
    let cta: String
    let hintTitle: String
    let hintBody: String
}

struct AssetsModel {
    let header: FlowHeaderModel
    let searchPlaceholder: String
    let rows: [AssetRowModel]
    /// T1's trailing link under the list.
    let addByAddress: String
    /// T4: the guided-empty body replaces the rows entirely.
    var empty: AssetsEmptyModel?
}

/// T2 — one token, opened from an assets row.
struct TokenDetailModel {
    let mark: TokenMarkModel
    let symbol: String
    let chain: String
    let closeLabel: String
    let balance: String
    let fiat: String
    let receive: String
    let send: String
    let facts: [FactRowModel]
    let transactionsTitle: String
    let rows: [ActivityRowModel]
    let viewOnExplorer: String
}

// MARK: - Add token

enum AddTokenTab: String {
    case erc20, native
}

/// The result card under the input: what the address or query resolved to.
enum AddTokenResult {
    case none
    case searching(String)
    case notFound(String)
    case token(mark: TokenMarkModel, name: String, detail: String, chip: StatusChipModel?)
    /// T5b's `link` is the "deploy the missing contracts" line under an
    /// incompatible chip.
    case network(
        mark: TokenMarkModel,
        name: String,
        chip: StatusChipModel,
        facts: [FactRowModel],
        link: String?
    )
}

struct AddTokenNetworkModel {
    let mark: TokenMarkModel
    let name: String
    let pickLabel: String
}

struct AddTokenModel {
    let title: String
    let closeLabel: String
    let tab: AddTokenTab
    let tabErc20: String
    let tabNative: String
    /// ERC-20 only: the network the contract is looked up on.
    var network: AddTokenNetworkModel?
    let fieldLabel: String
    // `var` since spec 051 phase 4: `manage_tokens` owns what is typed, what
    // the chains answered and whether the CTA may fire, and `FlowsLive` swaps
    // them the way `WalletLive` swaps the balance.
    var fieldValue: String
    let fieldPlaceholder: String
    /// Draws the field in its error state and prints this under it.
    var fieldError: String?
    var result: AddTokenResult
    let cta: String
    var ctaDisabled: Bool
}

// MARK: - Send

struct FilterChipModel: Identifiable {
    let id: String
    let label: String
    let selected: Bool
}

struct SendNoticeModel {
    let mark: TokenMarkModel
    let text: String
}

struct SendSelectionModel {
    let selected: [Bool]
    let dimmed: [Bool]
    let selectAll: String
}

struct SendCtaModel {
    let label: String
    let accent: Bool
}

/// SD1 / SD1b — pick the token, or several of them.
struct SendPickModel {
    let header: FlowHeaderModel
    let searchPlaceholder: String
    let filters: [FilterChipModel]
    /// SD1b: the chain lock, once the first token pins the network.
    var notice: SendNoticeModel?
    let rows: [AssetRowModel]
    var selection: SendSelectionModel?
    let cta: SendCtaModel
}

/// The token card at the top of the send form.
struct SendTokenCardModel {
    let mark: TokenMarkModel
    let symbol: String
    /// "Ethereum · Balance 53.4836".
    let detail: String
    var max: String?
}

/// SD2b's split row: who, how much, and a way to drop them.
struct RecipientCardModel: Identifiable {
    let id = UUID()
    let ordinal: String
    let name: String
    let identiconSeed: String
    let amount: String
    let removeLabel: String
}

/// SD2d's sweep row: one token, its amount, and a Max.
struct SweepRowModel: Identifiable {
    let id = UUID()
    let mark: TokenMarkModel
    let symbol: String
    let balanceLabel: String
    let amount: String
    let max: String
}

struct FeeRowModel {
    let label: String
    let mark: TokenMarkModel
    let value: String
    let openLabel: String
}

struct AmountFieldModel {
    let value: String
    let fiat: String
    let denomLabel: String
}

struct RecipientFieldModel {
    let label: String
    let lines: [String]
    let identiconSeed: String
    let pickLabel: String
    /// Sweep shows a scan button beside the picker; single does not.
    var scanLabel: String?
    /// Sweep's "every token goes to the same address".
    var note: String?
}

enum RecipientAction: String, Identifiable {
    case add, contacts, importList
    var id: String { rawValue }
}

struct RecipientActionModel: Identifiable {
    let id: RecipientAction
    let label: String
}

struct SummaryLineModel {
    let label: String
    let value: String
}

enum SendFormMode {
    case single, split, sweep
}

struct SendFormModel {
    let header: FlowHeaderModel
    let mode: SendFormMode
    var token: SendTokenCardModel?
    /// Sweep only: "3 tokens · Ethereum" plus the per-token rows.
    var sweepSummary: String?
    var sweepRows: [SweepRowModel] = []
    var amount: AmountFieldModel?
    var recipient: RecipientFieldModel?
    /// Single: the "+ add recipient" that turns this into a split.
    var addRecipient: String?
    var recipients: [RecipientCardModel] = []
    var recipientActions: [RecipientActionModel] = []
    var summary: SummaryLineModel?
    let fee: FeeRowModel
    let cta: String
}

/// SD2e — the contact picker.
struct ContactGroupModel: Identifiable {
    let id = UUID()
    let name: String
    let count: String
    let colors: [Color]
}

struct ContactEntryModel: Identifiable {
    let id = UUID()
    let name: String
    var group: String?
    let addressDisplay: String
    let identiconSeed: String
}

struct ContactPickModel {
    let title: String
    let closeLabel: String
    let searchPlaceholder: String
    let scanRow: String
    let groupsTitle: String
    let groups: [ContactGroupModel]
    let contactsTitle: String
    let contacts: [ContactEntryModel]
}

/// SD2f — the fee-token picker.
struct FeeTokenRowModel: Identifiable {
    let id = UUID()
    let mark: TokenMarkModel
    let symbol: String
    let balanceLabel: String
    let fee: String
    let selected: Bool
}

struct FeeTokenPickModel {
    let title: String
    let closeLabel: String
    let hint: String
    let estimateLabel: String
    let rows: [FeeTokenRowModel]
}

/// SD2c — the recipient importer.
enum BatchUnit: String {
    case fiat, token
}

struct BatchRowModel: Identifiable {
    let id = UUID()
    let ok: Bool
    let address: String
    let conversion: String
}

struct BatchImportModel {
    let title: String
    let closeLabel: String
    let unitFiat: String
    let unitToken: String
    let unit: BatchUnit
    let pasteValue: String
    let pastePlaceholder: String
    let importFile: String
    let template: String
    let rateSection: String
    let rateLabel: String
    let rateValue: String
    let rateHint: String
    let parsedLabel: String
    let rows: [BatchRowModel]
    var rejectedText: String?
    let cta: String
    let ctaDisabled: Bool
}

/// SD3 — the confirmation.
struct BreakdownRowModel: Identifiable {
    let id = UUID()
    var lead: TokenMarkModel?
    var identiconSeed: String?
    let label: String
    let value: String
}

struct SendConfirmModel {
    let header: FlowHeaderModel
    /// "120 USDT" / "3 assets".
    let amount: String
    /// "≈ $120.00" / "Total ≈ $200.90 · Ethereum".
    let subline: String
    let facts: [FactRowModel]
    var breakdown: [BreakdownRowModel] = []
    let cta: String
}

enum ReceiptStage {
    case submitting, submitted, confirmed, failed
}

struct ReceiptHashModel {
    let label: String
    let value: String
    let copyLabel: String
}

/// SD4 — the receipt, in whichever of its states the transaction is in.
struct SendReceiptModel {
    let header: FlowHeaderModel
    let stage: ReceiptStage
    let title: String
    /// Up to two lines under the title.
    let captions: [String]
    var hash: ReceiptHashModel?
    var viewOnExplorer: String?
    /// The single bottom button: "Close · keep running" or "Done".
    let cta: String
    let ctaAccent: Bool
}

// MARK: - The screens

/// The screen under a state.
enum FlowBase {
    case receive(ReceiveListModel)
    case share(ShareCardModel)
    case scan(ScanModel)
    case history(HistoryModel)
    case assets(AssetsModel)
    case sendPick(SendPickModel)
    case sendForm(SendFormModel)
    case sendConfirm(SendConfirmModel)
    case sendReceipt(SendReceiptModel)
}

/// The sheet over it, where the state has one.
enum WalletFlowSheet: Identifiable {
    case receiveQr(ReceiveQrModel)
    case txDetail(TxDetailModel)
    case tokenDetail(TokenDetailModel)
    case addToken(AddTokenModel)
    case contactPick(ContactPickModel)
    case feeToken(FeeTokenPickModel)
    case batchImport(BatchImportModel)

    var id: String {
        switch self {
        case .receiveQr: "receiveQr"
        case .txDetail: "txDetail"
        case .tokenDetail: "tokenDetail"
        case .addToken: "addToken"
        case .contactPick: "contactPick"
        case .feeToken: "feeToken"
        case .batchImport: "batchImport"
        }
    }

    var closeLabel: String {
        switch self {
        case .receiveQr(let m): m.closeLabel
        case .txDetail(let m): m.closeLabel
        case .tokenDetail(let m): m.closeLabel
        case .addToken(let m): m.closeLabel
        case .contactPick(let m): m.closeLabel
        case .feeToken(let m): m.closeLabel
        case .batchImport(let m): m.closeLabel
        }
    }

    /// The QR, the transaction and the token draw their own heading inside the
    /// body, so the sheet chrome would say it twice.
    var chromeTitle: String? {
        switch self {
        case .receiveQr, .txDetail, .tokenDetail: nil
        case .addToken(let m): m.title
        case .contactPick(let m): m.title
        case .feeToken(let m): m.title
        case .batchImport(let m): m.title
        }
    }
}

/// One state: the screen, and the sheet over it.
///
/// Sheets are an overlay on a base screen rather than states of their own
/// because that is what they are — A2 is the history with a transaction over
/// it, and the history behind it is still the history.
struct FlowScreenModel {
    let state: FlowStateId
    // `var` since spec 051 phase 4: `FlowsLive` swaps the assets list for the
    // person's own holdings.
    var base: FlowBase
    var sheet: WalletFlowSheet?
    /// 1 or 1.35 — threaded through `walletTextScale`, as spec 015's H7x is.
    var textScale: CGFloat = 1
}
