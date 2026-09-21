package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.feature.signing.core.SignResponsePayload
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `dapp_browser` machine's wire (spec 070) — mirrors of
 * `rust/crates/vela-core/src/app/dapp_browser.rs`, checked against the ts-rs
 * mirrors by `CoreWireDriftTest`. Views may be subsets; operations, results
 * and closed families are exhaustive.
 */

/** What an origin has been given (`vela.perm.<origin>`). `granted_at_ms` is `f64` in the core. */
@Serializable
data class DpermGrant(
    val origin: String,
    val address: String,
    val chain_id: Int,
    val granted_at_ms: Double,
)

/** One stored site: its grant and its chain, either of which may be absent. */
@Serializable
data class DbrStoredSite(
    val origin: String,
    val grant: DpermGrant? = null,
    val chain_id: Int? = null,
)

@Serializable
data class DbrConsentView(
    val tab: String = "",
    val origin: String = "",
    val methods: List<String> = emptyList(),
    val address: String? = null,
    val chain_id: Int = 1,
)

@Serializable
data class DbrTabView(
    val tab: String,
    val origin: String? = null,
    val connected_address: String? = null,
    val chain_id: Int = 1,
    val secure: Boolean = false,
    val crashed: Boolean = false,
)

@Serializable
data class DbrSiteView(
    val origin: String,
    val address: String,
    val chain_id: Int,
    val granted_at_ms: Double,
)

@Serializable
data class DbrSigningView(val tab: String, val id: String)

@Serializable
data class DbrView(
    val ready: Boolean = false,
    val consent: DbrConsentView? = null,
    val tabs: List<DbrTabView> = emptyList(),
    val sites: List<DbrSiteView> = emptyList(),
    val signing: DbrSigningView? = null,
    val queued_signing: Int = 0,
)

@Serializable
sealed class DbrOperation {
    @Serializable
    @SerialName("list_sites")
    data object ListSites : DbrOperation()

    @Serializable
    @SerialName("write_grant")
    data class WriteGrant(val grant: DpermGrant) : DbrOperation()

    @Serializable
    @SerialName("remove_grant")
    data class RemoveGrant(val origin: String) : DbrOperation()

    @Serializable
    @SerialName("write_site_chain")
    data class WriteSiteChain(val origin: String, val chain_id: Int) : DbrOperation()

    @Serializable
    @SerialName("deliver")
    data class Deliver(val tab: String, val doc: String, val message_json: String) : DbrOperation()

    @Serializable
    @SerialName("read")
    data class Read(
        val tab: String,
        val id: String,
        val chain_id: Int,
        val method: String,
        val params_json: String,
        val bundler: Boolean,
    ) : DbrOperation()

    @Serializable
    @SerialName("resolve_user_op")
    data class ResolveUserOp(val chain_id: Int, val user_op_hash: String) : DbrOperation()

    @Serializable
    @SerialName("forward_to_signing")
    data class ForwardToSigning(
        val tab: String,
        val id: String,
        val method: String,
        val params_json: String,
        val origin: String,
        val chain_id: Int,
        val granted_address: String,
    ) : DbrOperation()

    @Serializable
    @SerialName("cancel_signing")
    data class CancelSigning(val tab: String, val id: String) : DbrOperation()

    @Serializable
    @SerialName("save_connection_record")
    data class SaveConnectionRecord(val address: String, val chain_id: Int, val origin: String) : DbrOperation()
}

@Serializable
sealed class DbrShellResult {
    @Serializable
    @SerialName("sites_listed")
    data class SitesListed(val sites: List<DbrStoredSite>) : DbrShellResult()

    @Serializable
    @SerialName("read_answered")
    data class ReadAnswered(val body_json: String? = null) : DbrShellResult()

    @Serializable
    @SerialName("user_op_resolved")
    data class UserOpResolved(val tx_hash: String? = null) : DbrShellResult()

    @Serializable
    @SerialName("ack")
    data object Ack : DbrShellResult()
}

@Serializable
sealed class DbrEvent {
    @Serializable
    @SerialName("start")
    data object Start : DbrEvent()

    @Serializable
    @SerialName("networks_changed")
    data class NetworksChanged(val chain_ids: List<Int>) : DbrEvent()

    @Serializable
    @SerialName("accounts_updated")
    data class AccountsUpdated(val addresses: List<String>? = null) : DbrEvent()

    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val address: String, val now_ms: Double) : DbrEvent()

    @Serializable
    @SerialName("page_message")
    data class PageMessage(
        val tab: String,
        val frame_origin: String,
        val is_main_frame: Boolean,
        val message_json: String,
    ) : DbrEvent()

    @Serializable
    @SerialName("navigation_started")
    data class NavigationStarted(val tab: String, val url: String) : DbrEvent()

    @Serializable
    @SerialName("load_finished")
    data class LoadFinished(val tab: String, val url: String) : DbrEvent()

    @Serializable
    @SerialName("tab_closed")
    data class TabClosed(val tab: String) : DbrEvent()

    @Serializable
    @SerialName("renderer_gone")
    data class RendererGone(val tab: String) : DbrEvent()

    @Serializable
    @SerialName("consent_approved")
    data class ConsentApproved(val now_ms: Double) : DbrEvent()

    @Serializable
    @SerialName("consent_rejected")
    data object ConsentRejected : DbrEvent()

    @Serializable
    @SerialName("site_chain_picked")
    data class SiteChainPicked(val origin: String, val chain_id: Int) : DbrEvent()

    @Serializable
    @SerialName("revoke_requested")
    data class RevokeRequested(val origin: String) : DbrEvent()

    @Serializable
    @SerialName("revoke_all")
    data object RevokeAll : DbrEvent()

    @Serializable
    @SerialName("signing_answered")
    data class SigningAnswered(
        val tab: String,
        val id: String,
        val payload: SignResponsePayload,
        val user_op_hash: String? = null,
    ) : DbrEvent()
}
