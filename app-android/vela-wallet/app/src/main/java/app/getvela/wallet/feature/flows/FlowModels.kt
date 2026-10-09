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

    /**
     * The correctness batch's boards, drawn through the live builders and
     * never on the live flow stack: the confirm held while the account's
     * previous transaction on this network is in flight, with its one line
     * (SD3D); and a refused send told by its reason — another of the
     * account's transactions went first (SD4D).
     */
    SD3D, SD4D,
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
    data class Identicon(val seed: String) : FactLead
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
    /**
     * One calm sentence under the row saying why its value is what it is — the
     * confirm's speed row uses it for a speed taken because it was free (issue
     * 686), so the tier and its reason reach the last screen together.
     */
    val note: String? = null,
    /** Spec 093: the value is the risk to see (an unlimited spending cap) — the danger tone. */
    val danger: Boolean = false,
    /** Spec 093: further value lines under [value] (a dApp's balance changes, one per coin). */
    val lines: List<String> = emptyList(),
    /**
     * Spec 097 F: a quiet line under the value — whose word a payee's NAME is
     * and the short address it stands for. On the page that signs a name is a
     * claim and the address is what is paid, so both are drawn, on two lines:
     * a long name may be cut, the address under it is not. Issue #423: the
     * lead sits beside the name (the From row's line), and this line is in
     * the body face, not mono.
     */
    val detail: String? = null,
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
    /**
     * Spec 090: what the code encodes — the core's `qr_value`. Blank (the
     * gallery) encodes the address the card spells out.
     */
    val code: String = "",
    /** Spec 090: the "include network" switch, as the core offers it; `null` where it offers none. */
    val network: NetworkSwitchModel? = null,
)

/** The receive code's "include network" switch (spec 090): its position, and the calm hint while it is on. */
@Immutable
data class NetworkSwitchModel(val label: String, val isOn: Boolean, val hint: String? = null)

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
    /** Spec 048: what the code encodes — the address, at level H; blank draws the gallery's placeholder pattern. */
    val code: String = "",
    /**
     * The network's logo from the chain-data endpoint, drawn on a plate in the
     * code's centre; the capture fetches it before it draws. `null`, or a logo
     * that does not arrive in time, draws [networkMark]'s lettered disc.
     */
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
    /** `null` for a signature (spec 093): nothing settles it — [note] says so instead. */
    val status: StatusChipModel?,
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
    /**
     * Spec 082 RJ18: the record is still pending — 删除记录 is a quiet
     * secondary control, not the full-width danger button: the "don't send
     * it again" trace should stay in view while the op may still land.
     */
    val deleteQuiet: Boolean = false,
    /**
     * Spec 082 RJ16: whether the explorer control is drawn — only with a
     * transaction hash to open (an op hash is never an explorer link).
     */
    val explorerShown: Boolean = true,
    /** Spec 093: "Off-chain signature — nothing was sent on-chain", where a chip would be. */
    val note: String? = null,
    /** Spec 093: the figure is an unlimited allowance — the danger tone. */
    val amountDanger: Boolean = false,
    /** 083 F1: a swap's one coin back, under the figure ("≈ +0.03 ETH"). */
    val received: String? = null,
    /** Spec 093: a dApp record's collapsed "Technical details"; `null` for everything else. */
    val technical: TxTechnicalModel? = null,
)

/** Spec 093: the collapsed "Technical details" of a dApp record, in the core's order. */
@Immutable
data class TxTechnicalModel(
    val title: String,
    val lines: List<TxTechnicalLine>,
)

@Immutable
sealed interface TxTechnicalLine {
    data class Fact(val fact: FactRowModel) : TxTechnicalLine

    /**
     * The request the record kept — read from the store by [recordId] only
     * when the section is opened, shown as the core displays a request of
     * this [content] (`call_data`, `typed_data`, `message`: the core's word);
     * [missing] when it kept none.
     */
    data class Content(val label: String, val recordId: String, val missing: String, val content: String) : TxTechnicalLine
}

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

/**
 * Spec 098 §4: where the relay's gas goes, under its treasury stop — the
 * address in full and the button that copies it. Until 098 the phone said
 * "fund it" and never said where.
 */
@Immutable
data class FundAddressModel(val label: String, val address: String, val copy: String, val copied: String)

/** SD1 / SD1b — pick the token, or several of them. */
@Immutable
data class SendPickModel(
    val header: FlowHeaderModel,
    /**
     * Issue #332: whom the money is for, when the core already holds a
     * recipient — a code scanned from the home, a contact handed over. The
     * picker is where the person chooses WHAT to send; without this line a
     * scan that worked looked exactly like one that had done nothing.
     */
    val recipient: FactRowModel? = null,
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
    /**
     * Issue #326: present when tapping the card goes back to the asset picker
     * (the core's `can_change_token`), carrying the control's accessible name.
     */
    val change: String? = null,
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
    /** The core says the typed address is not one (`split_row_issues`). */
    val addressNote: String? = null,
    /** "Same address as recipient 2" — the core's repeat flag (issue 203). */
    val duplicateNote: String? = null,
    /** The core says the typed amount cannot be sent. */
    val amountNote: String? = null,
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
    /**
     * The refresh control's accessible name (spec 068; Android's since 069) —
     * `null` draws none. The fee is the one figure on the form that moves on
     * its own, and a person could neither re-read it nor be told it went old.
     */
    val refreshLabel: String? = null,
    /** A measurement is out, whoever started it: the control says so. */
    val refreshing: Boolean = false,
    /** The quote's 30 s TTL ran out — calm, never a fault. Its line is kept either way. */
    val staleNote: String? = null,
    /**
     * PR 2 note 1: why the fee failed, in the core's words — drawn in the
     * line [staleNote] keeps, so nothing moves when it comes and goes, and
     * kept while the core asks again by itself.
     */
    val reason: String? = null,
)

/** One option of the speed control (spec 068). */
@Immutable
data class FeeSpeedOptionModel(
    /** The wire tier — `fast` / `standard` / `slow`. */
    val id: String,
    /**
     * The SPEED — 超快 / 标准 / 较慢 — never a number. What it buys is not said
     * per payment (its price and bid are); Settings' default speed says it.
     */
    val label: String,
    /** This option's OWN fee, or the "…" / "—" standing in for it. */
    val value: String,
    /** Its gas bid as a range, already formatted by the core over the set. */
    val gasPrice: String? = null,
    val selected: Boolean = false,
)

/**
 * The speed control under the fee row, folded until opened (spec 068). Every
 * decision in it is the `fee_speed` core's (spec 069); this is only words.
 */
@Immutable
data class FeeSpeedModel(
    val label: String,
    /** The folded summary: the tier in force for THIS send. */
    val value: String,
    val open: Boolean,
    val onceNote: String,
    /** Why the tier in force is the fastest when the default is slower. */
    val freeNote: String? = null,
    /** This network has one speed: the options give way to it. */
    val singleNote: String? = null,
    val gasPriceLabel: String,
    /** Whether the options carry a gas-bid line at all. */
    val gasPriceLine: Boolean,
    val options: List<FeeSpeedOptionModel>,
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
    val pickLabel: String,
    /** Sweep shows a scan button beside the picker; single does not. */
    val scanLabel: String? = null,
    /** Sweep's "every token goes to the same address". */
    val note: String? = null,
    /** The note is a warning (spec 096 F12: a token's own contract). */
    val noteWarning: Boolean = false,
    /** Spec 043: the live address as typed; `null` = a drawn, read-only field. */
    val raw: String? = null,
)

enum class RecipientAction { Add, Contacts, Import }

@Immutable
data class RecipientActionModel(val id: RecipientAction, val label: String)

@Immutable
data class SummaryLineModel(
    val label: String,
    val value: String,
    /** The core's `split_over_balance`: the figure takes the refusal colour while the rows are typed. */
    val over: Boolean = false,
    /** "2.25 ETH left" (`split_remaining`), under the label. */
    val remaining: String? = null,
)

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
    /** The speed control (spec 068). `null` draws none. */
    val speed: FeeSpeedModel? = null,
    val cta: String,
    /** Spec 043: the core's `can_continue`; a drawn form is always enabled. */
    val ctaEnabled: Boolean = true,
    /** Spec 043 phase 5: the core's amount warning or same-asset fee ceiling, as a sentence. */
    val warning: String? = null,
    /** Spec 098 §4: the treasury stop's address, drawn under [warning]. */
    val fund: FundAddressModel? = null,
    /**
     * Issue #466: a relay stop's "Report this" — present only while the core
     * has a report for it (`SendView.relay_report`, a network Vela ships).
     */
    val report: String? = null,
    /** A split's dark Continue, explained: which recipient still needs what. */
    val hint: String? = null,
    /** Split only: "Use 0.5 ETH for the empty rows" — one typed figure into every row that has none. */
    val fillEmpty: FillEmptyModel? = null,
)

/** The web's `model.fillEmpty`: the words, and the figure exactly as it was typed. */
@Immutable
data class FillEmptyModel(val label: String, val amount: String)

/** SD2e — the contact picker. */
@Immutable
data class ContactGroupModel(val name: String, val count: String, val colors: Pair<Color, Color>)

@Immutable
data class ContactEntryModel(
    val name: String,
    val group: String? = null,
    val addressDisplay: String,
    val identiconSeed: String,
    /**
     * Issue #467: which person this row IS — the full address the pick sends.
     * A tap used to send the row's POSITION, looked up again in the book at
     * tap time; the core re-sorts the book (favourites, recency, display
     * names that change as ENS and registry names resolve), so a reorder
     * between drawing and tapping paid the neighbour.
     */
    val address: String = identiconSeed,
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
data class BatchRowModel(
    val ok: Boolean,
    val address: String,
    val conversion: String,
    /** Why this line is not sent — a duplicate, a bad address, a refused line's reason. */
    val note: String? = null,
)

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
    /** Live only: the file could not be read (`file_error`), said while nothing parsed. */
    val fileError: String? = null,
    /** Live only: "Total · N recipients", the sum, and the balance (or what is left) it is read against. */
    val total: SummaryLineModel? = null,
)

/** SD3 — the confirmation. */
@Immutable
data class BreakdownRowModel(
    val lead: TokenMarkModel? = null,
    val identiconSeed: String? = null,
    val label: String,
    val value: String,
    /** The label in the mono face: a payee with no name, drawn as their short address. */
    val mono: Boolean = false,
    /** Spec 097 F: the short address under a named payee, muted and mono. */
    val detail: String? = null,
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
    /** "120 USDT" / "3 assets" — or just "120" when [amountUnit] draws the unit. */
    val amount: String,
    /**
     * Spec 078 round 2: the unit drawn as its own piece beside the figure —
     * smaller, quieter, on one baseline — the way the Send form's hero draws
     * it. `null` = [amount] is one phrase (a split's total, a sweep's count).
     */
    val amountUnit: String? = null,
    /** "≈ $120.00" / "Total ≈ $200.90 · Ethereum". */
    val subline: String,
    val facts: List<FactRowModel>,
    val breakdown: List<BreakdownRowModel> = emptyList(),
    /**
     * "First time sending here" — the anti-poisoning tell, on the page that
     * signs. The core resolves it only while this page is up (`confirm_probes`),
     * so the form never had it to show.
     */
    val recipientTag: String? = null,
    val cta: String,
    /** Spec 043: the core's `can_confirm`; a drawn confirm is always enabled. */
    val ctaEnabled: Boolean = true,
    /**
     * The one line under a confirm held while the account's previous
     * transaction on this network is in flight (`SendView.previous_pending`):
     * "Waiting for your last transaction on this network…". `null` otherwise.
     */
    val ctaHold: String? = null,
    /** Spec 043 phase 5: the core's refusal on this page (treasury low, submit failed) and the action it offers. */
    val notice: String? = null,
    /** Spec 098 §4: the treasury stop's address, drawn under [notice]. */
    val noticeFund: FundAddressModel? = null,
    /** Issue #466: the stop's "Report this", as on the form ([SendFormModel.report]). */
    val noticeReport: String? = null,
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
    /** The lines under the title — with a split's people, or a sweep's coins (spec 097 F), listed one per line. */
    val captions: List<String>,
    val hash: ReceiptHashModel? = null,
    val viewOnExplorer: String? = null,
    /** The single bottom button: "Close · keep running" or "Done". */
    val cta: String,
    val ctaAccent: Boolean,
    /** Submitted only, and only where the chain has a typical time: the screen's clock runs off this. */
    val eta: ReceiptEtaModel? = null,
    /** Spec 096 F8: "Try again" beside the button — a dApp request that failed before anything was sent. */
    val retry: String? = null,
)

/**
 * The submitted receipt's wait (issue 199, the web's `SendReceiptModel.eta`):
 * the sentences arrive resolved; only the clock is the screen's. What the
 * count says at a given moment — remaining, elapsed, slow — and how full the
 * ring is are the core's (`landing_pace`, spec 099 R6), counted from when the
 * relay put the operation on the network, never from acceptance.
 */
@Immutable
data class ReceiptEtaModel(
    /** When the relay sent it (`TrackEntryView.relay_sent_at_ms`). */
    val sentAtMs: Double,
    val typicalS: Int,
    /** "Gnosis typically confirms in ~15s" — already filled. */
    val typicalLine: String,
    /** "~{{remaining}}s remaining" — inside the typical time. */
    val remainingTemplate: String,
    /** "{{elapsed}}s elapsed — almost there" — past it, where "almost" is true. */
    val elapsedTemplate: String,
    /** Past twice the typical time. */
    val slowLine: String,
) {
    /** The core's pace at the screen's [nowMs]. */
    fun pace(nowMs: Long): app.getvela.wallet.feature.send.core.LandingPace =
        app.getvela.wallet.feature.send.core.Landing.pace(sentAtMs, typicalS, nowMs.toDouble())

    /** The two lines under the title for [pace]: the chain's usual time, then the core's count. */
    fun lines(pace: app.getvela.wallet.feature.send.core.LandingPace): List<String> = when (pace.line) {
        app.getvela.wallet.feature.send.core.LandingLine.Remaining ->
            listOf(typicalLine, remainingTemplate.replace("{{remaining}}", pace.seconds.toString()))
        app.getvela.wallet.feature.send.core.LandingLine.Elapsed ->
            listOf(typicalLine, elapsedTemplate.replace("{{elapsed}}", pace.seconds.toString()))
        app.getvela.wallet.feature.send.core.LandingLine.Slow -> listOf(typicalLine, slowLine)
        // Nothing sent yet, or no usual time: the receipt never builds a clock for these.
        app.getvela.wallet.feature.send.core.LandingLine.Waiting, app.getvela.wallet.feature.send.core.LandingLine.None -> emptyList()
    }
}

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
