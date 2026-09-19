package app.getvela.wallet

import app.getvela.wallet.feature.send.core.RelayPort
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcResult
import org.json.JSONObject

/**
 * The relay and the chain, scripted (spec 043): a queue of bodies per JSON-RPC
 * method, REST answers per URL, and a log of every call in order. Shared by
 * the relay client's own tests and the machine tests that drive a whole send
 * through it.
 */
class FakeRelayPort : RelayPort {
    val rpc = HashMap<String, ArrayDeque<RpcResult>>()
    val rest = HashMap<String, RestAnswer>()
    /** Written by the machines' threads, counted by the test's: never a plain list. */
    val calls = java.util.concurrent.CopyOnWriteArrayList<String>()
    var base: String? = "https://relay.test"

    /** A default per method, served when the queue for it is empty. */
    val defaults = HashMap<String, (List<Any?>) -> RpcResult>()

    fun answer(method: String, vararg bodies: RpcResult) {
        rpc.getOrPut(method) { ArrayDeque() }.addAll(bodies)
    }

    fun always(method: String, body: (List<Any?>) -> RpcResult) {
        defaults[method] = body
    }

    override suspend fun call(chainId: Int, method: String, params: List<Any?>, kind: RpcKind): RpcResult {
        calls += "$kind:$method"
        return rpc[method]?.removeFirstOrNull()
            ?: defaults[method]?.invoke(params)
            ?: RpcResult.Failed(rateLimited = false)
    }

    override suspend fun bundlerBase(chainId: Int): String? = base

    override suspend fun bestRpcUrl(chainId: Int): String? = "https://rpc.test"

    override suspend fun restGet(url: String, xRpcUrl: String?): RestAnswer {
        calls += "GET:$url"
        return rest[url] ?: RestAnswer.Failed
    }

    companion object {
        fun body(result: Any?): RpcResult = RpcResult.Body(JSONObject().put("result", result))

        fun error(message: String): RpcResult =
            RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32000).put("message", message)))
    }
}
