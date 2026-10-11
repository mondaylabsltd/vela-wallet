package app.getvela.wallet

import app.getvela.wallet.feature.send.core.RelayPort
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withTimeoutOrNull
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

    /** How many calls have been logged: what [awaitCalls] wakes on. */
    private val logged = MutableStateFlow(0)

    private fun log(call: String) {
        calls += call
        logged.update { it + 1 }
    }

    /**
     * The log, once [test] holds of it — woken by each call, never by a
     * clock. Calls made by coroutines that run side by side arrive in no
     * order a test may count on: a test that waits for one of them and then
     * asserts another has already been made is asserting how the scheduler
     * ran. It waits here for everything it is about to assert, and a log
     * that never gets there fails with what was called.
     */
    suspend fun awaitCalls(what: String, timeoutMs: Long = 10_000, test: (List<String>) -> Boolean): List<String> =
        withTimeoutOrNull(timeoutMs) {
            logged.first { test(calls) }
            calls.toList()
        } ?: throw AssertionError("$what — called so far: $calls")
    var base: String? = "https://relay.test"

    /** A default per method, served when the queue for it is empty. */
    val defaults = HashMap<String, (List<Any?>) -> RpcResult>()

    fun answer(method: String, vararg bodies: RpcResult) {
        rpc.getOrPut(method) { ArrayDeque() }.addAll(bodies)
    }

    fun always(method: String, body: (List<Any?>) -> RpcResult) {
        defaults[method] = body
    }

    /**
     * When set, every call waits here before it is answered — how a test
     * holds a POST in flight (and sees what cancelling it does).
     */
    @Volatile
    var before: (suspend (method: String) -> Unit)? = null

    override suspend fun call(chainId: Int, method: String, params: List<Any?>, kind: RpcKind): RpcResult {
        log("$kind:$method")
        before?.invoke(method)
        return rpc[method]?.removeFirstOrNull()
            ?: defaults[method]?.invoke(params)
            ?: RpcResult.Failed(rateLimited = false)
    }

    override suspend fun bundlerBase(chainId: Int): String? = base

    override suspend fun bestRpcUrl(chainId: Int): String? = "https://rpc.test"

    override suspend fun restGet(url: String, xRpcUrl: String?): RestAnswer {
        log("GET:$url")
        return rest[url] ?: RestAnswer.Failed
    }

    companion object {
        fun body(result: Any?): RpcResult = RpcResult.Body(JSONObject().put("result", result))

        fun error(message: String): RpcResult =
            RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32000).put("message", message)))
    }
}
