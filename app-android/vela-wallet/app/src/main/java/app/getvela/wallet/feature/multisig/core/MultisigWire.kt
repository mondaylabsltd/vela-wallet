package app.getvela.wallet.feature.multisig.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject

/**
 * The multisig machines' shell wire — transcribed from
 * `rust/crates/vela-core/src/app/multisig_shell.rs` (mirror:
 * `app-web/vela-wallet/src/lib/multisig/generated/MultisigOperation.ts` and
 * `MultisigShellResult.ts`; docs/multisig-machines.md §1.1).
 *
 * All five machines (accounts, proposals, members, outside page, outside
 * session) ask the shell for the same operations and read the same answers,
 * so ONE executor ([MultisigExecutor]) serves them. Typed, so the executor's
 * `when` is exhaustive and a variant the core gains is a compile error here,
 * not an operation nobody answers (`CoreWireDriftTest`,
 * `scripts/check-native-wire.mjs` hold this file to the mirror).
 *
 * Parts the wire carries as `unknown` — JSON-RPC params, a SafeTx, a quote,
 * a PeerConnect message, a notice — stay [JsonElement]: the core reads them
 * back whole and the shell never reshapes them.
 */
@Serializable
sealed class MultisigOperation {
    @Serializable
    @SerialName("http")
    data class Http(
        val method: String,
        val url: String,
        val body: String? = null,
        val timeout_ms: Long = 15_000,
    ) : MultisigOperation()

    @Serializable
    @SerialName("rpc")
    data class Rpc(
        val chain_id: Long,
        val method: String,
        val params: JsonElement = JsonNull,
        val bound_ms: Long? = null,
    ) : MultisigOperation()

    @Serializable
    @SerialName("load_store")
    data class LoadStore(val keys: List<String> = emptyList()) : MultisigOperation()

    @Serializable
    @SerialName("write_store")
    data class WriteStore(val key: String, val value: String? = null) : MultisigOperation()

    @Serializable
    @SerialName("wait")
    data class Wait(val ms: Long = 0) : MultisigOperation()

    @Serializable
    @SerialName("now")
    data object Now : MultisigOperation()

    @Serializable
    @SerialName("random")
    data class Random(val bytes: Int = 0) : MultisigOperation()

    /** `purpose` is `MultisigPasskeyPurpose` (`kind`: approve / record), read by the shell's ports. */
    @Serializable
    @SerialName("passkey")
    data class Passkey(
        val owner: String,
        val chain_id: Long,
        val challenge: String,
        val purpose: JsonObject = JsonObject(emptyMap()),
    ) : MultisigOperation()

    @Serializable
    @SerialName("ask_risk")
    data class AskRisk(
        val chain_id: Long,
        val safe: String,
        val findings: List<JsonElement> = emptyList(),
    ) : MultisigOperation()

    /** `purpose` is `MultisigSendPurpose` (`kind`: execute / record); absent from an older machine. */
    @Serializable
    @SerialName("send_calls")
    data class SendCalls(
        val chain_id: Long,
        val from: String,
        val calls: List<JsonElement> = emptyList(),
        val fee: JsonElement = JsonNull,
        val purpose: JsonObject? = null,
    ) : MultisigOperation()

    @Serializable
    @SerialName("quote_calls")
    data class QuoteCalls(
        val chain_id: Long,
        val from: String,
        val calls: List<JsonElement> = emptyList(),
    ) : MultisigOperation()

    @Serializable
    @SerialName("peer_open")
    data class PeerOpen(
        val relay: String? = null,
        val invite: String? = null,
        val pairing: String? = null,
    ) : MultisigOperation()

    @Serializable
    @SerialName("peer_send")
    data class PeerSend(val room: String, val message: JsonElement = JsonNull) : MultisigOperation()

    @Serializable
    @SerialName("peer_receive")
    data class PeerReceive(val room: String) : MultisigOperation()

    @Serializable
    @SerialName("peer_close")
    data class PeerClose(
        val room: String,
        val cancel: Boolean = false,
        val end: Boolean = false,
    ) : MultisigOperation()

    @Serializable
    @SerialName("wallet_request")
    data class WalletRequest(val method: String, val params: JsonElement = JsonNull) : MultisigOperation()

    @Serializable
    @SerialName("simulate")
    data class Simulate(
        val chain_id: Long,
        val from: String,
        val calls: List<JsonElement> = emptyList(),
    ) : MultisigOperation()

    @Serializable
    @SerialName("recipient_risk")
    data class RecipientRisk(val chain_id: Long, val address: String) : MultisigOperation()

    @Serializable
    @SerialName("user_op_status")
    data class UserOpStatus(val chain_id: Long, val user_op_hash: String) : MultisigOperation()

    /** `notice` is a `MultisigNotice` (tag `kind`), routed by [MultisigHub.route]. */
    @Serializable
    @SerialName("notice")
    data class Notice(val notice: JsonObject = JsonObject(emptyMap())) : MultisigOperation()
}

/** What the shell answers: exactly one per operation. */
@Serializable
sealed class MultisigShellResult {
    @Serializable
    @SerialName("http_answered")
    data class HttpAnswered(val status: Int, val retry_after: String? = null, val body: String) : MultisigShellResult()

    @Serializable
    @SerialName("http_unreachable")
    data class HttpUnreachable(val detail: String) : MultisigShellResult()

    @Serializable
    @SerialName("rpc_result")
    data class RpcResult(val result: JsonElement = JsonNull) : MultisigShellResult()

    @Serializable
    @SerialName("rpc_error")
    data class RpcError(val code: Long? = null, val message: String) : MultisigShellResult()

    @Serializable
    @SerialName("rpc_unanswered")
    data class RpcUnanswered(val detail: String) : MultisigShellResult()

    @Serializable
    @SerialName("store_loaded")
    data class StoreLoaded(val entries: Map<String, String?> = emptyMap()) : MultisigShellResult()

    @Serializable
    @SerialName("store_written")
    data object StoreWritten : MultisigShellResult()

    @Serializable
    @SerialName("store_failed")
    data class StoreFailed(val detail: String) : MultisigShellResult()

    @Serializable
    @SerialName("waited")
    data object Waited : MultisigShellResult()

    @Serializable
    @SerialName("now")
    data class Now(val now_ms: Double) : MultisigShellResult()

    @Serializable
    @SerialName("random")
    data class Random(val hex: String) : MultisigShellResult()

    @Serializable
    @SerialName("asserted")
    data class Asserted(
        val credential_id: String,
        val authenticator_data: String,
        val client_data_json: String,
        val signature: String,
    ) : MultisigShellResult()

    /** The member's approval made on its trusted page, as the core accepted it. */
    @Serializable
    @SerialName("page_approved")
    data class PageApproved(val approval: JsonElement) : MultisigShellResult()

    @Serializable
    @SerialName("passkey_cancelled")
    data object PasskeyCancelled : MultisigShellResult()

    /** `detail`: the shell's failure ([MultisigShellFailure]) as JSON, or the platform's words. */
    @Serializable
    @SerialName("passkey_failed")
    data class PasskeyFailed(val detail: String) : MultisigShellResult()

    @Serializable
    @SerialName("risk_answered")
    data class RiskAnswered(val signs: Boolean) : MultisigShellResult()

    @Serializable
    @SerialName("simulated")
    data class Simulated(val reply: String? = null) : MultisigShellResult()

    @Serializable
    @SerialName("user_op_status")
    data class UserOpStatus(val status: String? = null, val tx_hash: String? = null) : MultisigShellResult()

    @Serializable
    @SerialName("recipient_facts")
    data class RecipientFacts(val is_contract: Boolean? = null, val first_time: Boolean? = null) : MultisigShellResult()

    @Serializable
    @SerialName("calls_sent")
    data class CallsSent(val user_op_hash: String, val tx_hash: String? = null) : MultisigShellResult()

    @Serializable
    @SerialName("calls_failed")
    data class CallsFailed(val detail: String, val maybe_sent: Boolean) : MultisigShellResult()

    @Serializable
    @SerialName("calls_cancelled")
    data object CallsCancelled : MultisigShellResult()

    @Serializable
    @SerialName("quoted")
    data class Quoted(val quote: JsonElement) : MultisigShellResult()

    @Serializable
    @SerialName("quote_failed")
    data class QuoteFailed(val detail: String) : MultisigShellResult()

    @Serializable
    @SerialName("peer_opened")
    data class PeerOpened(val room: String, val invite: String? = null, val pairing: String? = null) : MultisigShellResult()

    @Serializable
    @SerialName("peer_message")
    data class PeerMessage(val message: JsonElement) : MultisigShellResult()

    @Serializable
    @SerialName("peer_closed")
    data class PeerClosed(val reason: String) : MultisigShellResult()

    @Serializable
    @SerialName("wallet_answered")
    data class WalletAnswered(val result: JsonElement) : MultisigShellResult()

    @Serializable
    @SerialName("wallet_refused")
    data class WalletRefused(val code: Long? = null, val message: String) : MultisigShellResult()

    @Serializable
    @SerialName("ack")
    data object Ack : MultisigShellResult()
}
