package app.getvela.wallet.feature.signing.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `sign_request` machine's wire (spec 044) — a dApp request's life
 * from arrival to answer. `chain_id`/`active_index`/`index` are `u32`,
 * `now_ms`/`request_ts_ms` are `f64`, an error `code` is `i32`.
 */

@Serializable
enum class SignSurface {
    @SerialName("hidden") Hidden,

    @SerialName("sheet") Sheet,

    @SerialName("funding") Funding,
}

@Serializable
enum class SignSwipeAction {
    @SerialName("none") None,

    @SerialName("reject") Reject,

    @SerialName("dismiss") Dismiss,

    @SerialName("funding_cancel") FundingCancel,
}

@Serializable
enum class SignMethodKind {
    @SerialName("transaction") Transaction,

    @SerialName("batch") Batch,

    @SerialName("personal_sign") PersonalSign,

    @SerialName("eth_sign") EthSign,

    @SerialName("typed_data") TypedData,

    @SerialName("generic") Generic,
}

@Serializable
enum class SignErrorKind {
    @SerialName("user_rejected") UserRejected,

    @SerialName("wallet_switched_chains") WalletSwitchedChains,

    @SerialName("unsupported_chain") UnsupportedChain,

    @SerialName("unauthorized_account") UnauthorizedAccount,

    @SerialName("invalid_params") InvalidParams,

    @SerialName("unsupported_capability") UnsupportedCapability,

    @SerialName("unlimited_approval") UnlimitedApproval,

    @SerialName("funding_cancelled") FundingCancelled,

    @SerialName("submit_failed") SubmitFailed,

    @SerialName("stale_fee_quote") StaleFeeQuote,
}

@Serializable
enum class SignFundingPresentation {
    @SerialName("topup") Topup,

    @SerialName("confirming") Confirming,
}

@Serializable
enum class SignRecordKind {
    @SerialName("dapp_tx") DappTx,

    @SerialName("sign_typed_data") SignTypedData,

    @SerialName("sign_message") SignMessage,
}

@Serializable
enum class SignRecordStatus {
    @SerialName("pending") Pending,

    @SerialName("confirmed") Confirmed,
}

@Serializable
enum class SignSettledOutcome {
    @SerialName("submitted") Submitted,

    @SerialName("rejected") Rejected,
}

@Serializable
data class SignAccountRef(val address: String, val credential_id: String)

@Serializable
data class SignDappIdentity(val name: String, val url: String? = null)

@Serializable
data class SignQuotedFee(
    val amount: String,
    val recipient: String,
    /** The speed the displayed fee was priced at (spec 069), copied from the same estimate. */
    val tier: app.getvela.wallet.feature.send.core.FeeTier? = null,
)

@Serializable
data class SignErrorNotice(val kind: SignErrorKind, val detail: String? = null)

@Serializable
data class SignFundingNeeded(
    val deposit_address: String,
    val safe_address: String,
    val chain_id: Int,
    val native_symbol: String,
    val threshold_wei: String,
    val recommended_wei: String,
    val current_balance_wei: String,
)

@Serializable
data class SignFundingView(
    val data: SignFundingNeeded,
    val presentation: SignFundingPresentation,
    val denial_reason: String? = null,
)

@Serializable
data class SignApproveOpts(
    val max_fee_per_gas: String? = null,
    val bundler_cost_wei: String? = null,
    val gas_fee_token: String? = null,
    val quoted_fee: SignQuotedFee? = null,
    val fee_collector: String? = null,
    val params_override_json: String? = null,
    val intent: String? = null,
)

@Serializable
data class SignRecord(
    val record_id: String,
    val kind: SignRecordKind,
    val method: String,
    val params_json: String,
    val result: String,
    val from: String,
    val chain_id: Int,
    val now_ms: Double,
    val status: SignRecordStatus,
    val user_op_hash: String,
    val dapp_origin: String,
    val intent: String? = null,
)

@Serializable
sealed class SignRecordClose {
    @Serializable
    @SerialName("confirmed")
    data class Confirmed(val tx_hash: String) : SignRecordClose()

    @Serializable
    @SerialName("failed")
    data object Failed : SignRecordClose()
}

@Serializable
sealed class SignResponsePayload {
    @Serializable
    @SerialName("ok")
    data class Ok(val result: String? = null) : SignResponsePayload()

    @Serializable
    @SerialName("err")
    data class Err(val code: Int, val kind: SignErrorKind, val message: String? = null) : SignResponsePayload()
}

@Serializable
sealed class SignNotice {
    @Serializable
    @SerialName("expired")
    data object Expired : SignNotice()

    @Serializable
    @SerialName("already_settled")
    data class AlreadySettled(val outcome: SignSettledOutcome) : SignNotice()
}

@Serializable
sealed class SignSponsorship {
    @Serializable
    @SerialName("funded")
    data object Funded : SignSponsorship()

    @Serializable
    @SerialName("confirming")
    data object Confirming : SignSponsorship()

    @Serializable
    @SerialName("denied")
    data class Denied(val reason: String? = null) : SignSponsorship()
}

@Serializable
sealed class SignSubmitOutcome {
    @Serializable
    @SerialName("succeeded")
    data class Succeeded(val result: String) : SignSubmitOutcome()

    /** Accepted, but the receipt did not arrive inside the wait: the page gets the op hash, the record stays pending (issue 262). */
    @Serializable
    @SerialName("receipt_pending")
    data class ReceiptPending(val user_op_hash: String) : SignSubmitOutcome()

    @Serializable
    @SerialName("passkey_cancelled")
    data object PasskeyCancelled : SignSubmitOutcome()

    @Serializable
    @SerialName("underfunded")
    data class Underfunded(val message: String, val funding: SignFundingNeeded? = null) : SignSubmitOutcome()

    @Serializable
    @SerialName("failed")
    data class Failed(val message: String) : SignSubmitOutcome()
}

@Serializable
data class SignTrackerHandoff(val user_op_hash: String, val record_ids: List<String> = emptyList(), val chain_id: Int)

@Serializable
data class SignRequestView(
    val id: String,
    val method: String,
    val kind: SignMethodKind,
    val params_json: String,
    val origin: String,
    val dapp: SignDappIdentity? = null,
    val chain_id: Int,
    val signer_address: String? = null,
)

@Serializable
data class SignView(
    val surface: SignSurface = SignSurface.Hidden,
    val request: SignRequestView? = null,
    val is_signing: Boolean = false,
    val is_submitting: Boolean = false,
    val pending_op_hash: String? = null,
    val error: SignErrorNotice? = null,
    val funding: SignFundingView? = null,
    val confirm_gate_open: Boolean = false,
    val reconcile_pending: Boolean = false,
    val swipe_action: SignSwipeAction = SignSwipeAction.None,
    val tracker_handoff: SignTrackerHandoff? = null,
    val notice: SignNotice? = null,
    val global_chain_id: Int = 0,
)

@Serializable
sealed class SignOperation {
    @Serializable
    @SerialName("send_response")
    data class SendResponse(val transport_id: String, val id: String, val payload: SignResponsePayload) : SignOperation()

    @Serializable
    @SerialName("check_bundler_funding")
    data class CheckBundlerFunding(
        val chain_id: Int,
        val account: String,
        val bundler_cost_wei: String? = null,
        val bust_cache: Boolean = false,
    ) : SignOperation()

    @Serializable
    @SerialName("attempt_sponsorship")
    data class AttemptSponsorship(val funding: SignFundingNeeded, val force: Boolean = false) : SignOperation()

    @Serializable
    @SerialName("sign_and_submit")
    data class SignAndSubmit(
        val id: String,
        val method: String,
        val params_json: String,
        val chain_id: Int,
        val address: String,
        val credential_id: String,
        val max_fee_per_gas: String? = null,
        val gas_fee_token: String? = null,
        val quoted_fee: SignQuotedFee? = null,
    ) : SignOperation()

    @Serializable
    @SerialName("persist_record")
    data class PersistRecord(val record: SignRecord) : SignOperation()

    @Serializable
    @SerialName("update_record")
    data class UpdateRecord(val record_id: String, val close: SignRecordClose) : SignOperation()

    @Serializable
    @SerialName("switch_active_account")
    data class SwitchActiveAccount(val index: Int) : SignOperation()
}

@Serializable
sealed class SignShellResult {
    @Serializable
    @SerialName("pre_check")
    data class PreCheck(val funding: SignFundingNeeded? = null) : SignShellResult()

    @Serializable
    @SerialName("sponsorship")
    data class Sponsorship(val outcome: SignSponsorship) : SignShellResult()

    @Serializable
    @SerialName("submit")
    data class Submit(val outcome: SignSubmitOutcome, val now_ms: Double) : SignShellResult()

    @Serializable
    @SerialName("responded")
    data object Responded : SignShellResult()

    @Serializable
    @SerialName("record_persisted")
    data object RecordPersisted : SignShellResult()

    @Serializable
    @SerialName("record_updated")
    data object RecordUpdated : SignShellResult()

    @Serializable
    @SerialName("account_switched")
    data object AccountSwitched : SignShellResult()
}

@Serializable
sealed class SignEvent {
    @Serializable
    @SerialName("networks_changed")
    data class NetworksChanged(val chain_ids: List<Int>) : SignEvent()

    @Serializable
    @SerialName("accounts_changed")
    data class AccountsChanged(val accounts: List<SignAccountRef>, val active_index: Int) : SignEvent()

    @Serializable
    @SerialName("request_arrived")
    data class RequestArrived(
        val id: String,
        val method: String,
        val params_json: String,
        val origin: String,
        val transport_id: String,
        val dedicated_transport: Boolean,
        val per_request_chain: Int? = null,
        val dapp: SignDappIdentity? = null,
        val granted_address: String? = null,
        val requested_address: String? = null,
        val request_ts_ms: Double? = null,
        val now_ms: Double,
    ) : SignEvent()

    @Serializable
    @SerialName("chain_switch_requested")
    data class ChainSwitchRequested(
        val id: String? = null,
        val transport_id: String? = null,
        val chain_id_param: String? = null,
    ) : SignEvent()

    @Serializable
    @SerialName("approve_tapped")
    data class ApproveTapped(val opts: SignApproveOpts) : SignEvent()

    @Serializable
    @SerialName("reject_tapped")
    data object RejectTapped : SignEvent()

    @Serializable
    @SerialName("dismiss_tapped")
    data object DismissTapped : SignEvent()

    @Serializable
    @SerialName("swipe_dismissed")
    data object SwipeDismissed : SignEvent()

    @Serializable
    @SerialName("funding_complete_tapped")
    data object FundingCompleteTapped : SignEvent()

    @Serializable
    @SerialName("funding_cancelled")
    data object FundingCancelled : SignEvent()

    @Serializable
    @SerialName("op_submitted")
    data class OpSubmitted(val id: String, val user_op_hash: String, val now_ms: Double) : SignEvent()

    @Serializable
    @SerialName("transport_dropped")
    data class TransportDropped(val transport_id: String) : SignEvent()
}
