package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `activity_feed` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/activity_feed.rs`.
 *
 * Numeric types come from the RUST, never from the generated TypeScript, which
 * writes `u32`, `u64` and `f64` all as `number` — the blind spot 040 shipped a
 * bug through. Here: `chain_id`/`decimals`/`count`/`read_id`/`generation` are
 * `u32` → `Int`; `timestamp`, `day_start_ms`, `usd_value`, `now_ms` are `f64`
 * → `Double`.
 *
 * **`timestamp` is epoch SECONDS. `day_start_ms` is epoch MILLISECONDS.** They
 * sit next to each other in the same struct and differ by a factor of a
 * thousand; mixing them puts every row in 1970.
 */

// -- value types -------------------------------------------------------------

@Serializable
enum class FeedTxKind {
    @SerialName("send") Send,

    @SerialName("receive") Receive,

    @SerialName("dapp_tx") DappTx,

    @SerialName("sign_message") SignMessage,

    @SerialName("sign_typed_data") SignTypedData,

    @SerialName("connect") Connect,
}

@Serializable
enum class FeedTxStatus {
    @SerialName("pending") Pending,

    @SerialName("confirmed") Confirmed,

    @SerialName("failed") Failed,

    /**
     * 087 F04: a pending record nothing will ever settle (no operation hash
     * past the core's grace, or past the tracker's 24 h). Only a row says it.
     */
    @SerialName("unknown") Unknown,
}

@Serializable
enum class FeedDirection {
    @SerialName("in") In,

    @SerialName("out") Out,
}

/** `split` = one token → N recipients; `multi_select` = N tokens → 1 recipient. */
@Serializable
enum class FeedBatchKind {
    @SerialName("split") Split,

    @SerialName("multi_select") MultiSelect,
}

/**
 * One stored transaction, as this device persists it.
 *
 * `day_start_ms` is the shell's job and only the shell's: it is local midnight
 * for `timestamp` on THIS device, in THIS timezone, which is the one fact the
 * core cannot compute. Getting it wrong groups a payment under the wrong day.
 */
@Serializable
data class FeedTxRecord(
    val id: String,
    val user_op_hash: String = "",
    val tx_hash: String = "",
    val from: String,
    val to: String,
    val to_name: String? = null,
    val value: String,
    val symbol: String,
    val decimals: Int,
    val logo_urls: List<String>? = null,
    val chain_id: Int,
    /** Epoch **seconds**. */
    val timestamp: Double,
    /** Local-midnight epoch **milliseconds** for [timestamp]. */
    val day_start_ms: Double,
    val status: FeedTxStatus,
    /** `null` = a legacy untyped record, which the core reads as a send. */
    val kind: FeedTxKind? = null,
    val usd: String? = null,
    /**
     * Spec 082 RG1 / 083 H2: the origin a dApp's request arrived from, as
     * stored (`dappUrl`) — never `dappOrigin`, which may hold the dApp's own
     * name. The core names the row's site from it.
     */
    val dapp_url: String? = null,
    /**
     * Spec 082 RJ16: a dApp record's first call `data`, from the stored
     * request — what lets the core tell a token transfer's recipient from the
     * contract the call went to.
     */
    val call_data: String? = null,
    /** A dApp record's intent, recorded at approve time (`intent`). */
    val intent: String? = null,
    /** What the sheet's simulation said it moves, as stored (`balanceChanges`). */
    val balance_changes: List<TrustSimJudgment>? = null,
    /** Spec 093: the record's summary, stored verbatim (`dappSummary`) and handed back untouched. */
    val summary: DappSummary? = null,
    /** Spec 097: how its operation ended, as the tracker's patch stored it (`settlement`). */
    val settlement: app.getvela.wallet.feature.send.core.TrackSettlement? = null,
)

/** What a dApp request did (spec 093) — `dapp_activity::DappAction`. */
@Serializable
enum class DappAction {
    @SerialName("call") Call,

    @SerialName("batch") Batch,

    @SerialName("approve") Approve,

    @SerialName("permit") Permit,

    @SerialName("sign_in") SignIn,

    @SerialName("message") Message,

    @SerialName("typed_data") TypedData,

    @SerialName("blind_sign") BlindSign,
}

/**
 * The facts of one dApp request a record keeps (spec 093), as the core built
 * them at approve time. The shell stores them verbatim with the record
 * (`dappSummary`) and hands them back to the feed untouched; it reads none of
 * them. EVERY field of the Rust struct is declared here — a field left out
 * would be lost on the disk — and `CoreWireDriftTest` checks it both ways.
 */
@Serializable
data class DappSummary(
    val action: DappAction,
    val calls: Int = 0,
    val contract: String? = null,
    val spender: String? = null,
    val token: String? = null,
    val symbol: String? = null,
    val decimals: Int? = null,
    /** Raw base units, decimal string. */
    val amount: String? = null,
    val unlimited: Boolean = false,
    val revoke: Boolean = false,
    /** Epoch **seconds**. */
    val expires_at: Double? = null,
    val signin_domain: String? = null,
    val primary_type: String? = null,
    /** Spec 097 N8: what the sheet's reading called [contract], and its owner. */
    val contract_name: String? = null,
    val owner: String? = null,
    /** Spec 097 N5: the network fee the wallet added in a token — its contract and base units. */
    val fee_token: String? = null,
    val fee_amount: String? = null,
    /** Spec 097 N5: the coins the sheet's reading named. */
    val tokens: List<DappToken> = emptyList(),
)

/** A coin a reading named: its contract, and the symbol and decimals it answered (`DappToken`). */
@Serializable
data class DappToken(val address: String, val symbol: String, val decimals: Int)

/**
 * What a signing sheet's reading named (spec 097, `DappReading`): copied from
 * `ClearSigningView.record_reading` to `SignApproveOpts.reading`, verbatim.
 */
@Serializable
data class DappReading(
    val address: String? = null,
    val name: String? = null,
    val owner: String? = null,
    val tokens: List<DappToken> = emptyList(),
)

/** Who a row's `counterparty` is (spec 082 RJ16, G52): the person paid, or the contract a call went to. */
@Serializable
enum class FeedCounterpartyRole {
    @SerialName("recipient") Recipient,

    @SerialName("contract") Contract,
}

@Serializable
data class FeedBatchTransfer(
    val to: String,
    val to_name: String? = null,
    val value: String,
    val symbol: String,
    val decimals: Int,
    val usd_value: Double,
    val logo_urls: List<String>? = null,
)

@Serializable
data class FeedBatch(
    val kind: FeedBatchKind,
    val count: Int,
    val total_usd: Double,
    val transfers: List<FeedBatchTransfer> = emptyList(),
    val ids: List<String> = emptyList(),
    val from: String,
    val chain_id: Int,
    val timestamp: Double,
    val status: FeedTxStatus,
    val tx_hash: String = "",
    val user_op_hash: String = "",
    val symbol: String? = null,
    val logo_urls: List<String>? = null,
    val to: String? = null,
    val to_name: String? = null,
)

/** One feed row's payload. The shell formats; the core decides what is in it. */
@Serializable
data class FeedItem(
    val id: String,
    val direction: FeedDirection,
    val counterparty: String? = null,
    /** The resolved name, or the one stored at send time. */
    val alias: String? = null,
    /** `null` for a multi-token batch row: mixed tokens cannot be summed. */
    val value: String? = null,
    val symbol: String = "",
    val decimals: Int? = null,
    /** Numeric USD, `0` when unknown. */
    val usd_value: Double = 0.0,
    /** Spec 097 N7: [usd_value] is a price the core knows; `false` draws no fiat at all — unknown is not "$0.00". */
    val priced: Boolean = false,
    val chain_id: Int,
    /** Epoch **seconds**. */
    val timestamp: Double,
    /** Local-midnight epoch **milliseconds**. */
    val day_start_ms: Double,
    val tx_hash: String? = null,
    val batch: FeedBatch? = null,
    /**
     * Spec 082 RG1: what the row is — the core decides, the shell never
     * guesses from a hash. A folded batch is a send.
     */
    val kind: FeedTxKind = FeedTxKind.Send,
    /**
     * The record's status (a batch's first line's). Absent reads `Pending`,
     * as the core's own default: it claims nothing — only the tracker closes
     * a record, and a default must never say money landed or failed.
     */
    val status: FeedTxStatus = FeedTxStatus.Pending,
    /** A dApp row only: `host[:port]` of the asking site. */
    val site: String? = null,
    /**
     * Spec 082 RJ16: [counterparty] is the recipient (the default) or the
     * contract a call went to — labelled `componentsUi.signing.interactingLabel`.
     */
    val counterparty_role: FeedCounterpartyRole = FeedCounterpartyRole.Recipient,
    /**
     * Spec 093: a dApp interaction — a transaction or a signature. A row with
     * one is a dApp row. Fail-soft: a payload this build cannot read is
     * absent, never a broken feed.
     */
    @Serializable(with = FeedDappFailSoft::class)
    val dapp: FeedDapp? = null,
    /** Spec 093: the second line, in order — worded part by part, joined " · ". */
    @Serializable(with = FeedLineList::class)
    val subtitle: List<FeedLine> = emptyList(),
    /**
     * The row's own figure is money, so it draws as the mask while the
     * balance is hidden (`app::privacy::figure_maskable`): an amount, a
     * batch's count, a capped allowance. `false` for an unlimited allowance (a
     * risk to see) and for a row with no figure. A dApp row's `received`
     * masks whenever hidden, whatever this says.
     */
    val figure_maskable: Boolean = false,
)

/** One part of a row's second line (spec 093). */
@Serializable
sealed class FeedLine {
    /** Not confirmed: pending, failed or unknown — never on a signature. */
    @Serializable
    @SerialName("status")
    data class Status(val status: FeedTxStatus) : FeedLine()

    /** `history.toName` with [name], else the short [address]. */
    @Serializable
    @SerialName("to")
    data class To(val address: String, val name: String? = null) : FeedLine()

    /** `history.fromName` with [name], else the short [address]. */
    @Serializable
    @SerialName("from")
    data class From(val address: String, val name: String? = null) : FeedLine()

    /** A site's `host[:port]`, verbatim. */
    @Serializable
    @SerialName("site")
    data class Site(val site: String) : FeedLine()

    /** The network, by its id; the shell names it. */
    @Serializable
    @SerialName("network")
    data class Network(val chain_id: Int) : FeedLine()

    /**
     * The day, by its local-midnight key (epoch **milliseconds**) — worded as
     * the date headers word it. Only on a contact's rows, which have no headers.
     */
    @Serializable
    @SerialName("day")
    data class Day(val day_start_ms: Double) : FeedLine()
}

/** An allowance as Activity states it (spec 093). */
@Serializable
data class FeedAllowance(
    /** Empty when nobody could name the token. */
    val symbol: String = "",
    /** The cap as a human decimal; `null` when [unlimited]. */
    val value: String? = null,
    val decimals: Int? = null,
    /** `componentsUi.signingApprove.unlimitedValue`, in the danger tone. */
    val unlimited: Boolean = false,
    val token: String? = null,
)

/**
 * One line of what a dApp transaction moved (083 F1), from the sheet's own
 * simulation; an unverified token carries no figure.
 */
@Serializable
data class FeedDappChange(
    val direction: FeedDirection,
    val verified: Boolean = false,
    val symbol: String = "",
    /** Unsigned human decimal; `null` = no figure to show. */
    val value: String? = null,
    val decimals: Int? = null,
    /** The wallet vouches for it to the unit: no "≈". */
    val exact: Boolean = false,
)

/** What a dApp record's operation was, as the detail names it. */
@Serializable
sealed class FeedDappOperation {
    /** `componentsTx.detail.opContractInteraction`. */
    @Serializable
    @SerialName("contract_interaction")
    data object ContractInteraction : FeedDappOperation()

    /** `componentsUi.signing.batchSubtitle` {count}. */
    @Serializable
    @SerialName("batch")
    data class Batch(val calls: Int) : FeedDappOperation()

    /** `componentsTx.detail.opSignature`. */
    @Serializable
    @SerialName("signature")
    data object Signature : FeedDappOperation()

    /** `componentsTx.detail.opTypedDataSignature`. */
    @Serializable
    @SerialName("typed_data_signature")
    data object TypedDataSignature : FeedDappOperation()
}

/** What a stored request holds (`connect.detail.content*`). */
@Serializable
enum class FeedDappContent {
    @SerialName("call_data") CallData,

    @SerialName("typed_data") TypedData,

    @SerialName("message") Message,
}

/** One line of a dApp row's detail (spec 093): the core picks the lines and their order; the shell labels them. */
@Serializable
sealed class FeedFact {
    @Serializable
    @SerialName("site")
    data class Site(val site: String) : FeedFact()

    @Serializable
    @SerialName("network")
    data class Network(val chain_id: Int) : FeedFact()

    /** The contract a call went to (`tokenDetail.labelContract`), with its built-in name when the wallet knows it. */
    @Serializable
    @SerialName("contract")
    data class Contract(val address: String, val name: String? = null) : FeedFact()

    /**
     * Who got the money (`componentsTx.detail.to`): a plain send's recipient,
     * or the one a token `transfer` names; [name] is the row's name for them.
     * A call states this or [Contract], never both.
     */
    @Serializable
    @SerialName("recipient")
    data class Recipient(val address: String, val name: String? = null) : FeedFact()

    @Serializable
    @SerialName("spender")
    data class Spender(val address: String, val name: String? = null) : FeedFact()

    @Serializable
    @SerialName("spending_cap")
    data class SpendingCap(val allowance: FeedAllowance) : FeedFact()

    /** Epoch **seconds**; `null` = it never expires. */
    @Serializable
    @SerialName("expires")
    data class Expires(val at: Double? = null) : FeedFact()

    /** The lines of [FeedDapp.changes]. */
    @Serializable
    @SerialName("balance_changes")
    data object BalanceChanges : FeedFact()

    /** Epoch **seconds**. */
    @Serializable
    @SerialName("date")
    data class Date(val timestamp: Double) : FeedFact()

    @Serializable
    @SerialName("operation")
    data class Operation(val operation: FeedDappOperation) : FeedFact()

    /** The stored request — read from the store only when the section is opened. */
    @Serializable
    @SerialName("content")
    data class Content(val content: FeedDappContent) : FeedFact()

    @Serializable
    @SerialName("primary_type")
    data class PrimaryType(val name: String) : FeedFact()

    @Serializable
    @SerialName("hash")
    data class Hash(val tx_hash: String) : FeedFact()

    @Serializable
    @SerialName("user_op_hash")
    data class UserOpHash(val hash: String) : FeedFact()
}

/**
 * What a dApp row says beyond its money (083 H2, spec 093) — the core's
 * headline, place, allowance and detail. The shell words and draws it.
 */
@Serializable
data class FeedDapp(
    val site: String? = null,
    /** The verb's text, shown only when [intent_term] is `null`. */
    val intent: String? = null,
    /** The headline verb: a key leaf under `componentsUi.signing`. */
    val intent_term: String? = null,
    @Serializable(with = FeedDappChangeList::class)
    val changes: List<FeedDappChange> = emptyList(),
    /** A swap's one coin back, drawn beside the figure. */
    val received: FeedDappChange? = null,
    /** The figure is the simulation's: "≈". */
    val estimated: Boolean = false,
    val contract_call: Boolean = false,
    /** The title's `{{place}}` (`history.dappRowTitle`); `null` = the verb alone. */
    val place: String? = null,
    /** A grant's allowance, drawn where a figure would be. */
    val allowance: FeedAllowance? = null,
    /** A signature: no status chip, `connect.detail.offChainNote` instead. */
    val off_chain: Boolean = false,
    @Serializable(with = FeedFactList::class)
    val facts: List<FeedFact> = emptyList(),
    @Serializable(with = FeedFactList::class)
    val technical: List<FeedFact> = emptyList(),
    /** Spec 097 N4: a failed operation — why. */
    val failure: app.getvela.wallet.feature.send.core.TrackFailure? = null,
)

object FeedLineList : app.getvela.wallet.core.crux.FailSoftListSerializer<FeedLine>(FeedLine.serializer())

object FeedFactList : app.getvela.wallet.core.crux.FailSoftListSerializer<FeedFact>(FeedFact.serializer())

object FeedDappChangeList : app.getvela.wallet.core.crux.FailSoftListSerializer<FeedDappChange>(FeedDappChange.serializer())

object FeedDappFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<FeedDapp>(FeedDapp.serializer())

/**
 * A date header or an item, already interleaved in render order.
 *
 * The core emits them interleaved precisely so a shell cannot sort a header
 * away from the day it belongs to.
 */
@Serializable
sealed class FeedRow {
    @Serializable
    @SerialName("header")
    data class Header(
        val id: String,
        val day_start_ms: Double,
        /** A timestamp inside the day, from which the shell writes the label. */
        val timestamp: Double,
    ) : FeedRow()

    @Serializable
    @SerialName("item")
    data class Item(val item: FeedItem) : FeedRow()
}

@Serializable
data class FeedToast(
    val item_id: String,
    val value: String,
    val symbol: String,
    val deadline_ms: Double,
)

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class FeedOperation {
    @Serializable
    @SerialName("read_tx_store")
    data class ReadTxStore(val address: String, val read_id: Int) : FeedOperation()

    @Serializable
    @SerialName("scan_incoming_transfers")
    data class ScanIncomingTransfers(val address: String) : FeedOperation()

    @Serializable
    @SerialName("delete_tx_record")
    data class DeleteTxRecord(val id: String) : FeedOperation()

    @Serializable
    @SerialName("resolve_recipient_identity")
    data class ResolveRecipientIdentity(val addr: String) : FeedOperation()

    @Serializable
    @SerialName("timer")
    data class Timer(val ms: Int, val generation: Int) : FeedOperation()

    @Serializable
    @SerialName("haptic")
    data object Haptic : FeedOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class FeedShellResult {
    @Serializable
    @SerialName("store_loaded")
    data class StoreLoaded(
        val records: List<FeedTxRecord> = emptyList(),
        val now_ms: Double,
        /** Echoed from the read that produced it — the celebration's binding. */
        val read_id: Int,
    ) : FeedShellResult()

    @Serializable
    @SerialName("sync_completed")
    data class SyncCompleted(val new_count: Int) : FeedShellResult()

    @Serializable
    @SerialName("delete_committed")
    data class DeleteCommitted(val id: String) : FeedShellResult()

    @Serializable
    @SerialName("delete_failed")
    data class DeleteFailed(val id: String) : FeedShellResult()

    @Serializable
    @SerialName("alias_resolved")
    data class AliasResolved(val addr: String, val name: String? = null) : FeedShellResult()

    @Serializable
    @SerialName("toast_expired")
    data class ToastExpired(val generation: Int) : FeedShellResult()

    @Serializable
    @SerialName("haptic_played")
    data object HapticPlayed : FeedShellResult()
}

// -- what the screen sends ---------------------------------------------------

@Serializable
sealed class FeedEvent {
    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val address: String) : FeedEvent()

    @Serializable
    @SerialName("focus_tick")
    data object FocusTick : FeedEvent()

    @Serializable
    @SerialName("live_tick")
    data object LiveTick : FeedEvent()

    @Serializable
    @SerialName("reconcile_completed")
    data class ReconcileCompleted(val resolved_count: Int) : FeedEvent()

    @Serializable
    @SerialName("privacy_changed")
    data class PrivacyChanged(val hidden: Boolean) : FeedEvent()

    @Serializable
    @SerialName("chain_filter_changed")
    data class ChainFilterChanged(val chain_id: Int? = null) : FeedEvent()

    /** Spec 093: a contact's page opened (its address) or closed (`null`) — the view's [FeedView.contact_rows]. */
    @Serializable
    @SerialName("contact_filter_changed")
    data class ContactFilterChanged(val address: String? = null) : FeedEvent()

    @Serializable
    @SerialName("delete_requested")
    data class DeleteRequested(val id: String) : FeedEvent()
}

// -- what the screen renders -------------------------------------------------

@Serializable
data class FeedView(
    val rows: List<FeedRow> = emptyList(),
    /**
     * Issue #469: the home's Activity — the newest three items of [rows], in
     * the same order, with only the day headers over them. The core makes the
     * cut; History, a token's detail and a contact's page keep [rows].
     */
    val home_rows: List<FeedRow> = emptyList(),
    /** Account-scoped records for the detail sheet — not tombstone-filtered. */
    val transactions: List<FeedTxRecord> = emptyList(),
    /** The row that just landed and should glow. */
    val new_item_id: String? = null,
    /** `null` while balance privacy is on — the core enforces that, not the shell. */
    val toast: FeedToast? = null,
    /** Spec 082 RG5: History's empty line, chosen by the chain filter. */
    val history_empty_key: String = "history.emptyTitle",
    /** The home Activity's empty line, chosen the same way. */
    val home_empty_key: String = "home.emptyNoActivity",
    /**
     * Spec 093: what passed between the account and the open contact
     * ([FeedEvent.ContactFilterChanged]) — every row with that counterparty,
     * on every network, newest first, worded as Activity words it; second
     * line: status (unsettled), network, day. Fail-soft per row.
     */
    @Serializable(with = FeedItemList::class)
    val contact_rows: List<FeedItem> = emptyList(),
    /**
     * Balance privacy is on — the feed's own flag, so every surface drawn from
     * this view (home Activity, History, a contact's page, a transfer's and a
     * dApp row's detail) masks on the feed's word: each row's figure by
     * [FeedItem.figure_maskable], a dApp row's "received" and every detail
     * figure always.
     */
    val hidden: Boolean = false,
)

object FeedItemList : app.getvela.wallet.core.crux.FailSoftListSerializer<FeedItem>(FeedItem.serializer())
