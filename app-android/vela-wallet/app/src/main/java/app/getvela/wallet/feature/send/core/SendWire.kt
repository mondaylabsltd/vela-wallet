package app.getvela.wallet.feature.send.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

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

@Serializable
enum class SendEstimateFailure {
    @SerialName("missing_public_key") MissingPublicKey,

    @SerialName("fee_token_unavailable") FeeTokenUnavailable,

    @SerialName("quote_unavailable") QuoteUnavailable,

    @SerialName("calculation_failed") CalculationFailed,

    @SerialName("estimate_failed") EstimateFailed,

    @SerialName("gas_quote_too_high") GasQuoteTooHigh,

    @SerialName("timeout") Timeout,

    @SerialName("other") Other,
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
}

@Serializable
data class SendRecipientIdentity(val name: String? = null, val source: String? = null)

@Serializable
data class SendRecipientRisk(val is_contract: Boolean? = null, val first_time: Boolean? = null)

@Serializable
enum class SendTimerTag {
    @SerialName("estimate_timeout") EstimateTimeout,

    @SerialName("form_estimate") FormEstimate,
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
}

@Serializable
enum class SendHoldReason {
    @SerialName("fee_hold") FeeHold,

    @SerialName("fee_rejected") FeeRejected,
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

@Serializable
data class SendReceiptView(
    val status: SendReceiptStatus,
    val hold_reason: SendHoldReason? = null,
    val kind: SendReceiptKind? = null,
    val transfers: List<SendReceiptTransfer> = emptyList(),
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
    data class Failed(val rejected: Boolean) : SendReceiptOutcome()

    @Serializable
    @SerialName("fee_held")
    data object FeeHeld : SendReceiptOutcome()
}

@Serializable
data class SendAccountRef(val id: String, val address: String, val name: String? = null)

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
    val locked: Boolean = false,
    val amount_locked: Boolean = false,
    val lock_error: SendLockError? = null,
    val resolving_lock: Boolean = false,
    val adding_network: Boolean = false,
    val add_network_msg: SendAddNetworkMsg? = null,
    val tokens: List<SendToken> = emptyList(),
    val selected_token: SendToken? = null,
    val recipient: String = "",
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
    val amount_warning: SendAmountWarning? = null,
    val same_asset_fee_issue: SendFeeIssueView? = null,
    val can_continue: Boolean = false,
    val can_confirm: Boolean = false,
    val sending: Boolean = false,
    val tx_status: SendTxStatus = SendTxStatus.Idle,
    val tx_error: SendTxErrorKey? = null,
    val tx_hash: String? = null,
    val user_op_hash: String? = null,
    val receipt: SendReceiptView? = null,
    val treasury_bootstrap: SendTreasuryStatus? = null,
    val recipient_identity: SendRecipientIdentity? = null,
    val recipient_risk: SendRecipientRisk? = null,
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
    ) : SendOperation()

    @Serializable
    @SerialName("probe_treasury")
    data class ProbeTreasury(val chain_id: Int) : SendOperation()

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
    ) : SendOperation()

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
    data class Submitted(val user_op_hash: String, val now_ms: Double) : SendShellResult()

    @Serializable
    @SerialName("submit_failed")
    data class SubmitFailed(val failure: SendSubmitFailure) : SendShellResult()

    @Serializable
    @SerialName("passkey_cancel_acknowledged")
    data object PasskeyCancelAcknowledged : SendShellResult()

    @Serializable
    @SerialName("records_persisted")
    data object RecordsPersisted : SendShellResult()

    @Serializable
    @SerialName("track_handed_off")
    data object TrackHandedOff : SendShellResult()

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

    @Serializable
    @SerialName("open_scanner")
    data object OpenScanner : SendEvent()

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

    @Serializable
    @SerialName("slide_confirm")
    data object SlideConfirm : SendEvent()

    @Serializable
    @SerialName("signing_started")
    data object SigningStarted : SendEvent()

    @Serializable
    @SerialName("cancel_signing")
    data object CancelSigning : SendEvent()

    @Serializable
    @SerialName("retry_after_bootstrap")
    data object RetryAfterBootstrap : SendEvent()

    @Serializable
    @SerialName("dismiss_treasury_sheet")
    data object DismissTreasurySheet : SendEvent()

    @Serializable
    @SerialName("retry_after_error")
    data object RetryAfterError : SendEvent()

    @Serializable
    @SerialName("receipt_update")
    data class ReceiptUpdate(val user_op_hash: String, val outcome: SendReceiptOutcome) : SendEvent()

    @Serializable
    @SerialName("done")
    data object Done : SendEvent()
}
