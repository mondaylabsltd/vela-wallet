package app.getvela.wallet.feature.send.core

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonPrimitive

/**
 * The `send` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/send.rs` (spec 043).
 *
 * The whole send controller is one machine: three modes (single, split
 * 一币多人, multi-select 多币一人), the step state, live validation,
 * string-exact Max, the same-asset fee ceiling, the treasury pre-check, the
 * sign→submit lifecycle with its cancel checkpoints and the single-flight
 * lock. **The shell contributes transports, storage, the assertion, a clock,
 * a haptic and an alert surface — and no decision.** Every wording regex that
 * classifies a relay's refusal lives in the core.
 *
 * Android drives single mode in 043; the split / multi-select events are
 * mirrored here so 045 adds a screen, not a wire.
 *
 * Numerics from the Rust: `chain_id`, `decimals`, `fiat_decimals`, `ms` are
 * `u32` → `Int`; `typical_inclusion_s` `u16` → `Int`; `price_usd`, `rate`,
 * `usd_value`, `timestamp_s`, `submitted_at_ms`, `now_ms` are `f64` →
 * `Double`. Every amount is a decimal string.
 */

// -- value types -------------------------------------------------------------

/**
 * A holding as the picker lists it. `balance` is the HUMAN decimal string,
 * exactly as the API reports it (not smallest units); `token_address = null`
 * is the chain's coin. Its id is derived by the core (`network` + address),
 * which is why there is no `id` field here — the drift gate said so.
 */
@Serializable
data class SendToken(
    val network: String,
    val chain_id: Int,
    val symbol: String,
    val balance: String,
    val decimals: Int,
    val token_address: String? = null,
    val price_usd: Double? = null,
    val logo_urls: List<String> = emptyList(),
    val spam: Boolean = false,
)

@Serializable
data class SendChainInfo(val chain_id: Int, val network: String, val native_symbol: String)

@Serializable
data class SendTokenMeta(val symbol: String, val decimals: Int)

@Serializable
sealed class SendAddNetworkOutcome {
    @Serializable
    @SerialName("added")
    data object Added : SendAddNetworkOutcome()

    @Serializable
    @SerialName("not_found")
    data object NotFound : SendAddNetworkOutcome()

    @Serializable
    @SerialName("not_compatible")
    data class NotCompatible(val detail: String? = null) : SendAddNetworkOutcome()

    @Serializable
    @SerialName("error")
    data object Error : SendAddNetworkOutcome()
}

@Serializable
sealed class SendAddNetworkMsg {
    @Serializable
    @SerialName("net_not_found")
    data object NetNotFound : SendAddNetworkMsg()

    @Serializable
    @SerialName("net_not_compatible")
    data class NetNotCompatible(val detail: String? = null) : SendAddNetworkMsg()

    @Serializable
    @SerialName("net_add_error")
    data object NetAddError : SendAddNetworkMsg()
}

/**
 * Why Continue's estimate failed (PR 2 note 13): every [FeeFailure], in the
 * fee machine's own wire shape and passed through as it is ([Fee]) — never
 * mapped to a nearer word here — plus the send side's [Timeout] and [Other].
 * The alert's body is the core's (`sendEstimateFailureBodyKey`), from [wire].
 */
@Serializable(with = SendEstimateFailure.WireSerializer::class)
sealed class SendEstimateFailure {
    /** What the core's `sendEstimateFailureBodyKey` takes: the wire name, or the whole JSON of one that carries data. */
    abstract val wire: String

    /** The fee machine's failure, verbatim. */
    data class Fee(val failure: FeeFailure) : SendEstimateFailure() {
        override val wire: String get() = failure.wire
    }

    /** The 15 s race lost. */
    data object Timeout : SendEstimateFailure() { override val wire = TIMEOUT }

    /** Nothing the fee machine said: the question was superseded, or no estimate came. */
    data object Other : SendEstimateFailure() { override val wire = OTHER }

    object WireSerializer : KSerializer<SendEstimateFailure> {
        override val descriptor: SerialDescriptor = JsonElement.serializer().descriptor

        override fun deserialize(decoder: Decoder): SendEstimateFailure {
            val json = decoder as? JsonDecoder ?: throw SerializationException("SendEstimateFailure crosses as JSON only")
            val element = json.decodeJsonElement()
            if (element is JsonPrimitive && element.isString) {
                when (element.content) {
                    TIMEOUT -> return Timeout
                    OTHER -> return Other
                }
            }
            // Anything else is a fee failure, in its own shape — or nothing this build knows (strict).
            return Fee(json.json.decodeFromJsonElement(FeeFailure.WireSerializer, element))
        }

        override fun serialize(encoder: Encoder, value: SendEstimateFailure) {
            val json = encoder as? JsonEncoder ?: throw SerializationException("SendEstimateFailure crosses as JSON only")
            when (value) {
                is Fee -> json.encodeSerializableValue(FeeFailure.WireSerializer, value.failure)
                Timeout -> json.encodeJsonElement(JsonPrimitive(TIMEOUT))
                Other -> json.encodeJsonElement(JsonPrimitive(OTHER))
            }
        }
    }

    companion object {
        private const val TIMEOUT = "timeout"
        private const val OTHER = "other"

        /** The two words of the send side's own — what the drift test holds beside [FeeFailure.PLAIN]. */
        val SEND_ONLY: List<String> = listOf(TIMEOUT, OTHER)
    }
}

@Serializable
sealed class SendFeeOutcome {
    @Serializable
    @SerialName("ok")
    data class Ok(val estimate: FeeEstimateView) : SendFeeOutcome()

    @Serializable
    @SerialName("failed")
    data class Failed(val kind: SendEstimateFailure) : SendFeeOutcome()
}

@Serializable
enum class SendTreasuryAsset {
    @SerialName("native") Native,

    @SerialName("path_usd") PathUsd,
}

@Serializable
data class SendTreasuryStatus(
    val chain_id: Int,
    val address: String,
    val asset: SendTreasuryAsset,
    val balance: String,
    val floor: String,
    val bootstrap_needed: Boolean,
    /**
     * Whether this is a network Vela ships, and so one whose relayer the
     * OPERATOR is expected to keep funded. The core decides; it changes what
     * the person is asked to do (spec 060).
     */
    val operator_served: Boolean = false,
    /**
     * `balance` and `floor` in the coin they are counted in, and what the stop
     * asks for (issue #422). The core fills it when it publishes the stop; a
     * probe this app reports leaves it out. `null` on a published stop only
     * when the relay's figures could not be read.
     */
    val coin: SendTreasuryCoin? = null,
)

/**
 * The relay's treasury figures in the coin they are counted in (issue #422):
 * the stop's own chain's coin, and plain whole-coin decimals the app only
 * writes with its decimal mark. This app used to name the coin by chain NAME
 * and said "ETH" for any name it did not know, and worked the figures out in
 * 18 decimals itself — so a stop asked for "0.0001 ETH" on Polygon.
 */
@Serializable
data class SendTreasuryCoin(
    /** pathUSD on Tempo, else the chain's own coin; `null` = the core does not know it (never a guess). */
    val symbol: String? = null,
    val balance: String,
    val floor: String,
    /** The suggested contribution: the relay's shortfall for this chain, in its coin. */
    val suggested: String,
)

/**
 * The relay said it cannot serve this chain (spec 098 §2) — a 404 from the
 * treasury probe. Not the funding sheet: gas cannot help a relay that cannot
 * reach the network. `operator_served` is the core's verdict, as above.
 */
/**
 * The coin the form's fee row names (`SendView.fee_coin`) — what the shell
 * hands the core's token mark to draw, figure or no figure.
 */
@Serializable
data class SendFeeCoin(
    /** The ticker as the wallet writes it. Empty only for an ERC-20 nothing on the form names: the mark draws its logo alone. */
    val symbol: String,
    /** `null` = the chain's own coin. */
    val contract: String? = null,
    /** The chain the fee is paid on — never 0. */
    val chain_id: Int,
)

@Serializable
data class SendRelayUnreachable(
    val chain_id: Int,
    val operator_served: Boolean = false,
)

/**
 * What a relay stop's "Report this" files (issue #466), built by the core once
 * so every shell files the same words under the same dedup key. English on
 * purpose — the operator reads it, not the person. It names the relay
 * treasury's address and figures, which are the operator's and public.
 */
@Serializable
data class SendRelayReport(
    /** The first line is the issue's title (≤ 80 characters); the facts follow after a blank line. */
    val what: String,
    /** How the person got there, numbered as the bug form asks. */
    val steps: String,
    /** The bug form's area option, verbatim (`"Send"`). */
    val area: String,
    /** `relay-gas-<chain>` / `relay-unreachable-<chain>`: one open issue per outage per chain. */
    val fingerprint: String,
)

/** What the relay's treasury can front on this chain — `unknown` when it could not be asked. */
@Serializable
sealed class SendTreasuryProbe {
    @Serializable
    @SerialName("low_float")
    data class LowFloat(val status: SendTreasuryStatus) : SendTreasuryProbe()

    @Serializable
    @SerialName("covered")
    data object Covered : SendTreasuryProbe()

    @Serializable
    @SerialName("uncovered")
    data object Uncovered : SendTreasuryProbe()

    @Serializable
    @SerialName("unknown")
    data object Unknown : SendTreasuryProbe()
}

/** Why a submit did not happen, on the axis the receipt speaks. */
@Serializable
sealed class SendSubmitFailure {
    @Serializable
    @SerialName("passkey_cancelled")
    data object PasskeyCancelled : SendSubmitFailure()

    @Serializable
    @SerialName("relayer_unavailable")
    data object RelayerUnavailable : SendSubmitFailure()

    @Serializable
    @SerialName("bundler_underfunded")
    data object BundlerUnderfunded : SendSubmitFailure()

    @Serializable
    @SerialName("other")
    data class Other(val message: String? = null) : SendSubmitFailure()

    /**
     * Spec 102: the account's venue cannot be used here — nothing was signed
     * or sent; the confirm screen says why ([SendView.tx_venue_block]).
     */
    @Serializable
    @SerialName("venue_blocked")
    data class VenueBlocked(val block: app.getvela.wallet.feature.signing.trustedsigner.VenueBlock) : SendSubmitFailure()

    /**
     * Another operation of this account still holds the nonce on this chain
     * (the relay's `nonce_in_flight`, or an older relay's `[existingHash:…]`
     * marker — the core's `RelayRejection::NonceHeld`). Nothing of this one
     * went out; the confirm screen says "waiting for your last transaction"
     * with Try again.
     */
    @Serializable
    @SerialName("previous_pending")
    data object PreviousPending : SendSubmitFailure()

    /**
     * send-11: the passkey prompt timed out — nothing was signed. The prompt
     * runs to the core's deadline (`passkey_sign_timeout_ms`), and a cancel at
     * it is a timeout by the core's rule (`passkey_ceremony_end`).
     */
    @Serializable
    @SerialName("passkey_timed_out")
    data object PasskeyTimedOut : SendSubmitFailure()

    /** send-15: another transaction of this account spent the nonce first; nothing was sent. */
    @Serializable
    @SerialName("nonce_used")
    data object NonceUsed : SendSubmitFailure()

    /** r5reg-3: the reads before the passkey did not get through — [busy]: the nodes answered "busy". */
    @Serializable
    @SerialName("chain_unreachable")
    data class ChainUnreachable(val busy: Boolean = false) : SendSubmitFailure()

    /**
     * Round 7 (r7-2, r7-7): the account's trusted signing page did not sign,
     * and not because the person declined there — [failure] says why. Sent
     * by the send executor whenever the page did not sign (R7, the page
     * everywhere); the core sets `tx_error` to its `page_*` key.
     */
    @Serializable
    @SerialName("page_failed")
    data class PageFailed(val failure: app.getvela.wallet.feature.signing.core.PageFailure) : SendSubmitFailure()
}

/** Present ⇔ in-band: the fee leg to sign EXACTLY as quoted (invariant ①). */
@Serializable
data class SendQuotedFee(
    val amount: String,
    val recipient: String,
    /**
     * The speed this fee was priced at, taken by the core from the same
     * estimate as the amount (spec 069) and named on the wire beside it.
     * `null` names nothing — the pre-068 wire.
     */
    val tier: FeeTier? = null,
)

/** The row the feed will show, written at submit — before tracking begins. */
@Serializable
data class SendTxRecord(
    val id: String,
    val user_op_hash: String,
    val tx_hash: String = "",
    val from: String,
    val to: String,
    val to_name: String? = null,
    val value: String,
    val symbol: String,
    val decimals: Int,
    val logo_urls: List<String> = emptyList(),
    val chain_id: Int,
    val timestamp_s: Double,
    val usd: String? = null,
    /** Spec 082 RA4/T184: the submit's reply was lost; persisted with the row. */
    val maybe_sent: Boolean = false,
    /** The head read before the first submit POST (`u64` → `Long`). */
    val submit_block: Long? = null,
)

@Serializable
enum class SendTxStatus {
    @SerialName("idle") Idle,

    @SerialName("preparing") Preparing,

    @SerialName("signing") Signing,

    @SerialName("submitting") Submitting,

    @SerialName("confirmed") Confirmed,

    @SerialName("error") Error,
}

@Serializable
enum class SendTxErrorKey {
    @SerialName("generic") Generic,

    @SerialName("bundler_fund") BundlerFund,

    /** Spec 102: this account cannot sign here; [SendView.tx_venue_block] says why. */
    @SerialName("venue_blocked") VenueBlocked,

    /**
     * The relay refused it because the account's previous transaction on this
     * network still holds the nonce: `componentsUi.signing.confirmBlock.previousPending`,
     * with Try again.
     */
    @SerialName("previous_pending") PreviousPending,

    /** send-15: another transaction of this account spent the nonce first. Nothing was sent: Review again. */
    @SerialName("nonce_used") NonceUsed,

    /** A multisig's proposal: its own words replace these (web-only today). */
    @SerialName("proposal_mailbox_down") ProposalMailboxDown,

    @SerialName("proposal_offline") ProposalOffline,

    /** A multisig's proposal this shell cannot make yet (`SendProposeFailure::NotSupported`): nothing was proposed. */
    @SerialName("proposal_not_supported") ProposalNotSupported,

    /** r5reg-3: the reads before the passkey did not get through — the chain's nodes were busy. */
    @SerialName("chain_busy") ChainBusy,

    /** r5reg-3: …the chain could not be reached at all. */
    @SerialName("chain_unreachable") ChainUnreachable,

    /** Round 7 (r7-2): the trusted signing page is not answering — nothing was signed (web's page). */
    @SerialName("page_unanswered") PageUnanswered,

    /** Round 7 (r7-7): the page's answer is not this operation's signature — nothing was sent. */
    @SerialName("page_mismatch") PageMismatch,

    /** The page's own rules refused it — nothing was signed. */
    @SerialName("page_refused") PageRefused,

    /** The page did not pass its check and was not opened — nothing was signed. */
    @SerialName("page_not_opened") PageNotOpened,
}

@Serializable
data class SendRecipientIdentity(val name: String? = null, val source: String? = null)

/**
 * Whose word a payee's name is (spec 097 F, S2) — the core reads the
 * resolver's label; the shell only picks the tag: none for [Own], the
 * corpus's "Vela User" for [Registry] (a public name anyone can register),
 * the service's own label for [Service].
 */
@Serializable
sealed class SendNameSource {
    @Serializable
    @SerialName("own")
    data object Own : SendNameSource()

    @Serializable
    @SerialName("registry")
    data object Registry : SendNameSource()

    @Serializable
    @SerialName("service")
    data class Service(val label: String) : SendNameSource()
}

/** A source this build cannot read is absent, never a broken send view. */
object SendNameSourceFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendNameSource>(SendNameSource.serializer())

/**
 * One payee as the confirm page names them (spec 097 F, S2): the address in
 * full, as signed, and a name only beside it — `name_source` is set exactly
 * when `name` is.
 */
@Serializable
data class SendPayee(
    val address: String,
    val name: String? = null,
    @Serializable(with = SendNameSourceFailSoft::class)
    val name_source: SendNameSource? = null,
)

/**
 * The recipient's risk signals, as the shell observed them (the web's
 * `recipient-risk.ts` + its `knownName` / `ownAddress` ports, r4a-18).
 * Facts, not verdicts: what the confirm says of them is the core's.
 */
@Serializable
data class SendRecipientRisk(
    /** Bytecode at the address: `true` contract, `false` an account (a 7702-delegated one too), `null` unknown. */
    val is_contract: Boolean? = null,
    /** Never sent here from this device. `null`: unknown — the history could not be read, or the address is the person's own. */
    val first_time: Boolean? = null,
    /** The history could not be read: `first_time` is `null` then, and the confirm says so, with a retry. */
    val history_unread: Boolean = false,
    /** The person's own name for it: one of their accounts here, else a contact they named. */
    val known_name: String? = null,
    /** One of the person's own accounts on this device: never told as a first transfer. */
    val own: Boolean = false,
)

/** Why a multisig's proposal was not made, as the shell knows it (`SendProposeFailure`). */
@Serializable
enum class SendProposeFailure {
    @SerialName("mailbox_down") MailboxDown,
    @SerialName("offline") Offline,
    /** This shell cannot propose a multisig's transaction yet — said as that, never as the mailbox. */
    @SerialName("not_supported") NotSupported,
}

/** One transfer the review showed (`ReviewedTransfer`): base units, `token` `null` for the native coin. */
@Serializable
data class ReviewedTransfer(val to: String, val token: String? = null, val amount: String)

/** `SendSubmitSlow`: the submit has run [seconds] without an answer; [busy] — the relay said it is busy. */
@Serializable
data class SendSubmitSlow(val seconds: Int = 0, val busy: Boolean = false)

/** `SendMaxMoved`: Max was tapped, and the figure moved from [from] to [to] as the fee settled. */
@Serializable
data class SendMaxMoved(val from: String = "", val to: String = "")

/** `SendPasskeyNote`: the passkey sheet closed without a signature — how. */
@Serializable
enum class SendPasskeyNote {
    @SerialName("cancelled") Cancelled,
    @SerialName("timed_out") TimedOut,
}

/** `SendRecipientWarning`: the address is one money must not go to. */
@Serializable
enum class SendRecipientWarning {
    @SerialName("zero_address") ZeroAddress,
    @SerialName("own_address") OwnAddress,
}

/** `SendMultisigRef`: the multisig this send is from — its rule. Web-only today; decoded so a view carrying it reads. */
@Serializable
data class SendMultisigRef(val threshold: Int = 0, val owners: Int = 0)

/** `SendProposed`: the proposal a multisig send became. */
@Serializable
data class SendProposed(val chain_id: Int, val safe_tx_hash: String)

/** `AmountIssue`: why the typed amount cannot be read. */
@Serializable
sealed class AmountIssue {
    @Serializable
    @SerialName("unreadable")
    data object Unreadable : AmountIssue()

    @Serializable
    @SerialName("too_many_decimals")
    data class TooManyDecimals(val decimals: Int) : AmountIssue()

    @Serializable
    @SerialName("too_large")
    data object TooLarge : AmountIssue()
}

object AmountIssueFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<AmountIssue>(AmountIssue.serializer())

object SendPasskeyNoteFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendPasskeyNote>(SendPasskeyNote.serializer())
object SendRecipientWarningFailSoft :
    app.getvela.wallet.core.crux.FailSoftSerializer<SendRecipientWarning>(SendRecipientWarning.serializer())
object SendSubmitSlowFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendSubmitSlow>(SendSubmitSlow.serializer())
object SendMaxMovedFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendMaxMoved>(SendMaxMoved.serializer())
object SendMultisigRefFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendMultisigRef>(SendMultisigRef.serializer())
object SendProposedFailSoft : app.getvela.wallet.core.crux.FailSoftSerializer<SendProposed>(SendProposed.serializer())

@Serializable
enum class SendTimerTag {
    @SerialName("estimate_timeout") EstimateTimeout,

    @SerialName("form_estimate") FormEstimate,

    /** The open treasury sheet asks the relay again (spec 098 §4). */
    @SerialName("treasury_watch") TreasuryWatch,

    /** The confirm's bounded wait for the recipient's check (send-18). */
    @SerialName("risk_wait") RiskWait,

    /** How long a may-have-been-sent receipt says "checking" before it says the outcome is still unknown (round 3). */
    @SerialName("maybe_sent_cap") MaybeSentCap,
}

@Serializable
enum class SendHapticKind {
    @SerialName("success") Success,

    @SerialName("error") Error,
}

@Serializable
sealed class SendAmountWarning {
    @Serializable
    @SerialName("not_enough_token")
    data class NotEnoughToken(val symbol: String) : SendAmountWarning()

    @Serializable
    @SerialName("insufficient_for_gas")
    data class InsufficientForGas(val symbol: String? = null) : SendAmountWarning()

    /** The fee alone outruns the balance — the state `Max` fills `0` for. */
    @Serializable
    @SerialName("insufficient_gas")
    data class InsufficientGas(val symbol: String? = null) : SendAmountWarning()

    @Serializable
    @SerialName("need_gas")
    data class NeedGas(val symbol: String? = null) : SendAmountWarning()

    @Serializable
    @SerialName("cannot_convert")
    data class CannotConvert(val code: String, val symbol: String) : SendAmountWarning()
}

/** The refusals the core voices through `show_alert`; the shell prints them. */
@Serializable
sealed class SendAlertKind {
    @Serializable
    @SerialName("invalid_address")
    data object InvalidAddress : SendAlertKind()

    @Serializable
    @SerialName("invalid_amount")
    data object InvalidAmount : SendAlertKind()

    @Serializable
    @SerialName("insufficient_balance")
    data class InsufficientBalance(val warning: SendAmountWarning? = null) : SendAlertKind()

    @Serializable
    @SerialName("split_over_balance")
    data object SplitOverBalance : SendAlertKind()

    @Serializable
    @SerialName("load_tokens_failed")
    data object LoadTokensFailed : SendAlertKind()

    @Serializable
    @SerialName("estimate_failed")
    data class EstimateFailed(val kind: SendEstimateFailure) : SendAlertKind()

    @Serializable
    @SerialName("account_unavailable")
    data object AccountUnavailable : SendAlertKind()
}

/** The same-asset fee ceiling, spelled out for the screen. */
@Serializable
data class SendFeeIssueView(
    val symbol: String,
    val transfer_amount: String,
    val balance: String,
    val fee_amount: String,
    val total: String,
    val max_transfer_amount: String,
)

@Serializable
data class SendUnitIssue(val code: String, val symbol: String)

@Serializable
enum class SendStage {
    @SerialName("lock_error") LockError,

    @SerialName("lock_resolving") LockResolving,

    @SerialName("receipt") Receipt,

    @SerialName("select_token") SelectToken,

    @SerialName("enter_details") EnterDetails,

    @SerialName("confirm") Confirm,
}

@Serializable
sealed class SendLockError {
    @Serializable
    @SerialName("network")
    data class Network(val chain_id: Int) : SendLockError()

    @Serializable
    @SerialName("token")
    data object Token : SendLockError()
}

@Serializable
data class SendRecipientDraft(
    val id: String,
    val address: String,
    val amount: String,
    val name: String? = null,
)

/** What one field of a split row still needs (`SendRowFieldState`). */
@Serializable
enum class SendRowFieldState {
    @SerialName("ok") Ok,

    /** Nothing typed yet — unfinished, not wrong. */
    @SerialName("empty") Empty,

    @SerialName("invalid") Invalid,
}

/** One split row `Continue` will not take, and which field is why. `ordinal` is `u32`, 1-based. */
@Serializable
data class SendSplitRowIssue(
    val id: String,
    val ordinal: Int,
    val address: SendRowFieldState,
    val amount: SendRowFieldState,
)

/** A split row that repeats an earlier row's payee (issue 203). `first_ordinal` is `u32`, 1-based. */
@Serializable
data class SendDuplicateRowView(val id: String, val first_ordinal: Int)

@Serializable
data class SendMultiSpecView(val token_address: String? = null, val decimals: Int, val amount: String)

@Serializable
sealed class SendScan {
    @Serializable
    @SerialName("request")
    data class Request(
        val recipient: String,
        val chain_id: Int? = null,
        val token_address: String? = null,
        val amount_base_units: String? = null,
    ) : SendScan()

    @Serializable
    @SerialName("text")
    data class Text(val data: String) : SendScan()
}

@Serializable
enum class SendReceiptStatus {
    @SerialName("submitted") Submitted,

    @SerialName("confirmed") Confirmed,

    @SerialName("failed") Failed,

    /** Spec 082 RA10: the reply was lost — "may have been sent", no Retry. */
    @SerialName("maybe_sent") MaybeSent,

    /** The relay never had it: "not sent". */
    @SerialName("not_sent") NotSent,
}

@Serializable
enum class SendHoldReason {
    @SerialName("fee_hold") FeeHold,

    @SerialName("fee_rejected") FeeRejected,

    /** The relay is topping up its gas on the chain before it sends (098 follow-up). */
    @SerialName("relay_funding") RelayFunding,
}

@Serializable
enum class SendReceiptKind {
    @SerialName("split") Split,

    @SerialName("multi_select") MultiSelect,
}

@Serializable
data class SendReceiptTransfer(
    val to: String,
    val to_name: String? = null,
    val amount: String,
    val symbol: String,
    val logo_urls: List<String> = emptyList(),
    val usd_value: Double,
)

/** One coin the operation sent, summed over its recipients (spec 097 F, S3). `token_address = null` is the native coin. */
@Serializable
data class SendReceiptCoin(
    /** Token units, as signed. */
    val amount: String,
    val symbol: String,
    val logo_urls: List<String> = emptyList(),
    val token_address: String? = null,
    val usd_value: Double,
)

@Serializable
data class SendReceiptView(
    /** "Send again" may be offered (round 4). */
    val can_send_again: Boolean = false,
    /** A may-have-been-sent outcome is still unknown past its wait (round 3). */
    val still_unknown: Boolean = false,
    val status: SendReceiptStatus,
    val hold_reason: SendHoldReason? = null,
    /**
     * A refusal's sentence, as the core keys it — the fee words only for a fee
     * refusal, `wentFirst` for a used nonce, else the plain "refused". Drawn
     * for every refusal; `null` otherwise.
     */
    val refusal_key: String? = null,
    val kind: SendReceiptKind? = null,
    val transfers: List<SendReceiptTransfer> = emptyList(),
    /** Spec 097 F (S3): every coin the operation sent, in signing order — a split's one total, a sweep's each. */
    val coins: List<SendReceiptCoin> = emptyList(),
    /** `coins[0].amount` when one coin was sent; `""` for a sweep of several. */
    val amount: String,
    val usd_value: Double,
    val submitted_at_ms: Double? = null,
    val typical_inclusion_s: Int? = null,
)

@Serializable
sealed class SendReceiptOutcome {
    @Serializable
    @SerialName("confirmed")
    data class Confirmed(val tx_hash: String) : SendReceiptOutcome()

    @Serializable
    @SerialName("failed")
    data class Failed(
        val rejected: Boolean,
        /** Spec 082: the tracker's `NotSent` — "not sent", never the fee-rejected words. */
        val not_sent: Boolean = false,
        /** `rejected` only: the relay's reason, carried to the send machine whole (it keys the words). */
        val refusal: RefusalReason? = null,
    ) : SendReceiptOutcome()

    @Serializable
    @SerialName("fee_held")
    data object FeeHeld : SendReceiptOutcome()

    /** The relay holds it while it tops up its gas; not sticky (098 follow-up). */
    @Serializable
    @SerialName("relay_funding")
    data object RelayFunding : SendReceiptOutcome()

    /** The relay has shown it holds an op whose reply was lost: back to "submitted". */
    @Serializable
    @SerialName("acknowledged")
    data object Acknowledged : SendReceiptOutcome()
}

@Serializable
data class SendAccountRef(
    val id: String,
    val address: String,
    val name: String? = null,
    /**
     * A multisig account (docs/multisig-vault-design.md): its send is a
     * proposal its members approve — no passkey and no fee of its own, the
     * confirm proposes (native multisig M2). `null`: a Vela account.
     */
    val multisig: SendMultisigRef? = null,
)

@Serializable
data class SendOpenParams(
    val preselected_symbol: String? = null,
    val preselected_network: String? = null,
    val prefilled_recipient: String? = null,
    val prefilled_chain_id: String? = null,
    val prefilled_token_address: String? = null,
    val prefilled_amount_base: String? = null,
    val locked: Boolean = false,
    val preselected_multi: String? = null,
)

/** The display currency the figures are typed and shown in. */
@Serializable
data class SendDisplayContext(val code: String, val rate: Double? = null, val fiat_decimals: Int)

/**
 * `SendView` — every field the core renders. Android reads a subset per
 * screen; nothing here is computed again in Kotlin.
 */
@Serializable
data class SendView(
    val stage: SendStage = SendStage.SelectToken,
    val loading: Boolean = false,
    /**
     * r10 follow-up (core §10.7): the token list could not be read — the
     * corpus key of the picker's line (`send.tokensUnavailable`), drawn in
     * place of the list with "Try again" ([SendEvent.RetryTokens]). `null`
     * otherwise. Drawn here, so the `load_tokens_failed` alert is not.
     */
    val tokens_failed_key: String? = null,
    val locked: Boolean = false,
    val amount_locked: Boolean = false,
    val lock_error: SendLockError? = null,
    val resolving_lock: Boolean = false,
    val adding_network: Boolean = false,
    val add_network_msg: SendAddNetworkMsg? = null,
    val tokens: List<SendToken> = emptyList(),
    val selected_token: SendToken? = null,
    val recipient: String = "",
    /** Issue #312: the network a scanned code named for the payer to choose on; `tokens` holds only its holdings. */
    val request_chain_id: Int? = null,
    /** Issue #326: the form's token card opens the asset picker. */
    val can_change_token: Boolean = false,
    val amount: String = "",
    /** The figure's OWN unit: `null` = token units, else that fiat code. */
    val amount_fiat_code: String? = null,
    val denom_toggle_shown: Boolean = false,
    val denom_toggle_enabled: Boolean = false,
    val denom_toggle_reason: SendUnitIssue? = null,
    val confirm_amount_issue: SendUnitIssue? = null,
    /** The ONE token-unit figure the confirm page may print (displayed = signed). */
    val token_amount: String = "",
    val confirm_amount: String = "",
    val split_mode: Boolean = false,
    val recipients: List<SendRecipientDraft> = emptyList(),
    val split_over_balance: Boolean = false,
    /** Split only: rows repeating an earlier payee — a note beside the row, never a refusal. */
    val split_duplicates: List<SendDuplicateRowView> = emptyList(),
    /** Split only: the gate's reasons, row by row. Empty exactly when the rows pass. */
    val split_row_issues: List<SendSplitRowIssue> = emptyList(),
    /** Split only: balance less the rows' sum, token units; `null` while unsummable or over. EXACT — for whoever counts. */
    val split_remaining: String? = null,
    /**
     * [split_remaining] as a person reads it: cut to the token ladder by the
     * core, never rounded up ("0.0003", not "0.000300194967818323") — a
     * figure of what is THERE may lose digits but must never gain value.
     * Dot-decimal; the one a shell draws.
     */
    val split_remaining_display: String? = null,
    /** How many more rows an import may add: the cap less the rows already started (the importer's own cap). */
    val split_import_room: Int = BATCH_MAX_RECIPIENTS,
    val picker_target: String? = null,
    val multi_select_mode: Boolean = false,
    val multi_selected_ids: List<String> = emptyList(),
    val multi_valuable_ids: List<String> = emptyList(),
    val multi_chain_id: Int? = null,
    val multi_specs: List<SendMultiSpecView> = emptyList(),
    val show_scanner: Boolean = false,
    val show_contact_picker: Boolean = false,
    val show_batch_import: Boolean = false,
    val estimating_gas: Boolean = false,
    val fee_busy: Boolean = false,
    /** Chain-guarded: never a prior network's quote. */
    val fee: FeeEstimateView? = null,
    val gas_fee_token: String? = null,
    /**
     * The coin the fee row wears, whether or not a figure is beside it — the
     * core's one answer for a quote out, a quote that failed and a speed
     * being measured (the estimate in hand, else the coin in force, else the
     * chain's own). `null` only while no chain is known.
     */
    val fee_coin: SendFeeCoin? = null,
    val amount_warning: SendAmountWarning? = null,
    /** Why the typed figure cannot be read (unreadable, too many decimals, too large). */
    @Serializable(with = AmountIssueFailSoft::class)
    val amount_issue: AmountIssue? = null,
    /** The recipient is mixed case and not its EIP-55 checksum — a typo or a forged copy. */
    val recipient_checksum_mismatch: Boolean = false,
    val same_asset_fee_issue: SendFeeIssueView? = null,
    val can_continue: Boolean = false,
    val can_confirm: Boolean = false,
    /** On the review: which review this is — what the confirm names back. */
    val review_digest: String? = null,
    /** The address money must not go to (zero, or this account's own), the core's word. */
    @Serializable(with = SendRecipientWarningFailSoft::class)
    val recipient_warning: SendRecipientWarning? = null,
    /** A multisig's send: its rule. Web-only today. */
    @Serializable(with = SendMultisigRefFailSoft::class)
    val multisig: SendMultisigRef? = null,
    /** A multisig's send became this proposal. */
    @Serializable(with = SendProposedFailSoft::class)
    val proposed: SendProposed? = null,
    /** The passkey sheet closed without a signature — how, for the line under the confirm. */
    @Serializable(with = SendPasskeyNoteFailSoft::class)
    val passkey_note: SendPasskeyNote? = null,
    /** The recipient's check is still out on the confirm. */
    val risk_checking: Boolean = false,
    /** The confirm opened without the recipient's check (send-18): said neutrally, with "Check again" (`retry_risk_check`). */
    val risk_unfinished: Boolean = false,
    /** …because the account's history could not be read (round 3) — never a first-time tag built on it. */
    val risk_history_unread: Boolean = false,
    /** Another tab of this account holds the submit (web); always `false` here. */
    val queued_behind_tab: Boolean = false,
    /** The submit is taking long (`submit_slow`). */
    @Serializable(with = SendSubmitSlowFailSoft::class)
    val submit_slow: SendSubmitSlow? = null,
    /** Max was tapped and the figure moved as the fee settled. */
    @Serializable(with = SendMaxMovedFailSoft::class)
    val max_moved: SendMaxMoved? = null,
    val sending: Boolean = false,
    val tx_status: SendTxStatus = SendTxStatus.Idle,
    val tx_error: SendTxErrorKey? = null,
    /** Spec 102: with `tx_error` = [SendTxErrorKey.VenueBlocked], why this account cannot sign here. */
    val tx_venue_block: app.getvela.wallet.feature.signing.trustedsigner.VenueBlock? = null,
    val tx_hash: String? = null,
    val user_op_hash: String? = null,
    val receipt: SendReceiptView? = null,
    val treasury_bootstrap: SendTreasuryStatus? = null,
    /** Spec 098 §2: the relay cannot serve this chain; the send stops here. */
    val relay_unreachable: SendRelayUnreachable? = null,
    /**
     * The account's previous transaction on this network is still in flight
     * (`in_flight_ops`): `can_confirm` is false, and the one line [SendPreviousPending.key]
     * sits under the held confirm — whatever `fee_busy` says. It opens by
     * itself once that one is final or has stalled for ten minutes.
     */
    val previous_pending: SendPreviousPending? = null,
    /**
     * Issue #466: what the stop's "Report this" files — set exactly while a
     * relay stop is up on a network Vela ships. Snapshot it at the tap: the
     * stop may close (funded) while the report is being read.
     */
    val relay_report: SendRelayReport? = null,
    val recipient_identity: SendRecipientIdentity? = null,
    /**
     * Spec 097 F (S2): who the money goes to, as the form line and the confirm
     * name them — one for a single send or a sweep (none until the address is
     * whole), one per `recipients` row for a split. Draw from these, never
     * from `recipient_identity`.
     */
    val payees: List<SendPayee> = emptyList(),
    val recipient_risk: SendRecipientRisk? = null,
    /** Spec 096 F12: the recipient is a token's own contract (the core's verdict). */
    val recipient_is_token_contract: Boolean = false,
    val sim_json: String? = null,
)

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class SendOperation {
    @Serializable
    @SerialName("fetch_tokens")
    data class FetchTokens(val address: String) : SendOperation()

    @Serializable
    @SerialName("clear_token_cache")
    data class ClearTokenCache(val address: String) : SendOperation()

    @Serializable
    @SerialName("resolve_token_metadata")
    data class ResolveTokenMetadata(val chain_id: Int, val address: String) : SendOperation()

    @Serializable
    @SerialName("add_network")
    data class AddNetwork(val chain_id: Int) : SendOperation()

    @Serializable
    @SerialName("estimate_fee")
    data class EstimateFee(
        val chain_id: Int,
        val account: String,
        val tx: FeeCall? = null,
        val batch: List<FeeCall>? = null,
        val gas_fee_token: String? = null,
        val public_key_hex: String? = null,
        /**
         * Nobody has chosen the fee coin on this form: handed on as the fee
         * session's `QuoteRequested.auto_fee_token`, so the fee machine pays in
         * a coin that can. Which one comes back in the estimate's `fee_asset`.
         */
        val auto_fee_token: Boolean = false,
    ) : SendOperation()

    @Serializable
    @SerialName("probe_treasury")
    data class ProbeTreasury(val chain_id: Int) : SendOperation()

    /**
     * Read ahead, into the relay client's own fee caches, what a quote on each
     * chain will need — answered `FeesPrewarmed` at once; the reads run on
     * without the core.
     */
    @Serializable
    @SerialName("prewarm_fees")
    data class PrewarmFees(val account: String, val chain_ids: List<Int> = emptyList()) : SendOperation()

    @Serializable
    @SerialName("load_account_credential")
    data class LoadAccountCredential(val account_id: String) : SendOperation()

    /** The passkey ceremony lives inside this one; `quoted_fee` present ⇔ in-band. */
    @Serializable
    @SerialName("submit_user_op")
    data class SubmitUserOp(
        val chain_id: Int,
        val account: String,
        val public_key_hex: String,
        val calls: List<FeeCall> = emptyList(),
        val max_fee_per_gas: String? = null,
        val gas_fee_token: String? = null,
        val quoted_fee: SendQuotedFee? = null,
        /** The review the person confirmed; the passkey's hash is computed against it. */
        val review: JsonElement? = null,
        /**
         * core-perf-1: the gas the confirmed fee was priced on, fresh, for
         * exactly these calls — signed without simulating and measuring
         * again (`UserOpSpine.submit`). Absent: measured as always.
         */
        val prepared: PreparedGas? = null,
    ) : SendOperation()

    @Serializable
    @SerialName("cancel_passkey_sign")
    data object CancelPasskeySign : SendOperation()

    @Serializable
    @SerialName("persist_tx_records")
    data class PersistTxRecords(val records: List<SendTxRecord> = emptyList()) : SendOperation()

    @Serializable
    @SerialName("track_submitted")
    data class TrackSubmitted(
        val user_op_hash: String,
        val record_ids: List<String> = emptyList(),
        val chain_id: Int,
        /** Spec 082: carried into the tracker's `Submitted`. */
        val maybe_sent: Boolean = false,
        val submit_block: Long? = null,
        /** Spec 082 RJ1: the relay accepted the op the write-ahead hand-off announced. */
        val admitted: Boolean = false,
        /** The account that signed it — forwarded to the tracker's `submitted` (`in_flight_ops`). */
        val sender: String? = null,
    ) : SendOperation()

    /**
     * Spec 082 RJ1: the write-ahead records are on disk and the tracker holds
     * them — the shell may POST [user_op_hash] now, and only now. With no
     * clearance within `userOpWriteAheadWaitMs()` it does not POST and answers
     * `submit_failed` (nothing sent). Answered `post_cleared`.
     */
    @Serializable
    @SerialName("clear_to_post")
    data class ClearToPost(val user_op_hash: String) : SendOperation()

    /** Spec 082 RJ1: the relay took the written-ahead op — these rows' `maybeSent` → false, in one write. Answered `records_persisted`. */
    @Serializable
    @SerialName("mark_admitted")
    data class MarkAdmitted(val record_ids: List<String> = emptyList()) : SendOperation()

    /** Spec 082 RJ1: written-ahead rows whose op is proven never sent, removed in one write. Answered `records_persisted`. */
    @Serializable
    @SerialName("delete_tx_records")
    data class DeleteTxRecords(val ids: List<String> = emptyList()) : SendOperation()

    /** Spec 082 RJ1: forwarded to the tracker's `Withdrawn`. Answered `track_handed_off`. */
    @Serializable
    @SerialName("track_withdrawn")
    data class TrackWithdrawn(val user_op_hash: String, val record_ids: List<String> = emptyList()) : SendOperation()

    @Serializable
    @SerialName("resolve_identity")
    data class ResolveIdentity(val address: String) : SendOperation()

    @Serializable
    @SerialName("resolve_risk")
    data class ResolveRisk(val chain_id: Int, val address: String) : SendOperation()

    @Serializable
    @SerialName("simulate_calls")
    data class SimulateCalls(
        val chain_id: Int,
        val account: String,
        val calls: List<FeeCall> = emptyList(),
    ) : SendOperation()

    @Serializable
    @SerialName("start_timer")
    data class StartTimer(val ms: Int, val tag: SendTimerTag) : SendOperation()

    @Serializable
    @SerialName("haptic")
    data class Haptic(val kind: SendHapticKind) : SendOperation()

    @Serializable
    @SerialName("show_alert")
    data class ShowAlert(val kind: SendAlertKind) : SendOperation()

    @Serializable
    @SerialName("close")
    data object Close : SendOperation()

    /**
     * A multisig's send becomes a proposal (web-only today: multisig
     * orchestration is not in this shell yet). Answered `propose_failed` —
     * never left unanswered, which is a confirm that spins for ever.
     */
    @Serializable
    @SerialName("propose_safe_tx")
    data class ProposeSafeTx(
        val chain_id: Int,
        val multisig: String,
        val transfers: List<ReviewedTransfer> = emptyList(),
    ) : SendOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class SendShellResult {
    /** `tokens = null` = the read failed (the core alerts); `[]` = nothing held. */
    @Serializable
    @SerialName("tokens_loaded")
    data class TokensLoaded(
        val tokens: List<SendToken>? = null,
        val chains: List<SendChainInfo> = emptyList(),
    ) : SendShellResult()

    @Serializable
    @SerialName("token_cache_cleared")
    data object TokenCacheCleared : SendShellResult()

    /** `PrewarmFees` was taken; the reads run on without the core. */
    @Serializable
    @SerialName("fees_prewarmed")
    data object FeesPrewarmed : SendShellResult()

    @Serializable
    @SerialName("token_metadata")
    data class TokenMetadata(val meta: SendTokenMeta? = null) : SendShellResult()

    @Serializable
    @SerialName("network_added")
    data class NetworkAdded(val outcome: SendAddNetworkOutcome) : SendShellResult()

    @Serializable
    @SerialName("fee_estimated")
    data class FeeEstimated(val outcome: SendFeeOutcome) : SendShellResult()

    @Serializable
    @SerialName("treasury_probed")
    data class TreasuryProbed(val probe: SendTreasuryProbe) : SendShellResult()

    @Serializable
    @SerialName("account_credential")
    data class AccountCredential(val public_key_hex: String? = null) : SendShellResult()

    @Serializable
    @SerialName("submitted")
    data class Submitted(
        val user_op_hash: String,
        val now_ms: Double,
        /** Spec 082 RA4: the reply was lost; [user_op_hash] is the local one. */
        val maybe_sent: Boolean = false,
        /** The head read before the first submit POST; `null` = unknown. */
        val submit_block: Long? = null,
    ) : SendShellResult()

    @Serializable
    @SerialName("submit_failed")
    data class SubmitFailed(val failure: SendSubmitFailure) : SendShellResult()

    @Serializable
    @SerialName("proposed")
    data class Proposed(val safe_tx_hash: String) : SendShellResult()

    /** [reason] `null`: none the shell can name — never a guessed `mailbox_down` / `offline`. */
    @Serializable
    @SerialName("propose_failed")
    data class ProposeFailed(val cancelled: Boolean, val reason: SendProposeFailure? = null) : SendShellResult()

    @Serializable
    @SerialName("passkey_cancel_acknowledged")
    data object PasskeyCancelAcknowledged : SendShellResult()

    @Serializable
    @SerialName("records_persisted")
    data object RecordsPersisted : SendShellResult()

    @Serializable
    @SerialName("track_handed_off")
    data object TrackHandedOff : SendShellResult()

    /** Spec 082 RJ1: `ClearToPost` was taken. */
    @Serializable
    @SerialName("post_cleared")
    data object PostCleared : SendShellResult()

    @Serializable
    @SerialName("identity_resolved")
    data class IdentityResolved(val identity: SendRecipientIdentity? = null) : SendShellResult()

    @Serializable
    @SerialName("risk_resolved")
    data class RiskResolved(val risk: SendRecipientRisk? = null) : SendShellResult()

    @Serializable
    @SerialName("sim_resolved")
    data class SimResolved(val sim_json: String? = null) : SendShellResult()

    @Serializable
    @SerialName("timer_elapsed")
    data class TimerElapsed(val tag: SendTimerTag) : SendShellResult()

    @Serializable
    @SerialName("alert_acknowledged")
    data object AlertAcknowledged : SendShellResult()

    @Serializable
    @SerialName("haptic_played")
    data object HapticPlayed : SendShellResult()

    @Serializable
    @SerialName("closed")
    data object Closed : SendShellResult()
}

// -- what the shell tells it -------------------------------------------------

@Serializable
sealed class SendEvent {
    @Serializable
    @SerialName("open")
    data class Open(
        val account: SendAccountRef? = null,
        val params: SendOpenParams = SendOpenParams(),
        val display: SendDisplayContext,
    ) : SendEvent()

    @Serializable
    @SerialName("display_changed")
    data class DisplayChanged(val display: SendDisplayContext) : SendEvent()

    @Serializable
    @SerialName("tokens_partial")
    data class TokensPartial(val tokens: List<SendToken>) : SendEvent()

    /**
     * The asset list's holdings moved while Send is open — mapped exactly as
     * `fetch_tokens` is answered. What follows them (the picker always, the
     * form's balance and Max, never a confirm page) is the core's to say.
     */
    @Serializable
    @SerialName("holdings_updated")
    data class HoldingsUpdated(val tokens: List<SendToken>) : SendEvent()

    @Serializable
    @SerialName("refresh_tokens")
    data object RefreshTokens : SendEvent()

    @Serializable
    @SerialName("select_token")
    data class SelectToken(val token_id: String) : SendEvent()

    @Serializable
    @SerialName("toggle_multi_token")
    data class ToggleMultiToken(val token_id: String) : SendEvent()

    @Serializable
    @SerialName("toggle_all_multi_tokens")
    data class ToggleAllMultiTokens(val visible_ids: List<String>) : SendEvent()

    @Serializable
    @SerialName("set_multi_network")
    data class SetMultiNetwork(val chain_id: Int? = null) : SendEvent()

    @Serializable
    @SerialName("confirm_multi_selection")
    data object ConfirmMultiSelection : SendEvent()

    @Serializable
    @SerialName("set_recipient")
    data class SetRecipient(val recipient: String) : SendEvent()

    @Serializable
    @SerialName("set_amount")
    data class SetAmount(val amount: String) : SendEvent()

    @Serializable
    @SerialName("toggle_fiat_input")
    data object ToggleFiatInput : SendEvent()

    @Serializable
    @SerialName("tap_max")
    data object TapMax : SendEvent()

    @Serializable
    @SerialName("enter_split_mode")
    data object EnterSplitMode : SendEvent()

    @Serializable
    @SerialName("seed_split_recipients")
    data class SeedSplitRecipients(val recipients: List<SendRecipientDraft>) : SendEvent()

    /** The same rows ADDED to the ones already typed (issue #271) — the web's default for an import or a group. */
    @Serializable
    @SerialName("append_split_recipients")
    data class AppendSplitRecipients(val recipients: List<SendRecipientDraft>) : SendEvent()

    @Serializable
    @SerialName("recipients_changed")
    data class RecipientsChanged(val recipients: List<SendRecipientDraft>) : SendEvent()

    @Serializable
    @SerialName("open_contact_picker")
    data class OpenContactPicker(val target: String? = null) : SendEvent()

    @Serializable
    @SerialName("close_contact_picker")
    data object CloseContactPicker : SendEvent()

    @Serializable
    @SerialName("picked_address")
    data class PickedAddress(val address: String) : SendEvent()

    /**
     * Issue #471: [target] is the split row the code lands in (its id), as
     * [OpenContactPicker]'s; `null` (omitted on the wire) is the single
     * field's / the home's scan.
     */
    @Serializable
    @SerialName("open_scanner")
    data class OpenScanner(val target: String? = null) : SendEvent()

    @Serializable
    @SerialName("close_scanner")
    data object CloseScanner : SendEvent()

    @Serializable
    @SerialName("scan_resolved")
    data class ScanResolved(val scan: SendScan) : SendEvent()

    @Serializable
    @SerialName("open_batch_import")
    data object OpenBatchImport : SendEvent()

    @Serializable
    @SerialName("close_batch_import")
    data object CloseBatchImport : SendEvent()

    @Serializable
    @SerialName("add_network_tapped")
    data class AddNetworkTapped(val chain_id: Int) : SendEvent()

    @Serializable
    @SerialName("continue")
    data object Continue : SendEvent()

    @Serializable
    @SerialName("back")
    data object Back : SendEvent()

    /** Issue #326: the form's token card — back to the picker, the payee kept. */
    @Serializable
    @SerialName("change_token")
    data object ChangeToken : SendEvent()

    @Serializable
    @SerialName("edit_amount")
    data object EditAmount : SendEvent()

    @Serializable
    @SerialName("choose_fee_token")
    data class ChooseFeeToken(val token: String? = null) : SendEvent()

    /** The fee session's estimate, handed over unchanged (displayed = signed). */
    @Serializable
    @SerialName("fee_updated")
    data class FeeUpdated(val estimate: FeeEstimateView) : SendEvent()

    @Serializable
    @SerialName("fee_busy_changed")
    data class FeeBusyChanged(val busy: Boolean) : SendEvent()

    /**
     * The fee card's `FeeView.failure` is set — a failure, or the core's own
     * re-ask after one — or no longer is: the fee machine holds no figure
     * then. While true the confirm is held, and on the confirm page the
     * figure kept from Continue is dropped, so the row draws the failure.
     * Sent beside [FeeBusyChanged] when it changes ([FeeFailedWord]).
     */
    @Serializable
    @SerialName("fee_failed_changed")
    data class FeeFailedChanged(val failed: Boolean) : SendEvent()

    /**
     * The fee card's coin in force (`FeeView.fee_token`, verbatim; `null` =
     * the chain's own coin), sent beside [FeeBusyChanged] whenever it differs
     * from what this send session was last told. It names the fee row's coin
     * while no estimate is in hand; it prices and signs nothing.
     */
    @Serializable
    @SerialName("fee_token_changed")
    data class FeeTokenChanged(val fee_token: String? = null) : SendEvent()

    @Serializable
    @SerialName("slide_confirm")
    data class SlideConfirm(
        /** The `review_digest` of the review the tapped confirm was drawn on. */
        val review: String? = null,
    ) : SendEvent()

    @Serializable
    @SerialName("signing_started")
    data object SigningStarted : SendEvent()

    /**
     * Spec 082 RJ1: inside `submit_user_op`, the op is signed and its hash
     * computed, and nothing has been POSTed. The core writes the records ahead
     * and answers `ClearToPost` once they are on disk and tracked.
     */
    @Serializable
    @SerialName("op_signed")
    data class OpSigned(val user_op_hash: String, val submit_block: Long? = null, val now_ms: Double) : SendEvent()

    @Serializable
    @SerialName("cancel_signing")
    data object CancelSigning : SendEvent()

    @Serializable
    @SerialName("retry_after_bootstrap")
    data object RetryAfterBootstrap : SendEvent()

    @Serializable
    @SerialName("dismiss_treasury_sheet")
    data object DismissTreasurySheet : SendEvent()

    /** Spec 098 §2: after the network's RPC or the relay changed — the pre-check again. */
    @Serializable
    @SerialName("retry_relay_unreachable")
    data object RetryRelayUnreachable : SendEvent()

    @Serializable
    @SerialName("dismiss_relay_unreachable")
    data object DismissRelayUnreachable : SendEvent()

    @Serializable
    @SerialName("retry_after_error")
    data object RetryAfterError : SendEvent()

    /** r10 follow-up: the picker's "Try again" under [SendView.tokens_failed_key] — the token list is asked again. */
    @Serializable
    @SerialName("retry_tokens")
    data object RetryTokens : SendEvent()

    /**
     * r10 follow-up: Send was left without the core's `close` — the system
     * back, another tab, a deep link over it. What is still out for it is
     * dropped: an answer landing after the person left raises nothing over
     * whatever they went to. The next `open` starts afresh.
     */
    @Serializable
    @SerialName("left")
    data object Left : SendEvent()

    /** The confirm opened without the recipient's check (`risk_unfinished`): ask it again ("Check again"). */
    @Serializable
    @SerialName("retry_risk_check")
    data object RetryRiskCheck : SendEvent()

    /**
     * Round 3 (send-15): inside `submit_user_op`, another sender of this
     * account on this network holds the submit, and this one waits its turn
     * ([SendView.queued_behind_tab]). The web's cross-tab lock; this app has
     * one Send per process and never waits on one, so nothing sends it here.
     */
    @Serializable
    @SerialName("send_queued")
    data object SendQueued : SendEvent()

    /**
     * r5reg-3: inside `submit_user_op`, before the passkey opens, the reads
     * the operation needs first have been out [elapsed_ms] without an answer —
     * [busy] when the network's nodes answer "too many requests"
     * ([SendView.submit_slow]). Cleared by `signing_started` or the submit's end.
     */
    @Serializable
    @SerialName("submit_slow")
    data class SubmitSlow(val elapsed_ms: Int, val busy: Boolean) : SendEvent()

    /**
     * Round 6: the form's network answered a bounded read again after a
     * submit failed as the network's (`chain_unreachable` / `chain_busy`) —
     * the confirm opens by itself; the shell asks the fee again with it.
     */
    @Serializable
    @SerialName("chain_answered")
    data class ChainAnswered(val chain_id: Int) : SendEvent()

    /**
     * The receipt's "Try again" (send-5) over a payment proven never sent
     * ([SendReceiptView.can_send_again]): back to the confirm, every value as
     * it was, for a new quote and a new signature.
     */
    @Serializable
    @SerialName("send_again")
    data object SendAgain : SendEvent()

    /**
     * Every operation in flight on this device, as the tracker last said —
     * `inFlightOps` of the tracker's own view JSON, forwarded on every tracker
     * render and after every open (the machine forgets it on open, and
     * dedupes an identical list).
     */
    @Serializable
    @SerialName("in_flight_ops")
    data class InFlightOps(val ops: List<InFlightOp> = emptyList()) : SendEvent()

    @Serializable
    @SerialName("receipt_update")
    data class ReceiptUpdate(val user_op_hash: String, val outcome: SendReceiptOutcome) : SendEvent()

    @Serializable
    @SerialName("done")
    data object Done : SendEvent()
}

/** [SendView.previous_pending]: the transaction this send waits for, and the line under the held confirm. */
@Serializable
data class SendPreviousPending(
    val chain_id: Int,
    val user_op_hash: String,
    /** `componentsUi.signing.confirmBlock.previousPending`. */
    val key: String,
)

/**
 * An operation that holds its account's nonce on a chain (`tx_tracker::InFlightOp`):
 * accepted by the relay, not final, still followed, not stalled. Read from
 * the tracker by the core (`inFlightOps`), forwarded to the send and signing
 * machines as it came.
 */
@Serializable
data class InFlightOp(
    /** The account, lower-cased. */
    val sender: String,
    val chain_id: Int,
    val user_op_hash: String,
)
