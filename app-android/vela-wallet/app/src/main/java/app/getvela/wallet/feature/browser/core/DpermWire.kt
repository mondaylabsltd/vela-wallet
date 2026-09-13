package app.getvela.wallet.feature.browser.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `dapp_permissions` machine's wire (spec 044) — mirrors of
 * `rust/crates/vela-core/src/app/dapp_permissions.rs`, checked against the
 * ts-rs mirrors by `CoreWireDriftTest`. Views may be subsets; operations,
 * results and closed families are exhaustive.
 */

/** What an origin has been given. `granted_at_ms` is `f64` in the core. */
@Serializable
data class DpermGrant(
    val origin: String,
    val address: String,
    val chain_id: Int,
    val granted_at_ms: Double,
)

@Serializable
enum class DpermRejectReason {
    @SerialName("unauthorized_frame") UnauthorizedFrame,

    @SerialName("no_account_available") NoAccountAvailable,

    @SerialName("consent_busy") ConsentBusy,

    @SerialName("insecure_origin") InsecureOrigin,

    @SerialName("user_rejected") UserRejected,

    @SerialName("navigated_away") NavigatedAway,

    @SerialName("browser_closed") BrowserClosed,

    @SerialName("not_connected") NotConnected,

    @SerialName("stale_authorized_address") StaleAuthorizedAddress,
}

@Serializable
sealed class DpermRespondPayload {
    @Serializable
    @SerialName("accounts")
    data class Accounts(val addresses: List<String>) : DpermRespondPayload()

    @Serializable
    @SerialName("permissions")
    data class Permissions(val granted: Boolean) : DpermRespondPayload()

    @Serializable
    @SerialName("error")
    data class Error(val code: Int, val reason: DpermRejectReason) : DpermRespondPayload()
}

@Serializable
sealed class DpermPageEvent {
    @Serializable
    @SerialName("accounts_changed")
    data class AccountsChanged(val addresses: List<String>) : DpermPageEvent()

    @Serializable
    @SerialName("chain_changed")
    data class ChainChanged(val chain_id_hex: String) : DpermPageEvent()

    @Serializable
    @SerialName("disconnect")
    data object Disconnect : DpermPageEvent()
}

@Serializable
sealed class DpermPopupOutcome {
    @Serializable
    @SerialName("respond")
    data class Respond(val payload: DpermRespondPayload) : DpermPopupOutcome()

    @Serializable
    @SerialName("consent")
    data object Consent : DpermPopupOutcome()

    @Serializable
    @SerialName("reject")
    data class Reject(val code: Int, val reason: DpermRejectReason) : DpermPopupOutcome()

    @Serializable
    @SerialName("forward_to_signing")
    data class ForwardToSigning(val granted_address: String) : DpermPopupOutcome()
}

@Serializable
data class DpermConsentView(val origin: String = "", val methods: List<String> = emptyList())

@Serializable
data class DpermPopupView(val outcome: DpermPopupOutcome, val granted: List<String> = emptyList())

@Serializable
data class DpermView(
    val consent: DpermConsentView? = null,
    val connected_address: String? = null,
    val current_origin: String? = null,
    val popup: DpermPopupView? = null,
)

@Serializable
sealed class DpermOperation {
    @Serializable
    @SerialName("read_grant")
    data class ReadGrant(val origin: String) : DpermOperation()

    @Serializable
    @SerialName("write_grant")
    data class WriteGrant(val grant: DpermGrant) : DpermOperation()

    @Serializable
    @SerialName("remove_grant")
    data class RemoveGrant(val origin: String) : DpermOperation()

    @Serializable
    @SerialName("respond")
    data class Respond(val id: String, val payload: DpermRespondPayload) : DpermOperation()

    @Serializable
    @SerialName("emit_event")
    data class EmitEvent(val event: DpermPageEvent) : DpermOperation()

    @Serializable
    @SerialName("settle_forwarded")
    data class SettleForwarded(val code: Int, val reason: DpermRejectReason) : DpermOperation()

    @Serializable
    @SerialName("save_connection_record")
    data class SaveConnectionRecord(val address: String, val chain_id: Int, val origin: String) : DpermOperation()

    @Serializable
    @SerialName("forward_to_signing")
    data class ForwardToSigning(val id: String, val method: String, val params_json: String, val origin: String) : DpermOperation()
}

@Serializable
sealed class DpermShellResult {
    @Serializable
    @SerialName("grant_read")
    data class GrantRead(val origin: String, val grant: DpermGrant? = null) : DpermShellResult()

    @Serializable
    @SerialName("ack")
    data object Ack : DpermShellResult()
}

@Serializable
sealed class DpermEvent {
    @Serializable
    @SerialName("provider_request")
    data class ProviderRequest(
        val id: String,
        val method: String,
        val params_json: String,
        val origin: String,
        val is_main_frame: Boolean,
    ) : DpermEvent()

    @Serializable
    @SerialName("consent_approved")
    data class ConsentApproved(val now_ms: Double) : DpermEvent()

    @Serializable
    @SerialName("consent_rejected")
    data object ConsentRejected : DpermEvent()

    @Serializable
    @SerialName("navigation_started")
    data class NavigationStarted(val url: String) : DpermEvent()

    @Serializable
    @SerialName("browser_closed")
    data object BrowserClosed : DpermEvent()

    @Serializable
    @SerialName("accounts_updated")
    data class AccountsUpdated(val addresses: List<String>? = null) : DpermEvent()

    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val address: String, val now_ms: Double) : DpermEvent()

    @Serializable
    @SerialName("chain_changed")
    data class ChainChanged(val chain_id: Int) : DpermEvent()

    @Serializable
    @SerialName("revoke_requested")
    data class RevokeRequested(val origin: String? = null) : DpermEvent()

    @Serializable
    @SerialName("popup_request")
    data class PopupRequest(
        val method: String,
        val grant: DpermGrant? = null,
        val current_addresses: List<String>? = null,
        val pinned_address: String? = null,
    ) : DpermEvent()
}
