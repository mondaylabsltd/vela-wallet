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
import VelaCore

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
    /// The letters the core chose for a mark it answered (`MarkView.glyph`).
    /// `nil` on a drawn mark, which letters itself from the ticker.
    var coreGlyph: String?

    /// The letters drawn in the circle, under any logo: the core's, else the
    /// ticker's first three upper-cased — the same rule, for the gallery's
    /// marks that never asked the core.
    var glyph: String { coreGlyph ?? String(ticker.prefix(3)).uppercased() }

    /// The mark for one holding — glyph, logo candidates and badge in one
    /// place, so a caller cannot fill three of the four and draw a mark that
    /// disagrees with itself. The core decides all of it (`Marks.token`).
    static func of(
        chainId: Int,
        symbol: String,
        tokenAddress: String? = nil,
        color: Color,
        named: [String] = []
    ) -> TokenMarkModel {
        from(Marks.token(chainId: chainId, symbol: symbol,
                         tokenAddress: tokenAddress, named: named),
             ticker: symbol, color: color)
    }

    /// A NETWORK by itself — its own logo, no badge (the kind rule: a network
    /// row never wears its coin's logo).
    static func chain(chainId: Int, symbol: String, color: Color) -> TokenMarkModel {
        from(Marks.chain(chainId: chainId, nativeSymbol: symbol), ticker: symbol, color: color)
    }

    /// The core's answer as the model the views draw. The badge is drawn
    /// exactly when the core names its chain, over `color` — that chain's dot.
    static func from(_ mark: MarkView, ticker: String, color: Color) -> TokenMarkModel {
        TokenMarkModel(
            ticker: ticker,
            badgeColor: color,
            logoURLs: mark.logoUrls,
            badgeLogoURL: mark.badgeLogoUrl,
            badgeHidden: mark.badgeChainId == nil,
            coreGlyph: mark.glyph
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
    /// A second line under the value, in the body face, never cut: under a
    /// NAME, whose word it is and the short address — "Vela User ·
    /// 0x14fB…eA5c" (spec 097 F). On the page that signs a name is a claim and
    /// the address is what is paid, so the two are drawn together — on two
    /// lines, so a long name can never push the tag or the address off the row.
    /// The lead sits beside the name, on its line, as the From row's (#423).
    var detail: String?
    /// Shows a copy affordance under this accessible name.
    var copy: String?
    /// What that affordance puts on the clipboard. `nil` copies `value`, which
    /// is right only when the value is whole — an address or a hash is shown
    /// ELLIPSED, and copying `0x1234…abcd` gives somebody a string no chain
    /// has ever heard of. Android's `FactRow` has carried this field since 043.
    var copyValue: String?
    /// One calm sentence under the row saying why its value is what it is —
    /// the confirm's speed row uses it for a speed taken because it was free
    /// (issue 686), so the tier and its reason reach the last screen together.
    var note: String?
    /// The value is a risk to see — an unlimited spending cap (spec 093).
    var danger = false
    /// Several values under one label — a dApp's balance changes (spec 093),
    /// one line each in the signing sheet's own form and tones. Drawn in
    /// place of `value` when present.
    var lines: [BalanceDeltaRow] = []
    /// The value may take a second line, broken only between its ` · `
    /// parts — never inside a figure or between a figure and its unit
    /// (`FactRowView.unbreakable`). The confirm's fee: cut to one line it
    /// read "~0.000173…B · ≈¥0.85" (iPhone pass 2026-10-09).
    var wraps = false
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
    /// The one address every row shows shortened — what each row's copy
    /// button puts on the clipboard. Only ever the person's own (set by
    /// `FlowsLive.receiveList`): the fixture leaves it empty, so a drawn
    /// board copies nothing rather than somebody else's address.
    var address = ""
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
    /// Spec 090: the "include network" switch under the code, as the core's
    /// `payment_request` offers it — `nil` where it offers none.
    var network: NetworkSwitchModel?
    let warning: String
    let saveImage: String
    let viewOnExplorer: String
    /// The real code's modules, when there is a real address to encode.
    /// `nil` keeps the drawn demo pattern — see `QrCode`.
    var modules: [[Bool]]?
}

/// The receive code's "include network" switch (spec 090), exactly as the
/// core says: its position, and the calm line under it while it is on.
struct NetworkSwitchModel: Equatable {
    let label: String
    let isOn: Bool
    /// "Some wallets can't read this code…" — present exactly while the core's
    /// `networkHint` is.
    let hint: String?
}

/// R4 — the image "Save image" produces, not a screen someone navigates to.
struct ShareCardModel {
    let headline: String
    var name: String
    var lines: [String]
    var networkNote: String
    /// The network the address may be paid on. Its `logoURLs` are what the
    /// save fetches for the code's centre; without them, or when the fetch
    /// fails, the lettered disc (ticker on `badgeColor`) stands in.
    var networkMark: TokenMarkModel
    var identiconSeed: String
    let wordmark: String
    /// The real code's modules, at level H (`QrCode.shareModules`) — the logo
    /// plate covers part of it. `nil` keeps the drawn demo pattern, which is
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
    /// `nil` for an off-chain signature (spec 093): nothing settles it, and
    /// `note` says so instead.
    let status: StatusChipModel?
    let closeLabel: String
    let amount: String
    let fiat: String
    let positive: Bool
    let facts: [FactRowModel]
    /// 在区块浏览器中查看 — `nil` where there is no transaction to open (spec
    /// 082 RJ16, G52): an op the relay refused, or one the chain has not
    /// shown yet, has only an op hash, and an op hash is never an explorer
    /// link. No label, no control.
    let viewOnExplorer: String?
    /// 删除记录 — the local record, not the transaction. Absent where there is
    /// nothing to delete (a fixture, a receipt still in flight). The web's
    /// `TxDetail.svelte` has drawn this button since 028 and never set the
    /// label, so it has been invisible on every client.
    var deleteLabel: String?
    /// The record is still pending (spec 082 RJ18, G51): its delete is a
    /// quiet control under the rest, never the most prominent thing on a
    /// "may have been sent" record — forgetting it is how a person sends the
    /// same money again.
    var deleteQuiet = false
    /// One calm sentence where the status chip would be: "Off-chain
    /// signature — nothing was sent on-chain" (spec 093).
    var note: String? = nil
    /// The figure is an unlimited allowance, drawn in the danger tone.
    var amountDanger = false
    /// A swap's coin back, under the figure (spec 093).
    var received: String? = nil
    /// A dApp record's collapsed "Technical details" (spec 093).
    var technical: TxTechnicalModel? = nil
}

/// A dApp record's "Technical details" (spec 093), collapsed until tapped.
struct TxTechnicalModel {
    let title: String
    let lines: [TxTechnicalLine]
}

enum TxTechnicalLine {
    case fact(FactRowModel)
    /// The request the record kept. Read from the store by record id only
    /// when the section is opened — never while the detail is built.
    case content(TxContentModel)
}

struct TxContentModel {
    let label: String
    /// Said when the record kept nothing (`connect.detail.contentMissing`).
    let missing: String
    let read: () -> String?
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
    /// The network the notice is about, in its own mark — `nil` for a notice
    /// that names no network (a blank disc said nothing).
    let mark: TokenMarkModel?
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
    /// Issue #332: whom the money is for, when the core already holds a
    /// recipient — a code scanned from the home, a contact handed over. The
    /// picker is where the person chooses WHAT to send; without this line a
    /// scan that worked looked exactly like one that had done nothing.
    var recipient: FactRowModel?
    let searchPlaceholder: String
    let filters: [FilterChipModel]
    /// SD1b: the chain lock, once the first token pins the network.
    var notice: SendNoticeModel?
    let rows: [AssetRowModel]
    var selection: SendSelectionModel?
    let cta: SendCtaModel
    /// What an empty list says once the core has looked — nothing held (on
    /// the network a scanned code named, issue #312), or nothing matching.
    /// The web's and Android's line since issue 209.
    var empty: String?
}

/// The token card at the top of the send form.
struct SendTokenCardModel {
    let mark: TokenMarkModel
    let symbol: String
    /// "Ethereum · Balance 53.4836".
    let detail: String
    var max: String?
    /// Issue #326: present when tapping the card goes back to the asset
    /// picker (the core's `can_change_token`), carrying its accessible name.
    var change: String?
}

/// SD2b's split row: who, how much, and a way to drop them.
struct RecipientCardModel: Identifiable {
    /// The core's row id; the ordinal in a fixture, which has none. It was a
    /// fresh `UUID()` per init, and the form is rebuilt on every render — a
    /// keystroke's, a fee quote's — so every render replaced every row, and
    /// the amount field being typed into went with it: the first key stayed,
    /// the focus and the rest did not ("0,25" typed, "0" and 金额无效 left;
    /// found 2026-09-28 beside the send amount's lost keys).
    var id: String { rowId.isEmpty ? ordinal : rowId }
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
    /// The refresh control's accessible name (spec 068; iOS's since 069) —
    /// `nil` draws none. The fee is the one figure on the form that moves on
    /// its own, and a person could neither re-read it nor be told it went old.
    var refreshLabel: String? = nil
    /// A measurement is out, whoever started it: the control says so.
    var refreshing = false
    /// The quote's 30 s TTL ran out — calm, never a fault. Its line is kept.
    var staleNote: String? = nil
}

/// One option of the speed control (spec 068).
struct FeeSpeedOptionModel: Identifiable {
    /// The wire tier — `fast` / `standard` / `slow`.
    let id: String
    /// The SPEED — 超快 / 标准 / 较慢 — never a number. What each one buys is
    /// said on Settings' default speed, not in this per-payment picker.
    let label: String
    /// This option's OWN fee, or the "…" / "—" standing in for it.
    let value: String
    /// Its gas bid as a range, already formatted by the core over the set.
    let gasPrice: String?
    let selected: Bool
}

/// The speed control under the fee row, folded until opened (spec 068). Every
/// decision in it is the `fee_speed` core's (spec 069); this is only words.
struct FeeSpeedModel {
    let label: String
    /// The folded summary: the tier in force for THIS send.
    let value: String
    let open: Bool
    let onceNote: String
    /// Why the tier in force is the fastest when the default is slower.
    let freeNote: String?
    /// This network has one speed: the options give way to it.
    let singleNote: String?
    let gasPriceLabel: String
    /// Whether the options carry a gas-bid line at all.
    let gasPriceLine: Bool
    let options: [FeeSpeedOptionModel]
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
    /// The note is a warning (spec 096 F12: a token's own contract).
    var noteWarn = false
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
    /// The speed control (spec 068). `nil` draws none.
    var speed: FeeSpeedModel? = nil
    let cta: String
    /// Split only: why `Continue` is dark — the FIRST unfinished row and the
    /// field it still needs, from the core's `split_row_issues`. Said only
    /// while no refusal is (the warning wins).
    var hint: String?
    /// Split only: "Use 0.5 ETH for the empty rows" — one typed figure into
    /// every row that has none (the web's `model.fillEmpty`).
    var fillEmpty: FillEmptyModel?
}

/// The words, and the figure exactly as it was typed.
struct FillEmptyModel: Equatable {
    let label: String
    let amount: String
}

/// SD2e — the contact picker.
///
/// Rows are keyed by what they ARE (issue #467): a group by the book's own
/// id, a person by their address. A `UUID()` minted per build gave every row
/// a new identity on each render — 56 rows rebuilt under the finger whenever
/// a fee, a balance or a name landed, which can drop a tap in progress.
struct ContactGroupModel: Identifiable {
    /// The book's group id; a pick adds THIS group's members.
    let id: String
    let name: String
    let count: String
    let colors: [Color]
}

struct ContactEntryModel: Identifiable {
    var id: String { address }
    let name: String
    var group: String?
    /// The person's full address — what a tap hands back. Never a position:
    /// the core re-sorts the book (favourites, recency, names as they
    /// resolve), so an index can name the neighbour by the time it lands.
    let address: String
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
    /// The core's verdict that this coin cannot pay the fee. The row is still
    /// drawn — it is context — but it answers to nothing: `select_fee_asset`
    /// refuses it, and a row that looks like the others and silently does
    /// nothing is how somebody believes they chose to pay gas in a coin they
    /// do not hold (web issue 211).
    var insufficient = false
    /// What such a row says in place of its balance.
    var insufficientNote: String?
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
    /// "Total · 3 recipients", in the token and the fiat, over the balance it
    /// is paid from. `nil` until a row parses, and in the gallery.
    var total: BatchTotalModel?
    /// How an import meets rows already on the form, and the way to choose the
    /// other. `nil` when the form is empty or the import cannot apply.
    var merge: BatchMergeModel?
    let cta: String
    let ctaDisabled: Bool
}

struct BatchTotalModel {
    let label: String
    let value: String
    var detail: String?
    /// "Balance 12.5 xDAI", or — adding to rows already typed — "3 xDAI left".
    let balance: String
    /// The total cannot be paid; the note above says so, this colours it.
    var over = false
}

struct BatchMergeModel {
    let note: String
    let action: String
}

/// SD3 — the confirmation.
struct BreakdownRowModel: Identifiable {
    let id = UUID()
    var lead: TokenMarkModel?
    var identiconSeed: String?
    let label: String
    let value: String
    /// A second line under the label: a named payee's tag and short address
    /// (spec 097 F), which the name may not push out.
    var detail: String?
    /// The label is an address, in the mono face.
    var mono = false
}

/// Spec 098 §4: where the relay's gas goes, under its treasury stop — the
/// address in full and the button that copies it. Until 098 the phone said
/// "fund it" and never said where.
struct FundAddressModel: Hashable {
    let label: String
    let address: String
    let copy: String
    let copied: String
}

struct SendConfirmModel {
    let header: FlowHeaderModel
    /// The coin being sent, drawn above the figure (founder, 2026-09-17). The
    /// confirm page named the asset in words only while every row beneath it
    /// carried art — the one screen where "which coin is this?" must be
    /// answerable at a glance. `nil` on a sweep: several coins, no one mark.
    var mark: TokenMarkModel?
    /// "120" beside `amountUnit`; a split's "120 USDT" / a sweep's "3 assets"
    /// whole, with no unit.
    let amount: String
    /// A single send's coin, drawn as its own smaller muted piece on the
    /// figure's baseline (round 2: the Send form's hero, on the page that
    /// signs). `nil` = `amount` is the whole headline.
    var amountUnit: String? = nil
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
    /// Spec 098 §4: the treasury stop's address, under [notice].
    var noticeFund: FundAddressModel? = nil
    /// The notice's own buttons. The primary retries what the notice is about;
    /// the secondary is 暂不, which keeps the facts and lets somebody go on
    /// looking at the page (spec 054 US4).
    var noticeAction: String?
    var noticeSecondary: String?
    /// Issue #466: a relay stop's "Report this" — the in-app report, seeded
    /// with the core's. `nil` when the core built no report (a network the
    /// person added: nobody else to tell).
    var noticeReport: String? = nil
    /// A split's repeated payees, said again on the page that signs (issue
    /// 203): two lines paying one address are hardest to spot exactly here,
    /// where the avatars are identical and the sum looks right.
    var repeatNote: String?
    /// "First time sending here" — the anti-poisoning tell, on the page that
    /// signs. The core resolves it only while this page is up
    /// (`confirm_probes`), so the form never has it to show. `nil` on a split.
    var recipientTag: String?
    var cta: String
    /// Spec 102: this account reviews and signs on a trusted page — the
    /// page, the key, its integrity line — and the CTA goes there.
    var handoff: HandoffCardModel? = nil
    /// PR 2 §3: the confirm is held while this account's previous
    /// transaction on this network is in flight — the core's one line, drawn
    /// under the held CTA for as long as it holds, whatever the fee says.
    var heldNote: String? = nil
}

enum ReceiptStage {
    case submitting, submitted, confirmed, failed
}

struct ReceiptHashModel {
    let label: String
    let value: String
    let copyLabel: String
    /// What the copy button copies when `value` is a short form (spec 079:
    /// the dApp receipt shows `0x1234…abcd`); `nil` copies `value`.
    var copyValue: String? = nil
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
    /// Spec 096 F8: a dApp request that failed before anything was sent —
    /// "Try again" beside the button, which takes the request back to review.
    var retry: String?
    /// While the passkey ceremony is up the button is 取消 and it must NOT
    /// leave the screen: it is the core's own checkpoint (`cancel_signing`),
    /// and navigating away from it would abandon a prompt nobody can answer.
    /// Android has drawn this distinction since 043.
    var ctaCancels: Bool = false
    /// While the relay has the op: when it was handed over and how long the
    /// chain usually takes. The screen counts; the sentences come from here.
    var eta: ReceiptEtaModel?
    /// A split: "N recipients", then every one of them (web spec 038 #D2).
    var breakdownTitle: String?
    var breakdown: [BreakdownRowModel] = []
}

/// The core's landing pace (`tx_tracker::LandingPace`, spec 099 R6): the
/// countdown line and the ring, counted from when the relay put the bundle on
/// the network — never from acceptance, which counted the relay's own queue
/// and funding as the chain being slow.
struct LandingPaceWire: Decodable, Equatable {
    /// `waiting` (the relay has not sent it: no countdown, the landing says
    /// what the relay is doing) · `none` (no usual time) · `remaining` ·
    /// `elapsed` · `slow`. A string: a line this build has never heard of
    /// draws no countdown rather than failing.
    let line: String
    /// What `remaining` / `elapsed` count.
    let seconds: Int
    /// How full the ring is; `nil` — the ring roams.
    let progress: Double?

    /// The relay has not put it on the network yet.
    var waiting: Bool { line == "waiting" }
    /// A countdown line is drawn.
    var counts: Bool { line == "remaining" || line == "elapsed" || line == "slow" }

    /// The core's `landingPace`. `typicalS` `nil` or out of range is "no usual
    /// time".
    static func of(sentAtMs: Double?, typicalS: Int?, nowMs: Double) -> LandingPaceWire {
        let typical = typicalS.flatMap { UInt16(exactly: $0) }
        let json = landingPace(sentAtMs: sentAtMs, typicalS: typical, nowMs: nowMs)
        return (try? CoreJSON.decoder.decode(LandingPaceWire.self, from: Data(json.utf8)))
            ?? LandingPaceWire(line: sentAtMs == nil ? "waiting" : "none", seconds: 0, progress: nil)
    }
}

/// The submitted receipt's clock (web `SendReceipt.svelte`, spec 038 #D3).
/// Only the number is the screen's; every sentence is filled here, and WHICH
/// sentence and how full the ring is are the core's `landingPace`.
struct ReceiptEtaModel: Equatable {
    /// When the relay put the bundle on the network (the tracker's
    /// `relay_sent_at_ms`) — where the countdown starts.
    let sentAtMs: Double
    let typicalS: Int
    /// "Gnosis typically confirms in ~15s" — already filled.
    let typicalLine: String
    /// "~{{remaining}}s remaining" — inside the typical time.
    let remainingTemplate: String
    /// "{{elapsed}}s elapsed — almost there" — past it, where "almost" is true.
    let elapsedTemplate: String
    /// Past twice the typical time.
    let slowLine: String

    func pace(nowMs: Double) -> LandingPaceWire {
        LandingPaceWire.of(sentAtMs: sentAtMs, typicalS: typicalS, nowMs: nowMs)
    }

    /// Inside the typical time the line counts DOWN — "~9s remaining" is a
    /// promise with an end, "6s elapsed" is a stopwatch. The core says which.
    func lines(nowMs: Double) -> [String] {
        let pace = pace(nowMs: nowMs)
        switch pace.line {
        case "remaining":
            return [typicalLine, remainingTemplate.replacingOccurrences(of: "{{remaining}}", with: String(pace.seconds))]
        case "elapsed":
            return [typicalLine, elapsedTemplate.replacingOccurrences(of: "{{elapsed}}", with: String(pace.seconds))]
        case "slow":
            return [typicalLine, slowLine]
        default:
            return [typicalLine]
        }
    }

    /// The ring round the disc, the core's curve: it eases toward full and
    /// never gets there; only the confirmation closes it. `nil` — it roams.
    func progress(nowMs: Double) -> Double? { pace(nowMs: nowMs).progress }

    /// The receipt's clock for an op the relay has sent, or `nil` when there
    /// is nothing to count (`landingPace` says `waiting` or `none`).
    static func counting(
        sentAtMs: Double?, typicalS: Int?, typicalLine: String?, loc: Loc, nowMs: Double
    ) -> ReceiptEtaModel? {
        guard let sentAtMs, let typicalS, let typicalLine,
              LandingPaceWire.of(sentAtMs: sentAtMs, typicalS: typicalS, nowMs: nowMs).counts
        else { return nil }
        return ReceiptEtaModel(
            sentAtMs: sentAtMs, typicalS: typicalS, typicalLine: typicalLine,
            // Filled with its own placeholder: the screen fills the number.
            remainingTemplate: loc.t("send.txRemaining", vars: ["remaining": "{{remaining}}"]),
            elapsedTemplate: loc.t("send.txElapsed", vars: ["elapsed": "{{elapsed}}"]),
            slowLine: loc.t("send.txSlowConfirm")
        )
    }
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
    /// The one button: `common.gotIt` ("知道了" / "Got it"). The alert only
    /// informs, so its button acknowledges. It was the hard-coded English "OK"
    /// on every language until round 2 (2026-09-26).
    let dismiss: String
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
