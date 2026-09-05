package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import org.json.JSONObject
import uniffi.vela_core_uniffi.RpcPoolCore

/**
 * How this app reads a chain.
 *
 * The pool machine is a **pure router**: it decides which endpoint to try, in
 * what order, with what backoff, and when to give up on one — six-tier source
 * scoring, EMA latency, cooldowns, temporary and permanent bans, the
 * four-way error classification, the three-pass jittered sweep and the
 * failed-versus-rate-limited verdict, all in `rpc_pool.rs` and all tested
 * there.
 *
 * This class owns exactly the two things the core cannot have: **the fetch,
 * and the promise the caller is waiting on.**
 *
 * ```text
 *   caller ──call(chain, method, params)──►│ registers payload + deferred
 *                                          │ CallRequested ──► core
 *                                          │   ◄── json_rpc_post ──► transport
 *                                          │   ◄── conclude(verdict) ─────────
 *   caller ◄──────── the body, or a failure ─┘
 * ```
 *
 * **One per process.** The ban map, the per-endpoint statistics and the
 * fastest-endpoint winners are facts about the network that every caller
 * shares; two pools would mean two opinions about a dead endpoint, and the
 * second would keep hammering a host the first had given up on (research D2).
 */
class RpcPool(
    store: KeyValueStore,
    endpoints: RpcEndpointSource,
    scope: CoroutineScope,
    transport: RpcTransport = OkHttpTransport(),
) {

    /** One in-flight routed call: what to send, what came back, who is waiting. */
    private class Waiting(val payload: RpcPayload) {
        val bodies = ConcurrentHashMap<String, JSONObject>()
        val settled = CompletableDeferred<RpcCallVerdict>()
    }

    private val calls = ConcurrentHashMap<String, Waiting>()
    private val nextId = AtomicLong(0)

    private val registry = object : RpcPoolCallRegistry {
        override fun payload(callId: String): RpcPayload? = calls[callId]?.payload

        override fun keepBody(callId: String, url: String, body: JSONObject) {
            calls[callId]?.bodies?.put(url, body)
        }

        override fun settle(callId: String, verdict: RpcCallVerdict) {
            // `complete` on an already-completed deferred is a no-op, which is
            // the right answer for a duplicate conclusion rather than a crash.
            calls[callId]?.settled?.complete(verdict)
        }
    }

    private val executor = RpcPoolExecutor(store, registry, endpoints, transport)

    private val host = CoreHost(
        bridge = RpcPoolCore().asBridge(),
        scope = scope,
        initial = RpcPoolView(),
        serializer = RpcPoolView.serializer(),
        perform = JsonShell.perform(
            RpcOperation.serializer(),
            RpcShellResult.serializer(),
            executor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            RpcOperation.serializer(),
            RpcShellResult.serializer(),
            fallback = RpcShellResult.Persisted,
            answer = executor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("rpc.pool.fault", "core fault", error) },
    )

    /** Which chains the core currently considers broken, and which merely busy. */
    val view: StateFlow<RpcPoolView> = host.view

    /** Hand the machine the bans this device already knew about. */
    suspend fun start() {
        host.dispatch(RpcEvent.BansLoaded(executor.storedBans()), RpcEvent.serializer())
    }

    /**
     * Ask a chain a question.
     *
     * Suspends until the core concludes the call. The caller learns what
     * happened from [RpcResult]; it never learns which endpoint answered,
     * because that is not a fact it should be able to act on.
     */
    suspend fun call(
        chainId: Int,
        method: String,
        params: List<Any?> = emptyList(),
        kind: RpcKind = RpcKind.Rpc,
    ): RpcResult {
        val callId = "c${nextId.incrementAndGet()}"
        val waiting = Waiting(RpcPayload(method, params))
        calls[callId] = waiting
        try {
            host.dispatch(
                RpcEvent.CallRequested(
                    call_id = callId,
                    chain_id = chainId,
                    kind = kind,
                    method = method,
                    now_ms = System.currentTimeMillis().toDouble(),
                ),
                RpcEvent.serializer(),
            )
            return when (val verdict = waiting.settled.await()) {
                is RpcCallVerdict.Respond ->
                    RpcResult.Body(waiting.bodies[verdict.url] ?: JSONObject())
                is RpcCallVerdict.RangeCap ->
                    RpcResult.RangeCapped(verdict.max_span)
                is RpcCallVerdict.Failed ->
                    RpcResult.Failed(rateLimited = verdict.rate_limited)
                // These two answer a different question and never a `call`.
                is RpcCallVerdict.BundlerBase -> RpcResult.Failed(rateLimited = false)
                is RpcCallVerdict.BestRpcUrl -> RpcResult.Failed(rateLimited = false)
            }
        } finally {
            // Whatever happened, stop holding the body. A late `json_rpc_post`
            // for this id now finds no payload and reports the endpoint as
            // unreachable, which is what tells the core to stop routing for a
            // caller that has gone.
            calls.remove(callId)
        }
    }

    /** Forget every endpoint verdict — the settings screen's "clear caches". */
    fun invalidateAll() = host.dispatch(RpcEvent.InvalidateAll, RpcEvent.serializer())

    /** Re-read one chain's endpoints, after its configuration changed. */
    fun refreshChain(chainId: Int) =
        host.dispatch(RpcEvent.RefreshChain(chainId), RpcEvent.serializer())
}

/** What a routed call ended as, for a caller that must not see endpoints. */
sealed class RpcResult {
    data class Body(val json: JSONObject) : RpcResult()

    /** The endpoint capped the block range; ask again within `maxSpan`. */
    data class RangeCapped(val maxSpan: Double) : RpcResult()

    /**
     * Nothing answered.
     *
     * [rateLimited] is the difference between "this chain is down" and "come
     * back in a moment", and a screen that shows them the same way is telling a
     * person their wallet is broken when it is busy.
     */
    data class Failed(val rateLimited: Boolean) : RpcResult()
}
