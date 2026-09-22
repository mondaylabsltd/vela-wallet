package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import org.json.JSONObject

/**
 * The `dapp_permissions` machine's eight arms (spec 044 T023; the desktop's
 * `wallet/browser_host.rs`).
 *
 * ## What this file must never decide
 *
 * Whether an origin is connected, whether a frame may ask, whether a
 * request is a read or a signature, and which error code a refusal
 * carries. All of those are the core's, and every one of them is a
 * security rule. This performs: it reads and writes grants under the
 * desktop's key, answers the page in the wire's shapes, emits the
 * provider events, writes the "connected to" row, and hands a forwarded
 * request to whoever routes it.
 */
class BrowserExecutor(
    private val store: KeyValueStore,
    private val ports: Ports,
) {
    interface Ports {
        /** `{dir:"res", id, result}` or `{dir:"res", id, error:{code,message}}`, to the page that asked. */
        fun respond(id: String, json: JSONObject)

        /** `{dir:"evt", event, data}` to the connected page. */
        fun emit(json: JSONObject)

        /** Every forwarded request still open is answered with this error. */
        fun settleForwarded(code: Int, message: String)

        /** The feed's "connected to <site>" row (`type: "connect"`). */
        fun saveConnectionRecord(row: JSONObject)

        /** A request the core forwarded: a signature, a wallet fact, or a chain read. */
        fun forward(id: String, method: String, paramsJson: String, origin: String)
    }

    suspend fun perform(operation: DpermOperation): DpermShellResult = when (operation) {
        is DpermOperation.ReadGrant -> DpermShellResult.GrantRead(
            origin = operation.origin,
            grant = store.read(grantKey(operation.origin))?.let { raw ->
                // A parse or storage failure answers "no grant": an unreadable
                // grant is not a grant, and guessing one from a half-written
                // record would connect a site nobody connected.
                runCatching { Wire.json.decodeFromString(DpermGrant.serializer(), raw) }.getOrNull()
            },
        )
        is DpermOperation.WriteGrant -> {
            store.write(grantKey(operation.grant.origin), Wire.json.encodeToString(DpermGrant.serializer(), operation.grant))
            DpermShellResult.Ack
        }
        is DpermOperation.RemoveGrant -> {
            store.write(grantKey(operation.origin), "")
            DpermShellResult.Ack
        }
        is DpermOperation.Respond -> {
            ports.respond(operation.id, responseJson(operation.id, operation.payload))
            DpermShellResult.Ack
        }
        is DpermOperation.EmitEvent -> {
            ports.emit(eventJson(operation.event))
            DpermShellResult.Ack
        }
        is DpermOperation.SettleForwarded -> {
            ports.settleForwarded(operation.code, rejectMessage(operation.reason))
            DpermShellResult.Ack
        }
        is DpermOperation.SaveConnectionRecord -> {
            ports.saveConnectionRecord(connectionRow(operation.address, operation.chain_id, operation.origin, System.currentTimeMillis()))
            DpermShellResult.Ack
        }
        is DpermOperation.ForwardToSigning -> {
            ports.forward(operation.id, operation.method, operation.params_json, operation.origin)
            DpermShellResult.Ack
        }
    }

    fun neutralAnswer(operation: DpermOperation): DpermShellResult = when (operation) {
        is DpermOperation.ReadGrant -> DpermShellResult.GrantRead(operation.origin, null)
        else -> DpermShellResult.Ack
    }

    companion object {
        /** `vela.perm.<origin>` — the desktop's key; one document per origin. */
        fun grantKey(origin: String): String = "vela.perm.$origin"

        /** The wire's shapes; the core said WHICH shape. The words for a rejection are the shell's. */
        fun responseJson(id: String, payload: DpermRespondPayload): JSONObject = when (payload) {
            is DpermRespondPayload.Accounts -> JSONObject().put("dir", "res").put("id", id).put("result", org.json.JSONArray(payload.addresses))
            is DpermRespondPayload.Permissions -> JSONObject().put("dir", "res").put("id", id).put(
                "result",
                if (payload.granted) org.json.JSONArray().put(JSONObject().put("parentCapability", "eth_accounts")) else org.json.JSONArray(),
            )
            is DpermRespondPayload.Error -> errorJson(id, payload.code, rejectMessage(payload.reason))
        }

        fun resultJson(id: String, result: Any?): JSONObject =
            JSONObject().put("dir", "res").put("id", id).put("result", result ?: JSONObject.NULL)

        /**
         * `kind` is the core's own vocabulary, passed through when there is
         * one (spec 081, matching the web shell's `error.kind`). A refused
         * self-call answers `-32603` with the refused function as its message,
         * which tells a page WHAT but not WHY; the kind is the machine-readable
         * half, and a page that does not know the field simply ignores it.
         */
        fun errorJson(id: String, code: Int, message: String, kind: String? = null): JSONObject {
            val error = JSONObject().put("code", code).put("message", message)
            if (kind != null) error.put("kind", kind)
            return JSONObject().put("dir", "res").put("id", id).put("error", error)
        }

        /** An EIP-1193 event, in the envelope the provider already listens for. */
        fun eventJson(event: DpermPageEvent): JSONObject = when (event) {
            is DpermPageEvent.AccountsChanged -> JSONObject().put("dir", "evt").put("event", "accountsChanged").put("data", org.json.JSONArray(event.addresses))
            is DpermPageEvent.ChainChanged -> JSONObject().put("dir", "evt").put("event", "chainChanged").put("data", event.chain_id_hex)
            DpermPageEvent.Disconnect -> JSONObject().put("dir", "evt").put("event", "disconnect")
        }

        /** The shell's words for the core's reason (the desktop's table). */
        fun rejectMessage(reason: DpermRejectReason): String = when (reason) {
            DpermRejectReason.UnauthorizedFrame -> "Unauthorized frame"
            DpermRejectReason.NoAccountAvailable -> "No wallet account available"
            DpermRejectReason.ConsentBusy -> "Another connection request is open"
            DpermRejectReason.InsecureOrigin -> "Signing requires a secure origin"
            DpermRejectReason.UserRejected -> "User rejected the request"
            DpermRejectReason.NavigatedAway -> "The page navigated away"
            DpermRejectReason.BrowserClosed -> "The browser was closed"
            DpermRejectReason.NotConnected -> "This site is not connected"
            DpermRejectReason.StaleAuthorizedAddress -> "The authorized address changed"
        }

        /** The "Connected to <app>" row, field for field the other clients' (`buildConnectionRecord`). */
        fun connectionRow(address: String, chainId: Int, origin: String, nowMs: Long): JSONObject = JSONObject()
            .put("id", "dapp-$nowMs-connect")
            .put("userOpHash", "")
            .put("txHash", "")
            .put("from", address)
            .put("to", "")
            .put("value", "0")
            .put("symbol", "")
            .put("decimals", 0)
            .put("chainId", chainId)
            .put("timestamp", nowMs / 1000)
            .put("status", "confirmed")
            .put("type", "connect")
            .put("dappOrigin", origin)
    }
}
