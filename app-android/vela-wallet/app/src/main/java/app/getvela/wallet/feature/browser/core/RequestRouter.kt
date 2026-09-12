package app.getvela.wallet.feature.browser.core

import org.json.JSONArray
import org.json.JSONObject

/**
 * Answers a forwarded request the shell routes itself (spec 044 T023; the
 * desktop's `page.rs` `Route::State / Switch / Ack / Read / Unsupported`
 * arms). A signature goes to the signing controller (phase 4); everything
 * else is answered here — a wallet fact from the browser's chain, a read
 * through the person's own pool, an acknowledgement, or a refusal.
 */
class RequestRouter(
    private val ports: Ports,
) {
    interface Ports {
        /** The chain the browser is on. */
        fun browserChain(): Int

        /** The chains this wallet has (the settings machine's rows). */
        fun knownChains(): List<Int>

        /** The browser moves to a known chain: remembered, and the permissions machine told. */
        fun switchChain(chainId: Int)

        /** `eth_*` through the pool: the JSON-RPC body (`result` or `error`), or `null` when nothing answered. */
        suspend fun poolCall(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject?

        fun respond(id: String, json: JSONObject)

        /** A signature request: the signing machines' (phase 4). */
        fun sign(id: String, method: String, paramsJson: String, origin: String)

        /**
         * A user-operation hash this wallet submitted for a page: the page
         * was answered with it as its "transaction hash" (the web's
         * non-blocking rule), so its receipt lookups are translated —
         * `null` when the hash is not one of ours.
         */
        suspend fun receiptFor(userOpHash: String): Receipt?
    }

    sealed class Receipt {
        data object Pending : Receipt()
        data class Landed(val txHash: String) : Receipt()
    }

    suspend fun route(id: String, method: String, paramsJson: String, origin: String) {
        when (val route = DappRpc.route(method)) {
            DappRpc.Route.Sign -> ports.sign(id, method, paramsJson, origin)
            // The chain the wallet is on, in the two notations the two methods
            // are specified in. `net_version` is decimal — a hex answer there
            // is a string comparison every dApp fails.
            DappRpc.Route.State -> ports.respond(
                id,
                BrowserExecutor.resultJson(id, if (method == "net_version") ports.browserChain().toString() else DappRpc.hexChainId(ports.browserChain())),
            )
            DappRpc.Route.Switch -> {
                val wanted = DappRpc.switchChainParam(paramsJson)
                when {
                    wanted == null -> ports.respond(id, BrowserExecutor.errorJson(id, -32602, "Invalid chainId"))
                    wanted !in ports.knownChains() -> ports.respond(id, BrowserExecutor.errorJson(id, 4902, "Unrecognized chain ID"))
                    else -> {
                        ports.switchChain(wanted)
                        ports.respond(id, BrowserExecutor.resultJson(id, null))
                    }
                }
            }
            // Acknowledged, and nothing changed. A network or a token is added
            // in this wallet's own settings, by a person looking at it — never
            // because a page asked while it had the floor.
            DappRpc.Route.Ack -> ports.respond(id, BrowserExecutor.resultJson(id, null))
            is DappRpc.Route.Read -> {
                var params = runCatching { JSONArray(paramsJson) }.getOrElse { JSONArray() }
                // A receipt asked for by a user-operation hash we answered
                // with: pending → nothing yet; landed → the node's receipt
                // for the real transaction (spec 028's translation).
                if (method == "eth_getTransactionReceipt" || method == "eth_getTransactionByHash") {
                    val asked = params.optString(0)
                    when (val receipt = asked.takeIf { it.startsWith("0x") }?.let { ports.receiptFor(it) }) {
                        RequestRouter.Receipt.Pending -> { ports.respond(id, BrowserExecutor.resultJson(id, null)); return }
                        is RequestRouter.Receipt.Landed -> params = JSONArray().put(receipt.txHash)
                        null -> Unit
                    }
                }
                val body = ports.poolCall(ports.browserChain(), method, params, route.bundler)
                val error = body?.optJSONObject("error")
                when {
                    body == null -> ports.respond(id, BrowserExecutor.errorJson(id, -32603, "No endpoint answered"))
                    error != null -> ports.respond(id, BrowserExecutor.errorJson(id, error.optInt("code", -32603), error.optString("message").ifBlank { "The node refused this call" }))
                    else -> ports.respond(id, BrowserExecutor.resultJson(id, body.opt("result")))
                }
            }
            // 4900 and not 4001: the person did not decline, and a dApp that
            // reads a decline where there was none tells them they refused
            // something they never saw.
            DappRpc.Route.Unsupported -> ports.respond(id, BrowserExecutor.errorJson(id, 4900, "Vela cannot answer $method yet"))
        }
    }
}
