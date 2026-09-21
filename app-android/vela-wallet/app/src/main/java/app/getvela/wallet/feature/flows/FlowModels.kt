package app.getvela.wallet.feature.flows

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.feature.wallet.ActivityGroupModel
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.AssetRowModel

/**
 * Wallet-flow view models (spec 021 — the Android port of the web's
 * `src/lib/flows/model.ts`).
 *
 * Components consume ONLY these display-ready shapes: no service types, no
 * formatting, no fetching. Every string arrives resolved and every number
 * arrives as text, so the later "real data" feature replaces the fixture layer
 * and nothing else.
 *
 * Shapes spec 015 already defined (`ActivityRowModel`, `AssetRowModel`) are
 * imported rather than restated — the send token picker and the assets list
 * render the SAME row as the wallet home, and a parallel type would be the
 * first step towards a parallel component.
 */

/** Mobile gallery ids — spec.md's state matrix, stable across all four clients. */
enum class FlowState {
    R1, R2, R2X, R3, R4,
    S1,
    A1, A2, A3,
    T1, T2, T3, T3B, T4, T5, T5B,
    SD1, SD1B, SD2, SD2B, SD2C, SD2D, SD2E, SD2F,
    SD3, SD3B, SD3C,
    SD4A, SD4B, SD4C,
}

/* ------------------------------------------------------------------ chrome */

/** A screen's own top bar: back, title, and at most one trailing text action. */
@Immutable
data class FlowHeaderModel(
    val title: String,
    val backLabel: String,
    /** e.g. T1's 添加. Null on screens whose header carries no action. */
    val action: String? = null,
    /** The network pill, where the screen filters by chain (A1, T1, SD1). */
    val pill: FlowPillModel? = null,
)

@Immutable
data class FlowPillModel(val dots: List<Color>, val label: String)

/* ------------------------------------------------------------------ shared */

/** A token's circular mark: three-letter glyph plus its chain colour. */
@Immutable
data class TokenMarkModel(
    val ticker: String,
    val badgeColor: Color,
    /** Spec 047: the web's `tokenMarkFor` — logo candidates in order, the chain badge's logo, and whether the badge is hidden because it would repeat the token. */
    val logoUrls: List<String> = emptyList(),
    val badgeLogoUrl: String? = null,
    val badgeHidden: Boolean = false,
)

/** Leading art on a fact row's value side. */
@Immutable
sealed interface FactLead {
    data class Dot(val color: Color) : FactLead
    data class Token(val mark: TokenMarkModel) : FactLead
    data class Identicon(val seed: String, /** Spec 049: the name beside it — the initials style's letter. */ val name: String? = null) : FactLead
}

/**
 * A label/value row. The single label-value primitive for the whole feature —
 * A2's transaction facts, SD3's summary, T2's token facts and T3b's chain facts
 * are the same row with different leading art.
 */
@Immutable
data class FactRowModel(
    val label: String,
    val value: String,
    val lead: FactLead? = null,
    /** Renders the value in the mono face (addresses, hashes). */
    val mono: Boolean = false,
    /** Shows a copy affordance under this accessible name. */
    val copy: String? = null,
    /** Spec 048: what the copy affordance puts on the clipboard when `value` is a shortened form. */
    val copyValue: String? = null,
)

enum class StatusTone { Success, Warning, Error, Info }

@Immutable
data class StatusChipModel(val text: String, val tone: StatusTone)

/* ----------------------------------------------------------------- receive */

/** One row of R1: a network, the address on it, and the two things you do. */
@Immutable
data class NetworkRowModel(
    val name: String,
    val code: String,
    val badgeColor: Color,
    val addressDisplay: String,
    val copyLabel: String,
    val qrLabel: String,
    /** Spec 047: the network's own logo from the chain-data endpoint; the drawn code stays the fallback. */
    val logoUrl: String? = null,
)

@Immutable
data class ReceiveListModel(
    val header: FlowHeaderModel,
    /** "One address across all 8 networks". */
    val subtitle: String,
    val searchPlaceholder: String,
    /** Shown in place of the rows when the search matches nothing. */
    val emptyText: String,
    val rows: List<NetworkRowModel>,
    /** Spec 048: the full address every row's copy puts on the clipboard. */
    val address: String = "",
)

/** The account card that sits above every QR: whose address this is. */
@Immutable
data class AddressCardModel(
    val name: String,
    val identiconSeed: String,
    /** The full address, pre-split into the two lines the mocks wrap it into. */
    val lines: Pair<String, String>,
    val copyLabel: String,
)

@Immutable
data class ContractLineModel(val label: String, val value: String, val copyLabel: String, val copyValue: String? = null)

@Immutable
data class ReceiveQrModel(
    val title: String,
    val closeLabel: String,
    /** R3 only: the token's contract, above the account card. */
    val contract: ContractLineModel? = null,
    val account: AddressCardModel,
    /** The mark drawn in the middle of the code — the token, or the network. */
    val centre: TokenMarkModel,
    val warning: String,
    val saveImage: String,
    val viewOnExplorer: String,
    /** Spec 048: where 在区块浏览器中查看 goes; `null` when the chain has no explorer. */
    val explorerUrl: String? = null,
)

/** R4 — the image "Save image" produces, not a screen someone navigates to. */
@Immutable
data class ShareCardModel(
    val headline: String,
    val name: String,
    val lines: Pair<String, String>,
    val networkNote: String,
    val networkMark: TokenMarkModel,
    val identiconSeed: String,
    val wordmark: String,
    /** Spec 048: what the code encodes — the address; blank draws the gallery's placeholder pattern. */
    val code: String = "",
    /** Spec 048: the network pill's logo from the chain-data endpoint; the lettered disc is the fallback. */
    val chainLogoUrl: String? = null,
)

/* -------------------------------------------------------------------- scan */

enum class ScanTool { Gallery, Torch, Flip }

@Immutable
data class ScanToolModel(val id: ScanTool, val label: String)

@Immutable
data class ScanModel(
    val title: String,
    val hint: String,
    val closeLabel: String,
    val tools: List<ScanToolModel>,
)

/* ---------------------------------------------------------------- activity */

enum class HistoryMode { Rows, Empty, Loading }

@Immutable
data class HistoryModel(
    val header: FlowHeaderModel,
    val mode: HistoryMode,
    val emptyText: String,
    val groups: List<ActivityGroupModel>,
)

/** A2 / A3 — one transaction, opened from a history row. */
@Immutable
data class TxDetailModel(
    val title: String,
    val status: StatusChipModel,
    val closeLabel: String,
    val amount: String,
    val fiat: String,
    val positive: Boolean,
    val facts: List<FactRowModel>,
    val viewOnExplorer: String,
    /** Spec 048: where 在区块浏览器中查看 goes; `null` when the chain has no explorer. */
    val explorerUrl: String? = null,
    /**
     * 删除记录 — the LOCAL record, not the transaction (spec 058). `null` where
     * there is nothing to delete. The web has drawn this button since 028 and
     * never set its label, and `deleteActivity` on this client had no caller,
     * so it has been unreachable everywhere.
     */
    val deleteLabel: String? = null,
)

/* ------------------------------------------------------------------ assets */

@Immutable
data class AssetsEmptyModel(
    val title: String,
    val caption: String,
    val cta: String,
    val hintTitle: String,
    val hintBody: String,
)

@Immutable
data class AssetsModel(
    val header: FlowHeaderModel,
    val searchPlaceholder: String,
    val rows: List<AssetRowModel>,
    /** T1's trailing link under the list. */
    val addByAddress: String,
    /** T4: the guided-empty body replaces the rows entirely. */
    val empty: AssetsEmptyModel? = null,
)

/** T2 — one token, opened from an assets row. */
@Immutable
data class TokenDetailModel(
    val mark: TokenMarkModel,
    val symbol: String,
    val chain: String,
    val closeLabel: String,
    val balance: String,
    val fiat: String,
    val receive: String,
    val send: String,
    val facts: List<FactRowModel>,
    val transactionsTitle: String,
    val rows: List<ActivityRowModel>,
    val viewOnExplorer: String,
    /** Spec 048: where 在区块浏览器中查看 goes; `null` when the chain has no explorer. */
    val explorerUrl: String? = null,
)

/* -------------------------------------------------------------- add token  */

enum class AddTokenTab { Erc20, Native }

/** The result card under the input: what the address or query resolved to. */
@Immutable
sealed interface AddTokenResult {
    data object None : AddTokenResult
    data class Searching(val text: String) : AddTokenResult
    data class NotFound(val text: String) : AddTokenResult
    data class Token(
        val mark: TokenMarkModel,
        val name: String,
        val detail: String,
        val chip: StatusChipModel? = null,
    ) : AddTokenResult

    data class Network(
        val mark: TokenMarkModel,
        val name: String,
        val chip: StatusChipModel,
        val facts: List<FactRowModel>,
        /** T5b's "deploy the missing contracts" line, under an incompatible chip. */
        val link: String? = null,
    ) : AddTokenResult
}

@Immutable
data class AddTokenNetworkModel(
    val mark: TokenMarkModel,
    val name: String,
    val pickLabel: String,
)

@Immutable
data class AddTokenModel(
    val title: String,
    val closeLabel: String,
    val tab: AddTokenTab,
    val tabErc20: String,
    val tabNative: String,
    /** ERC-20 only: the network the contract is looked up on. */
    val network: AddTokenNetworkModel? = null,
    val fieldLabel: String,
    val fieldValue: String,
    val fieldPlaceholder: String,
    /** Draws the field in its error state and prints this under it. */
    val fieldError: String? = null,
    val result: AddTokenResult,
    val cta: String,
    val ctaDisabled: Boolean,
)

/* -------------------------------------------------------------------- send */

@Immutable
data class FilterChipModel(val id: String, val label: String, val selected: Boolean)

@Immutable
data class SendNoticeModel(val mark: TokenMarkModel, val text: String)

@Immutable
data class SendSelectionModel(
    val selected: List<Boolean>,
    val dimmed: List<Boolean>,
    val selectAll: String,
)

@Immutable
data class SendCtaModel(val label: String, val accent: Boolean)

/** SD1 / SD1b — pick the token, or several of them. */
@Immutable
data class SendPickModel(
    val header: FlowHeaderModel,
    val searchPlaceholder: String,
    val filters: List<FilterChipModel>,
    /** SD1b: the chain lock, once the first token pins the network. */
    val notice: SendNoticeModel? = null,
    val rows: List<AssetRowModel>,
    val selection: SendSelectionModel? = null,
    val cta: SendCtaModel,
    /** Issue 209: what an empty list says — nothing held, or nothing matching. */
    val empty: String? = null,
)

/** The token card at the top of the send form. */
@Immutable
data class SendTokenCardModel(
    val mark: TokenMarkModel,
    val symbol: String,
    /** "Ethereum · Balance 53.4836". */
    val detail: String,
    val max: String? = null,
)

/** SD2b's split row: who, how much, and a way to drop them. */
@Immutable
data class RecipientCardModel(
    val ordinal: String,
    val name: String,
    val identiconSeed: String,
    val amount: String,
    val removeLabel: String,
    /** The core's row id, the address and the bare amount — set on a LIVE row, which is editable in place (spec 045). */
    val id: String = "",
    val address: String = "",
    val amountValue: String? = null,
    val addressPlaceholder: String = "",
)

/** SD2d's sweep row: one token, its amount, and a Max. */
@Immutable
data class SweepRowModel(
    val mark: TokenMarkModel,
    val symbol: String,
    val balanceLabel: String,
    val amount: String,
    val max: String,
)

@Immutable
data class FeeRowModel(
    val label: String,
    val mark: TokenMarkModel,
    val value: String,
    val openLabel: String,
)

@Immutable
data class AmountFieldModel(
    val value: String,
    val fiat: String,
    val denomLabel: String,
    /** Spec 043: the live figure as typed; `null` = a drawn, read-only field. */
    val raw: String? = null,
    /** Issue 231: the figure's unit, drawn on it — "$" before, or "BNB" / "PLN" after. */
    val unitPrefix: String? = null,
    val unitSuffix: String? = null,
    /** Issue 197: the ⇄ row exists only where the core offers it, and is live only where it would change something. */
    val denomShown: Boolean = true,
    val denomEnabled: Boolean = true,
)

@Immutable
data class RecipientFieldModel(
    val label: String,
    val lines: Pair<String, String>,
    val identiconSeed: String,
    /** Spec 049: the recipient's name when known — the initials disc's letter. */
    val name: String? = null,
    val pickLabel: String,
    /** Sweep shows a scan button beside the picker; single does not. */
    val scanLabel: String? = null,
    /** Sweep's "every token goes to the same address". */
    val note: String? = null,
    /** Spec 043: the live address as typed; `null` = a drawn, read-only field. */
    val raw: String? = null,
)

enum class RecipientAction { Add, Contacts, Import }

@Immutable
data class RecipientActionModel(val id: RecipientAction, val label: String)

@Immutable
data class SummaryLineModel(val label: String, val value: String)

enum class SendFormMode { Single, Split, Sweep }

@Immutable
data class SendFormModel(
    val header: FlowHeaderModel,
    val mode: SendFormMode,
    val token: SendTokenCardModel? = null,
    /** Sweep only: "3 tokens · Ethereum" plus the per-token rows. */
    val sweepSummary: String? = null,
    val sweepRows: List<SweepRowModel> = emptyList(),
    val amount: AmountFieldModel? = null,
    val recipient: RecipientFieldModel? = null,
    /** Single: the "+ add recipient" that turns this into a split. */
    val addRecipient: String? = null,
    val recipients: List<RecipientCardModel> = emptyList(),
    val recipientActions: List<RecipientActionModel> = emptyList(),
    val summary: SummaryLineModel? = null,
    val fee: FeeRowModel,
    val cta: String,
    /** Spec 043: the core's `can_continue`; a drawn form is always enabled. */
    val ctaEnabled: Boolean = true,
    /** Spec 043 phase 5: the core's amount warning or same-asset fee ceiling, as a sentence. */
    val warning: String? = null,
)

/** SD2e — the contact picker. */
@Immutable
data class ContactGroupModel(val name: String, val count: String, val colors: Pair<Color, Color>)

@Immutable
data class ContactEntryModel(
    val name: String,
    val group: String? = null,
    val addressDisplay: String,
    val identiconSeed: String,
)

@Immutable
data class ContactPickModel(
    val title: String,
    val closeLabel: String,
    val searchPlaceholder: String,
    val scanRow: String,
    val groupsTitle: String,
    val groups: List<ContactGroupModel>,
    val contactsTitle: String,
    val contacts: List<ContactEntryModel>,
)

/** SD2f — the fee-token picker. */
@Immutable
data class FeeTokenRowModel(
    val mark: TokenMarkModel,
    val symbol: String,
    val balanceLabel: String,
    val fee: String,
    val selected: Boolean,
    /** Issue 211: the core's verdict that this coin cannot pay — drawn dimmed, answers to nothing. */
    val insufficient: Boolean = false,
    /** What the row says instead of its balance when [insufficient]. */
    val insufficientNote: String? = null,
)

@Immutable
data class FeeTokenPickModel(
    val title: String,
    val closeLabel: String,
    val hint: String,
    val estimateLabel: String,
    val rows: List<FeeTokenRowModel>,
)

/** SD2c — the recipient importer. */
enum class BatchUnit { Fiat, Token }

@Immutable
data class BatchRowModel(val ok: Boolean, val address: String, val conversion: String)

@Immutable
data class BatchImportModel(
    val title: String,
    val closeLabel: String,
    val unitFiat: String,
    val unitToken: String,
    val unit: BatchUnit,
    val pasteValue: String,
    val pastePlaceholder: String,
    val importFile: String,
    val template: String,
    val rateSection: String,
    val rateLabel: String,
    val rateValue: String,
    val rateHint: String,
    val parsedLabel: String,
    val rows: List<BatchRowModel>,
    val rejectedText: String? = null,
    val cta: String,
    val ctaDisabled: Boolean,
    /** Live only (spec 045): the editable rate, whether it was edited, the "auto" reset word, and one note (cap, balance, template). */
    val rateInput: String? = null,
    val rateEdited: Boolean = false,
    val rateReset: String? = null,
    val note: String? = null,
    /**
     * The note is a refusal (over the balance, over the cap) — the reason the
     * button is dim — and is drawn as one, not as helper text (issue #272).
     */
    val noteWarning: Boolean = false,
    /** Issue #271: what applying does to the rows already on the form, and the way to choose the other. */
    val merge: String? = null,
    val mergeAction: String? = null,
)

/** SD3 — the confirmation. */
@Immutable
data class BreakdownRowModel(
    val lead: TokenMarkModel? = null,
    val identiconSeed: String? = null,
    val label: String,
    val value: String,
)

@Immutable
data class SendConfirmModel(
    val header: FlowHeaderModel,
    /**
     * The coin being sent, drawn above the figure (founder, 2026-09-17).
     * The confirm page named the asset in words only while every row beneath
     * it carried art — the one screen where "which coin is this?" must be
     * answerable at a glance. Absent on a sweep: several coins, no one mark.
     */
    val mark: TokenMarkModel? = null,
    /** "120 USDT" / "3 assets". */
    val amount: String,
    /** "≈ $120.00" / "Total ≈ $200.90 · Ethereum". */
    val subline: String,
    val facts: List<FactRowModel>,
    val breakdown: List<BreakdownRowModel> = emptyList(),
    val cta: String,
    /** Spec 043: the core's `can_confirm`; a drawn confirm is always enabled. */
    val ctaEnabled: Boolean = true,
    /** Spec 043 phase 5: the core's refusal on this page (treasury low, submit failed) and the action it offers. */
    val notice: String? = null,
    val noticeAction: String? = null,
    /** Spec 045 US4: the notice's second exit — "not now" beside the treasury retry, the facts kept. */
    val noticeSecondary: String? = null,
)

enum class ReceiptStage { Submitting, Submitted, Confirmed, Failed }

@Immutable
data class ReceiptHashModel(val label: String, val value: String, val copyLabel: String, val copyValue: String? = null)

/** SD4 — the receipt, in whichever of its states the transaction is in. */
@Immutable
data class SendReceiptModel(
    val header: FlowHeaderModel,
    val stage: ReceiptStage,
    val title: String,
    /** Up to two lines under the title. */
    val captions: List<String>,
    val hash: ReceiptHashModel? = null,
    val viewOnExplorer: String? = null,
    /** The single bottom button: "Close · keep running" or "Done". */
    val cta: String,
    val ctaAccent: Boolean,
)

/* ------------------------------------------------------------- the screens */

/** The screen under a state. */
@Immutable
sealed interface FlowBase {
    data class Receive(val model: ReceiveListModel) : FlowBase
    data class Share(val model: ShareCardModel) : FlowBase
    data class Scan(val model: ScanModel) : FlowBase
    data class History(val model: HistoryModel) : FlowBase
    data class Assets(val model: AssetsModel) : FlowBase
    data class SendPick(val model: SendPickModel) : FlowBase
    data class SendForm(val model: SendFormModel) : FlowBase
    data class SendConfirm(val model: SendConfirmModel) : FlowBase
    data class SendReceipt(val model: SendReceiptModel) : FlowBase
}

/** The sheet over it, where the state has one. */
@Immutable
sealed interface FlowSheet {
    data class ReceiveQr(val model: ReceiveQrModel) : FlowSheet
    data class TxDetail(val model: TxDetailModel) : FlowSheet
    data class TokenDetail(val model: TokenDetailModel) : FlowSheet
    data class AddToken(val model: AddTokenModel) : FlowSheet
    data class ContactPick(val model: ContactPickModel) : FlowSheet
    data class FeeToken(val model: FeeTokenPickModel) : FlowSheet
    data class BatchImport(val model: BatchImportModel) : FlowSheet
}

/**
 * One state: the screen, and the sheet over it.
 *
 * Sheets are an overlay on a base screen rather than states of their own
 * because that is what they are — A2 is the history with a transaction over it,
 * and the history behind it is still the history.
 */
@Immutable
data class FlowScreenModel(
    val state: FlowState,
    val base: FlowBase,
    val sheet: FlowSheet? = null,
    /** 1f or 1.35f — applied through LocalDensity, as spec 015's H7x is. */
    val textScale: Float = 1f,
)
