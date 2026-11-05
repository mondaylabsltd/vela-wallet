package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.feature.send.core.relayRpcHeader
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.net.NetHealth
import app.getvela.wallet.core.net.VelaHttp
import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlin.random.Random
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `rpc_pool` core touches the outside world — and, through
 * it, the only place this app touches a chain.
 *
 * Seven operations and **not one routing decision**. Which endpoint to try,
 * when to ban it, whether a 429 means "broken" or "later", when to sweep the
 * pool again — all of that is `rpc_pool.rs`. This class posts, probes, draws a
 * random number, sleeps, and writes a ban list.
 *
 * Port source: `app-web/vela-wallet/src/lib/wallet/core/rpc-pool-executor.ts`.
 *
 * **The registry is the shell's half of a routed call.** The core carries no
 * params and no response bodies, so what must survive between "a call was
 * requested" and "here is the verdict" is held by whoever started the call —
 * reachable here, but owned by the facade. That keeps this class free of
 * module state and drivable from a test with its own registry.
 */
class RpcPoolExecutor(
    private val store: KeyValueStore,
    private val registry: RpcPoolCallRegistry,
    private val endpoints: RpcEndpointSource,
    /** Seam for tests: the transport, and the clock. */
    private val transport: RpcTransport = OkHttpTransport(),
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    private val random: Random = Random.Default,
) {

    suspend fun perform(operation: RpcOperation): RpcShellResult = when (operation) {

        is RpcOperation.LoadPoolConfig -> {
            val seeds = endpoints.forChain(operation.chain_id)
            RpcShellResult.PoolConfig(
                chain_id = operation.chain_id,
                rpc_endpoints = seeds.rpc,
                bundler_endpoints = seeds.bundler,
                own_only = seeds.ownOnly,
                now_ms = now(),
            )
        }

        is RpcOperation.JsonRpcPost -> {
            val payload = registry.payload(operation.call_id)
            val started = now()
            if (payload == null) {
                // The caller is already gone — a cancelled screen, a superseded
                // refresh. Report the endpoint as unreachable rather than
                // leaving the core waiting on a call nobody wants; it will route
                // on and conclude. Nothing was posted: `not_connected`, never a
                // "may have been delivered" (spec 082 RA1).
                RpcShellResult.PostOutcome(
                    call_id = operation.call_id,
                    url = operation.url,
                    outcome = RpcTransportOutcome.NotConnected,
                    latency_ms = 0.0,
                    now_ms = started,
                )
            } else {
                val result = transport.post(
                    url = operation.url,
                    method = operation.method,
                    params = payload.params,
                    xRpcUrl = operation.x_rpc_url,
                    timeoutMs = operation.timeout_ms,
                )
                result.body?.let { registry.keepBody(operation.call_id, operation.url, it) }
                health(result, payload.chainId)
                // Debug trace (spec 043 phase 4, device-found): which host,
                // which method, what came back. The pool's verdicts are the
                // core's; this is the only place the raw outcome is visible.
                VelaLog.event(
                    "rpc.post", operation.method,
                    "host" to runCatching { java.net.URI(operation.url).host }.getOrNull(),
                    "outcome" to when (val o = result.outcome) {
                        is RpcTransportOutcome.HttpError -> "http ${o.status}"
                        is RpcTransportOutcome.Response -> if (o.error == null) "ok" else "rpc error ${o.error.code}"
                        else -> o::class.simpleName
                    },
                    // How long the WIRE took. Without it a slow call cannot be
                    // told from one that waited its turn somewhere else.
                    "ms" to (now() - started).toLong(),
                )
                RpcShellResult.PostOutcome(
                    call_id = operation.call_id,
                    url = operation.url,
                    outcome = result.outcome,
                    latency_ms = now() - started,
                    now_ms = now(),
                )
            }
        }

        is RpcOperation.ProbeChainId -> {
            val started = now()
            val result = transport.post(
                url = operation.url,
                method = "eth_chainId",
                params = emptyList(),
                xRpcUrl = null,
                timeoutMs = operation.timeout_ms,
            )
            health(result, operation.chain_id)
            RpcShellResult.ChainIdProbed(
                chain_id = operation.chain_id,
                url = operation.url,
                // A hex quantity, or nothing. Parsing it is mechanical; what an
                // unexpected id MEANS is the core's (a mismatch is its verdict,
                // not this class's).
                reported = result.body?.optString("result")?.let(::parseChainId),
                latency_ms = now() - started,
                now_ms = now(),
            )
        }

        // The core owns the backoff curve and the cap; the shell owns only the
        // unpredictability, so two devices retrying the same dead endpoint do
        // not march in step.
        is RpcOperation.DrawJitter ->
            RpcShellResult.Jitter(call_id = operation.call_id, value = random.nextDouble())

        is RpcOperation.StartBackoff -> {
            delay(operation.delay_ms.toLong())
            RpcShellResult.BackoffElapsed(call_id = operation.call_id, now_ms = now())
        }

        // Best effort, and shared with the other clients: a device that learned
        // an endpoint is dead should agree with its siblings.
        is RpcOperation.PersistBans -> {
            store.write(
                KeyValueStore.Keys.RPC_BANNED,
                JSONArray().apply {
                    operation.entries.forEach { entry ->
                        put(
                            JSONObject()
                                .put("url", entry.url)
                                .put("bannedAt", entry.banned_at_ms.toLong())
                                .put("permanent", entry.permanent),
                        )
                    }
                }.toString(),
            )
            RpcShellResult.Persisted
        }

        is RpcOperation.Conclude -> {
            registry.settle(operation.call_id, operation.verdict)
            RpcShellResult.Concluded
        }
    }

    /**
     * What one POST says about the network, per source (spec 082 RJ14): an
     * answer of any kind reached a server; a transport failure did not; a
     * timeout says nothing either way (a slow server is not a missing
     * network) — [NetHealthFeed]. The chain is the source — one chain whose
     * nodes are all down is that chain's notice, never "offline" (G53).
     */
    private fun health(result: RpcPostResult, chainId: Int?) {
        when (NetHealthFeed.of(result.outcome, result.failure)) {
            true -> NetHealth.reached(chainId)
            false -> NetHealth.unreached(chainId)
            null -> Unit
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: RpcOperation): RpcShellResult = when (operation) {
        is RpcOperation.LoadPoolConfig ->
            RpcShellResult.PoolConfig(operation.chain_id, emptyList(), emptyList(), own_only = false, now_ms = now())
        // An escaped exception is a dead endpoint as far as the core is
        // concerned: report it and let the routing continue.
        is RpcOperation.JsonRpcPost -> RpcShellResult.PostOutcome(
            operation.call_id,
            operation.url,
            RpcTransportOutcome.Network,
            0.0,
            now(),
        )
        is RpcOperation.ProbeChainId ->
            RpcShellResult.ChainIdProbed(operation.chain_id, operation.url, null, 0.0, now())
        is RpcOperation.DrawJitter -> RpcShellResult.Jitter(operation.call_id, 0.0)
        is RpcOperation.StartBackoff -> RpcShellResult.BackoffElapsed(operation.call_id, now())
        is RpcOperation.PersistBans -> RpcShellResult.Persisted
        is RpcOperation.Conclude -> {
            registry.settle(operation.call_id, operation.verdict)
            RpcShellResult.Concluded
        }
    }

    /** The bans this device remembers, for the machine's boot event. */
    suspend fun storedBans(): List<RpcBanEntry> {
        val raw = store.read(KeyValueStore.Keys.RPC_BANNED) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { index ->
            val record = array.optJSONObject(index) ?: return@mapNotNull null
            val url = record.optString("url").ifEmpty { return@mapNotNull null }
            RpcBanEntry(
                url = url,
                banned_at_ms = record.optDouble("bannedAt", 0.0),
                permanent = record.optBoolean("permanent", false),
            )
        }
    }

    private fun parseChainId(hex: String): Int? {
        val body = hex.removePrefix("0x").ifEmpty { return null }
        return body.toLongOrNull(16)?.takeIf { it in 0..Int.MAX_VALUE.toLong() }?.toInt()
    }
}

/** What the shell must hold between a call starting and its verdict. */
interface RpcPoolCallRegistry {
    /** The payload to POST for `callId`; `null` once the call concluded. */
    fun payload(callId: String): RpcPayload?

    /** Remember the body `url` returned — verdicts name a URL, not a body. */
    fun keepBody(callId: String, url: String, body: JSONObject)

    /** A verdict arrived: settle whoever is waiting. */
    fun settle(callId: String, verdict: RpcCallVerdict)
}

/** [chainId]: the chain the call reads — the source network health counts it under (spec 082 RJ14). */
data class RpcPayload(val method: String, val params: List<Any?>, val chainId: Int? = null)

/** Where a chain's candidate endpoints come from (research D4). */
interface RpcEndpointSource {
    suspend fun forChain(chainId: Int): RpcSeeds
}

data class RpcSeeds(
    val rpc: List<RpcEndpointSeed> = emptyList(),
    val bundler: List<RpcEndpointSeed> = emptyList(),
    /** "Only use my node" is on for the chain (the network's saved `only_mine`). */
    val ownOnly: Boolean = false,
)

/** The POST, and nothing about what it means. */
interface RpcTransport {
    suspend fun post(
        url: String,
        method: String,
        params: List<Any?>,
        xRpcUrl: String?,
        timeoutMs: Int,
    ): RpcPostResult
}

data class RpcPostResult(
    val outcome: RpcTransportOutcome,
    val body: JSONObject? = null,
    /**
     * r14-3: how the POST failed, as the transport classed it (the core's
     * `TransportFailure` wire: `dns`, `connect_refused`, `connect_timeout`,
     * `read_timeout`, …); `null` for an answer, or a transport that does not say.
     */
    val failure: String? = null,
)

/**
 * r14-3: what one POST tells [NetHealth] (`true` reached, `false` did not,
 * `null` nothing), under the core's rule (`net_health`: "that one request
 * timing out is not fed — a slow server is not a missing network").
 *
 * The delivery verdict and this are different questions. A connect that
 * TIMED OUT is `not_connected` for delivery (nothing left the device), but it
 * says nothing about the device's network: a node that takes the connection
 * and never answers — or a proxy whose tunnel never opens — times the connect
 * out on a phone that is perfectly online. Fed as a miss, every hung node of
 * one network (and of a second, slow one) became "Offline" for a device whose
 * other networks answered, flipping with "Can't reach Arc Testnet" every
 * 8–16 s (r14-3). Only a failure that shows the way out is closed counts: the
 * name did not resolve, the connection was refused or unroutable, the
 * handshake or the socket broke.
 */
object NetHealthFeed {
    /** The transport's failure classes that are timeouts: they say nothing either way. */
    val TIMEOUTS = setOf("connect_timeout", "read_timeout", "timeout")

    fun of(outcome: RpcTransportOutcome, failure: String?): Boolean? = when {
        outcome is RpcTransportOutcome.Response || outcome is RpcTransportOutcome.HttpError || outcome == RpcTransportOutcome.NonJson -> true
        outcome == RpcTransportOutcome.Timeout -> null
        failure in TIMEOUTS -> null
        else -> false
    }
}

/**
 * The shipping transport.
 *
 * It classifies **mechanically** — a status, a shape, a timeout, a socket
 * failure — and never interprets. Whether HTTP 429 means "back off" or "ban" is
 * the core's call, and it can only make it if this reports what actually
 * happened.
 */
class OkHttpTransport : RpcTransport {

    /**
     * **The blocking call confines itself.**
     *
     * `execute()` blocks, so this suspend function is only safe on a dispatcher
     * that tolerates blocking — and nothing in its signature says so. Leaving
     * that to the caller made an unrelated file's dispatcher choice
     * (`AppContainer.wallet`) load-bearing: harmonising that scope with its
     * `Main.immediate` siblings would have crashed the read path with
     * `NetworkOnMainThreadException`. The confinement belongs here, where the
     * blocking is.
     */
    override suspend fun post(
        url: String,
        method: String,
        params: List<Any?>,
        xRpcUrl: String?,
        timeoutMs: Int,
    ): RpcPostResult = withContext(Dispatchers.IO) {
        val body = JSONObject()
            .put("jsonrpc", "2.0")
            .put("id", 1)
            .put("method", method)
            .put("params", JSONArray(params))
            .toString()
            .toRequestBody(JSON)

        val request = Request.Builder()
            .url(url)
            .post(body)
            // Spec 098 §5: a bundler call names the chain's RPC to the relay;
            // the core sets `x_rpc_url` on bundler calls only.
            .relayRpcHeader(xRpcUrl)
            .build()

        // r11-9: a write (a submit) goes on a connection of its own, never a
        // pooled one a server or a tunnel may have closed meanwhile: its
        // failure is then about this request alone. A read that met a dead
        // pooled connection before any answer came is asked once again on a
        // new one — what a browser does with a reused keep-alive socket — so
        // a stale connection is not an unreachable relay ("Couldn't check").
        val read = readOnly(method)
        val first = attempt(request, method, timeoutMs, fresh = !read)
        if (first !is Attempt.Failed) return@withContext (first as Attempt.Answered).result
        if (read && first.stages.reused && first.kind in STALE_CLASSES) {
            VelaLog.event("rpc.transport", "stale connection: asked again on a new one", "host" to request.url.host, "method" to method)
            when (val again = attempt(request, method, timeoutMs, fresh = true)) {
                is Attempt.Answered -> return@withContext again.result
                is Attempt.Failed -> return@withContext RpcPostResult(outcomeOfClass(again.kind), failure = again.kind)
            }
        }
        RpcPostResult(outcomeOfClass(first.kind), failure = first.kind)
    }

    private sealed interface Attempt {
        data class Answered(val result: RpcPostResult) : Attempt
        data class Failed(val kind: String, val stages: CallStages) : Attempt
    }

    /** One POST: its answer, or how it failed and how far it got. */
    private suspend fun attempt(request: Request, method: String, timeoutMs: Int, fresh: Boolean): Attempt {
        // r11-9: how far the request got — what tells a request that never
        // left whole (not sent) from one that may have been acted on.
        val stages = CallStages()
        val call = (if (fresh) VelaHttp.fresh else VelaHttp.client).newBuilder()
            .callTimeout(timeoutMs.toLong(), TimeUnit.MILLISECONDS)
            .eventListener(stages)
            .build()
            .newCall(request)

        return try {
            Attempt.Answered(postAnswer(call))
        } catch (failure: IOException) {
            val kind = failureClass(failure, stages)
            VelaLog.event(
                "rpc.transport", "failed",
                "host" to request.url.host, "port" to request.url.port, "method" to method,
                "class" to kind, "error" to failure.javaClass.simpleName, "message" to failure.message?.take(80),
                "reused" to stages.reused, "connected" to stages.connected, "written" to stages.bodyWritten, "fresh" to fresh,
            )
            Attempt.Failed(kind, stages)
        }
    }

    private suspend fun postAnswer(call: okhttp3.Call): RpcPostResult =
            call.await().use { response ->
                if (!response.isSuccessful) {
                    return@use RpcPostResult(RpcTransportOutcome.HttpError(response.code))
                }
                val text = response.body?.string().orEmpty()
                val json = runCatching { JSONObject(text) }.getOrNull()
                    ?: return@use RpcPostResult(RpcTransportOutcome.NonJson)
                val error = json.optJSONObject("error")?.let {
                    RpcErrorInfo(
                        code = if (it.isNull("code")) null else it.optInt("code"),
                        message = if (it.isNull("message")) null else it.optString("message"),
                    )
                }
                RpcPostResult(RpcTransportOutcome.Response(error), json)
            }

    /**
     * r11-9: what one call got to (okhttp's [okhttp3.EventListener]) — a
     * connection in hand (`connectEnd`, or a pooled one acquired), the
     * request's body written whole (`requestBodyEnd`). Read on failure only.
     */
    internal class CallStages : okhttp3.EventListener() {
        @Volatile var connected = false
        @Volatile var reused = false
        @Volatile var bodyWritten = false

        override fun connectEnd(call: okhttp3.Call, inetSocketAddress: java.net.InetSocketAddress, proxy: java.net.Proxy, protocol: okhttp3.Protocol?) {
            connected = true
        }

        override fun connectionAcquired(call: okhttp3.Call, connection: okhttp3.Connection) {
            if (!connected) reused = true
            connected = true
        }

        override fun requestBodyEnd(call: okhttp3.Call, byteCount: Long) {
            bodyWritten = true
        }

        override fun requestHeadersEnd(call: okhttp3.Call, request: Request) {
            // A request with no body is whole once its headers are written.
            if (request.body == null) bodyWritten = true
        }
    }

    /**
     * The call, SUSPENDED rather than blocked on.
     *
     * `execute()` inside `withContext(Dispatchers.IO)` looks confined and is
     * not: the pool's scope IS `Dispatchers.IO`, and `withContext` to the
     * dispatcher a coroutine is already on does not dispatch — it runs in
     * place. The driver starts every effect UNDISPATCHED on its one consumer,
     * so the blocking call ran ON the consumer, and the whole pool sent one
     * request at a time: ~95 calls queued behind a home-screen scan at half a
     * second each, and a sign-in's registry proof waited 30 s for its turn
     * (device-found 2026-09-19, `rpc.call slow … inFlight=96`). `enqueue`
     * suspends for real, so the consumer moves on and requests overlap.
     */
    private suspend fun okhttp3.Call.await(): okhttp3.Response =
        suspendCancellableCoroutine { continuation ->
            continuation.invokeOnCancellation { cancel() }
            enqueue(object : okhttp3.Callback {
                override fun onResponse(call: okhttp3.Call, response: okhttp3.Response) {
                    continuation.resume(response) { _, _, _ -> response.close() }
                }

                override fun onFailure(call: okhttp3.Call, e: IOException) {
                    continuation.resumeWithException(e)
                }
            })
        }

    internal companion object {
        val JSON = "application/json; charset=utf-8".toMediaType()

        /**
         * What a failed POST says about delivery (spec 082 RA1, contract §2).
         *
         * `NotConnected` only when nothing can have left the device — the name
         * did not resolve, the connection was refused or unroutable, the TLS
         * handshake failed, or the connect itself timed out. That is the one
         * fact that lets a submit say "not sent — try again". A call timeout
         * after connecting (okhttp's `InterruptedIOException("timeout")`) is a
         * `Timeout`: the request may have been written and acted on; so is any
         * other socket failure (`Network`). An `SSLHandshakeException` is
         * matched before its `IOException` parent, a connect-timeout
         * `SocketTimeoutException` before the `InterruptedIOException` it is.
         */
        fun outcomeOf(failure: IOException): RpcTransportOutcome = outcomeOfClass(failureClass(failure, null))

        /** r11-9: a pooled connection found dead — broken with no answer, the request written or not. */
        val STALE_CLASSES = setOf("after_write", "write_failed")

        /**
         * r11-9: a JSON-RPC method that only reads — asked once again on a
         * new connection when a pooled one was found dead. Anything else (a
         * submit, a signature's service call) is never sent twice by the
         * transport: it goes on a connection of its own, and what its failure
         * means is the core's.
         */
        fun readOnly(method: String): Boolean =
            method == "eth_call" || method.startsWith("eth_get") || method.startsWith("eth_estimate") ||
                method in setOf("eth_blockNumber", "eth_chainId", "eth_gasPrice", "eth_maxPriorityFeePerGas", "eth_feeHistory", "eth_supportedEntryPoints", "net_version") ||
                method.startsWith("pimlico_get") || method.startsWith("vela_get")

        /**
         * r11-9: the platform's failure as the core's CLASS
         * (`rpc_pool::TransportFailure`, vector `transport-failures.json`) —
         * what it means (not sent, may have been sent, timed out) is the
         * core's `transport_outcome_of`, never this file's. [stages]: how far
         * the call got (`null`: not known — the exception alone decides).
         *
         * - the name, a refused or unroutable connect, TLS: nothing left;
         * - a time-out before a connection: the connect's; once the request
         *   was written whole: the answer's; in between: it never left whole;
         * - any other break: after the body was written whole, `after_write`
         *   (it may have been acted on); before it, `write_failed`; with no
         *   connection ever made, a refused connect.
         */
        fun failureClass(failure: IOException, stages: CallStages?): String = when (failure) {
            is java.net.UnknownHostException -> "dns"
            is javax.net.ssl.SSLHandshakeException, is javax.net.ssl.SSLPeerUnverifiedException -> "tls"
            is java.net.ConnectException, is java.net.NoRouteToHostException, is java.net.PortUnreachableException -> "connect_refused"
            // A SocketTimeoutException IS an InterruptedIOException: matched first.
            is java.net.SocketTimeoutException -> when {
                stages == null -> if (failure.message.orEmpty().contains("connect", ignoreCase = true)) "connect_timeout" else "read_timeout"
                !stages.connected -> "connect_timeout"
                stages.bodyWritten -> "read_timeout"
                else -> "write_failed"
            }
            // okhttp's call timeout ("timeout"), at whatever stage the call was.
            is java.io.InterruptedIOException -> when {
                stages == null -> "timeout"
                !stages.connected -> "connect_timeout"
                stages.bodyWritten -> "read_timeout"
                else -> "write_failed"
            }
            else -> when {
                stages == null -> "unknown"
                stages.bodyWritten -> "after_write"
                stages.connected -> "write_failed"
                else -> "connect_refused"
            }
        }

        /** The core's verdict on a failure [kind] (`transport_outcome_of`). */
        fun outcomeOfClass(kind: String): RpcTransportOutcome {
            val type = runCatching { JSONObject(uniffi.vela_core_uniffi.transportOutcomeOf(kind)).optString("type") }.getOrNull()
            return when (type) {
                "not_connected" -> RpcTransportOutcome.NotConnected
                "timeout" -> RpcTransportOutcome.Timeout
                else -> RpcTransportOutcome.Network
            }
        }
    }
}
