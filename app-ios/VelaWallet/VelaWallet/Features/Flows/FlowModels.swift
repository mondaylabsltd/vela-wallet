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
    /// Logo candidates from the chain-data endpoint, best first. Empty draws
    /// the lettermark, which is what every mark on this client did before 058.
    var logoURLs: [String] = []
    /// The chain badge's own logo, where there is one.
    var badgeLogoURL: String?
    /// ETH on Ethereum: the badge would repeat the token, so there is none.
    var badgeHidden: Bool = false

    /// The mark for one holding — glyph, logo candidates and badge in one
    /// place, so a caller cannot fill three of the four and draw a mark that
    /// disagrees with itself.
    static func of(
        chainId: Int,
        symbol: String,
        tokenAddress: String? = nil,
        color: Color,
        named: [String] = []
    ) -> TokenMarkModel {
        let mark = Marks.token(chainId: chainId, symbol: symbol,
                               tokenAddress: tokenAddress, named: named)
        return TokenMarkModel(
            ticker: symbol,
            badgeColor: color,
            logoURLs: mark.logoURLs,
            badgeLogoURL: mark.badgeLogoURL,
            badgeHidden: mark.badgeHidden
        )
    }

    /// A NETWORK by itself — its own logo, no badge.
    static func chain(chainId: Int, symbol: String, color: Color) -> TokenMarkModel {
        TokenMarkModel(
            ticker: symbol,
            badgeColor: color,
            logoURLs: [Marks.chainLogoURL(chainId)].compactMap { $0 },
            badgeLogoURL: nil,
            badgeHidden: true
        )
    }
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
    /// What that affordance puts on the clipboard. `nil` copies `value`, which
    /// is right only when the value is whole — an address or a hash is shown
    /// ELLIPSED, and copying `0x1234…abcd` gives somebody a string no chain
    /// has ever heard of. Android's `FactRow` has carried this field since 043.
    var copyValue: String?
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
    /// The network's own logo (058). Empty keeps the coloured disc with the
    /// coin's ticker on it — the drawing's mark, and the fallback.
    var logoURLs: [String] = []
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
    /// Shortened for the line; `copyValue` is what a person actually needs.
    let value: String
    let copyLabel: String
    /// The whole contract address. A copy button that put the ELLIPSED form on
    /// the clipboard would be worse than no button — Android carries the same
    /// field for the same reason.
    var copyValue: String?
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
    var name: String
    var lines: [String]
    var networkNote: String
    var networkMark: TokenMarkModel
    var identiconSeed: String
    let wordmark: String
    /// The real code's modules. `nil` keeps the drawn demo pattern, which is
    /// what the gallery renders — see `QrCode`.
    var modules: [[Bool]]?
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
    /// 删除记录 — the local record, not the transaction. Absent where there is
    /// nothing to delete (a fixture, a receipt still in flight). The web's
    /// `TxDetail.svelte` has drawn this button since 028 and never set the
    /// label, so it has been invisible on every client.
    var deleteLabel: String?
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
    /// The core's row id, so an edit can say which row it edited. Empty in
    /// the fixtures, which have no machine behind them.
    var rowId: String = ""
    /// The core's verdict on this row, if it has one — an address that is
    /// not one, an amount that cannot be sent, a repeat of an earlier payee.
    /// A row's problem belongs on the row, not in a sentence at the bottom of
    /// a list of six.
    var problem: String?
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
    /// Whether the ⇄ row is offered at all — the core's `denomToggleShown`.
    /// A token with no price cannot be counted in a currency, and a chevron
    /// there is an invitation the wallet cannot honour.
    var denomShown = true
    /// Offered but REFUSED, with the core's reason underneath. Different from
    /// absent: this one says why.
    var denomEnabled = true
    var denomReason: String?
    /// The amount came from a scanned code or a link and is not the person's
    /// to change. The field goes read-only rather than silently ignoring
    /// typing.
    var locked = false
    /// The unit, on the figure itself (issue 231): a currency symbol leads
    /// ("$4.00"), a code or a ticker follows ("4.00 PLN", "0.00075 BNB").
    /// Keyed on the FIGURE's own unit, never the display currency. `nil` on
    /// both is no adornment — the drawing, or a unit nobody can name.
    var unitPrefix: String?
    var unitSuffix: String?
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
    /// The split's total is over the balance — the core's
    /// `split_over_balance`, the same predicate `Continue` refuses on.
    var over = false
    /// "1.5 XDAI left" — the core's `split_remaining`, worded.
    var remaining: String?
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
    /// Split only: why `Continue` is dark — the FIRST unfinished row and the
    /// field it still needs, from the core's `split_row_issues`. Said only
    /// while no refusal is (the warning wins).
    var hint: String?
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
///
/// `Decodable` since 054: the core's `BatchUnit` has these two cases with
/// these two spellings, and a second identical type would be two places to
/// change and one of them forgotten. The house rule that keeps wire types
/// apart from display models is about shapes that can drift; this one cannot
/// without the drift test failing first.
enum BatchUnit: String, Decodable {
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
    /// 自动 — back to the fetched rate. Shown only once somebody has typed
    /// their own, because until then there is nothing to go back from.
    var rateReset: String = ""
    var rateEdited = false
    /// Whether the amounts are being read as FIAT and converted. When they are
    /// not, there is no rate — and the whole rate block goes away rather than
    /// standing there saying "1 xDAI = " with nothing after it.
    var priced = true
    let parsedLabel: String
    let rows: [BatchRowModel]
    var rejectedText: String?
    /// The one thing the core wants said that is not a rejected row: an
    /// unreadable file, over the balance, over the cap, or a template saved.
    /// Mirrors Android's `note` and the desktop's `notice`.
    var note: String?
    /// Whether that note is a refusal. The desktop colours the same three
    /// facts this way: a file that could not be read and a total that cannot
    /// be paid are errors; a trimmed list and a saved template are not.
    var noteIsError = false
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
    /// The coin being sent, drawn above the figure (founder, 2026-09-17). The
    /// confirm page named the asset in words only while every row beneath it
    /// carried art — the one screen where "which coin is this?" must be
    /// answerable at a glance. `nil` on a sweep: several coins, no one mark.
    var mark: TokenMarkModel?
    /// "120 USDT" / "3 assets".
    let amount: String
    /// "≈ $120.00" / "Total ≈ $200.90 · Ethereum".
    let subline: String
    let facts: [FactRowModel]
    var breakdown: [BreakdownRowModel] = []
    /// What stopped this page, in the core's words: a depleted relayer, a
    /// submit the relay refused, or the passkey prompt that is up right now.
    ///
    /// Without this the page was silent about all three — the CTA simply
    /// stopped working and nothing said why.
    var notice: String?
    /// The notice's own buttons. The primary retries what the notice is about;
    /// the secondary is 暂不, which keeps the facts and lets somebody go on
    /// looking at the page (spec 054 US4).
    var noticeAction: String?
    var noticeSecondary: String?
    /// A split's repeated payees, said again on the page that signs (issue
    /// 203): two lines paying one address are hardest to spot exactly here,
    /// where the avatars are identical and the sum looks right.
    var repeatNote: String?
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
    /// While the passkey ceremony is up the button is 取消 and it must NOT
    /// leave the screen: it is the core's own checkpoint (`cancel_signing`),
    /// and navigating away from it would abandon a prompt nobody can answer.
    /// Android has drawn this distinction since 043.
    var ctaCancels: Bool = false
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

/// What a sheet has to say after an action — 已保存, 需要权限, or a failure.
///
/// It is drawn as the platform's alert on purpose: the mocks have no alert
/// component, and inventing one for two sentences the corpus already carries
/// would be a new surface where a standard one does.
struct FlowAlertModel {
    let title: String
    let message: String
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
