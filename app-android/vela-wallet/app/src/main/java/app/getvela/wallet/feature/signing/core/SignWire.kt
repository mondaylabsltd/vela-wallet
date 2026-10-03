package app.getvela.wallet.feature.signing.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

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

    /** The request would have changed who controls the account (spec 081). */
    @SerialName("self_call_blocked") SelfCallBlocked,

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

/**
 * Where a request is between the approve and its answer (spec 082 RA9): the
 * sheet's words come from this, never from shell flags. `preparing` →
 * `send.txPreparing`; `awaiting_signature` → `send.txSigning` (a message:
 * `componentsUi.signing.signing`); `submitting` → `send.txSubmitting`.
 */
@Serializable
enum class SignPhase {
    @SerialName("idle") Idle,

    @SerialName("preparing") Preparing,

    @SerialName("awaiting_signature") AwaitingSignature,

    @SerialName("submitting") Submitting,
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

/** Why a request is refused outright: the Safe function it would have called. */
@Serializable
data class SignBlockedView(
    val function: String,
    val selector: String = "",
    val leg_index: Int? = null,
    val nested: Boolean = false,
)

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
    /** Spec 093: `ClearSigningView.record_intent`, copied — never decided here. */
    val intent: String? = null,
    val unlimited_approved: Boolean = false,
    /** 083 F1: the sheet's simulation judgments, exactly as drawn under "Balance changes". */
    val balance_changes: List<app.getvela.wallet.feature.wallet.core.TrustSimJudgment>? = null,
    /** Spec 093: the approval surface's token (`GuardView.meta`), copied verbatim. */
    val token_meta: GuardTokenMetaView? = null,
    /** Spec 097: `ClearSigningView.record_reading` — what the reading named — copied verbatim. */
    val reading: app.getvela.wallet.feature.wallet.core.DappReading? = null,
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
    /** 083 H2: the origin the request arrived from — what Activity names the site by. */
    val dapp_url: String = "",
    val intent: String? = null,
    /** Spec 082 RA3/T184: the submit's reply was lost; persisted with the row. */
    val maybe_sent: Boolean = false,
    /** The head read before the first submit POST (`u64` → `Long`), persisted likewise. */
    val submit_block: Long? = null,
    /** 083 F1: what the sheet's simulation said it moves — stored verbatim (`balanceChanges`). */
    val balance_changes: List<app.getvela.wallet.feature.wallet.core.TrustSimJudgment>? = null,
    /** Spec 093: what the request was — stored verbatim (`dappSummary`). */
    val summary: app.getvela.wallet.feature.wallet.core.DappSummary? = null,
    /** Spec 093: the request as the record keeps it (≤ 8 KB, cut by the core) — stored as `signedRequest`. */
    val stored_request: String = "",
    /** [stored_request] is shorter than the request was (`requestTruncated`). */
    val request_truncated: Boolean = false,
)

@Serializable
sealed class SignRecordClose {
    @Serializable
    @SerialName("confirmed")
    data class Confirmed(val tx_hash: String) : SignRecordClose()

    @Serializable
    @SerialName("failed")
    data object Failed : SignRecordClose()

    /**
     * Spec 082 RJ1: the relay accepted the write-ahead record's op — the
     * record's `maybeSent` becomes false and it stays pending; only the
     * tracker closes it.
     */
    @Serializable
    @SerialName("admitted")
    data object Admitted : SignRecordClose()
}

@Serializable
sealed class SignResponsePayload {
    /**
     * The page's `result` exactly as the core formed it: a hash or a
     * signature (a JSON string), or a `wallet_sendCalls` batch's id in the
     * shape its request declared — `{ "id": … }` for EIP-5792 2.0.0 (spec
     * 097 G). Forwarded to the browser untouched; this shell never shapes it.
     */
    @Serializable
    @SerialName("ok")
    data class Ok(val result: JsonElement? = null) : SignResponsePayload()

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

    /**
     * Accepted, but the receipt did not arrive inside the wait: the record
     * stays pending (issue 262). For a transaction the core answers the page
     * "not confirmed yet" (083) — the op hash only as a batch's id.
     */
    @Serializable
    @SerialName("receipt_pending")
    data class ReceiptPending(val user_op_hash: String) : SignSubmitOutcome()

    /** 083: included and REVERTED — the page hears the revert, naming the transaction. */
    @Serializable
    @SerialName("reverted")
    data class Reverted(val user_op_hash: String, val tx_hash: String) : SignSubmitOutcome()

    /** 083: the whole wait went by with no transaction — "not confirmed yet", the record stays pending. */
    @Serializable
    @SerialName("not_confirmed")
    data class NotConfirmed(val user_op_hash: String) : SignSubmitOutcome()

    @Serializable
    @SerialName("passkey_cancelled")
    data object PasskeyCancelled : SignSubmitOutcome()

    @Serializable
    @SerialName("underfunded")
    data class Underfunded(val message: String, val funding: SignFundingNeeded? = null) : SignSubmitOutcome()

    /**
     * [refused] (spec 082 RJ3): the relay refused the op — a rejection that is
     * not "relayer unavailable". The page is answered the core's fixed
     * "refused" sentence whatever [message] says, and the sheet never says
     * "try again".
     */
    @Serializable
    @SerialName("failed")
    data class Failed(val message: String, val refused: Boolean = false) : SignSubmitOutcome()

    /** Spec 082 RB2: the asker is gone — nothing was sent, nobody is answered, nothing is recorded. */
    @Serializable
    @SerialName("asker_gone")
    data object AskerGone : SignSubmitOutcome()
}

@Serializable
data class SignTrackerHandoff(
    val user_op_hash: String,
    val record_ids: List<String> = emptyList(),
    val chain_id: Int,
    /** Spec 082: carried into the tracker's `Submitted`, so a lost reply is followed as one. */
    val maybe_sent: Boolean = false,
    val submit_block: Long? = null,
    /** Spec 082 RJ1: the relay accepted the op the write-ahead hand-off announced. */
    val admitted: Boolean = false,
)

/**
 * Spec 082 RJ1: a write-ahead record proven never sent — fed to the tracker's
 * `Withdrawn` the moment it appears (idempotent).
 */
@Serializable
data class SignTrackerWithdraw(
    val user_op_hash: String,
    val record_ids: List<String> = emptyList(),
)

/**
 * What the answer that went to the page stands for (spec 082 RA8) — the
 * core's `sign_ending_of`, decoded; never derived here.
 */
@Serializable
sealed class SignEnding {
    @Serializable
    @SerialName("signed")
    data object Signed : SignEnding()

    @Serializable
    @SerialName("landed")
    data class Landed(val tx_hash: String, val user_op_hash: String? = null) : SignEnding()

    /** 083: included and reverted — drawn as reverted at once. */
    @Serializable
    @SerialName("reverted")
    data class Reverted(val tx_hash: String, val user_op_hash: String? = null) : SignEnding()

    @Serializable
    @SerialName("still_confirming")
    data class StillConfirming(val user_op_hash: String) : SignEnding()
}

/**
 * What the sheet draws for an ending once the tracker has had its say — the
 * core's `sign_ending_state`. A landed op is never drawn confirmed until the
 * tracker says so (W3).
 */
@Serializable
sealed class SignEndingState {
    @Serializable
    @SerialName("signed")
    data object Signed : SignEndingState()

    @Serializable
    @SerialName("confirmed")
    data class Confirmed(val tx_hash: String) : SignEndingState()

    @Serializable
    @SerialName("reverted")
    data class Reverted(val tx_hash: String) : SignEndingState()

    @Serializable
    @SerialName("not_sent")
    data object NotSent : SignEndingState()

    /**
     * Spec 082 RJ3: the relay refused it (the tracker's `Rejected`) — cross,
     * `statusFailed` + `componentsUi.signing.refused`, and no Retry words:
     * the same request would be refused again.
     */
    @Serializable
    @SerialName("refused")
    data object Refused : SignEndingState()

    @Serializable
    @SerialName("following")
    data class Following(
        val user_op_hash: String,
        val outcome: app.getvela.wallet.feature.send.core.TrackOutcome,
        val fee_held: Boolean = false,
        /** The relay is topping up its gas before it sends (098 follow-up). */
        val relay_funding: Boolean = false,
    ) : SignEndingState()
}

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
    /** Spec 082 RA9: the words' source. */
    val phase: SignPhase = SignPhase.Idle,
    val pending_op_hash: String? = null,
    /** Spec 082: the pending op's submit reply was lost. */
    val pending_op_maybe_sent: Boolean = false,
    val error: SignErrorNotice? = null,
    val funding: SignFundingView? = null,
    val confirm_gate_open: Boolean = false,
    val reconcile_pending: Boolean = false,
    val swipe_action: SignSwipeAction = SignSwipeAction.None,
    val tracker_handoff: SignTrackerHandoff? = null,
    /** Spec 082 RJ1: fed to the tracker's `Withdrawn` the moment it appears. */
    val tracker_withdraw: SignTrackerWithdraw? = null,
    /** Spec 082 RJ3: [error] is the relay refusing the op — `refused` under `statusFailed`, never "try again". */
    val failure_refused: Boolean = false,
    /**
     * Spec 096 F8: the failure sent nothing, was no refusal, and its answer is
     * still held — the receipt offers Try again ([SignEvent.RetryTapped]) beside
     * Done, which answers the page.
     */
    val failure_retryable: Boolean = false,
    val notice: SignNotice? = null,
    val global_chain_id: Int = 0,
    val blocked: SignBlockedView? = null,
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

    /**
     * Spec 082 RJ1: the write-ahead record for request [id] is on disk — the
     * shell may POST [user_op_hash] now, and only now (after the asker check,
     * immediately before the POST). Answered `responded`.
     */
    @Serializable
    @SerialName("clear_to_post")
    data class ClearToPost(val id: String, val user_op_hash: String) : SignOperation()

    /** Spec 082 RJ1: a write-ahead record whose op is proven never sent. Answered `record_updated`. */
    @Serializable
    @SerialName("delete_record")
    data class DeleteRecord(val record_id: String) : SignOperation()

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

    /** Spec 096 F8: Try again on a failure that sent nothing. */
    @Serializable
    @SerialName("retry_tapped")
    data object RetryTapped : SignEvent()

    @Serializable
    @SerialName("funding_complete_tapped")
    data object FundingCompleteTapped : SignEvent()

    @Serializable
    @SerialName("funding_cancelled")
    data object FundingCancelled : SignEvent()

    @Serializable
    @SerialName("op_submitted")
    data class OpSubmitted(
        val id: String,
        val user_op_hash: String,
        val now_ms: Double,
        /** Spec 082 RA3: the reply was lost; [user_op_hash] is the local one. */
        val maybe_sent: Boolean = false,
        /** The head read before the first submit POST; `null` = unknown. */
        val submit_block: Long? = null,
    ) : SignEvent()

    /**
     * Spec 082 RJ1: the op for request [id] is signed and its hash computed —
     * after the passkey, the local hash and the head read, BEFORE any POST.
     * The core writes the record ahead and answers `ClearToPost` once it is on
     * disk.
     */
    @Serializable
    @SerialName("op_signed")
    data class OpSigned(
        val id: String,
        val user_op_hash: String,
        val submit_block: Long? = null,
        val now_ms: Double,
    ) : SignEvent()

    /**
     * Spec 082 RJ4: the tracker's entry for the in-flight op changed. Past
     * `OpSubmitted` and still unanswered, the core answers the page from it
     * (Confirmed / Dropped with a tx hash → the tx hash; Rejected → refused;
     * NotSent → not sent); anything else waits.
     */
    @Serializable
    @SerialName("op_tracked")
    data class OpTracked(
        val user_op_hash: String,
        val status: app.getvela.wallet.feature.send.core.TrackStatus,
        val tx_hash: String? = null,
        val now_ms: Double,
    ) : SignEvent()

    /** Spec 082 RA9: the passkey (or the Trusted Signer's page) is up for request [id]. */
    @Serializable
    @SerialName("ceremony_started")
    data class CeremonyStarted(val id: String) : SignEvent()

    /** It returned a signature. */
    @Serializable
    @SerialName("ceremony_done")
    data class CeremonyDone(val id: String) : SignEvent()

    @Serializable
    @SerialName("transport_dropped")
    data class TransportDropped(val transport_id: String) : SignEvent()
}
