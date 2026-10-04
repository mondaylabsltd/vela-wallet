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
    /** Spec 099 R2: something of this tab's is open — its engine is never suspended. */
    val busy: Boolean = false,
    val page: DbrPageState = DbrPageState.Blank,
    val provider: DbrProviderState = DbrProviderState.Pending,
    val open_requests: Int = 0,
    /** Failures worth attention among the tab's last few requests. */
    val failed_recent: Int = 0,
    /** The latest of them — the status entry's line (spec 099 FR-014). */
    val last_failure: DbrFailureNote? = null,
)

// -- spec 099: the request record and its words (`dapp_record.rs`) ------------

/** What kind of request a row is — which layers it can reach. */
@Serializable
enum class DbrRequestClass {
    @SerialName("local") Local,

    @SerialName("consent") Consent,

    @SerialName("read") Read,

    @SerialName("relay_read") RelayRead,

    @SerialName("signing") Signing,
}

/** Which layer decided how a request ended. */
@Serializable
enum class DbrLayer {
    @SerialName("browser") Browser,

    @SerialName("provider") Provider,

    @SerialName("wallet") Wallet,

    @SerialName("network") Network,

    @SerialName("relay") Relay,

    @SerialName("sheet") Sheet,

    @SerialName("signer") Signer,
}

/**
 * Why a request ended in an error. [key] is the line that says it — the
 * core's `DbrReason::key` spelled out (no UniFFI export carries it for a row;
 * `DbrFailureNote.key` carries it for the status line). `tab_closed` has no
 * line of its own: a closed tab has no panel, so it reads as navigated away.
 */
@Serializable
enum class DbrReason(val key: String) {
    @SerialName("navigated_away") NavigatedAway(REASON + "navigatedAway"),

    @SerialName("page_crashed") PageCrashed(REASON + "pageCrashed"),

    @SerialName("tab_closed") TabClosed(REASON + "navigatedAway"),

    @SerialName("wallet_withdrawn") WalletWithdrawn(REASON + "walletWithdrawn"),

    @SerialName("insecure_origin") InsecureOrigin(REASON + "insecureOrigin"),

    @SerialName("not_connected") NotConnected(REASON + "notConnected"),

    @SerialName("account_mismatch") AccountMismatch(REASON + "accountMismatch"),

    @SerialName("no_account") NoAccount(REASON + "noAccount"),

    @SerialName("consent_busy") ConsentBusy(REASON + "consentBusy"),

    @SerialName("unsupported_method") UnsupportedMethod(REASON + "unsupportedMethod"),

    @SerialName("unknown_chain") UnknownChain(REASON + "unknownChain"),

    @SerialName("bad_params") BadParams(REASON + "badParams"),

    @SerialName("too_many_reads") TooManyReads(REASON + "tooManyReads"),

    @SerialName("unknown_batch") UnknownBatch(REASON + "unknownBatch"),

    @SerialName("wallet_refused") WalletRefused(REASON + "walletRefused"),

    @SerialName("no_endpoint") NoEndpoint(REASON + "noEndpoint"),

    @SerialName("timed_out") TimedOut(REASON + "timedOut"),

    @SerialName("rate_limited") RateLimited(REASON + "rateLimited"),

    @SerialName("endpoint_error") EndpointError(REASON + "endpointError"),

    @SerialName("reverted") Reverted(REASON + "reverted"),

    @SerialName("rejected_by_person") RejectedByPerson(REASON + "rejectedByPerson"),

    @SerialName("relay_refused") RelayRefused(REASON + "relayRefused"),

    @SerialName("relay_unreachable") RelayUnreachable(REASON + "relayUnreachable"),

    @SerialName("not_confirmed_yet") NotConfirmedYet(REASON + "notConfirmedYet"),

    @SerialName("relay_failed") RelayFailed(REASON + "relayFailed"),

    @SerialName("signer_unavailable") SignerUnavailable(REASON + "signerUnavailable"),

    @SerialName("signer_not_discoverable") SignerNotDiscoverable(REASON + "signerNotDiscoverable"),

    @SerialName("signer_failed") SignerFailed(REASON + "signerFailed"),

    /** Spec 100: the network a page asked to add lacks Vela's contracts (4902) — Settings' own line. */
    @SerialName("not_compatible") NotCompatible("addToken.errorNotCompatible"),

    /** Spec 100: not in the catalog, and no RPC the page gave answers for it (-32602). */
    @SerialName("bad_rpc") BadRpc(REASON + "badRpc"),
}

private const val REASON = "componentsUi.browserStatus.reason."

@Serializable
enum class DbrOutcome {
    @SerialName("open") Open,

    @SerialName("answered") Answered,

    @SerialName("failed") Failed,
}

/** The browser layer, as the wallet sees it; [key] is `componentsUi.browserStatus.page.*` (`DbrPageState::key`). */
@Serializable
enum class DbrPageState(val key: String) {
    @SerialName("blank") Blank("componentsUi.browserStatus.page.blank"),

    @SerialName("loading") Loading("componentsUi.browserStatus.page.loading"),

    @SerialName("ready") Ready("componentsUi.browserStatus.page.ready"),

    @SerialName("crashed") Crashed("componentsUi.browserStatus.page.crashed"),
}

/** Was the wallet offered to the page showing now; [key] is `componentsUi.browserStatus.provider.*` (`DbrProviderState::key`). */
@Serializable
enum class DbrProviderState(val key: String) {
    @SerialName("pending") Pending("componentsUi.browserStatus.provider.pending"),

    @SerialName("offered") Offered("componentsUi.browserStatus.provider.offered"),

    @SerialName("insecure_origin") InsecureOrigin("componentsUi.browserStatus.provider.insecureOrigin"),

    @SerialName("no_hello") NoHello("componentsUi.browserStatus.provider.noHello"),
}

/** Why a read came back with no body (spec 099 FR-009). */
@Serializable
enum class DbrReadFailure {
    @SerialName("no_endpoint") NoEndpoint,

    @SerialName("timed_out") TimedOut,

    @SerialName("rate_limited") RateLimited,
}

/** The status entry's one line about the latest trouble; [key] is the core's line for [reason]. */
@Serializable
data class DbrFailureNote(
    val layer: DbrLayer,
    val reason: DbrReason,
    val method: String,
    val key: String,
)

/** One request a page sent. No params, results or addresses — ever. */
@Serializable
data class DbrRequestRow(
    val id: String,
    val method: String,
    @SerialName("class") val kind: DbrRequestClass,
    /** The shell's clock; `0` = unknown. */
    val started_ms: Double = 0.0,
    val ended_ms: Double? = null,
    val outcome: DbrOutcome = DbrOutcome.Open,
    /** The EIP-1193 / JSON-RPC error code, for a failure (`i64`). */
    val code: Long? = null,
    val layer: DbrLayer? = null,
    val reason: DbrReason? = null,
)

/** The inspected tab, whole (spec 099 FR-014): the status panel draws this, and [report] is what Copy copies. */
@Serializable
data class DbrInspectorView(
    val tab: String,
    val origin: String? = null,
    val page: DbrPageState = DbrPageState.Blank,
    val provider: DbrProviderState = DbrProviderState.Pending,
    val connected: Boolean = false,
    val chain_id: Int = 1,
    /** Oldest first. */
    val rows: List<DbrRequestRow> = emptyList(),
    val report: String = "",
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
    /** Spec 099: the inspected tab's whole record, while its status panel is open (`inspector_opened`). */
    val inspector: DbrInspectorView? = null,
    /** Spec 100: the add-network request on Vela's sheet, by tab and id. */
    val adding_network: DbrSigningView? = null,
)

/**
 * Spec 100: what a page's `wallet_addEthereumChain` asks for, as the core read
 * it (`dapp_rpc::DappChainAsk`). Carried from the browser machine to
 * `network_admin` untouched — never interpreted here.
 */
@Serializable
data class DappChainAsk(
    val chain_id: Int,
    val chain_name: String? = null,
    val native_symbol: String? = null,
    val rpc_urls: List<String> = emptyList(),
    val refused_rpc_urls: Int = 0,
    val explorer_url: String? = null,
)

/** Spec 100: how a page's add-network request ended (`dapp_rpc::DappAddOutcome`) — carried, never read. */
@Serializable
sealed class DappAddOutcome {
    @Serializable
    @SerialName("added")
    data class Added(val chain_id: Int) : DappAddOutcome()

    @Serializable
    @SerialName("declined")
    data object Declined : DappAddOutcome()

    @Serializable
    @SerialName("not_compatible")
    data object NotCompatible : DappAddOutcome()

    @Serializable
    @SerialName("bad_rpc")
    data object BadRpc : DappAddOutcome()

    @Serializable
    @SerialName("busy")
    data object Busy : DappAddOutcome()
}

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
        /** Spec 099 FR-008: the answer is owed by then (ms), whatever the endpoints are doing. `0` = none. */
        val deadline_ms: Double = 0.0,
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

    /** Spec 100: hand this request to `network_admin`'s add-network sheet; answered once with [DbrEvent.AddNetworkAnswered]. */
    @Serializable
    @SerialName("forward_to_add_network")
    data class ForwardToAddNetwork(val tab: String, val id: String, val origin: String, val ask: DappChainAsk) : DbrOperation()

    /** Spec 100: the page that asked is gone — close its add-network sheet, unanswered. */
    @Serializable
    @SerialName("cancel_add_network")
    data class CancelAddNetwork(val tab: String, val id: String) : DbrOperation()

    @Serializable
    @SerialName("save_connection_record")
    data class SaveConnectionRecord(val address: String, val chain_id: Int, val origin: String) : DbrOperation()

    /** Spec 099 FR-015: one line per request end and tab state change, written as is. */
    @Serializable
    @SerialName("log")
    data class Log(val line: String) : DbrOperation()
}

@Serializable
sealed class DbrShellResult {
    @Serializable
    @SerialName("sites_listed")
    data class SitesListed(val sites: List<DbrStoredSite>) : DbrShellResult()

    /**
     * The JSON-RPC body, or `null` when nothing answered — and then [failure]
     * says why (spec 099 FR-009; `null` reads as no endpoint). [now_ms] is the
     * shell's clock: the core owns none.
     */
    @Serializable
    @SerialName("read_answered")
    data class ReadAnswered(
        val body_json: String? = null,
        val now_ms: Double = 0.0,
        val failure: DbrReadFailure? = null,
    ) : DbrShellResult()

    @Serializable
    @SerialName("user_op_resolved")
    data class UserOpResolved(val tx_hash: String? = null, val now_ms: Double = 0.0) : DbrShellResult()

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

    /** Settings' debug mode (spec 091), stated at start and on every change. */
    @Serializable
    @SerialName("debug_mode_changed")
    data class DebugModeChanged(val on: Boolean) : DbrEvent()

    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val address: String, val now_ms: Double) : DbrEvent()

    /** [now_ms] (spec 099): when the request arrived, for its row in the record — the shell's clock, ms since the epoch. */
    @Serializable
    @SerialName("page_message")
    data class PageMessage(
        val tab: String,
        val frame_origin: String,
        val is_main_frame: Boolean,
        val message_json: String,
        val now_ms: Double = 0.0,
    ) : DbrEvent()

    @Serializable
    @SerialName("navigation_started")
    data class NavigationStarted(val tab: String, val url: String, val now_ms: Double = 0.0) : DbrEvent()

    @Serializable
    @SerialName("load_finished")
    data class LoadFinished(val tab: String, val url: String, val now_ms: Double = 0.0) : DbrEvent()

    @Serializable
    @SerialName("tab_closed")
    data class TabClosed(val tab: String, val now_ms: Double = 0.0) : DbrEvent()

    @Serializable
    @SerialName("renderer_gone")
    data class RendererGone(val tab: String, val now_ms: Double = 0.0) : DbrEvent()

    @Serializable
    @SerialName("consent_approved")
    data class ConsentApproved(val now_ms: Double) : DbrEvent()

    /** An object since spec 099 (it carries the clock), no longer a unit variant. */
    @Serializable
    @SerialName("consent_rejected")
    data class ConsentRejected(val now_ms: Double = 0.0) : DbrEvent()

    /** Spec 099 FR-014: [tab]'s status panel opened — the view carries its whole record until it closes. */
    @Serializable
    @SerialName("inspector_opened")
    data class InspectorOpened(val tab: String) : DbrEvent()

    @Serializable
    @SerialName("inspector_closed")
    data object InspectorClosed : DbrEvent()

    @Serializable
    @SerialName("site_chain_picked")
    data class SiteChainPicked(val origin: String, val chain_id: Int) : DbrEvent()

    @Serializable
    @SerialName("revoke_requested")
    data class RevokeRequested(val origin: String) : DbrEvent()

    @Serializable
    @SerialName("revoke_all")
    data object RevokeAll : DbrEvent()

    /** Spec 100: the add-network sheet's ending for a forwarded request (`network_admin`'s `dapp_add_settled`). */
    @Serializable
    @SerialName("add_network_answered")
    data class AddNetworkAnswered(
        val tab: String,
        val id: String,
        val outcome: DappAddOutcome,
        val now_ms: Double = 0.0,
    ) : DbrEvent()

    @Serializable
    @SerialName("signing_answered")
    data class SigningAnswered(
        val tab: String,
        val id: String,
        val payload: SignResponsePayload,
        val user_op_hash: String? = null,
        val now_ms: Double = 0.0,
    ) : DbrEvent()
}
